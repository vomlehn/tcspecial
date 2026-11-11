//! What more than one of these tests does as the ground.

use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command as Process, Stdio};
use std::time::{Duration, Instant};

use tcslibgs::config_digest::{digest_of_graph, ConfigDigest};
use tcslibgs::graph::Graph;
use tcslibgs::{Command, ConnectCommand, Telemetry};

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
    let socket = UdpSocket::bind("127.0.0.1:0").expect("a port of its own");
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

/// A port nothing holds, for tcspecial to bind.
#[allow(dead_code)]
pub fn free() -> u16 {
    port(&bound())
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

    /// Starts tcspecial on a configuration, `--simulate=all`, its output
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
        let out = std::fs::File::create(log).expect("tcspecial's log");
        let child = Process::new(env!("CARGO_BIN_EXE_tcspecial"))
            .arg(configuration)
            .arg("--simulate=all")
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
            Some(log) => std::fs::read_to_string(log).unwrap_or_default(),
            None => return false,
        };
        self.exited() && said.contains("Address already in use")
    }
}

impl Drop for Flying {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let (true, Some(log)) = (std::thread::panicking(), &self.log) {
            let said = std::fs::read_to_string(log).unwrap_or_default();
            eprintln!("what tcspecial said, in {}:\n{said}", log.display());
        }
    }
}

/// Writes `was` as `now` in `text`, where it appears exactly once: a
/// socket the example stops naming is a test that no longer moves it, and
/// would bind the example's own port.
///
/// # Arguments
///
/// * `text` - The example's configuration.
/// * `was`  - What it says now, which must appear exactly once.
/// * `now`  - What to say instead.
///
/// # Returns
///
/// `text` with the one `was` replaced by `now`.
///
/// # Panics
///
/// If `text` does not say `was` exactly once.
#[allow(dead_code)]
pub fn moved(text: &str, was: &str, now: &str) -> String {
    moved_each(text, was, now, 1)
}

/// Writes `was` as `now` in `text`, where it appears exactly `times` times:
/// for what several nodes of an example name alike, such as the one bus two
/// payloads are on.
///
/// # Arguments
///
/// * `text`  - The example's configuration.
/// * `was`   - What it says now.
/// * `now`   - What to say instead.
/// * `times` - How many times `text` must say `was`.
///
/// # Returns
///
/// `text` with every `was` replaced by `now`.
///
/// # Panics
///
/// If `text` does not say `was` exactly `times` times.
#[allow(dead_code)]
pub fn moved_each(text: &str, was: &str, now: &str, times: usize) -> String {
    assert_eq!(
        text.matches(was).count(),
        times,
        "the example should say {was} {times} times"
    );
    text.replace(was, now)
}
