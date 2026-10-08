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
            if pacing.produces() {
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

            // Try to receive data
            let mut buf = vec![0u8; 4096];
            if let Ok(n) = stream.read(&mut buf) {
                if n > 0 {
                    let mut guard = stats.lock().unwrap();
                    guard.packets_recv += 1;
                    guard.bytes_recv += n as u64;
                }
            }
        }

        wait_out_packet(&pacing, started, &running);
    }
}

