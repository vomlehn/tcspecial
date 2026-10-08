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

use crate::payload::{
    a_packet, lateness, send_in_segments, wait_out_packet, wait_while_running, Pacing,
    PayloadConfig, PayloadStats, Produce,
};

/// Run device payload simulation (simulates /dev/urandom-like behavior)
pub fn run_device_payload(config: PayloadConfig, running: Arc<AtomicBool>, stats: Arc<std::sync::Mutex<PayloadStats>>) {
    let mut rng = rand::thread_rng();
    // Packets that have gone, which the faults that are counted rather than
    // random ask about.
    let mut sent = 0u64;

    while running.load(Ordering::SeqCst) {
        let started = Instant::now();
        let pacing = Pacing::read(&config);

        if pacing.a_packet_is_due(config.triggered, 0) {
            // Generate random data (simulating /dev/urandom). There is
            // nothing here to write it to -- tcspecial opens the device
            // itself and reads from it, and this stands in only for the
            // device producing data -- so a segment is counted as produced
            // rather than sent anywhere. The pacing is the point: segments
            // appear at the segment rate, as they would from the far end of a
            // device that delivers a packet in pieces.
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
                &mut |segment| Ok(segment.len()),
            );

            if bytes > 0 {
                let mut guard = stats.lock().unwrap();
                guard.bytes_sent += bytes;
                if whole {
                    guard.packets_sent += 1;
                    sent += 1;
                }
            }
        }

        wait_out_packet(&pacing, started, &running);
    }
}

