//! What more than one of these tests does as the ground.

use std::path::{Path, PathBuf};
use tcsos::net::{SocketAddr, UdpSocket};
use tcsos::process::{Child, Command as Process, Stdio};
use tcsos::time::{Duration, Instant};

use tcsprotocol::config_digest::{digest_of_graph, ConfigDigest};
use tcsprotocol::graph::Graph;
use tcsprotocol::{Command, ConnectCommand, Telemetry};

/// The checksum of the configuration in `path`, as tcspecial computes it.
///
/// # Arguments
///
/// * `path` - The configuration.
///
/// # Returns
///
/// Its checksum.
///
/// # Panics
///
/// If it does not load or cannot be digested.
#[allow(dead_code)]
pub fn checksum_of(path: &Path) -> ConfigDigest {
    digest_of_graph(&Graph::load(path).expect("it reads")).expect("a digest")
}

/// Sends CONNECT with `digest` from `from` to `to` until tcspecial answers,
/// as a ground station does once tcspecial is up: until then nothing goes
/// down but beacons, and a CONNECT carrying the configuration's checksum is
/// what opens the downlink.
///
/// # Arguments
///
/// * `from`     - Where the configuration's `tcs_cmd.send` says the ground
///                sends from, with a short read timeout.
/// * `to`       - Where it says tcspecial takes them, `tcs_cmd.recv`.
/// * `sequence` - The CONNECT's sequence number.
/// * `digest`   - The checksum it carries.
///
/// # Returns
///
/// The answer.
///
/// # Panics
///
/// If nothing answers within 20 seconds.
#[allow(dead_code)]
pub fn connect(
    from: &UdpSocket,
    to: SocketAddr,
    sequence: u32,
    digest: ConfigDigest,
) -> Telemetry {
    connect_unless(from, to, sequence, digest, || false)
        .unwrap_or_else(|| panic!("nothing answered a CONNECT at {to}"))
}

/// Sends CONNECT as [`connect`] does, but gives up as soon as `gone` says
/// there is no longer a tcspecial to answer.
///
/// # Arguments
///
/// * `from`     - Where the ground sends from, with a short read timeout.
/// * `to`       - Where tcspecial takes commands.
/// * `sequence` - The CONNECT's sequence number.
/// * `digest`   - The checksum it carries.
/// * `gone`     - Says whether tcspecial has exited; asked between sends.
///
/// # Returns
///
/// The answer, or `None` if tcspecial went, or nothing answered within 20
/// seconds.
#[allow(dead_code)]
pub fn connect_unless(
    from: &UdpSocket,
    to: SocketAddr,
    sequence: u32,
    digest: ConfigDigest,
    mut gone: impl FnMut() -> bool,
) -> Option<Telemetry> {
    let command =
        Command::Connect(ConnectCommand::new(sequence, digest)).to_bytes();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut buffer = [0u8; 65535];
    while Instant::now() < deadline && !gone() {
        from.send_to(&command, to).expect("sent");
        let wait = Instant::now() + Duration::from_millis(500);
        while Instant::now() < wait {
            if let Ok(got) = from.recv(&mut buffer) {
                if let Ok(answer) = Telemetry::read(&buffer[..got]) {
                    return Some(answer);
                }
            }
        }
    }
    None
}

/// Binds a socket on a loopback port nothing holds.
///
/// # Returns
///
/// The socket, with a read timeout of 100 ms.
///
/// # Panics
///
/// If no port can be bound or the timeout cannot be set.
#[allow(dead_code)]
pub fn bound() -> UdpSocket {
    let socket = tcs_testing::bound();
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .expect("a timeout");
    socket
}

/// The port a socket is bound to.
#[allow(dead_code)]
pub fn port(socket: &UdpSocket) -> u16 {
    socket.local_addr().expect("its address").port()
}

/// A port nothing holds, for tcspecial to bind, never given twice in this
/// test's process: see `tcs_testing`, which says why a port let go is not
/// enough.
#[allow(dead_code)]
pub fn free() -> u16 {
    tcs_testing::free()
}

/// tcspecial, stopped however the test ends: a vehicle left running would
/// hold its ports, and its devices, for the next test to trip over.
pub struct Flying {
    child: Child,
    /// Where what it said went, shown should the test fail, which is when
    /// it is wanted; `None` where it said nothing anybody keeps.
    log: Option<PathBuf>,
}

impl Flying {
    /// Takes charge of a tcspecial already started, whose output went
    /// nowhere.
    ///
    /// # Arguments
    ///
    /// * `child` - The tcspecial.
    ///
    /// # Returns
    ///
    /// It, to be stopped when this is dropped.
    #[allow(dead_code)]
    pub fn quiet(child: Child) -> Flying {
        Flying { child, log: None }
    }

    /// Starts tcspecial on a configuration, simulating all, its output
    /// written to `log` and shown if the test fails.
    ///
    /// # Arguments
    ///
    /// * `configuration` - The configuration to fly.
    /// * `log`           - Where its output goes.
    ///
    /// # Returns
    ///
    /// It, to be stopped when this is dropped.
    ///
    /// # Panics
    ///
    /// If the log cannot be made or tcspecial cannot be started.
    #[allow(dead_code)]
    pub fn logged(configuration: &Path, log: &Path) -> Flying {
        let out = tcsos::fs::File::create(log).expect("tcspecial's log");
        let child = Process::new(env!("CARGO_BIN_EXE_tcspecial"))
            .arg(configuration)
            .env(
                "PAYLOAD_SIM_YAML",
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/simulate/all-sim.yaml"
                ),
            )
            .arg("--no-memlock")
            .env_remove("RUST_LOG")
            .stdout(Stdio::null())
            .stderr(out)
            .spawn()
            .expect("tcspecial starts");
        Flying {
            child,
            log: Some(log.to_path_buf()),
        }
    }
}

impl Flying {
    /// Whether tcspecial has exited.
    ///
    /// # Returns
    ///
    /// `true` once it has, `false` while it runs.
    #[allow(dead_code)]
    pub fn exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }

    /// Whether tcspecial stopped because a port it was to bind was taken:
    /// a port a test found free and handed it can be taken by another test
    /// in between, which is no fault of tcspecial's.
    ///
    /// # Returns
    ///
    /// `true` if it has exited and its log says a port was taken.
    #[allow(dead_code)]
    pub fn lost_a_port(&mut self) -> bool {
        let said = match &self.log {
            Some(log) => tcsos::fs::read_to_string(log).unwrap_or_default(),
            None => return false,
        };
        self.exited() && said.contains("Address already in use")
    }
}

impl Drop for Flying {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let (true, Some(log)) = (tcsos::thread::panicking(), &self.log) {
            let said = tcsos::fs::read_to_string(log).unwrap_or_default();
            eprintln!("what tcspecial said, in {}:\n{said}", log.display());
        }
    }
}

/// Writes each `was` as its `now` in `text`, where each appears exactly
/// once: a socket the example stops naming is a test that no longer moves
/// it, and would bind the example's own port. All are written at once, as
/// [`moved_each`] says.
///
/// # Arguments
///
/// * `text`  - The example's configuration.
/// * `moves` - What it says now, each of which must appear exactly once,
///             and what to say instead.
///
/// # Returns
///
/// `text` with each `was` replaced by its `now`.
///
/// # Panics
///
/// If `text` does not say a `was` exactly once, or says one inside
/// another.
#[allow(dead_code)]
pub fn moved<W: AsRef<str>, N: AsRef<str>>(
    text: &str,
    moves: &[(W, N)],
) -> String {
    let each: Vec<(&str, &str, usize)> = moves
        .iter()
        .map(|(was, now)| (was.as_ref(), now.as_ref(), 1))
        .collect();
    moved_each(text, &each)
}

/// Writes each `was` as its `now` in `text`, where it appears exactly
/// `times` times: for what several nodes of an example name alike, such as
/// the one bus two payloads are on.
///
/// Every `was` is found in `text` as it was given, and all are replaced in
/// one pass, so that what one move writes is never taken for what another
/// replaces. Made one after another, a port found free that happens to be
/// one the example names, such as 4300 written for 5650 before 4300 is
/// moved, would be said twice, and moved twice.
///
/// # Arguments
///
/// * `text`  - The example's configuration.
/// * `moves` - What it says now, what to say instead, and how many times
///             it must say it.
///
/// # Returns
///
/// `text` with every `was` replaced by its `now`.
///
/// # Panics
///
/// If `text` does not say a `was` exactly `times` times, or says one
/// inside another.
#[allow(dead_code)]
pub fn moved_each<W: AsRef<str>, N: AsRef<str>>(
    text: &str,
    moves: &[(W, N, usize)],
) -> String {
    // Where each `was` is, as (start, end, which move).
    let mut found = Vec::new();
    for (which, (was, _, times)) in moves.iter().enumerate() {
        let was = was.as_ref();
        let at: Vec<usize> = text.match_indices(was).map(|(i, _)| i).collect();
        assert_eq!(
            at.len(),
            *times,
            "the example should say {was} {times} times"
        );
        found.extend(at.into_iter().map(|i| (i, i + was.len(), which)));
    }
    found.sort_unstable();
    let mut out = String::with_capacity(text.len());
    let mut from = 0;
    for (start, end, which) in found {
        assert!(
            start >= from,
            "the example says {} inside another text being moved",
            moves[which].0.as_ref()
        );
        out.push_str(&text[from..start]);
        out.push_str(moves[which].1.as_ref());
        from = end;
    }
    out.push_str(&text[from..]);
    out
}

/// An example's configuration with every loopback host it names made
/// `127.0.0.1`: an example has a pair of hosts of its own, the MOC's and the
/// vehicle's, which a test that moves its links onto ports of its own, and
/// plays both ends itself, need not know.
///
/// # Arguments
///
/// * `text` - The example's configuration.
///
/// # Returns
///
/// The text, every `127.0.0.N:` written `127.0.0.1:`.
#[allow(dead_code)]
pub fn on_one_host(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("127.0.0.") {
        let (before, from) = rest.split_at(at);
        out.push_str(before);
        let after = &from["127.0.0.".len()..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && after[digits..].starts_with(':') {
            out.push_str("127.0.0.1");
            rest = &after[digits..];
        } else {
            out.push_str("127.0.0.");
            rest = after;
        }
    }
    out.push_str(rest);
    out
}
