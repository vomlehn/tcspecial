//! A simulated device at the far end of a Unix-domain socket.
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
//! * A stream device listens, as a TCP one does, so the simulator binds the
//!   path the handler was told to connect to. The path is the device's.
//! * A datagram device sends first, as a UDP one does, so the handler binds
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
    a_packet, hangs_up, ignore_this_request, lateness, send_in_segments, wait_out_packet,
    wait_while_running, Pacing, PayloadConfig, PayloadStats, Produce,
};

/// What a datagram device's own path is called, given the handler's.
///
/// Derived rather than configured: nothing in a tcspecial configuration names
/// it, and nothing needs to -- the handler learns where its device is from the
/// first datagram, as it does over UDP. Beside the handler's path so that the
/// two are plainly a pair when something goes wrong and somebody is looking at
/// a directory.
fn a_path_of_its_own(handler: &str) -> PathBuf {
    PathBuf::from(format!("{handler}.device"))
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
            std::fs::remove_file(path)
                .map_err(|e| format!("{} cannot be replaced: {e}", path.display()))
        }
    }
}

/// Run Unix-domain stream device simulation
///
/// The simulator listens at the configured path and the handler connects to
/// it, as for TCP. Nothing is sent until there is a connection to send it on,
/// and from then on a packet goes every packet interval whether anything has
/// been received or not.
pub fn run_unix_stream_node(
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
) {
    let path = PathBuf::from(&config.address);
    if let Err(e) = clear_a_dead_socket(&path) {
        eprintln!(
            "{}: failed to stand in for a Unix stream device: {}",
            config.name, e
        );
        return;
    }

    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!(
                "{}: failed to listen at {}: {}",
                config.name,
                path.display(),
                e
            );
            return;
        }
    };
    let _bound = BoundPath { path: path.clone() };
    listener.set_nonblocking(true).ok();
    eprintln!(
        "{}: a Unix stream device is listening at {}",
        config.name,
        path.display()
    );

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

            // And hanging up, for a device told to. Dropping the stream
            // closes it, which the handler sees as the end of its link; the
            // next pass listens again, so a device that hung up is one that
            // can be reconnected to rather than one that has gone for good.
            if hangs_up(&config, sent) {
                eprintln!("{}: hanging up after {sent} packets", config.name);
                connection = None;
                sent = 0;
            }
        }

        wait_out_packet(&pacing, started, &running);
    }
}

/// Run Unix-domain datagram device simulation
///
/// The handler binds the configured path and this end sends to it, as for UDP
/// and for the same reason: a datagram connect tells the far end nothing, and
/// a device produces data because it is running rather than because it was
/// asked. A path of its own is bound so that the handler can answer.
///
/// A device that is asked is the other way round: the vehicle speaks first,
/// so this end binds the configured path and answers whoever asked.
pub fn run_unix_dgram_node(
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
) {
    let handler = PathBuf::from(&config.address);

    // A heater is the other way round, exactly as it is over UDP. It sends
    // nothing -- being driven is the whole of what it is for -- so the
    // vehicle cannot learn where it is from a datagram and sends to the path
    // the configuration gives instead. Then this end is the one that has to
    // be findable, so it binds that path rather than one of its own.
    //
    // The vehicle's end is unbound, so nothing arrives here from a path: see
    // `tcspecial::node::payload::network::unix::UnixDatagramEndpoint::reaching`.
    if matches!(config.thermal, Some(crate::thermal::Thermal::Heats { .. })) {
        run_driven_unix_dgram_node(config, running, stats, &handler);
        return;
    }
    // And a device that is asked waits at the path as a heater does: the
    // vehicle speaks first, with its trigger, and the answer goes back to
    // the name it asked from. The rule is the vehicle's own.
    let speaks_first = tcslibgs::graph::interface::speaks_first(true, config.triggered);

    // The path this end answers from: the one the simulator file asked for,
    // or one beside the handler's. A file that says nothing gets the second,
    // which is what every simulator file asked for before this could be
    // stated. A device that is asked is found at the configuration's path.
    let mine = match config.own_address.as_deref() {
        _ if !speaks_first => handler.clone(),
        Some(path) => PathBuf::from(path),
        None => a_path_of_its_own(&config.address),
    };

    if let Err(e) = clear_a_dead_socket(&mine) {
        eprintln!(
            "{}: failed to stand in for a Unix datagram device: {}",
            config.name, e
        );
        return;
    }

    let socket = match UnixDatagram::bind(&mine) {
        Ok(socket) => socket,
        Err(e) => {
            eprintln!("{}: failed to bind {}: {}", config.name, mine.display(), e);
            return;
        }
    };
    let _bound = BoundPath { path: mine.clone() };
    socket.set_nonblocking(true).ok();
    if speaks_first {
        eprintln!(
            "{}: a Unix datagram device at {} is sending to {}",
            config.name,
            mine.display(),
            handler.display()
        );
    } else {
        eprintln!(
            "{}: a Unix datagram device is waiting to be asked at {}",
            config.name,
            mine.display()
        );
    }
    // Who last asked, for a device that answers: where its answer goes.
    let mut asker = None;

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
        if let Ok((n, from)) = socket.recv_from(&mut buf) {
            if n > 0 {
                asked = n;
                asker = Some(from);
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
                &mut |segment| match &asker {
                    Some(to) if !speaks_first => socket.send_to_addr(segment, to),
                    _ => socket.send_to(segment, &handler),
                },
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

/// Stand in for a datagram payload the vehicle drives and never reads.
///
/// The mirror of `run_driven_udp_node`, and the same loop with the sending
/// taken out: whatever the vehicle writes is counted and acted on, and
/// nothing goes the other way. The pacing is still the file's, and for a
/// device that sends nothing it means the one thing left -- how often it
/// looks for what the vehicle decided.
///
/// This end binds the path the configuration names, which is where the
/// vehicle's end sends. The dead-socket rule applies to it as it does to
/// every path this crate binds, and the path is unlinked when the device
/// goes.
fn run_driven_unix_dgram_node(
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
    at: &Path,
) {
    if let Err(e) = clear_a_dead_socket(at) {
        eprintln!(
            "{}: failed to stand in for a driven Unix datagram device: {}",
            config.name, e
        );
        return;
    }

    let socket = match UnixDatagram::bind(at) {
        Ok(socket) => socket,
        Err(e) => {
            eprintln!(
                "{}: failed to wait for the vehicle at {}: {}",
                config.name,
                at.display(),
                e
            );
            return;
        }
    };
    let _bound = BoundPath {
        path: at.to_path_buf(),
    };
    socket.set_nonblocking(true).ok();
    eprintln!(
        "{}: a driven Unix datagram device is waiting at {}",
        config.name,
        at.display()
    );

    while running.load(Ordering::SeqCst) {
        let started = Instant::now();
        let pacing = Pacing::read(&config);

        // Everything that has arrived, and not one datagram a pass: a
        // decision says what the device should be *now*, so an older one is
        // superseded rather than queued. Taking one a pass means a vehicle
        // deciding faster than this device looks leaves it acting on a
        // decision that is older every pass -- which is a heater that goes on
        // when it is told to and stays on through every later word to the
        // contrary. The reasoning is `run_driven_udp_node`'s, which found it
        // the hard way.
        let mut buf = vec![0u8; 4096];
        while let Ok((n, _from)) = socket.recv_from(&mut buf) {
            if n == 0 {
                break;
            }
            // The mass is told before the statistics are, so that nothing
            // holds the stats lock across the mass's own.
            crate::thermal::driven(&config.thermal, &config.name, &buf[..n]);
            let mut guard = stats.lock().unwrap();
            guard.a_packet_has_come(&buf[..n]);
            guard.bytes_recv += n as u64;
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

    fn device_at(path: &Path, protocol: PayloadProtocol) -> SimulatedPayload {
        SimulatedPayload::new(PayloadConfig {
            thermal: None,
            name: "DH0".to_string(),
            protocol,
            address: path.display().to_string(),
            port: 0,
            bus_address: 0,
            can_id: 0,
            can_fd: false,
            gpio_lines: Vec::new(),
            faults: Default::default(),
            own_address: None,
            own_port: None,
            packet_size: Arc::new(AtomicU32::new(12)),
            segment_size: Arc::new(AtomicU32::new(5)),
            packet_interval_ms: Arc::new(AtomicU32::new(100)),
            segment_interval_ms: Arc::new(AtomicU32::new(20)),
            triggered: false,
        })
    }

    /// A stream device listens where the handler was told to connect, and
    /// sends once connected.
    #[test]
    fn a_stream_device_listens_and_sends_once_connected() {
        let path = a_path("stream");
        std::fs::remove_file(&path).ok();

        let mut device = device_at(&path, PayloadProtocol::UnixStream);
        device.start().expect("the device starts");

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
            assert_ne!(n, 0, "the device closed the connection");
            got.extend_from_slice(&buf[..n]);
        }

        let stats = device.stats();
        device.stop();

        assert!(got.len() >= 12, "only {} bytes arrived", got.len());
        assert!(stats.bytes_sent >= 12, "{} bytes counted", stats.bytes_sent);

        // The socket file goes with the device, rather than being left for
        // the next run to clear.
        assert!(!path.exists(), "{} outlived the device", path.display());
    }

    /// A datagram device sends to where the handler waits, unprompted, from a
    /// path of its own so that it can be answered.
    #[test]
    fn a_datagram_device_sends_to_where_the_handler_waits() {
        let path = a_path("dgram");
        std::fs::remove_file(&path).ok();
        std::fs::remove_file(a_path_of_its_own(&path.display().to_string())).ok();

        // Standing in for the handler, which is the end that binds.
        let handler = UnixDatagram::bind(&path).expect("the handler binds");
        handler
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a read timeout");

        let mut device = device_at(&path, PayloadProtocol::UnixDgram);
        device.start().expect("the device starts");

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

        device.stop();

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
            "the device sent from an unnamed socket, which cannot be answered"
        );

        std::fs::remove_file(&path).ok();
    }

    /// A device that is asked waits at the path the configuration names, and
    /// answers the name it was asked from -- which the vehicle's end has,
    /// outside the filesystem, so that it can be answered.
    #[test]
    fn an_asked_datagram_device_waits_at_its_path_and_answers_the_asker() {
        use std::os::linux::net::SocketAddrExt;
        use std::os::unix::net::SocketAddr;

        let path = a_path("asked");
        std::fs::remove_file(&path).ok();
        let mut device = SimulatedPayload::new(PayloadConfig {
            thermal: None,
            name: "temp".to_string(),
            protocol: PayloadProtocol::UnixDgram,
            address: path.display().to_string(),
            port: 0,
            bus_address: 0,
            can_id: 0,
            can_fd: false,
            gpio_lines: Vec::new(),
            triggered: true,
            faults: Default::default(),
            own_address: None,
            own_port: None,
            packet_size: Arc::new(AtomicU32::new(12)),
            segment_size: Arc::new(AtomicU32::new(12)),
            packet_interval_ms: Arc::new(AtomicU32::new(10)),
            segment_interval_ms: Arc::new(AtomicU32::new(0)),
        });
        device.start().expect("the device starts");

        // The vehicle's end, named outside the filesystem as it is aboard.
        let name = format!("tcssim-asked-{}", std::process::id());
        let vehicle = UnixDatagram::bind_addr(
            &SocketAddr::from_abstract_name(name.as_bytes()).expect("a name"),
        )
        .expect("the vehicle's end binds");
        vehicle
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("a read timeout");

        let mut buf = [0u8; 64];
        let deadline = Instant::now() + Duration::from_secs(5);
        let n = loop {
            // Asked until the device is up to hear it.
            let _ = vehicle.send_to(b"READ", &path);
            match vehicle.recv(&mut buf) {
                Ok(n) => break n,
                Err(_) if Instant::now() < deadline => continue,
                Err(e) => panic!("the device never answered: {e}"),
            }
        };
        device.stop();
        assert_eq!(n, 12, "a packet of twelve bytes, in one segment");

        std::fs::remove_file(&path).ok();
    }

    /// A socket file left by a run that was killed is replaced: the path is
    /// the address, and a dead socket at it would stop every run from then on.
    /// A driven datagram device waits where the vehicle sends, and the mass
    /// is warmed by what arrives.
    ///
    /// The case the ordinary arrangement cannot serve: a heater sends nothing,
    /// so the vehicle cannot learn where it is and sends to the path the
    /// configuration names -- which means this end has to be waiting there.
    /// Bound the other way round, the vehicle's write would have had nowhere
    /// to land and the mass would never have warmed.
    #[test]
    fn a_driven_datagram_device_waits_where_the_vehicle_sends() {
        use crate::thermal::{Thermal, ThermalMass};
        use tcslibgs::sim_config::ResolvedMass;

        let path = a_path("driven");
        std::fs::remove_file(&path).ok();

        let mass = Arc::new(ThermalMass::new(
            &ResolvedMass {
                name: "cabin".to_string(),
                environment: 283.15,
                time_constant: Duration::from_secs(50),
            },
            Instant::now(),
        ));
        let cold = mass.temperature();

        // A heater at the path the configuration names, looking for what the
        // vehicle decided every 20 ms -- which is all pacing means for a
        // device that sends nothing.
        let mut device = SimulatedPayload::new(PayloadConfig {
            thermal: Some(Thermal::Heats {
                mass: mass.clone(),
                rate: 3.0,
            }),
            name: "heaters_1".to_string(),
            protocol: PayloadProtocol::UnixDgram,
            address: path.display().to_string(),
            port: 0,
            bus_address: 0,
            can_id: 0,
            can_fd: false,
            gpio_lines: Vec::new(),
            triggered: false,
            faults: Default::default(),
            own_address: None,
            own_port: None,
            packet_size: Arc::new(AtomicU32::new(1)),
            segment_size: Arc::new(AtomicU32::new(1)),
            packet_interval_ms: Arc::new(AtomicU32::new(20)),
            segment_interval_ms: Arc::new(AtomicU32::new(0)),
        });
        device.start().expect("the device starts");

        // The vehicle's end: unbound, sending where the file says the device
        // is. Written in a loop because the device binds the path on a thread
        // of its own and may not have reached it yet.
        let vehicle = UnixDatagram::unbound().expect("the vehicle's socket");
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && vehicle.send_to(b"1", &path).is_err() {
            std::thread::sleep(Duration::from_millis(10));
        }
        vehicle
            .send_to(b"1", &path)
            .expect("the heater is driven on");

        // Warming, which is the whole of what a heater does: three kelvin a
        // second, so a tenth of a second is plenty to tell from nothing.
        let deadline = Instant::now() + Duration::from_secs(5);
        while mass.temperature() <= cold && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            mass.temperature() > cold,
            "the mass should have warmed, and is at {}",
            mass.temperature()
        );

        device.stop();
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_dead_socket_is_replaced() {
        let path = a_path("stale");
        std::fs::remove_file(&path).ok();
        drop(UnixListener::bind(&path).expect("a socket to abandon"));
        assert!(path.exists(), "the file should outlive the socket");

        clear_a_dead_socket(&path).expect("a dead socket is not a reason to refuse");
        assert!(!path.exists(), "it was not cleared");
    }

    /// A socket something is listening on is not taken: two devices at one
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
