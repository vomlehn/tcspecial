//! A simulated payload standing in for a SPI peripheral.
//!
//! The one kind with nothing to be. A serial line has a pty, which is a line
//! in everything but its rate. A device on a bus has the kernel's `i2c-stub`,
//! which really does answer an address with a register bank. There is no
//! equivalent for SPI: no module emulates a peripheral, because a peripheral
//! is a thing a controller clocks, and nothing in user space can be clocked.
//!
//! So what stands in is a pty, as for a serial line, and the difference is
//! worth being plain about. The bytes are real: a packet the simulator writes
//! arrives at the handler, in the segments it was divided into, at the pace
//! the simulator file gives. The clocking is not simulated at all -- there is
//! no clock, no mode, no chip select and no word width -- and a handler that
//! opens this finds a node that will not take its terms and says so.
//!
//! What that tests is the handler's reading and writing of a peripheral, and
//! the pacing of a payload that produces data in blocks. What it cannot test
//! is whether the peripheral would stand being clocked that way, which is a
//! question for the peripheral.
//!
//! See [`crate::payload`] for the pacing, and [`crate::pty`] for the pty.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::payload::{
    a_packet, ignore_this_request, lateness, send_in_segments, wait_out_packet, wait_while_running, Pacing,
    PayloadConfig, PayloadStats, Produce,
};
use crate::pty::SimulatedNode;

/// Run SPI payload simulation
///
/// As every other kind: a packet every packet interval, in segments, from
/// Start until Stop. A segment is one write, which is as near as a stand-in
/// gets to a transfer: a real peripheral answers a transfer the controller
/// starts, and this one simply has the bytes ready.
pub fn run_spi_payload(
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
) {
    let mut node = match SimulatedNode::open(&format!("{}: a SPI peripheral", config.name), &config.address) {
        Ok(node) => node,
        Err(e) => {
            eprintln!("{}: failed to stand in for a SPI peripheral: {}", config.name, e);
            return;
        }
    };

    let mut rng = rand::thread_rng();
    // Packets that have gone, which the faults that are counted rather than
    // random ask about.
    let mut sent = 0u64;

    while running.load(Ordering::SeqCst) {
        let started = Instant::now();
        let pacing = Pacing::read(&config);

        // Whatever the handler has sent this way.
        let mut buf = vec![0u8; 4096];
        let mut asked = 0;
        if let Ok(n) = node.master.read(&mut buf) {
            if n > 0 {
                asked = n;
                let mut guard = stats.lock().unwrap();
                guard.a_packet_has_come(&buf[..n]);
                guard.bytes_recv += n as u64;
            }
        }

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
                &mut |segment| node.master.write(segment),
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

    // The link goes with the node; see SimulatedNode::drop.
    drop(node);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::{PayloadProtocol, SimulatedPayload};
    use crate::pty::make_raw;
    use std::fs::File;
    use std::os::unix::io::AsRawFd;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;

    fn a_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "tcssim-spi-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    /// The configured path leads to a node the handler can open, and the data
    /// there is the packets the payload is producing.
    ///
    /// Which is all a stand-in for a peripheral can offer: the bytes, in their
    /// segments, at their pace. Nothing here says anything about a clock,
    /// because there is nothing here that has one.
    #[test]
    fn a_spi_payload_is_a_node_the_configured_path_leads_to() {
        let path = a_path();
        let mut payload = SimulatedPayload::new(PayloadConfig {
            _id: 0,
            name: "DH0".to_string(),
            protocol: PayloadProtocol::Spi,
            address: path.display().to_string(),
            port: 0,
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

        let deadline = Instant::now() + Duration::from_secs(5);
        while !path.is_symlink() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            path.is_symlink(),
            "{} is not a link to anything",
            path.display()
        );
        assert!(
            std::fs::read_link(&path)
                .expect("where it leads")
                .starts_with("/dev/pts/"),
            "it does not lead to a pty slave"
        );

        let node = File::options()
            .read(true)
            .write(true)
            .open(&path)
            .expect("the node opens");
        make_raw(node.as_raw_fd()).expect("it takes raw terms");

        let mut got = Vec::new();
        let mut buf = [0u8; 64];
        let deadline = Instant::now() + Duration::from_secs(5);
        while got.len() < 12 && Instant::now() < deadline {
            match (&node).read(&mut buf) {
                Ok(n) if n > 0 => got.extend_from_slice(&buf[..n]),
                _ => std::thread::sleep(Duration::from_millis(10)),
            }
        }

        let stats = payload.stats();
        payload.stop();

        assert!(got.len() >= 12, "only {} bytes arrived", got.len());
        assert!(
            stats.bytes_sent >= 12,
            "the payload counted {} bytes sent",
            stats.bytes_sent
        );
        assert!(
            !path.is_symlink(),
            "{} outlived the node it led to",
            path.display()
        );
    }

    /// A real device at the path is left alone, as for every kind that stands
    /// in for hardware: see payload::make_the_path_lead_to.
    #[test]
    fn a_real_device_at_the_path_is_left_alone() {
        let e = match SimulatedNode::open("a SPI peripheral", "/dev/null") {
            Ok(_) => panic!("a real device is not ours to displace"),
            Err(e) => e,
        };
        assert!(
            e.contains("real serial device") || e.contains("not a link the simulator made"),
            "{e}"
        );
        assert!(Path::new("/dev/null").exists());
    }
}
