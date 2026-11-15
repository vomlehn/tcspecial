//! Handing a consolidator's inputs on to a ground station that reads the
//! gateway node's ports itself, as Yamcs does.
//!
//! A consolidator packs its inputs into one datum only for the link from the
//! vehicle, where bandwidth is short; nothing on the ground past the gateway
//! node should have to take it apart. A station built on
//! [`crate::gateway::GatewayReceiver`] never sees the datum, since a receive
//! takes it apart. One that binds the node's ports itself would see it, so a
//! [`Relay`] binds the consolidators' ports in its place, takes each datum
//! apart and sends each new input on to the station as the vehicle would have
//! sent it were its source drawn into the node: on the source's own port,
//! under its own APID, after a count where it is a string or bytes -- the
//! link `tcsprotocol::graph::telemetry::relayed` gives it. The station reads
//! those as it reads every other source, and decodes them by the same
//! description of the source's data.
//!
//! The relay is a thread, not a program, so that it ends with the station:
//! dropping it stops the thread, and the process ending ends it with
//! everything else.

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use tcsprotocol::ccsds::{self, PacketType, Sender};
use tcsprotocol::graph::telemetry::{self, Via};
use tcsprotocol::graph::Graph;
use tcsprotocol::link_type::{counted, Count, COUNT_BYTES};

use crate::gateway::GatewayReceiver;

/// How long a receive waits before the thread looks whether it has been
/// told to stop: short enough that dropping a relay is not felt.
const POLL: Duration = Duration::from_millis(100);

/// Where one input's readings are sent on, and how each is framed.
#[derive(Debug)]
struct Target {
    /// The station's port for the input's source.
    to: SocketAddr,
    /// What puts the CCSDS header on, counting the packets, where the
    /// gateway node's packets carry one.
    sender: Option<Sender>,
    /// How a string or bytes is counted, or `None` for a type with a width
    /// of its own.
    count: Option<Count>,
}

/// A consolidator's inputs, being handed on by a thread of their own.
#[derive(Debug)]
pub struct Relay {
    /// Set to tell the thread to stop.
    stop: Arc<AtomicBool>,
    /// The thread, until it is joined.
    thread: Option<JoinHandle<()>>,
}

impl Relay {
    /// Starts handing on the inputs of every consolidator drawn into a
    /// gateway node.
    ///
    /// # Arguments
    ///
    /// * `graph`   - The configuration's graph.
    /// * `gateway` - The gateway node's name.
    ///
    /// # Returns
    ///
    /// The relay, or `None` where no consolidator is drawn into the node,
    /// or none of their inputs needs relaying, each being drawn in itself.
    ///
    /// # Errors
    ///
    /// What [`GatewayReceiver::open_consolidators`] refuses, a link the
    /// configuration's telemetry refuses, an input's port that does not
    /// resolve, or a socket that cannot be bound to send from.
    pub fn start(
        graph: &Graph,
        gateway: &str,
    ) -> Result<Option<Relay>, String> {
        let Some(receiver) =
            GatewayReceiver::open_consolidators(graph, gateway)?
        else {
            return Ok(None);
        };
        let relayed: Vec<_> = telemetry::relayed(graph)
            .map_err(|refused| refused.why)?
            .into_iter()
            .filter(|link| link.via == Via::Gateway(gateway.to_string()))
            .collect();
        if relayed.is_empty() {
            return Ok(None);
        }

        // Allocates, once, before the thread starts: a target for each of
        // the receiver's sources, by its place, and the buffers each datum
        // and packet is made in.
        let mut targets = Vec::new();
        let mut largest = receiver.datum_bytes();
        for source in receiver.sources() {
            let Some(link) = relayed.iter().find(|l| l.source == source) else {
                // Drawn in itself, so the station has its readings already.
                targets.push(None);
                continue;
            };
            let Some(ground) = link.ground() else {
                targets.push(None);
                continue;
            };
            let at = format!("{}:{}", ground.address, ground.port);
            let to = at
                .to_socket_addrs()
                .ok()
                .and_then(|mut each| each.next())
                .ok_or_else(|| {
                    format!(
                        "\"{gateway}\": \"{}\"'s port, {at}, does not resolve",
                        link.source
                    )
                })?;
            let sender = link
                .apid
                .map(|apid| Sender::new(PacketType::Telemetry, apid))
                .transpose()
                .map_err(|why| {
                    format!("\"{gateway}\": \"{}\": {why}", link.source)
                })?;
            largest = largest.max(link.largest);
            targets.push(Some(Target {
                to,
                sender,
                count: link.count(),
            }));
        }
        let socket = UdpSocket::bind("0.0.0.0:0").map_err(|why| {
            format!("\"{gateway}\": no socket to relay from: {why}")
        })?;
        let datum = vec![0u8; largest];
        let body = Vec::with_capacity(COUNT_BYTES + largest);
        let packet = vec![0u8; ccsds::HEADER_BYTES + COUNT_BYTES + largest];

        let stop = Arc::new(AtomicBool::new(false));
        let told = Arc::clone(&stop);
        let mut running = Running {
            receiver,
            targets,
            socket,
            datum,
            body,
            packet,
        };
        let thread = std::thread::Builder::new()
            .name(format!("relay {gateway}"))
            .spawn(move || running.run(&told))
            .map_err(|why| {
                format!("\"{gateway}\": the relay did not start: {why}")
            })?;
        Ok(Some(Relay {
            stop,
            thread: Some(thread),
        }))
    }
}

impl Drop for Relay {
    /// Stops the thread and waits for it, which is at most one wait of a
    /// receive.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// What the relay's thread owns while it runs.
struct Running {
    /// The consolidators' ports.
    receiver: GatewayReceiver,
    /// Where each of the receiver's sources' readings go, by its place in
    /// its sources, or `None` for one not relayed.
    targets: Vec<Option<Target>>,
    /// What every packet is sent from.
    socket: UdpSocket,
    /// Where each input is received.
    datum: Vec<u8>,
    /// The input after its count, where it has one.
    body: Vec<u8>,
    /// The packet, after its header.
    packet: Vec<u8>,
}

impl Running {
    /// Hands each new input on until told to stop, allocating nothing but
    /// where something is wrong and is said.
    ///
    /// # Arguments
    ///
    /// * `stop` - Set when the thread is to stop.
    fn run(&mut self, stop: &AtomicBool) {
        while !stop.load(Ordering::Relaxed) {
            if let Err(why) = self.pass() {
                // Allocates, to say what went wrong: a damaged datum is
                // dropped, said, and the relay goes on.
                eprintln!("{why}");
            }
        }
    }

    /// Waits for the next input, and sends it on where it is relayed.
    ///
    /// # Errors
    ///
    /// What the receive refuses, or where the packet cannot be framed or
    /// sent.
    fn pass(&mut self) -> Result<(), String> {
        let Some(arrival) =
            self.receiver.receive_into(Some(POLL), &mut self.datum)?
        else {
            return Ok(());
        };
        let Some(Some(target)) = self.targets.get_mut(arrival.source) else {
            return Ok(());
        };
        let reading = &self.datum[..arrival.length];
        // The body reuses the room made for it: clearing it and writing no
        // more than its capacity allocates nothing.
        let body: &[u8] = match target.count {
            Some(count) => {
                counted(reading, count, &mut self.body)?;
                &self.body
            }
            None => reading,
        };
        let packet = match target.sender.as_mut() {
            Some(sender) => sender
                .frame(&mut self.packet, body)
                .map_err(|why| format!("relaying: {why}"))?,
            None => body,
        };
        self.socket
            .send_to(packet, target.to)
            .map_err(|why| format!("relaying to {}: {why}", target.to))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tcsprotocol::consolidation::{Layout, State};

    /// The configuration of a consolidator to Yamcs the tests share.
    fn graph() -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Graph::load(root.join("tests/consolidator/yamcs.ytcs"))
            .expect("the file reads")
    }

    /// A consolidator's datum, framed as the vehicle sends it, is handed on
    /// as each new input's own packet: on its source's port, under its own
    /// APID. A held input is not, nor one whose source is drawn into the
    /// gateway node itself, whose readings the station has already.
    #[test]
    fn each_new_input_is_sent_on_as_its_source_would_send_it() {
        let graph = graph();
        // The station's ports for temp and valve, as Yamcs would bind them.
        let temp = UdpSocket::bind("127.0.0.1:26703").expect("temp's port");
        let valve = UdpSocket::bind("127.0.0.1:26704").expect("valve's port");
        for socket in [&temp, &valve] {
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("a timeout");
        }
        let relay = Relay::start(&graph, "gateway")
            .expect("it starts")
            .expect("there is something to relay");

        let node = graph.node("pack").expect("the consolidator");
        let (_, layout): (_, Layout) = Layout::of_node(&graph, node)
            .expect("a consolidator")
            .expect("it lays out");
        let send = |readings: &[(State, &[u8])], sequence: u16| {
            let mut datum = [0u8; 64];
            let used = layout.pack(readings, &mut datum).expect("it packs");
            let mut body = Vec::new();
            counted(&datum[..used], Count::Xtce, &mut body).expect("counted");
            let mut packet = vec![0u8; ccsds::HEADER_BYTES + body.len()];
            packet[ccsds::HEADER_BYTES..].copy_from_slice(&body);
            ccsds::write_header(
                &mut packet,
                PacketType::Telemetry,
                0x202,
                sequence,
            )
            .expect("a header");
            UdpSocket::bind("127.0.0.1:0")
                .expect("the vehicle's socket")
                .send_to(&packet, "127.0.0.1:26702")
                .expect("sent");
        };
        // temp new, valve held, the name new: only temp is relayed, the
        // name's source being drawn in itself. Then valve new.
        send(
            &[
                (State::New, &[0x12, 0x34]),
                (State::Held, &[1]),
                (State::New, b"hot"),
            ],
            0,
        );
        send(
            &[
                (State::Held, &[0x12, 0x34]),
                (State::New, &[1]),
                (State::Held, b"hot"),
            ],
            1,
        );

        let mut room = [0u8; 64];
        let got = temp.recv(&mut room).expect("temp's reading");
        let data =
            ccsds::read_header(&room[..got], PacketType::Telemetry, 0x203)
                .expect("temp's header");
        assert_eq!(data, [0x12, 0x34]);
        let got = valve.recv(&mut room).expect("valve's reading");
        let data =
            ccsds::read_header(&room[..got], PacketType::Telemetry, 0x204)
                .expect("valve's header");
        assert_eq!(data, [1]);

        // Nothing more: temp's held reading was not sent again.
        temp.set_read_timeout(Some(Duration::from_millis(300)))
            .expect("a timeout");
        assert!(temp.recv(&mut room).is_err());
        drop(relay);
    }

    /// A gateway node with no consolidator drawn into it has nothing to
    /// relay.
    #[test]
    fn with_no_consolidator_there_is_no_relay() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let graph =
            Graph::load(root.join("examples/ping-yamcs/ping-yamcs.ytcs"))
                .expect("the example reads");
        assert!(Relay::start(&graph, "gateway")
            .expect("it starts")
            .is_none());
    }
}
