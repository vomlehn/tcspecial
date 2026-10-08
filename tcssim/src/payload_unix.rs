//! A simulated payload at the far end of a Unix-domain socket.
//!
//! The easiest of the kinds to stand in for: the simulator can simply be one.
//! A Unix socket is a socket, so there is no emulation here and nothing the
//! simulation leaves out -- unlike a bus, a line, or a peripheral, where
//! something had to be found to stand in with and something had to be given
//! up.
//!
//! What a Unix socket does bring is a path, and which end owns it differs by
//! protocol, because which end binds does:
//!
//! * A stream payload listens, as a TCP one does, so the simulator binds the
//!   path the handler was told to connect to. The path is the payload's.
//! * A datagram payload sends first, as a UDP one does, so the handler binds
//!   that path and the simulator binds one of its own beside it -- a datagram
//!   socket needs a path of its own to be answered at, which a UDP one gets
//!   for nothing from its port.
//!
//! Either way the file outlives the process that bound it, so what is already
//! at a path is looked at before binding: a socket nothing answers is one a
//! killed run left and is replaced, a socket something answers belongs to
//! something running and is left alone, and anything else is left for the bind
//! to refuse. Tcspecial applies the same rule to the paths it binds; see
//! `endpoint_unix`.
//!
//! See [`crate::payload`] for the pacing, which is common to every kind.

use std::io::{Read, Write};
use std::os::unix::net::{UnixDatagram, UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::payload::{
    a_packet, hangs_up, ignore_this_request, lateness, send_in_segments, wait_out_packet, wait_while_running, Pacing,
    PayloadConfig, PayloadStats, Produce,
};

/// What a datagram payload's own path is called, given the handler's.
///
/// Derived rather than configured: nothing in a payload file names it, and
/// nothing needs to -- the handler learns where its payload is from the first
/// datagram, as it does over UDP. Beside the handler's path so that the two are
/// plainly a pair when something goes wrong and somebody is looking at a
/// directory.
fn a_path_of_its_own(handler: &str) -> PathBuf {
    PathBuf::from(format!("{handler}.payload"))
}

/// A socket bound at a path, which is removed with it.
struct BoundPath {
    path: PathBuf,
}

impl Drop for BoundPath {
    /// Take the file away with the socket. One left behind is what the next
    /// run has to clear before it can bind, and a path that still looks
    /// bound is a path nothing answers.
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Make a path available to bind, or say what is in the way.
///
/// See the module comment. This is tcssim's copy of a rule tcspecial applies
/// to its own binds; the two crates share no code and the rule is four lines,
/// so it is written twice rather than made into a dependency.
fn clear_a_dead_socket(path: &Path) -> Result<(), String> {
    let found = match std::fs::symlink_metadata(path) {
        Ok(found) => found,
        // Nothing there, which is the ordinary case.
        Err(_) => return Ok(()),
    };

    use std::os::unix::fs::FileTypeExt;
    if !found.file_type().is_socket() {
        return Err(format!(
            "{} already exists and is not a socket, so the simulator will not take \
             its place",
            path.display()
        ));
    }

    // A datagram sent to it is the question: a live socket takes it, a dead
    // one answers that there is nothing there.
    let probe = UnixDatagram::unbound().map_err(|e| format!("no socket to ask with: {e}"))?;
    match probe.send_to(&[], path) {
        Ok(_) => Err(format!(
            "{} is a socket something is already listening on: another simulator, or \
             a handler, is using that path",
            path.display()
        )),
        Err(_) => {
            eprintln!(
                "{} is a socket nothing answers, left by a run that was killed rather \
                 than stopped; replacing it",
                path.display()
            );
            std::fs::remove_file(path).map_err(|e| format!("{} cannot be replaced: {e}", path.display()))
        }
    }
}

/// Run Unix-domain stream payload simulation
///
/// The simulator listens at the configured path and the handler connects to
/// it, as for TCP. Nothing is sent until there is a connection to send it on,
/// and from then on a packet goes every packet interval whether anything has
/// been received or not.
pub fn run_unix_stream_payload(
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
) {
    let path = PathBuf::from(&config.address);
    if let Err(e) = clear_a_dead_socket(&path) {
        eprintln!("Failed to stand in for the Unix stream payload: {}", e);
        return;
    }

    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("Failed to listen at {}: {}", path.display(), e);
            return;
        }
    };
    let _bound = BoundPath { path: path.clone() };
    listener.set_nonblocking(true).ok();
    eprintln!("a Unix stream payload is listening at {}", path.display());

    let mut connection: Option<UnixStream> = None;
    let mut rng = rand::thread_rng();
    // Packets that have gone, which the faults that are counted rather than
    // random ask about.
    let mut sent = 0u64;

    while running.load(Ordering::SeqCst) {
        let started = Instant::now();
        let pacing = Pacing::read(&config);

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
                    &mut |segment| stream.write(segment),
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

            // And hanging up, for a payload told to. Dropping the stream
            // closes it, which the handler sees as the end of its link; the
            // next pass listens again, so a payload that hung up is one that
            // can be reconnected to rather than one that has gone for good.
            if hangs_up(&config, sent) {
                eprintln!("the payload is hanging up after {sent} packets");
                connection = None;
                sent = 0;
            }
        }

        wait_out_packet(&pacing, started, &running);
    }
}

/// Run Unix-domain datagram payload simulation
///
/// The handler binds the configured path and this end sends to it, as for UDP
/// and for the same reason: a datagram connect tells the far end nothing, and
/// a payload produces data because it is running rather than because it was
/// asked. A path of its own is bound so that the handler can answer.
pub fn run_unix_dgram_payload(
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
) {
    let handler = PathBuf::from(&config.address);
    let mine = a_path_of_its_own(&config.address);

    if let Err(e) = clear_a_dead_socket(&mine) {
        eprintln!("Failed to stand in for the Unix datagram payload: {}", e);
        return;
    }

    let socket = match UnixDatagram::bind(&mine) {
        Ok(socket) => socket,
        Err(e) => {
            eprintln!("Failed to bind {}: {}", mine.display(), e);
            return;
        }
    };
    let _bound = BoundPath { path: mine.clone() };
    socket.set_nonblocking(true).ok();
    eprintln!(
        "a Unix datagram payload at {} is sending to {}",
        mine.display(),
        handler.display()
    );

    let mut rng = rand::thread_rng();
    // Packets that have gone, which the faults that are counted rather than
    // random ask about.
    let mut sent = 0u64;

    while running.load(Ordering::SeqCst) {
        let started = Instant::now();
        let pacing = Pacing::read(&config);

        // Whatever the handler has sent down.
        let mut buf = vec![0u8; 4096];
        let mut asked = 0;
        if let Ok((n, _)) = socket.recv_from(&mut buf) {
            if n > 0 {
                asked = n;
                let mut guard = stats.lock().unwrap();
                guard.a_packet_has_come(&buf[..n]);
                guard.bytes_recv += n as u64;
            }
        }

        // And a packet of its own, asked for or not. A send before the
        // handler has bound its path fails, which is counted as nothing sent
        // rather than as a packet: the handler is not there yet.
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
                &mut |segment| socket.send_to(segment, &handler),
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
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;

    fn a_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "tcssim-unix-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    fn payload_at(path: &Path, protocol: PayloadProtocol) -> SimulatedPayload {
        SimulatedPayload::new(PayloadConfig {
            _id: 0,
            protocol,
            address: path.display().to_string(),
            port: 0,
            bus_address: 0,
            faults: Default::default(),
            packet_size: Arc::new(AtomicU32::new(12)),
            segment_size: Arc::new(AtomicU32::new(5)),
            packet_interval_ms: Arc::new(AtomicU32::new(100)),
            segment_interval_ms: Arc::new(AtomicU32::new(20)),
                    triggered: false,
        })
    }

    /// A stream payload listens where the handler was told to connect, and
    /// sends once connected.
    #[test]
    fn a_stream_payload_listens_and_sends_once_connected() {
        let path = a_path("stream");
        std::fs::remove_file(&path).ok();

        let mut payload = payload_at(&path, PayloadProtocol::UnixStream);
        payload.start().expect("the payload starts");

        // Wait for the socket, as a handler started first would.
        let deadline = Instant::now() + Duration::from_secs(5);
        while !path.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(path.exists(), "{} was never bound", path.display());

        // Standing in for the handler.
        let mut handler = UnixStream::connect(&path).expect("the handler connects");
        handler
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a read timeout");

        let mut got = Vec::new();
        let mut buf = [0u8; 64];
        while got.len() < 12 {
            let n = handler.read(&mut buf).expect("a segment");
            assert_ne!(n, 0, "the payload closed the connection");
            got.extend_from_slice(&buf[..n]);
        }

        let stats = payload.stats();
        payload.stop();

        assert!(got.len() >= 12, "only {} bytes arrived", got.len());
        assert!(stats.bytes_sent >= 12, "{} bytes counted", stats.bytes_sent);

        // The socket file goes with the payload, rather than being left for
        // the next run to clear.
        assert!(
            !path.exists(),
            "{} outlived the payload",
            path.display()
        );
    }

    /// A datagram payload sends to where the handler waits, unprompted, from a
    /// path of its own so that it can be answered.
    #[test]
    fn a_datagram_payload_sends_to_where_the_handler_waits() {
        let path = a_path("dgram");
        std::fs::remove_file(&path).ok();
        std::fs::remove_file(a_path_of_its_own(&path.display().to_string())).ok();

        // Standing in for the handler, which is the end that binds.
        let handler = UnixDatagram::bind(&path).expect("the handler binds");
        handler
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a read timeout");

        let mut payload = payload_at(&path, PayloadProtocol::UnixDgram);
        payload.start().expect("the payload starts");

        // Twelve bytes in fives, one datagram each, and nothing was said to
        // it first.
        let mut sizes = Vec::new();
        let mut buf = [0u8; 64];
        let mut from = None;
        while sizes.len() < 6 {
            let (n, who) = handler.recv_from(&mut buf).expect("a segment");
            sizes.push(n);
            from = who.as_pathname().map(|p| p.to_path_buf());
        }

        payload.stop();

        let ended = sizes
            .iter()
            .position(|&n| n == 2)
            .unwrap_or_else(|| panic!("no segment was the short last one: {sizes:?}"));
        assert_eq!(
            &sizes[ended + 1..ended + 4],
            &[5, 5, 2][..],
            "twelve bytes in fives did not arrive as 5, 5 and 2: {sizes:?}"
        );

        // And it has a path, so the handler can answer it.
        assert_eq!(
            from,
            Some(a_path_of_its_own(&path.display().to_string())),
            "the payload sent from an unnamed socket, which cannot be answered"
        );

        std::fs::remove_file(&path).ok();
    }

    /// A socket file left by a run that was killed is replaced: the path is
    /// the address, and a dead socket at it would stop every run from then on.
    #[test]
    fn a_dead_socket_is_replaced() {
        let path = a_path("stale");
        std::fs::remove_file(&path).ok();
        drop(UnixListener::bind(&path).expect("a socket to abandon"));
        assert!(path.exists(), "the file should outlive the socket");

        clear_a_dead_socket(&path).expect("a dead socket is not a reason to refuse");
        assert!(!path.exists(), "it was not cleared");
    }

    /// A socket something is listening on is not taken: two payloads at one
    /// path is a configuration to report.
    #[test]
    fn a_live_socket_is_not_taken() {
        let path = a_path("live");
        std::fs::remove_file(&path).ok();
        let live = UnixDatagram::bind(&path).expect("a socket that is listening");

        let e = clear_a_dead_socket(&path).expect_err("a live socket is not ours");
        assert!(e.contains("already listening"), "{e}");

        drop(live);
        std::fs::remove_file(&path).ok();
    }

    /// Anything that is not a socket is left alone and said to be there.
    #[test]
    fn a_file_that_is_not_a_socket_is_left_alone() {
        let path = a_path("file");
        std::fs::write(&path, b"not a socket").expect("a file in the way");

        let e = clear_a_dead_socket(&path).expect_err("a file is not ours");
        assert!(e.contains("not a socket"), "{e}");
        assert_eq!(std::fs::read(&path).unwrap(), b"not a socket");

        std::fs::remove_file(&path).ok();
    }
}
