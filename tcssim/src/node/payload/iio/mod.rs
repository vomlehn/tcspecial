//! A simulated device that stands in for a sensor's sample buffer.
//!
//! **Experimental, and not yet supported.** Run against a pseudo-terminal
//! standing in for the buffer, and against no sensor.
//!
//! What a handler does with an IIO device is open `/dev/iio:deviceN` and
//! read packed samples out of it. The simulator can be the other end of
//! exactly that, because a pty's slave is a character device a handler reads
//! the same way -- so this is [`crate::pty`] again, as a serial line and a
//! SPI peripheral are, with the configured path made to lead to the slave for
//! as long as the device runs.
//!
//! What it is not is a sensor. The kernel's `iio_dummy` module provides one,
//! and provides the wrong thing for this: it generates values of its own, so
//! a simulator file would decide nothing about what the device produced --
//! where the whole point of a simulated device is that its file decides. So
//! the bytes here are the simulator's, as they are for every other kind, and
//! what the stand-in does not reproduce is the half of IIO that is not the
//! data: the channels and the buffer in `sysfs`. A handler does not touch
//! those either -- the configuration records them and applies nothing -- so
//! what is missing from the stand-in is missing from the handler too.
//!
//! One direction only. A buffer is a stream out of a sensor, so nothing is
//! ever sent to this device: an IIO device cannot be triggered, which the
//! configuration refuses, and this loop never reads.
//!
//! See [`crate::payload`] for the pacing, which is common to every kind.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::payload::{
    a_packet, lateness, send_in_segments, wait_out_packet, wait_while_running, Pacing,
    PayloadConfig, PayloadStats, Produce,
};
use crate::pty::SimulatedNode;

/// Run a simulated device that is a sensor's sample buffer.
///
/// The ordinary loop with the reading taken out: a sample set every packet
/// interval, in as many segments as the file asked for, for as long as the
/// device is running.
pub fn run_iio_node(
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
) {
    let mut buffer = match SimulatedNode::open(
        &format!("{}: an IIO sample buffer", config.name),
        &config.address,
    ) {
        Ok(node) => node,
        Err(e) => {
            eprintln!(
                "{}: failed to stand in for a sample buffer: {}",
                config.name, e
            );
            return;
        }
    };

    let mut rng = rand::thread_rng();
    let mut sent = 0u64;

    while running.load(Ordering::SeqCst) {
        let started = Instant::now();
        let pacing = Pacing::read(&config);

        // Nothing is read: a sample buffer is one way, which is why an IIO
        // device cannot be triggered. `a_packet_is_due` is asked all the
        // same, so that a device whose interval is nought is silent here as
        // it is everywhere else.
        if pacing.a_packet_is_due(false, 0) {
            let late = lateness(&config, &mut rng);
            let packet = match a_packet(&config, &pacing, sent, &mut rng) {
                Produce::Send(packet) => packet,
                Produce::Nothing => Vec::new(),
            };
            wait_while_running(late, &running);

            let (bytes, whole) = send_in_segments(
                &packet,
                pacing.segment_size,
                pacing.segment_interval,
                &running,
                &mut |segment| buffer.master.write(segment),
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
    use std::fs::File;
    use std::io::Read;
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;
    use tcslibgs::Faults;

    /// A sensor whose buffer the handler would open at `path`.
    fn sensor_at(path: &str) -> PayloadConfig {
        PayloadConfig {
            thermal: None,
            name: "sensor".to_string(),
            protocol: PayloadProtocol::Iio,
            address: path.to_string(),
            port: 0,
            bus_address: 0,
            can_id: 0,
            can_fd: false,
            gpio_lines: Vec::new(),
            triggered: false,
            faults: Faults::default(),
            own_address: None,
            own_port: None,
            packet_size: Arc::new(AtomicU32::new(8)),
            segment_size: Arc::new(AtomicU32::new(8)),
            packet_interval_ms: Arc::new(AtomicU32::new(20)),
            segment_interval_ms: Arc::new(AtomicU32::new(0)),
        }
    }

    /// The samples a simulated sensor produces are readable at the path a
    /// handler was told to open.
    ///
    /// Which is the whole of the stand-in: the handler opens a character
    /// device and reads packed samples, and that is what a pty's slave is.
    #[test]
    fn a_simulated_sensor_offers_samples_at_its_path() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("iio:device0");
        let path = path.display().to_string();

        let mut device = SimulatedPayload::new(sensor_at(&path));
        device.start().expect("the sensor starts");

        // The path leads to the buffer while the device runs, and reading it
        // gives what the device produced.
        let samples = (0..100)
            .find_map(|_| {
                std::thread::sleep(Duration::from_millis(20));
                let mut file = File::open(&path).ok()?;
                let mut buffer = [0u8; 64];
                match file.read(&mut buffer) {
                    Ok(n) if n > 0 => Some(n),
                    _ => None,
                }
            })
            .expect("the sensor produced samples");
        assert!(samples > 0);

        device.stop();

        // And the path is the device's for as long as it runs, so it is gone
        // with it: a sensor that is not running has no buffer to open.
        std::thread::sleep(Duration::from_millis(100));
        assert!(
            !std::path::Path::new(&path).exists(),
            "the stand-in outlived the device"
        );
    }

    /// A path the simulator cannot make is reported and the device stops.
    #[test]
    fn a_path_that_cannot_be_made_is_reported() {
        let said = match SimulatedNode::open("sensor", "/proc/nonesuch/iio:device0") {
            Err(e) => e,
            Ok(_) => panic!("a node was made under /proc"),
        };
        assert!(
            !said.is_empty(),
            "the error should say something about the path"
        );
    }
}
