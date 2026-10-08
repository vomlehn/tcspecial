//! A simulated payload that sends datagrams to where its handler waits.
//!
//! The one kind that has to speak first. A datagram connect sends nothing, so
//! a payload waiting at an address is never spoken to, and a payload produces
//! data because it is running rather than because it was asked -- so this end
//! reaches out and the handler is the end that binds.
//!
//! See [`crate::payload`] for the pacing, which is common to every kind.

use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use rand::Rng;

use crate::payload::{send_in_segments, wait_out_packet, Pacing, PayloadConfig, PayloadStats};

/// Run UDP payload simulation
///
/// The payload speaks first and keeps speaking: from Start to Stop it sends a
/// packet every packet interval whether or not anything has been heard from
/// the handler. That is what a payload does -- it produces data because it is
/// running, not because it was asked -- and it is why this end does not bind
/// the handler's address but reaches out to it.
///
/// It used to bind that address and answer whoever reached it, which meant
/// nothing was ever sent: a handler reaches its payload by connecting, and a
/// UDP connect sends nothing, so the payload heard nothing and stayed silent
/// while the handler waited for data that was waiting for it. Both ends sat
/// there with Start pressed and every counter at zero.
///
/// The socket is connected rather than merely aimed, so that a handler that
/// is not there is reported: a send to a port nothing is bound to comes back
/// refused instead of being quietly dropped, and the failed sends are what
/// the window shows as nothing moving.
pub fn run_udp_payload(config: PayloadConfig, running: Arc<AtomicBool>, stats: Arc<std::sync::Mutex<PayloadStats>>) {
    let handler = format!("{}:{}", config.address, config.port);

    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to bind UDP socket: {}", e);
            return;
        }
    };

    // Nothing has to find the payload at a particular port -- the handler
    // learns where it is from the first packet it gets -- so the local port
    // is whatever is free.
    if let Err(e) = socket.connect(&handler) {
        eprintln!("Failed to reach the handler at {}: {}", handler, e);
        return;
    }

    socket.set_nonblocking(true).ok();

    let mut rng = rand::thread_rng();

    while running.load(Ordering::SeqCst) {
        let started = Instant::now();
        let pacing = Pacing::read(&config);

        // Whatever the handler has sent down. The socket is connected, so
        // this is the handler's data and nobody else's.
        let mut buf = vec![0u8; 4096];
        let mut asked = 0;
        if let Ok(n) = socket.recv(&mut buf) {
            if n > 0 {
                asked = n;
                let mut guard = stats.lock().unwrap();
                guard.packets_recv += 1;
                guard.bytes_recv += n as u64;
            }
        }

        // And a packet of its own, asked for or not. Each segment is a
        // datagram, so a packet in four segments arrives as four reads at the
        // handler.
        if pacing.a_packet_is_due(config.triggered, asked) {
            let packet: Vec<u8> = (0..pacing.packet_size).map(|_| rng.gen()).collect();
            let (bytes, whole) = send_in_segments(
                &packet,
                pacing.segment_size,
                pacing.segment_interval,
                &running,
                &mut |segment| socket.send(segment),
            );

            if bytes > 0 {
                let mut guard = stats.lock().unwrap();
                guard.bytes_sent += bytes;
                if whole {
                    guard.packets_sent += 1;
                }
            }
        }

        wait_out_packet(&pacing, started, &running);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::{PayloadProtocol, SimulatedPayload};
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;

    /// A UDP payload sends because it is running, not because it was asked.
    ///
    /// Nothing speaks to it here: the handler's end only waits at the address
    /// the payload was configured to reach, and the packets arrive anyway.
    /// That is the whole of what Start means, and it is what the payload did
    /// not do before -- it bound the handler's address and answered whoever
    /// reached it, and since a handler reaches a UDP payload by connecting,
    /// which sends nothing, it was never spoken to and never sent.
    ///
    /// The packet arrives as the segments the settings ask for, one datagram
    /// each, which is where the division is visible: three segments are three
    /// reads at the handler's end.
    #[test]
    fn a_udp_payload_sends_unprompted_as_the_segments_asked_for() {
        // The handler's end of the link, which is the end that waits.
        let handler = UdpSocket::bind("127.0.0.1:0").expect("a socket for the handler");
        let at = handler.local_addr().expect("its address");
        handler
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a read timeout");

        let mut payload = SimulatedPayload::new(PayloadConfig {
            _id: 0,
            protocol: PayloadProtocol::Udp,
            address: at.ip().to_string(),
            port: at.port(),
            bus_address: 0,
            packet_size: Arc::new(AtomicU32::new(12)),
            segment_size: Arc::new(AtomicU32::new(5)),
            packet_interval_ms: Arc::new(AtomicU32::new(200)),
            segment_interval_ms: Arc::new(AtomicU32::new(40)),
                    triggered: false,
        });
        payload.start().expect("the payload starts");

        // Enough for two packets, whichever segment of one arrives first.
        let mut buf = [0u8; 64];
        let mut datagrams: Vec<(usize, Instant)> = Vec::new();
        while datagrams.len() < 6 {
            let (n, _) = handler.recv_from(&mut buf).expect("a segment");
            datagrams.push((n, Instant::now()));
        }

        let stats = payload.stats();
        payload.stop();

        let sizes: Vec<usize> = datagrams.iter().map(|(n, _)| *n).collect();

        // Line up on a packet boundary -- the short segment ends a packet --
        // and look at the packet after it.
        let ended = sizes
            .iter()
            .position(|&n| n == 2)
            .unwrap_or_else(|| panic!("no segment was the short last one: {sizes:?}"));
        assert_eq!(
            &sizes[ended + 1..ended + 4],
            &[5, 5, 2][..],
            "twelve bytes in fives did not arrive as 5, 5 and 2: {sizes:?}"
        );

        let apart = datagrams[ended + 2].1 - datagrams[ended + 1].1;
        assert!(
            apart >= Duration::from_millis(40),
            "two segments of one packet arrived {apart:?} apart, not the 40ms asked for"
        );

        assert!(
            stats.packets_sent >= 1 && stats.bytes_sent >= 12,
            "a packet of three segments counts as one packet of twelve bytes, \
             not {} of {}",
            stats.packets_sent,
            stats.bytes_sent
        );
    }

    /// Stop stops it. The sending is what Start began and what Stop ends, so
    /// nothing arrives afterwards however long the handler's end waits.
    #[test]
    fn a_stopped_udp_payload_sends_nothing_more() {
        let handler = UdpSocket::bind("127.0.0.1:0").expect("a socket for the handler");
        let at = handler.local_addr().expect("its address");
        handler
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a read timeout");

        let mut payload = SimulatedPayload::new(PayloadConfig {
            _id: 0,
            protocol: PayloadProtocol::Udp,
            address: at.ip().to_string(),
            port: at.port(),
            bus_address: 0,
            packet_size: Arc::new(AtomicU32::new(8)),
            segment_size: Arc::new(AtomicU32::new(8)),
            packet_interval_ms: Arc::new(AtomicU32::new(50)),
            segment_interval_ms: Arc::new(AtomicU32::new(50)),
                    triggered: false,
        });
        payload.start().expect("the payload starts");

        let mut buf = [0u8; 64];
        handler.recv_from(&mut buf).expect("a packet while running");

        // stop() does not return until the sending thread has joined, so
        // anything still in flight is already in the socket by now.
        payload.stop();
        while handler.recv_from(&mut buf).is_ok() {}

        // Several packet intervals' worth of silence.
        handler
            .set_read_timeout(Some(Duration::from_millis(400)))
            .expect("a read timeout");
        let after_stop = handler.recv_from(&mut buf);
        assert!(
            after_stop.is_err(),
            "a stopped payload sent {:?}",
            after_stop.map(|(n, _)| n)
        );
    }
}
