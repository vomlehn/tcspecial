//! The MOC's link to a command interpreter, which may be up or down.
//!
//! Every control in the window that sends a command works through one of
//! these. It exists because the link is no longer something the MOC opens
//! once and keeps: Disconnect takes it down, Connect brings it back up, and
//! possibly to a different address than before. A [`TcsClient`] cannot
//! express that by itself -- it owns its connection from the moment it is
//! built -- so what the handlers share is this, and they ask it for a client
//! each time rather than holding one.
//!
//! That is the whole reason for the indirection. The poll thread, the menu,
//! Query All, and every panel's Start and Stop button each kept a clone of
//! one `Arc<Mutex<TcsClient>>`, so a disconnect had nowhere to put the fact
//! that there was no longer a connection: taking the client away from one
//! holder would have left the others talking to a socket that was closed.
//!
//! The panel poller is the one holder that does not share the window's link.
//! It has a link of its own and keeps it wherever that one is, by
//! [`CiLink::follow`]: its commands wait on the spacecraft for as long as the
//! timeout allows, and a window whose buttons waited behind that is a window
//! that does nothing at the moment the operator is trying to disconnect.
//! Following keeps one account of what the operator asked for -- the link the
//! buttons control -- without the poller having to hold it.

use tcslib::{TcsClient, UdpConnection};
use tcslibgs::{HostPort, TcsResult, Timestamp};

/// The local address the MOC sends from where nothing says otherwise: any
/// interface, any port.
///
/// What a test that does not care passes. Every link a running MOC opens
/// binds an address the configuration states -- `tcs_cmd.send` for the
/// spacecraft's, `payload_command.source_base` counted on by a payload's id
/// for that payload's -- which is what lets tcspecial refuse a command from
/// anywhere else.
#[cfg(test)]
pub const LOCAL_BIND_ANY: &str = "0.0.0.0:0";


/// What the second command link is, which is where the two versions differ.
///
/// Where every node's commands go, which is the second of the two links.
///
/// Commands go on two links, for the reason they always have: what the ground
/// needs in a hurry must never be queued behind what it asked for at leisure.
/// One is the spacecraft's own -- `tcs_cmd` -- and this is the other, which
/// serves every node because a command carries the node's id.
///
/// It was an enum of two, the other being version 1's arrangement: a socket
/// per payload, that being how a command said which payload it was about. One
/// socket and an id is what there is now.
#[derive(Debug, Clone)]
pub struct Commanded {
    /// Where node commands are sent from, which is
    /// `payload_command.source_base` and what tcspecial checks against.
    pub from: HostPort,
    /// And where they go, which tcspecial binds.
    pub to: HostPort,
}

/// What a control says when it is pressed with the link down.
///
/// One wording for all of them, because the reason is the same whichever
/// control it was and the remedy is always the same button.
pub const NOT_CONNECTED: &str = "Not connected - press Connect first";

/// A link to a command interpreter at a particular address.
///
/// The address is remembered whether the link is up or down, so what the
/// window shows after a disconnect is still where it would reconnect to.
pub struct CiLink {
    address: String,
    /// What the spacecraft link binds locally: `tcs_cmd.send`.
    ///
    /// Stated because the other end checks it. tcspecial takes commands only
    /// from where its configuration says the MOC sends them, so the socket
    /// this sends from is a socket the file names -- which is also what lets
    /// a firewall rule or a capture filter describe the link beforehand.
    from: String,
    /// What the second link is: one per payload, or one for every node.
    ///
    /// Held rather than passed to [`CiLink::connect`]: it is the same for the
    /// life of the link, as the file states it, and Connect moves only the
    /// spacecraft link's host.
    commanded: Commanded,
    client: Option<TcsClient>,
}

impl CiLink {

    /// And one whose second link is whatever `commanded` says.
    ///
    /// What a version 2 configuration uses: see [`Commanded`].
    pub fn down_commanding(from: &str, commanded: Commanded) -> Self {
        Self {
            address: String::new(),
            from: from.to_string(),
            commanded,
            client: None,
        }
    }

    /// Where this link goes, up or down.
    pub fn address(&self) -> &str {
        &self.address
    }


    /// The client to send with, or `None` if the link is down.
    ///
    /// Asked for one command at a time rather than kept, so that a caller
    /// cannot go on using a client the link no longer has.
    /// When the last command went out and when its answer came back, if the
    /// link is up and has carried one.
    ///
    /// The client's own account of it: every command goes through one place
    /// there, so nothing here has to remember at each press. A link that is
    /// down has nothing to say -- the times belonged to a client that is
    /// closed, and the next Connect starts a fresh one.
    pub fn command_times(&self) -> Option<(Option<Timestamp>, Option<Timestamp>)> {
        self.client
            .as_ref()
            .map(|client| (client.last_sent(), client.last_received()))
    }

    pub fn client(&mut self) -> Option<&mut TcsClient> {
        self.client.as_mut()
    }

    /// Bring the link up at `address`, replacing whatever it had.
    ///
    /// Any existing connection is closed first, so pressing Connect with a
    /// new address in the box moves the link rather than leaving two. A
    /// failure to open leaves the link down and the new address remembered:
    /// the address is what was asked for, and showing the old one would
    /// invite a second press that looks like it should work.
    /// Both links are opened, or neither: a client with one of them could
    /// send only half of what there is to send, and the half it could not
    /// send is the half the panels use.
    pub fn connect(&mut self, address: &str) -> TcsResult<()> {
        self.disconnect();
        self.address = address.to_string();

        let connection = UdpConnection::new(&self.from, address)?;
        let Commanded { from, to } = &self.commanded;
        let nodes = UdpConnection::new(&from.to_string(), &to.to_string())?;
        self.client = Some(TcsClient::of_a_graph(
            Box::new(connection),
            Box::new(nodes),
        ));
        Ok(())
    }


    /// Take the link down.
    ///
    /// The close error is not returned, and deliberately: this is what
    /// Disconnect does, and a link whose close complained is down either way.
    /// There is nothing the MOC or the operator would do differently about
    /// it, and reporting it would put a failure in front of someone whose
    /// request succeeded.
    pub fn disconnect(&mut self) {
        if let Some(mut client) = self.client.take() {
            let _ = client.close();
        }
    }


}

#[cfg(test)]
mod tests {
    use super::*;

    /// A link says when it last sent a command and when the answer came.
    ///
    /// Which is what the window shows under Last Response. The times are the
    /// client's, so this is the chain from the socket to the thing the window
    /// reads: a link that is up and has carried a command has both, and one
    /// that is down has nothing to say.
    #[test]
    fn a_link_says_when_it_last_sent_and_received() {
        use std::net::UdpSocket;
        use tcslibgs::{Command, PingTelemetry, Telemetry};

        // A stub spacecraft that answers one PING on the command link. The
        // payload link is never sent anything by a ping.
        let spacecraft = UdpSocket::bind("127.0.0.1:0").expect("a stub socket");
        let at = spacecraft.local_addr().expect("where it is").to_string();
        let answering = std::thread::spawn(move || {
            let mut buffer = [0u8; 65535];
            let (size, from) = spacecraft.recv_from(&mut buffer).expect("a command");
            let command: Command =
                serde_json::from_slice(&buffer[..size]).expect("a valid command");
            let answer = Telemetry::Ping(PingTelemetry::new(
                command.sequence(),
                tcslibgs::CommandStatus::Success,
            ));
            let data = serde_json::to_vec(&answer).expect("it serializes");
            spacecraft.send_to(&data, from).expect("the answer is sent");
        });

        let mut link = CiLink::down_commanding(LOCAL_BIND_ANY, commanding());
        assert_eq!(link.command_times(), None, "a link that is down has no times");

        link.connect(&at).expect("the link comes up");
        assert_eq!(
            link.command_times(),
            Some((None, None)),
            "a fresh link has sent nothing"
        );

        link.client()
            .expect("the link is up")
            .ping()
            .expect("the stub answers");
        answering.join().expect("the stub finishes");

        let (sent, received) = link.command_times().expect("the link is up");
        let sent = sent.expect("the command was timed");
        let received = received.expect("so was the answer");
        assert!(received >= sent, "the answer came before the command went");

        // And a disconnect takes them with the client: the times belonged to
        // it, and the next Connect starts a fresh one.
        link.disconnect();
        assert_eq!(link.command_times(), None);
    }

    /// The node link a test's link carries beside the spacecraft's.
    ///
    /// Nothing answers on it in these tests: what they are about is the link
    /// the operator controls, and the node link is opened beside it by the
    /// same call. Its own end is a port of the system's choosing, so that two
    /// tests running at once do not want one socket.
    fn commanding() -> Commanded {
        Commanded {
            from: HostPort {
                address: "127.0.0.1".to_string(),
                port: 0,
            },
            to: HostPort {
                address: "127.0.0.1".to_string(),
                port: 4200,
            },
        }
    }

    /// A link that is down has no client to hand out, and one that is up has.
    ///
    /// No command interpreter is needed for any of this: these are UDP
    /// sockets, which bind locally and are told where to send without anyone
    /// having to be there.
    #[test]
    fn a_link_hands_out_a_client_only_while_it_is_up() {
        let mut link = CiLink::down_commanding(LOCAL_BIND_ANY, commanding());
        assert!(link.client().is_none());
        assert!(link.client().is_none());

        link.connect("127.0.0.1:4000").unwrap();
        assert!(link.client().is_some());
        assert!(link.client().is_some());
        assert_eq!(link.address(), "127.0.0.1:4000");

        link.disconnect();
        assert!(link.client().is_none());
        assert!(link.client().is_none());
    }

    /// Disconnect and connect again, which is the sequence the two buttons
    /// make, leaves a link that works.
    #[test]
    fn a_link_can_be_brought_back_up() {
        let mut link = CiLink::down_commanding(LOCAL_BIND_ANY, commanding());
        link.connect("127.0.0.1:4000").unwrap();
        link.disconnect();
        link.connect("127.0.0.1:4000").unwrap();
        assert!(link.client().is_some());
    }

    /// Connecting somewhere else moves the link rather than adding one.
    #[test]
    fn connecting_elsewhere_moves_the_link() {
        let mut link = CiLink::down_commanding(LOCAL_BIND_ANY, commanding());
        link.connect("127.0.0.1:4000").unwrap();

        link.connect("127.0.0.1:4001").unwrap();
        assert!(link.client().is_some());
        assert_eq!(link.address(), "127.0.0.1:4001");
    }

    /// An address that cannot be used leaves the link down, and remembering
    /// the address that was asked for.
    #[test]
    fn an_address_that_cannot_be_used_leaves_the_link_down() {
        let mut link = CiLink::down_commanding(LOCAL_BIND_ANY, commanding());
        link.connect("127.0.0.1:4000").unwrap();

        assert!(link.connect("this is not an address").is_err());
        assert!(link.client().is_none());
        assert_eq!(link.address(), "this is not an address");
    }

    /// Every node is commanded on one socket, and the link opens it with the
    /// spacecraft's.
    ///
    /// One socket and not one per node: the node's id is in the message, so
    /// one socket says which node every command is about. Version 1 bound one
    /// per payload, which is the arrangement this replaced.
    #[test]
    fn every_node_is_commanded_on_one_socket() {
        let here = |port| HostPort {
            address: "127.0.0.1".to_string(),
            port,
        };
        let mut link = CiLink::down_commanding(
            LOCAL_BIND_ANY,
            Commanded {
                from: here(0),
                to: here(4200),
            },
        );

        link.connect("127.0.0.1:4000").expect("the link comes up");
        assert!(
            link.client().is_some(),
            "a link that is up has a client to send node commands with"
        );

        // And down and up again, as Connect and Disconnect do it: the node
        // link is rebuilt from the file each time, so it cannot be left
        // pointing where an earlier connection pointed.
        link.disconnect();
        assert!(link.client().is_none());
        link.connect("127.0.0.1:4001").expect("it comes up again");
        assert_eq!(link.address(), "127.0.0.1:4001");
    }

    /// Disconnecting a link that is already down is not an error, since the
    /// button can be pressed twice.
    #[test]
    fn disconnecting_twice_is_harmless() {
        let mut link = CiLink::down_commanding(LOCAL_BIND_ANY, commanding());
        link.connect("127.0.0.1:4000").unwrap();
        link.disconnect();
        link.disconnect();
        assert!(link.client().is_none());
    }
}
