//! A simulated payload standing in for a device.
//!
//! Nothing is sent anywhere. A device is opened and read by tcspecial itself,
//! so what there is to simulate is the production of data: a packet's worth
//! appears every packet interval, segment by segment, and is counted. The
//! pacing is the whole of the behaviour.
//!
//! See [`crate::payload`] for that pacing, which is common to every kind.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use rand::Rng;

use crate::payload::{send_in_segments, wait_out_packet, Pacing, PayloadConfig, PayloadStats};

/// Run device payload simulation (simulates /dev/urandom-like behavior)
pub fn run_device_payload(config: PayloadConfig, running: Arc<AtomicBool>, stats: Arc<std::sync::Mutex<PayloadStats>>) {
    let mut rng = rand::thread_rng();

    while running.load(Ordering::SeqCst) {
        let started = Instant::now();
        let pacing = Pacing::read(&config);

        if pacing.produces() {
            // Generate random data (simulating /dev/urandom). There is
            // nothing here to write it to -- tcspecial opens the device
            // itself and reads from it, and this stands in only for the
            // device producing data -- so a segment is counted as produced
            // rather than sent anywhere. The pacing is the point: segments
            // appear at the segment rate, as they would from the far end of a
            // device that delivers a packet in pieces.
            let packet: Vec<u8> = (0..pacing.packet_size).map(|_| rng.gen()).collect();
            let (bytes, whole) = send_in_segments(
                &packet,
                pacing.segment_size,
                pacing.segment_interval,
                &running,
                &mut |segment| Ok(segment.len()),
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

