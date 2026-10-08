//! A simulated payload at the far end of a stream.
//!
//! The handler connects and this end listens, which is what a stream allows:
//! the connection itself says who is at the other end. Nothing is sent until
//! there is a connection to send it on -- there is nowhere to put it -- and
//! from then on a packet goes every packet interval whether anything has been
//! received or not.
//!
//! See [`crate::payload`] for the pacing, which is common to every kind.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use rand::Rng;

use crate::payload::{send_in_segments, wait_out_packet, Pacing, PayloadConfig, PayloadStats};

/// Run TCP payload simulation
pub fn run_tcp_payload(config: PayloadConfig, running: Arc<AtomicBool>, stats: Arc<std::sync::Mutex<PayloadStats>>) {
    let addr = format!("{}:{}", config.address, config.port);

    // Try to bind as server
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind TCP listener: {}", e);
            return;
        }
    };

    listener.set_nonblocking(true).ok();

    let mut connection: Option<TcpStream> = None;
    let mut rng = rand::thread_rng();

    while running.load(Ordering::SeqCst) {
        let started = Instant::now();
        let pacing = Pacing::read(&config);

        // Accept new connections
        if connection.is_none() {
            if let Ok((stream, _)) = listener.accept() {
                stream.set_nonblocking(true).ok();
                connection = Some(stream);
            }
        }

        if let Some(ref mut stream) = connection {
            // Read first, so that a trigger is answered in the pass it
            // arrives in rather than the one after.
            let mut asked = 0;
            let mut buf = vec![0u8; 4096];
            if let Ok(n) = stream.read(&mut buf) {
                if n > 0 {
                    asked = n;
                    let mut guard = stats.lock().unwrap();
                    guard.packets_recv += 1;
                    guard.bytes_recv += n as u64;
                }
            }

            if pacing.a_packet_is_due(config.triggered, asked) {
                let packet: Vec<u8> = (0..pacing.packet_size).map(|_| rng.gen()).collect();
                let (bytes, whole) = send_in_segments(
                    &packet,
                    pacing.segment_size,
                    pacing.segment_interval,
                    &running,
                    &mut |segment| stream.write(segment),
                );

                if bytes > 0 {
                    let mut guard = stats.lock().unwrap();
                    guard.bytes_sent += bytes;
                    if whole {
                        guard.packets_sent += 1;
                    }
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
    use std::net::TcpStream;
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;

    fn payload_at(port: u16, triggered: bool, interval_ms: u32) -> SimulatedPayload {
        SimulatedPayload::new(PayloadConfig {
            _id: 0,
            protocol: PayloadProtocol::Tcp,
            address: "127.0.0.1".to_string(),
            port,
            bus_address: 0,
            triggered,
            packet_size: Arc::new(AtomicU32::new(12)),
            segment_size: Arc::new(AtomicU32::new(12)),
            packet_interval_ms: Arc::new(AtomicU32::new(interval_ms)),
            segment_interval_ms: Arc::new(AtomicU32::new(interval_ms)),
        })
    }

    fn a_free_port() -> u16 {
        let socket = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
        socket.local_addr().expect("its address").port()
    }

    /// A triggered payload answers a request and says nothing otherwise.
    ///
    /// Which is the whole of what the kind means. A payload that answered on
    /// a clock as well would be two kinds at once, and the file that
    /// described it would have no way to say which rate was which.
    #[test]
    fn a_triggered_payload_answers_a_request_and_nothing_else() {
        let port = a_free_port();
        // An interval is passed and must be ignored: for a triggered payload
        // the simulator configuration may not state one, and this stands for
        // what a stale one would do.
        let mut payload = payload_at(port, true, 50);
        payload.start().expect("the payload starts");

        // Connect, as the handler does.
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut handler = loop {
            match TcpStream::connect(("127.0.0.1", port)) {
                Ok(stream) => break stream,
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(e) => panic!("the payload never listened: {e}"),
            }
        };
        handler
            .set_read_timeout(Some(Duration::from_millis(300)))
            .expect("a read timeout");

        // Silence, for several of the intervals it was given.
        let mut buf = [0u8; 64];
        assert!(
            handler.read(&mut buf).is_err(),
            "a triggered payload sent without being asked"
        );

        // Then a request, and exactly one packet of answer.
        handler.write_all(b"READ\r").expect("the handler asks");
        handler
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a read timeout");

        let mut got = Vec::new();
        while got.len() < 12 {
            let n = handler.read(&mut buf).expect("an answer");
            assert_ne!(n, 0, "the payload closed the connection");
            got.extend_from_slice(&buf[..n]);
        }
        assert_eq!(got.len(), 12, "a request was answered with {} bytes", got.len());

        // And silence again: one request, one answer.
        handler
            .set_read_timeout(Some(Duration::from_millis(300)))
            .expect("a read timeout");
        assert!(
            handler.read(&mut buf).is_err(),
            "a triggered payload answered a request it was not sent"
        );

        let stats = payload.stats();
        payload.stop();
        assert_eq!(stats.packets_sent, 1, "one request, one packet");
        assert!(stats.packets_recv >= 1, "the request was not counted");
    }

    /// And a periodic payload on the same loop still sends on its own, which
    /// is what the mode has to leave alone.
    #[test]
    fn a_periodic_payload_still_sends_unasked() {
        let port = a_free_port();
        let mut payload = payload_at(port, false, 50);
        payload.start().expect("the payload starts");

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut handler = loop {
            match TcpStream::connect(("127.0.0.1", port)) {
                Ok(stream) => break stream,
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(e) => panic!("the payload never listened: {e}"),
            }
        };
        handler
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a read timeout");

        let mut buf = [0u8; 64];
        let n = handler.read(&mut buf).expect("a packet, unasked");
        assert_ne!(n, 0);

        payload.stop();
    }
}
