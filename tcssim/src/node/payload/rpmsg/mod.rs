//! A simulated device that stands in for software on another core.
//!
//! **Experimental, and not yet supported.** Run against a pseudo-terminal
//! standing in for the channel, and against no remote processor.
//!
//! An rpmsg channel is a character device carrying whole messages both ways,
//! and a pty is a character device carrying bytes both ways -- so the
//! stand-in is [`crate::pty`] again, with the configured path led to the
//! slave while the device runs. The handler writes a message and reads an
//! answer exactly as it would on a real channel.
//!
//! Two things a pty is not. It has no entry under `/sys/class/rpmsg`, so the
//! handler's check of which channel it is finds nothing to compare against
//! and passes -- which is deliberate: that check exists to catch a node whose
//! number moved, and a stand-in has no number to move. And a pty keeps no
//! message boundaries: the kernel's rpmsg driver hands over one message per
//! read, where a pty hands over whatever bytes are there. A packet sent as
//! one write commonly arrives as one read, so the difference shows only where
//! a device writes faster than the handler reads.
//!
//! Both directions, so a triggered device works here: the simulator reads
//! the handler's request off the same pty and answers it.
//!
//! See [`crate::payload`] for the pacing, which is common to every kind.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::payload::{
    a_packet, ignore_this_request, lateness, send_in_segments, wait_out_packet,
    wait_while_running, Pacing, PayloadConfig, PayloadStats, Produce,
};
use crate::pty::SimulatedNode;

/// Run a simulated device on an rpmsg channel.
pub fn run_rpmsg_node(
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
) {
    let mut channel = match SimulatedNode::open(
        &format!("{}: an rpmsg channel", config.name),
        &config.address,
    ) {
        Ok(node) => node,
        Err(e) => {
            eprintln!(
                "{}: failed to stand in for an rpmsg channel: {}",
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

        // Whatever the handler has sent down the channel, which is a request
        // for a device that answers them.
        let mut buf = vec![0u8; 4096];
        let mut asked = 0;
        if let Ok(n) = channel.master.read(&mut buf) {
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
                Produce::Nothing => Vec::new(),
            };
            wait_while_running(late, &running);

            let (bytes, whole) = send_in_segments(
                &packet,
                pacing.segment_size,
                pacing.segment_interval,
                &running,
                &mut |segment| channel.master.write(segment),
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
    use std::fs::OpenOptions;
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;
    use tcslibgs::Faults;

    /// A channel the handler would open at `path`.
    fn channel_at(path: &str, triggered: bool, interval_ms: u32) -> PayloadConfig {
        PayloadConfig {
            thermal: None,
            name: "remote".to_string(),
            protocol: PayloadProtocol::Rpmsg,
            address: path.to_string(),
            port: 0,
            bus_address: 0,
            can_id: 0,
            can_fd: false,
            gpio_lines: Vec::new(),
            triggered,
            faults: Faults::default(),
            own_address: None,
            own_port: None,
            packet_size: Arc::new(AtomicU32::new(8)),
            segment_size: Arc::new(AtomicU32::new(8)),
            packet_interval_ms: Arc::new(AtomicU32::new(interval_ms)),
            segment_interval_ms: Arc::new(AtomicU32::new(0)),
        }
    }

    /// The messages a simulated remote sends arrive at the path a handler was
    /// told to open.
    #[test]
    fn a_simulated_remote_sends_messages_at_its_path() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("rpmsg0").display().to_string();

        let mut device = SimulatedPayload::new(channel_at(&path, false, 20));
        device.start().expect("the remote starts");

        let got = (0..100)
            .find_map(|_| {
                std::thread::sleep(Duration::from_millis(20));
                let mut file = OpenOptions::new().read(true).open(&path).ok()?;
                let mut buffer = [0u8; 64];
                match file.read(&mut buffer) {
                    Ok(n) if n > 0 => Some(n),
                    _ => None,
                }
            })
            .expect("the remote sent a message");
        // A whole number of messages, rather than exactly one of them. The
        // stand-in is a pty and a pty keeps no message boundaries: the remote
        // writes one message every 20 ms and a read that arrives between two
        // of them returns both, which a real channel would have handed over
        // one at a time. Asserting one message read this as a failure about
        // one run in three.
        assert!(
            got % 8 == 0,
            "a read should be whole messages of the configured size, and {got} \
             bytes is not"
        );

        device.stop();
    }

    /// A triggered remote answers a request and says nothing otherwise.
    ///
    /// Which a channel can do and a sample buffer cannot: rpmsg carries both
    /// directions, so the simulator really does read the handler's request.
    #[test]
    fn a_triggered_remote_answers_a_request() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("rpmsg1").display().to_string();

        // A triggered device sends when it is asked and at no rate of its
        // own, which is what the device configuration holds it to.
        let mut device = SimulatedPayload::new(channel_at(&path, true, 0));
        device.start().expect("the remote starts");

        // The handler's end of the channel, once the stand-in has made it.
        let mut handler = (0..100)
            .find_map(|_| {
                std::thread::sleep(Duration::from_millis(20));
                OpenOptions::new().read(true).write(true).open(&path).ok()
            })
            .expect("the channel appeared");

        // Nothing until it is asked.
        let mut buffer = [0u8; 64];
        std::thread::sleep(Duration::from_millis(200));
        handler
            .set_nonblocking_for_test()
            .expect("the handler's end does not block");
        assert!(
            matches!(handler.read(&mut buffer), Err(_) | Ok(0)),
            "a triggered remote sent without being asked"
        );

        // Asked, and the answer comes back.
        handler.write_all(b"READ").expect("the request goes out");
        let answered = (0..100)
            .find_map(|_| {
                std::thread::sleep(Duration::from_millis(20));
                match handler.read(&mut buffer) {
                    Ok(n) if n > 0 => Some(n),
                    _ => None,
                }
            })
            .expect("the request was answered");
        assert_eq!(answered, 8, "one message of the configured size");

        device.stop();
    }

    /// Reading a file without blocking, which the test above needs of the
    /// handler's end: a pty read blocks until something arrives, and what the
    /// test is checking is that nothing does.
    trait NonBlockingForTest {
        fn set_nonblocking_for_test(&self) -> std::io::Result<()>;
    }

    impl NonBlockingForTest for std::fs::File {
        fn set_nonblocking_for_test(&self) -> std::io::Result<()> {
            use std::os::unix::io::AsRawFd;

            // SAFETY: both calls take this file's descriptor by value and
            // return a status.
            let flags = unsafe { libc::fcntl(self.as_raw_fd(), libc::F_GETFL) };
            if flags < 0
                || unsafe { libc::fcntl(self.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) }
                    < 0
            {
                return Err(std::io::Error::last_os_error());
            }

            Ok(())
        }
    }
}
