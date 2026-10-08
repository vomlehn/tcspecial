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

use crate::payload::{
    a_packet, ignore_this_request, lateness, send_in_segments, wait_out_packet, wait_while_running, Pacing,
    PayloadConfig, PayloadStats, Produce,
};

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

    // What this end answers from. Nothing has to find the payload at a
    // particular port -- the handler learns where it is from the first packet
    // it gets -- so a file that says nothing gets any interface and whatever
    // port is free. A file that does say is obeyed: something outside the
    // simulation, a rule on a firewall or a capture being read afterwards,
    // may need to know which socket the payload answers from.
    let mine = format!(
        "{}:{}",
        config.own_address.as_deref().unwrap_or("0.0.0.0"),
        config.own_port.unwrap_or(0)
    );

    let socket = match UdpSocket::bind(&mine) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to bind the UDP payload at {}: {}", mine, e);
            return;
        }
    };
    if let Err(e) = socket.connect(&handler) {
        eprintln!("Failed to reach the handler at {}: {}", handler, e);
        return;
    }

    socket.set_nonblocking(true).ok();

    let mut rng = rand::thread_rng();
    // Packets that have gone, which the faults that are counted rather than
    // random ask about.
    let mut sent = 0u64;

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
                guard.a_packet_has_come(&buf[..n]);
                guard.bytes_recv += n as u64;
            }
        }

        // And a packet of its own, asked for or not. Each segment is a
        // datagram, so a packet in four segments arrives as four reads at the
        // handler.
        if pacing.a_packet_is_due(config.triggered, asked)
                && !ignore_this_request(&config, &mut rng)
            {
            let late = lateness(&config, &mut rng);
            let packet = match a_packet(&config, &pacing, sent, &mut rng) {
                Produce::Send(packet) => packet,
                // Dropped, or gone quiet: nothing goes, and nothing
                // is counted, because nothing moved.
                Produce::Nothing => Vec::new(),
            };
            // However late this one is being made, before it goes.
            wait_while_running(late, &running);
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
                    guard.a_packet_has_gone(&packet);
                    sent += 1;
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
    use tcslibgs::Faults;
    use std::sync::atomic::AtomicU32;
    use tcslibgs::Timestamp;
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
            faults: Default::default(),
            own_address: None,
            own_port: None,
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

    /// A payload at the handler's address, with faults.
    fn faulty(at: std::net::SocketAddr, faults: Faults) -> SimulatedPayload {
        answering_from(at, faults, None, None)
    }

    /// A payload at the handler's address, answering from the socket given.
    fn answering_from(
        at: std::net::SocketAddr,
        faults: Faults,
        own_address: Option<String>,
        own_port: Option<u16>,
    ) -> SimulatedPayload {
        SimulatedPayload::new(PayloadConfig {
            _id: 0,
            protocol: PayloadProtocol::Udp,
            address: at.ip().to_string(),
            port: at.port(),
            bus_address: 0,
            triggered: false,
            faults,
            own_address,
            own_port,
            packet_size: Arc::new(AtomicU32::new(12)),
            segment_size: Arc::new(AtomicU32::new(12)),
            packet_interval_ms: Arc::new(AtomicU32::new(20)),
            segment_interval_ms: Arc::new(AtomicU32::new(0)),
        })
    }

    /// A handler's end of the link, waiting with a deadline.
    fn handler_end(wait: Duration) -> (UdpSocket, std::net::SocketAddr) {
        let handler = UdpSocket::bind("127.0.0.1:0").expect("a socket for the handler");
        let at = handler.local_addr().expect("its address");
        handler.set_read_timeout(Some(wait)).expect("a read timeout");
        (handler, at)
    }

    /// A payload told which socket to answer from answers from it.
    ///
    /// Nothing in the link needs it -- the handler learns where the payload
    /// is from the first packet it gets -- but something outside the
    /// simulation may: a rule on a firewall, or a capture being read
    /// afterwards. The handler's end sees which socket a datagram came from,
    /// which is how this is checked.
    #[test]
    fn a_payload_answers_from_the_socket_it_was_given() {
        let (handler, at) = handler_end(Duration::from_secs(5));

        // A port nothing else is on, found by binding and letting go.
        let spare = UdpSocket::bind("127.0.0.1:0").expect("a spare port");
        let mine = spare.local_addr().expect("its address").port();
        drop(spare);

        let mut payload = answering_from(
            at,
            Faults::default(),
            Some("127.0.0.1".to_string()),
            Some(mine),
        );
        payload.start().expect("the payload starts");

        let mut buf = [0u8; 64];
        let (_, from) = handler.recv_from(&mut buf).expect("a packet");
        payload.stop();

        assert_eq!(
            from.port(), mine,
            "the payload answered from {} rather than the {mine} it was given",
            from.port()
        );
    }

    /// The stats carry when the last packet went, not just how many.
    ///
    /// Taken at the moment the packet goes rather than when the window next
    /// looks, which is what makes the panel's time the time of the transfer:
    /// a time stamped by the refresh would creep forward on a payload that
    /// had stopped sending.
    #[test]
    fn a_sent_packet_is_timed_as_well_as_counted() {
        let (handler, at) = handler_end(Duration::from_secs(5));
        let mut payload = faulty(at, Faults::default());

        let before = Timestamp::now().seconds;
        payload.start().expect("the payload starts");

        let mut buf = [0u8; 64];
        handler.recv_from(&mut buf).expect("a packet");
        // The sending thread stamps the stats after the write, so give it the
        // moment between the two.
        let deadline = Instant::now() + Duration::from_secs(5);
        let sent = loop {
            let sample = payload.stats().last_sent;
            match sample.time {
                Some(_) => break sample,
                None if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                None => panic!("a packet arrived and nothing recorded when"),
            }
        };
        let after = Timestamp::now().seconds;
        payload.stop();

        let went = sent.time.expect("a time, having just been matched");
        assert!(
            (before..=after).contains(&went.seconds),
            "a packet sent between {before} and {after} was timed at {}",
            went.seconds
        );

        // And what went: the head of the packet, and its whole length, which
        // is what lets the panel say a packet was longer than what it shows.
        assert_eq!(
            sent.total, 12,
            "a packet of twelve bytes was sampled as {} bytes",
            sent.total
        );
        assert_eq!(&buf[..sent.data().len()], sent.data(), "the sample is not the bytes that arrived");
        assert!(
            payload.stats().last_recv.is_empty(),
            "nothing was sent to this payload, so it has no received line"
        );
    }

    /// And when the last one arrived, which is the other row of the panel.
    ///
    /// The handler's end learns where the payload is from the packet it gets,
    /// a datagram's sender being the only statement of where it came from, so
    /// this is also the only order in which the two can be timed: the payload
    /// speaks, and then can be spoken to.
    #[test]
    fn a_received_packet_is_timed_too() {
        let (handler, at) = handler_end(Duration::from_secs(5));
        let mut payload = faulty(at, Faults::default());

        let before = Timestamp::now().seconds;
        payload.start().expect("the payload starts");

        let mut buf = [0u8; 64];
        let (_, from) = handler.recv_from(&mut buf).expect("a packet");
        handler.send_to(b"ASK", from).expect("the handler asks");

        let deadline = Instant::now() + Duration::from_secs(5);
        let received = loop {
            let sample = payload.stats().last_recv;
            match sample.time {
                Some(_) => break sample,
                None if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                None => panic!("the payload was spoken to and recorded no time"),
            }
        };
        let after = Timestamp::now().seconds;
        payload.stop();

        let came = received.time.expect("a time, having just been matched");
        assert!(
            (before..=after).contains(&came.seconds),
            "a packet received between {before} and {after} was timed at {}",
            came.seconds
        );
        assert_eq!(
            received.data(),
            b"ASK",
            "the sample is not what the handler sent"
        );
    }

    /// Every packet dropped is a running payload that sends nothing.
    ///
    /// Which is the point of the fault: the simulator is up, the handler is
    /// started, the link is fine, and the data does not come. Nothing is
    /// counted either -- a dropped packet never existed, so counting it as
    /// sent would make the statistics disagree with the handler's about what
    /// moved, and the statistics are what tell the two apart.
    #[test]
    fn a_payload_dropping_everything_sends_nothing() {
        let (handler, at) = handler_end(Duration::from_millis(500));
        let mut payload = faulty(
            at,
            Faults {
                drop_percent: 100,
                ..Faults::default()
            },
        );
        payload.start().expect("the payload starts");

        // Many packet intervals' worth of waiting.
        let mut buf = [0u8; 64];
        let heard = handler.recv_from(&mut buf);
        let stats = payload.stats();
        payload.stop();

        assert!(
            heard.is_err(),
            "a payload dropping every packet sent {:?}",
            heard.map(|(n, _)| n)
        );
        assert_eq!(
            (stats.packets_sent, stats.bytes_sent),
            (0, 0),
            "a dropped packet was counted as sent"
        );
    }

    /// A truncated packet arrives short, and arrives.
    ///
    /// The handler is left holding part of a packet, which is the case its
    /// own framing has to notice. Short and never empty: a packet of nothing
    /// is the dropping fault rather than this one, and a handler that saw an
    /// empty datagram would be reading a different fault than the one asked
    /// for.
    #[test]
    fn a_truncated_packet_arrives_short_of_its_size() {
        let (handler, at) = handler_end(Duration::from_secs(5));
        let mut payload = faulty(
            at,
            Faults {
                truncate_percent: 100,
                ..Faults::default()
            },
        );
        payload.start().expect("the payload starts");

        // A segment size equal to the packet size makes each packet one
        // datagram, so a datagram's length is a packet's length.
        let mut buf = [0u8; 64];
        let mut lengths = Vec::new();
        while lengths.len() < 5 {
            let (n, _) = handler.recv_from(&mut buf).expect("a packet");
            lengths.push(n);
        }
        payload.stop();

        assert!(
            lengths.iter().all(|&n| (1..12).contains(&n)),
            "a truncated packet of twelve bytes arrived as {lengths:?}: every one \
             of these should be short of twelve and none of them empty"
        );
    }

    /// A payload told to go quiet after so many packets goes quiet, and stays
    /// that way.
    ///
    /// The link is still up and the handler is still waiting: what a stopped
    /// instrument looks like from the other end, and what a handler reporting
    /// no data has to be told apart from a handler that was never started.
    #[test]
    fn a_payload_going_silent_sends_that_many_and_no_more() {
        let (handler, at) = handler_end(Duration::from_secs(5));
        let mut payload = faulty(
            at,
            Faults {
                silent_after: 3,
                ..Faults::default()
            },
        );
        payload.start().expect("the payload starts");

        let mut buf = [0u8; 64];
        for which in 1..=3 {
            handler
                .recv_from(&mut buf)
                .unwrap_or_else(|e| panic!("packet {which} of three never came: {e}"));
        }

        // Then silence, for many more intervals than the three took.
        handler
            .set_read_timeout(Some(Duration::from_millis(400)))
            .expect("a read timeout");
        let after = handler.recv_from(&mut buf);
        let stats = payload.stats();
        payload.stop();

        assert!(
            after.is_err(),
            "a payload silent after three packets sent a fourth"
        );
        assert_eq!(
            stats.packets_sent, 3,
            "a payload silent after three packets sent {}",
            stats.packets_sent
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
            faults: Default::default(),
            own_address: None,
            own_port: None,
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
