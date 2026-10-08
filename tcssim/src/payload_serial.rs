//! A simulated payload at the far end of a serial line, by way of a pty.
//!
//! The one kind of hardware the simulator can honestly be. A pseudo-terminal
//! is a pair: a master, which this end holds, and a slave, which is a device
//! file with a line discipline behind it -- so a handler opens the slave, sets
//! it to the rate and framing its configuration gives, and reads and writes it
//! exactly as it would a real port. Whatever is written to the master arrives
//! there.
//!
//! What a pty cannot be is the line itself. The kernel accepts a data rate and
//! then ignores it: bytes cross at memory speed whatever the configuration
//! says, so a simulated line tests the handler's reading and writing and the
//! terms it sets, and says nothing about whether 115200 is too fast for the
//! cable. That is what a cable is for.
//!
//! The slave's name is the kernel's to choose -- `/dev/pts/4` and the like --
//! and the configuration names a port, so the path the handler was told to
//! open is made a symbolic link to the slave for as long as the payload runs.
//! A path that is anything other than a link the simulator made is left alone
//! and reported: a simulator that unlinked `/dev/ttyS0` to stand in for it
//! would be worse than one that could not.
//!
//! See [`crate::payload`] for the pacing, which is common to every kind.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::payload::{
    a_packet, ignore_this_request, lateness, send_in_segments, wait_out_packet, wait_while_running, Pacing,
    PayloadConfig, PayloadStats, Produce,
};
use crate::pty::SimulatedNode;

/// Run serial payload simulation
///
/// As every other kind: a packet every packet interval, in segments, from
/// Start until Stop. What a handler reads is the slave side of the pty, which
/// is a device file with a line discipline -- so the bytes arrive as bytes on
/// a port, not as datagrams with edges.
pub fn run_serial_payload(
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
) {
    let mut line = match SimulatedNode::open("a serial line", &config.address) {
        Ok(line) => line,
        Err(e) => {
            eprintln!("Failed to stand in for the serial line: {}", e);
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

        // Whatever the handler has written down the line.
        let mut buf = vec![0u8; 4096];
        let mut asked = 0;
        if let Ok(n) = line.master.read(&mut buf) {
            if n > 0 {
                asked = n;
                let mut guard = stats.lock().unwrap();
                guard.a_packet_has_come(&buf[..n]);
                guard.bytes_recv += n as u64;
            }
        }

        // And a packet of this payload's own. A segment is a write rather
        // than a datagram: a line has no edges, so what divides a packet for
        // a handler reading one is when the bytes arrive, which is what the
        // segment interval decides.
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
                &mut |segment| line.master.write(segment),
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

    // The link goes with the line; see SimulatedLine::drop. Named here so
    // that it is plain the line is not merely dropped at the end of a scope.
    drop(line);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pty::make_raw;
    use std::fs::File;
    use std::os::unix::fs::symlink;
    use std::os::unix::io::AsRawFd;
    use std::path::{Path, PathBuf};
    use crate::payload::{PayloadProtocol, SimulatedPayload};
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;

    fn a_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "tcssim-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    fn payload_at(path: &Path, packet: u32, segment: u32) -> SimulatedPayload {
        SimulatedPayload::new(PayloadConfig {
            _id: 0,
            protocol: PayloadProtocol::Serial,
            address: path.display().to_string(),
            port: 0,
            bus_address: 0,
            faults: Default::default(),
            own_address: None,
            own_port: None,
            packet_size: Arc::new(AtomicU32::new(packet)),
            segment_size: Arc::new(AtomicU32::new(segment)),
            packet_interval_ms: Arc::new(AtomicU32::new(200)),
            segment_interval_ms: Arc::new(AtomicU32::new(40)),
                    triggered: false,
        })
    }

    /// The configured path leads to a line a handler can open, and the data a
    /// handler reads there is the packets the payload is producing.
    ///
    /// This is what the simulator could not do at all: a serial handler had no
    /// far end to talk to, so a payload file with a line in it could be run
    /// only against real hardware.
    #[test]
    fn a_serial_payload_is_a_line_the_configured_path_leads_to() {
        let path = a_path("line");
        let mut payload = payload_at(&path, 12, 5);
        payload.start().expect("the payload starts");

        // Wait for the line to be there, as a handler started first would.
        let deadline = Instant::now() + Duration::from_secs(5);
        while !path.is_symlink() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            path.is_symlink(),
            "{} is not a link to anything",
            path.display()
        );
        let slave = std::fs::read_link(&path).expect("where it leads");
        assert!(
            slave.starts_with("/dev/pts/"),
            "it leads to {}, which is not a pty slave",
            slave.display()
        );

        // Open it as the handler does, and read what the payload is sending.
        // Raw, so that nothing is waiting for a line of text: this is the
        // same thing SerialEndpoint does when it applies the configured
        // terms.
        let port = File::options()
            .read(true)
            .write(true)
            .open(&path)
            .expect("the line opens");
        make_raw(port.as_raw_fd()).expect("it takes raw terms");

        let mut got = Vec::new();
        let mut buf = [0u8; 64];
        let deadline = Instant::now() + Duration::from_secs(5);
        while got.len() < 12 && Instant::now() < deadline {
            match (&port).read(&mut buf) {
                Ok(n) if n > 0 => got.extend_from_slice(&buf[..n]),
                _ => std::thread::sleep(Duration::from_millis(10)),
            }
        }

        let stats = payload.stats();
        payload.stop();

        assert!(
            got.len() >= 12,
            "only {} bytes arrived down the line",
            got.len()
        );
        assert!(
            stats.bytes_sent >= 12,
            "the payload counted {} bytes sent",
            stats.bytes_sent
        );

        // And the link goes when the payload does, rather than pointing at a
        // pty that no longer exists.
        assert!(
            !path.is_symlink(),
            "{} outlived the line it led to",
            path.display()
        );
    }

    /// A real device at the configured path is left alone, and said to be
    /// there. A simulator that unlinked /dev/ttyS0 to stand in for it would
    /// be worse than one that could not.
    #[test]
    fn a_real_device_at_the_path_is_left_alone() {
        let real = Path::new("/dev/null");
        let e = match SimulatedNode::open("a serial line", &real.display().to_string()) {
            Ok(_) => panic!("a real device is not ours to displace"),
            Err(e) => e,
        };
        assert!(
            e.contains("real serial device") || e.contains("not a link the simulator made"),
            "the refusal does not say what is there: {e}"
        );
        assert!(real.exists(), "/dev/null was displaced");
    }

    /// A file somebody else put there is likewise left alone.
    #[test]
    fn a_file_at_the_path_is_left_alone() {
        let path = a_path("occupied");
        std::fs::write(&path, b"not a line").expect("a file in the way");

        let e = match SimulatedNode::open("a serial line", &path.display().to_string()) {
            Ok(_) => panic!("a file is not ours to displace"),
            Err(e) => e,
        };
        assert!(e.contains("already exists"), "{e}");
        assert_eq!(
            std::fs::read(&path).expect("it is still there"),
            b"not a line"
        );

        std::fs::remove_file(&path).ok();
    }

    /// A link left by a run that was killed rather than stopped is replaced:
    /// it points at a pty that is gone.
    #[test]
    fn a_link_from_an_earlier_run_is_replaced() {
        let path = a_path("stale");
        std::fs::remove_file(&path).ok();
        symlink("/dev/pts/999", &path).expect("a stale link");

        let line = SimulatedNode::open("a serial line", &path.display().to_string())
            .expect("a link of our own making is ours to replace");
        assert_eq!(
            std::fs::read_link(&path).expect("where it leads now"),
            Path::new(line.slave())
        );
        drop(line);
        assert!(!path.is_symlink());
    }
}
