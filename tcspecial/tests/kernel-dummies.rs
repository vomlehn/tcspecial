//! tcspecial flown against the kernel's dummy devices for a CAN bus, a GPIO
//! chip and an IIO sensor: what makes CAN and GPIO supported, and what will
//! promote IIO, which stays experimental until its test has passed against a
//! kernel whose `iio_dummy` has a buffer -- which a stock distribution kernel's
//! has not.
//!
//! Each test flies an example -- `examples/can`, `examples/gpio` or
//! `examples/iio` -- with its sockets moved to ports nothing holds and its
//! device moved to the one this host has, and plays the device's far end
//! itself, through the kernel and not through tcs-sim: frames on the virtual
//! bus `vcan`, pulls and values of a `gpio-sim` chip's lines, and the buffer
//! of an `iio_dummy` sensor, which the kernel fills itself. What is checked
//! is that the device's data reaches the ground, and that the ground's
//! commands reach the device where the type has an output, which an IIO
//! sensor has not.
//!
//! The devices need root to make, so nothing here makes them: `sudo
//! bin/mk-kernel-dummies` does, once, and gives them to the user who ran it. A
//! test whose device is not there, or is not this user's to use, says which and
//! what makes it, and passes having tested nothing -- so that `cargo test`
//! passes on a host that has never been set up. Run them with `--nocapture` to
//! see which did:
//!
//! ```text
//! cargo test -p tcspecial --test kernel-dummies -- --nocapture
//! ```

use std::path::{Path, PathBuf};
use tcsos::net::{SocketAddr, UdpSocket};
use tcsos::time::{Duration, Instant};

use tcsprotocol::graph::telemetry;
use tcsprotocol::graph::Graph;
use tcsprotocol::node::payload::gpio;
use tcsprotocol::simulate::Host;

mod common;

/// What makes every device here, for a skipped test to name.
const MAKE: &str = "sudo bin/mk-kernel-dummies";

/// How long anything here is waited for: tcspecial's start, a datum's
/// arrival. Generous, because a loaded host running the whole workspace's
/// tests at once is slow, and a wait that ends early is a false failure.
const PATIENCE: Duration = Duration::from_secs(20);

/// An example flown: tcspecial on its configuration, connected to, with a
/// socket where the ground hears each of its sources and the address the
/// ground's commands go up to.
struct Flown {
    /// Declared first so that it is dropped first: tcspecial is stopped,
    /// and its log shown should the test have failed, before the directory
    /// it writes into goes.
    _flying: common::Flying,
    /// Holds the configuration, the log and the state file, and goes with
    /// the test.
    _dir: tempfile::TempDir,
    /// The configuration as flown, for where the ground hears a source.
    graph: Graph,
    /// Where the gateway node binds aboard: what the ground sends goes here.
    up: SocketAddr,
}

impl Flown {
    /// Flies an example with its sockets moved and its device replaced.
    ///
    /// Every example here has the same `tcspecial` section and gateway node,
    /// with ports a fixed offset from the CAN example's -- 3200, 3210, 3300
    /// and so on, each plus `offset` -- so one list moves the sockets of all
    /// three.
    ///
    /// # Arguments
    ///
    /// * `name`   - The example: `can`, `gpio` or `iio`.
    /// * `offset` - Its ports' offset from the CAN example's.
    /// * `device` - What the example says its device is, quoted as it
    ///              says it, what to say instead, and how many of its
    ///              payloads say it.
    ///
    /// # Returns
    ///
    /// The example, flying and connected to.
    ///
    /// # Panics
    ///
    /// If the example does not say what it is expected to, cannot be
    /// written, or tcspecial does not start and answer a CONNECT.
    fn fly(name: &str, offset: u16, device: (&str, &str, usize)) -> Flown {
        Flown::fly_with(name, offset, device, &[])
    }

    /// Flies an example as [`Flown::fly`] does, with some of what it says
    /// changed first.
    ///
    /// # Arguments
    ///
    /// * `name`    - The example.
    /// * `offset`  - Its ports' offset from the CAN example's.
    /// * `device`  - What it says its device is, what to say instead, and
    ///               how many of its payloads say it.
    /// * `changed` - Text it says, each once, and what to say instead.
    ///
    /// # Returns
    ///
    /// The example, flying and connected to.
    ///
    /// # Panics
    ///
    /// As [`Flown::fly`] does, and if a change's text is not said once.
    fn fly_with(
        name: &str,
        offset: u16,
        device: (&str, &str, usize),
        changed: &[(&str, &str)],
    ) -> Flown {
        let example = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../examples/{name}/{name}.ytcs"));
        let text = common::on_one_host(
            &tcsos::fs::read_to_string(&example).expect("the example reads"),
        );
        let dir = tempfile::tempdir().expect("a directory");
        // tcspecial writes its log into a directory that is there, and does
        // not make one.
        tcsos::fs::create_dir(dir.path().join("telemetry"))
            .expect("the log's directory");

        // Every port is one found free and let go, for tcspecial to bind;
        // another test can take one in between, and tcspecial then stops
        // at once. That is the test's doing, not tcspecial's, so the
        // ports are found afresh and it is flown again, a few times.
        let at = |port: u16| port + offset;
        let mut tries = 0;
        let (flying, graph, uplink) = loop {
            tries += 1;
            let mut written = text.clone();
            let (commands, take_at, uplink) =
                (common::bound(), common::free(), common::free());
            let ports = [
                (
                    format!("127.0.0.1:{}", at(3200)),
                    format!("127.0.0.1:{take_at}"),
                ),
                (
                    format!("127.0.0.1:{}", at(3210)),
                    format!("127.0.0.1:{}", common::port(&commands)),
                ),
                (
                    format!("127.0.0.1:{}", at(3300)),
                    format!("127.0.0.1:{}", common::free()),
                ),
                (
                    format!("127.0.0.1:{}", at(3310)),
                    format!("127.0.0.1:{}", common::free()),
                ),
                (
                    format!("127.0.0.1:{}", at(3400)),
                    format!("127.0.0.1:{}", common::free()),
                ),
                (
                    format!("127.0.0.1:{}", at(3410)),
                    format!("127.0.0.1:{}", common::free()),
                ),
                (
                    format!("127.0.0.1:{}", at(3500)),
                    format!("127.0.0.1:{}", common::free()),
                ),
                (
                    format!("127.0.0.1:{}", at(3510)),
                    format!("127.0.0.1:{}", common::free()),
                ),
                (format!(":{}\"", at(3600)), format!(":{uplink}\"")),
                (
                    format!("\"127.0.0.1:{}\"", at(3700)),
                    format!("\"127.0.0.1:{}\"", common::free()),
                ),
                (
                    format!("/tmp/telemetry/{name}"),
                    dir.path().join("telemetry").display().to_string(),
                ),
                (
                    format!("/tmp/tcspecial-{name}.state"),
                    dir.path().join("tcspecial.state").display().to_string(),
                ),
            ];
            // All at once, so that a port found free is never taken for
            // one of the example's, nor a change for either.
            let mut moves: Vec<(&str, &str, usize)> = ports
                .iter()
                .map(|(was, now)| (was.as_str(), now.as_str(), 1))
                .collect();
            moves.push(device);
            moves.extend(changed.iter().map(|&(was, now)| (was, now, 1)));
            written = common::moved_each(&written, &moves);

            let configuration = dir.path().join(format!("{name}.ytcs"));
            tcsos::fs::write(&configuration, &written).expect("written");
            let graph = Graph::load(&configuration).expect("it reads");

            let mut flying = common::Flying::logged(
                &configuration,
                &dir.path().join("tcspecial.log"),
            );

            // Nothing comes down until the ground has matched the start,
            // with a CONNECT carrying the configuration's checksum.
            let take: SocketAddr =
                format!("127.0.0.1:{take_at}").parse().expect("an address");
            let answered = common::connect_unless(
                &commands,
                take,
                1,
                common::checksum_of(&configuration),
                || flying.exited(),
            );
            if answered.is_some() {
                break (flying, graph, uplink);
            }
            if tries < 5 && flying.lost_a_port() {
                continue;
            }
            panic!("nothing answered a CONNECT at {take}");
        };

        Flown {
            _flying: flying,
            _dir: dir,
            graph,
            up: format!("127.0.0.1:{uplink}").parse().expect("an address"),
        }
    }

    /// Binds the socket the ground hears a source on: the gateway node's
    /// ground_base plus the source's device id, as the vehicle works it out.
    ///
    /// # Arguments
    ///
    /// * `source` - The payload whose data is listened for.
    ///
    /// # Returns
    ///
    /// The socket, with a read timeout of 100 ms.
    ///
    /// # Panics
    ///
    /// If the source sends nothing to the ground, or its port is taken.
    fn ground_of(&self, source: &str) -> UdpSocket {
        let links = telemetry::of(&self.graph)
            .expect("its links to the ground work out");
        let link = links
            .iter()
            .find(|link| link.source == source)
            .unwrap_or_else(|| panic!("{source} sends to the ground"));
        let ground = link.ground().expect("over UDP");
        let socket =
            UdpSocket::bind(format!("{}:{}", ground.address, ground.port))
                .expect("the ground's port");
        socket
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("a timeout");
        socket
    }

    /// Sends a command up, as the ground does, until `arrived` says it has.
    ///
    /// Again and again, because the gateway node may not have bound its
    /// socket the first time, and a command sent before then is lost.
    ///
    /// # Arguments
    ///
    /// * `command` - What the ground sends.
    /// * `arrived` - Says whether the device has it.
    ///
    /// # Panics
    ///
    /// If the device does not have it within PATIENCE.
    fn command_until(&self, command: &[u8], mut arrived: impl FnMut() -> bool) {
        let ground = common::bound();
        let deadline = Instant::now() + PATIENCE;
        while Instant::now() < deadline {
            ground.send_to(command, self.up).expect("sent");
            let wait = Instant::now() + Duration::from_millis(250);
            while Instant::now() < wait {
                if arrived() {
                    return;
                }
                tcsos::thread::sleep(Duration::from_millis(10));
            }
        }
        panic!(
            "{command:?}, sent up to {}, did not reach the device",
            self.up
        );
    }
}

/// Listens on `at` while `poke` is called again and again, until a datagram
/// `wanted` takes arrives.
///
/// # Arguments
///
/// * `at`     - The ground's socket for the source.
/// * `poke`   - Makes the device produce something; called every 250 ms.
/// * `wanted` - Says whether a datagram is the one looked for; every
///              datagram is shown it, in the order they arrive.
///
/// # Returns
///
/// The one it took.
///
/// # Panics
///
/// If none is taken within PATIENCE.
fn heard_while(
    at: &UdpSocket,
    mut poke: impl FnMut(),
    mut wanted: impl FnMut(&[u8]) -> bool,
) -> Vec<u8> {
    let deadline = Instant::now() + PATIENCE;
    let mut buffer = [0u8; 65536];
    while Instant::now() < deadline {
        poke();
        let wait = Instant::now() + Duration::from_millis(250);
        while Instant::now() < wait {
            if let Ok(got) = at.recv(&mut buffer) {
                if wanted(&buffer[..got]) {
                    return buffer[..got].to_vec();
                }
            }
        }
    }
    panic!("nothing arrived at {:?} for the ground", at.local_addr());
}

/// Says a test is skipped, and why, where `cargo test -- --nocapture` shows
/// it.
fn skipped(why: &str) {
    eprintln!("skipped: {why}");
}

// ---------------------------------------------------------------- CAN

/// The identifiers the CAN example's payloads use: the motor's status, which
/// goes to the ground, and its commands, which come from it.
const STATUS_ID: u32 = 0x123;
const COMMAND_ID: u32 = 0x124;

/// Finds a virtual CAN interface that is up, as tcspecial judges one.
///
/// # Returns
///
/// Its name.
///
/// # Errors
///
/// Why there is none, with what makes one.
fn virtual_can() -> Result<String, String> {
    let host = Host::this();
    let mut names: Vec<String> = tcsos::fs::read_dir("/sys/class/net")
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    for name in names {
        if !tcsprotocol::node::payload::can::judged(&host, &name).is_dummy() {
            continue;
        }
        // A bus that is there and down binds a socket and then fails every
        // send, so only one that is up will do.
        if tcsos::linux::can::is_up(&name) {
            return Ok(name);
        }
    }
    Err(format!(
        "no virtual CAN interface is up on this host: make one with \
         `{MAKE} can`"
    ))
}

/// A member of a virtual bus, filtered to one identifier and sending with
/// it: the far end of a CAN payload, as tcs-sim's CAN device is.
struct Bus(tcsos::linux::can::Bus);

impl Bus {
    /// Joins a bus.
    ///
    /// # Arguments
    ///
    /// * `interface` - The bus.
    /// * `can_id`    - The identifier received and sent with.
    ///
    /// # Returns
    ///
    /// The member, reading without waiting, so that a test looking for a
    /// frame can go on sending while it looks.
    ///
    /// # Panics
    ///
    /// If the socket cannot be put on the bus.
    fn on(interface: &str, can_id: u32) -> Bus {
        let settings = tcsos::linux::can::Settings {
            can_id,
            extended: false,
            fd: false,
            loopback: true,
        };
        Bus(tcsos::linux::can::Bus::join(interface, &settings)
            .unwrap_or_else(|refused| panic!("{interface}: {refused:?}")))
    }

    /// Sends one classic frame.
    ///
    /// # Arguments
    ///
    /// * `can_id` - The identifier, which is the one this member was made
    ///              with.
    /// * `data`   - What it carries.
    ///
    /// # Panics
    ///
    /// If the frame does not go.
    fn send(&self, can_id: u32, data: &[u8]) {
        let _ = can_id;
        assert_eq!(self.0.send(data).expect("a frame"), data.len());
    }

    /// A frame's data, where one has arrived.
    ///
    /// # Returns
    ///
    /// The data, or `None` where nothing has.
    fn received(&self) -> Option<Vec<u8>> {
        let mut data = [0u8; tcsos::linux::can::CLASSIC_BYTES];
        match self.0.receive(&mut data) {
            Ok(0) | Err(_) => None,
            Ok(got) => Some(data[..got].to_vec()),
        }
    }
}

#[test]
fn a_can_payload_is_flown_over_a_virtual_bus() {
    let interface = match virtual_can() {
        Ok(interface) => interface,
        Err(why) => return skipped(&why),
    };

    // The motor: one socket for each of its identifiers, as tcspecial has.
    let status = Bus::on(&interface, STATUS_ID);
    let commands = Bus::on(&interface, COMMAND_ID);

    let flown =
        Flown::fly("can", 0, ("\"vcan0\"", &format!("\"{interface}\""), 2));
    let ground = flown.ground_of("motor_status");

    // A status frame, sent until one reaches the ground: frames sent before
    // tcspecial had joined the bus reach nobody.
    let reading = [0xCA, 0xFE, 0x00, 0x2A];
    // Bytes, which reach the ground after the count of them, in groups of
    // seven bits.
    heard_while(
        &ground,
        || status.send(STATUS_ID, &reading),
        |datum| datum[..1] == [4] && datum[1..] == reading,
    );

    // A command, sent up until it is on the bus as a frame carrying the
    // command identifier.
    let mut got = None;
    flown.command_until(b"spin", || {
        got = commands.received();
        got.is_some()
    });
    assert_eq!(
        got.as_deref(),
        Some(&b"spin"[..]),
        "the command, as a frame"
    );
}

// ---------------------------------------------------------------- GPIO

/// Held by a test while it flies a gpio-sim chip: the tests share the one
/// chip, and its line 0 can be asked for by one tcspecial at a time.
static CHIP: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A gpio-sim chip this user may use: its device, and its first two lines'
/// directories, where a line's pull is set and its value read.
struct Chip {
    device: String,
    lines: [PathBuf; 2],
}

impl Chip {
    /// Pulls the switch's line, line 0, high or low.
    ///
    /// # Arguments
    ///
    /// * `high` - Up, where true; down, where false.
    fn pull(&self, high: bool) {
        tcsos::fs::write(
            self.lines[0].join("pull"),
            if high { "pull-up" } else { "pull-down" },
        )
        .expect("the switch's line is pulled");
    }

    /// What the lamp's line, line 1, is: what the vehicle drove it to.
    ///
    /// # Returns
    ///
    /// Its value as gpio-sim says it, `0` or `1`.
    fn lamp(&self) -> String {
        tcsos::fs::read_to_string(self.lines[1].join("value"))
            .expect("the lamp's line reads")
            .trim()
            .to_string()
    }
}

/// Finds a chip of the kernel's gpio-sim, as tcspecial judges one, whose
/// device and first line's pull are this user's to use and which has the
/// two lines the example names.
///
/// # Returns
///
/// The chip.
///
/// # Errors
///
/// Why there is none, with what makes one.
fn gpio_sim() -> Result<Chip, String> {
    let host = Host::this();
    let chips = gpio::simulated_chips(Path::new(gpio::SIM_DEVICES));
    if chips.is_empty() {
        return Err(format!(
            "no gpio-sim chip is on this host: make one with `{MAKE} gpio`"
        ));
    }

    for (name, dir) in chips {
        let device = format!("/dev/{name}");
        if !gpio::judged(&host, &device).is_dummy() {
            continue;
        }
        let lines = [dir.join("sim_gpio0"), dir.join("sim_gpio1")];
        if !lines.iter().all(tcsos::fs::is_dir) {
            continue;
        }
        // Opened to see, and closed: whether this user may is the question,
        // and nothing is asked of the chip.
        let usable = tcsos::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&device)
            .is_ok()
            && tcsos::fs::OpenOptions::new()
                .write(true)
                .open(lines[0].join("pull"))
                .is_ok();
        if usable {
            return Ok(Chip { device, lines });
        }
    }
    Err(format!(
        "a gpio-sim chip is on this host, but none with two lines whose \
         device and line 0's pull are this user's to use: remake it with \
         `{MAKE} gpio`, which gives it to the user who runs it"
    ))
}

/// The switch's edges reach the ground as records, and the ground's 1 and 0
/// drive the lamp, on a chip of the kernel's gpio-sim.
#[test]
fn a_gpio_payload_is_flown_on_a_simulated_chip() {
    let _chip = CHIP.lock().unwrap_or_else(|held| held.into_inner());
    let chip = match gpio_sim() {
        Ok(chip) => chip,
        Err(why) => return skipped(&why),
    };

    // Low before tcspecial asks for the line, so that a report of it high
    // is one of the pull up.
    chip.pull(false);

    let flown = Flown::fly(
        "gpio",
        20,
        ("\"/dev/gpiochip1\"", &format!("\"{}\"", chip.device), 2),
    );
    let ground = flown.ground_of("switch");

    // The line is pulled up and down until a report of it high arrives:
    // reports made before the ground had connected reach nobody. Every
    // report on the way is held to the form the type promises: the u8 the
    // switch's one line is bit 0 of.
    let mut high = false;
    heard_while(
        &ground,
        || {
            high = !high;
            chip.pull(high);
        },
        |datum| {
            assert_eq!(datum.len(), 1, "a line's report is a u8: {datum:?}");
            assert!(datum[0] <= 1, "one line is bit 0 alone, not {datum:?}");
            datum[0] == 1
        },
    );
    chip.pull(false);

    // The lamp lit, and put out, by what the ground sends.
    // The lamp's one line is driven with a u8, its bit 0 the line.
    flown.command_until(&[1], || chip.lamp() == "1");
    flown.command_until(&[0], || chip.lamp() == "0");
}

/// A GPIO payload reports what its lines are every `state_interval`, with
/// nothing to wake it: the gpio example's switch, reported every fifth of a
/// second, is heard high and then low with the line held still between.
#[test]
fn a_gpio_payload_read_for_its_state_reports_it_on_its_interval() {
    let _chip = CHIP.lock().unwrap_or_else(|held| held.into_inner());
    let chip = match gpio_sim() {
        Ok(chip) => chip,
        Err(why) => return skipped(&why),
    };
    chip.pull(true);

    let flown = Flown::fly_with(
        "gpio",
        20,
        ("\"/dev/gpiochip1\"", &format!("\"{}\"", chip.device), 2),
        &[(
            "        state_interval: \"500 ms\"\n",
            "        state_interval: \"200 ms\"\n",
        )],
    );
    let ground = flown.ground_of("switch");

    for level in [1u32, 0] {
        chip.pull(level == 1);
        heard_while(&ground, || {}, |datum| datum == [level as u8]);
    }
}

// ---------------------------------------------------------------- IIO

/// A sensor of the kernel's iio_dummy with its buffer enabled, and how many
/// bytes one of its sample sets takes.
struct Sensor {
    device: String,
    scan_bytes: usize,
}

/// Works out how many bytes one sample set of a buffer takes, from the
/// channels enabled in it.
///
/// Each enabled channel's `_type` says its storage, such as `le:s16/16>>0`
/// for sixteen bits, and each is aligned to its own size, which is how the
/// kernel packs a scan.
///
/// # Arguments
///
/// * `scan` - The directory the channels' `_en` and `_type` files are in.
///
/// # Returns
///
/// The bytes in one set; 0 if no channel is enabled.
fn scan_bytes(scan: &Path) -> usize {
    let mut enabled: Vec<(usize, usize)> = Vec::new();
    for entry in tcsos::fs::read_dir(scan).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(channel) = name.strip_suffix("_en") else {
            continue;
        };
        let on = tcsos::fs::read_to_string(entry.path()).unwrap_or_default();
        if on.trim() != "1" {
            continue;
        }
        let index =
            tcsos::fs::read_to_string(scan.join(format!("{channel}_index")))
                .ok()
                .and_then(|index| index.trim().parse().ok())
                .unwrap_or(usize::MAX);
        let storage =
            tcsos::fs::read_to_string(scan.join(format!("{channel}_type")))
                .ok()
                .and_then(|kind| {
                    let bits = kind.split('/').nth(1)?.split(">>").next()?;
                    bits.trim().parse::<usize>().ok()
                })
                .unwrap_or(0);
        enabled.push((index, storage / 8));
    }
    // In the order of their indices, which is the order they are packed in.
    enabled.sort();
    let mut bytes: usize = 0;
    let mut widest = 1;
    for (_, size) in enabled {
        if size == 0 {
            continue;
        }
        bytes = bytes.div_ceil(size) * size + size;
        widest = widest.max(size);
    }
    // And the set as a whole to its widest, so that the next set's first
    // channel is aligned too.
    bytes.div_ceil(widest) * widest
}

/// Finds a sensor of the kernel's iio_dummy, as tcspecial judges one, whose
/// buffer is enabled and whose device this user may read.
///
/// # Returns
///
/// The sensor.
///
/// # Errors
///
/// Why there is none, with what makes one.
fn iio_dummy() -> Result<Sensor, String> {
    let host = Host::this();
    let mut devices: Vec<PathBuf> = tcsos::fs::read_dir("/sys/bus/iio/devices")
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| {
                    path.file_name().is_some_and(|n| {
                        n.to_string_lossy().starts_with("iio:")
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    devices.sort();

    let mut unbuffered = false;
    for sys in devices {
        let node = sys.file_name().expect("a name").to_string_lossy();
        let device = format!("/dev/{node}");
        if !tcsos::fs::path_exists(Path::new(&device)) {
            // A dummy without a buffer has no device node at all, which is
            // what an iio_dummy built without its buffer makes.
            let name =
                tcsos::fs::read_to_string(sys.join("name")).unwrap_or_default();
            unbuffered |= !name.trim().is_empty()
                && tcsos::fs::is_dir(
                    Path::new("/sys/kernel/config/iio/devices/dummy")
                        .join(name.trim()),
                );
            continue;
        }
        if !tcsprotocol::node::payload::iio::is_dummy(&host, &device) {
            continue;
        }
        let (scan, enable) = if tcsos::fs::is_dir(sys.join("buffer0")) {
            (sys.join("buffer0"), sys.join("buffer0/enable"))
        } else {
            (sys.join("scan_elements"), sys.join("buffer/enable"))
        };
        let enabled = tcsos::fs::read_to_string(&enable).unwrap_or_default();
        if enabled.trim() != "1" {
            return Err(format!(
                "{device} is an iio_dummy sensor whose buffer is not enabled: \
                 remake it with `{MAKE} iio`, which enables it"
            ));
        }
        if tcsos::fs::File::open(&device).is_err() {
            return Err(format!(
                "{device} is an iio_dummy sensor that is not this user's to \
                 read: remake it with `{MAKE} iio`, which gives it to the \
                 user who runs it"
            ));
        }
        let scan_bytes = scan_bytes(&scan);
        if scan_bytes == 0 {
            return Err(format!(
                "{device} is an iio_dummy sensor with no channel enabled: \
                 remake it with `{MAKE} iio`, which enables them"
            ));
        }
        return Ok(Sensor { device, scan_bytes });
    }
    if unbuffered {
        return Err(
            "an iio_dummy sensor is on this host, but with no buffer to read: \
             this kernel's iio_dummy was built without \
             CONFIG_IIO_SIMPLE_DUMMY_BUFFER, and one built with it is needed"
                .to_string(),
        );
    }
    Err(format!(
        "no iio_dummy sensor is on this host: make one with `{MAKE} iio`, on \
         a kernel whose iio_dummy was built with CONFIG_IIO_SIMPLE_DUMMY_BUFFER"
    ))
}

/// The sensor's samples reach the ground, in whole sample sets, from the
/// buffer of a sensor of the kernel's iio_dummy, which the kernel fills
/// from its own trigger.
#[test]
fn an_iio_payload_is_flown_on_a_dummy_sensor() {
    let sensor = match iio_dummy() {
        Ok(sensor) => sensor,
        Err(why) => return skipped(&why),
    };

    let flown = Flown::fly(
        "iio",
        40,
        ("\"/dev/iio:device1\"", &format!("\"{}\"", sensor.device), 1),
    );
    let ground = flown.ground_of("sensor");

    // Nothing to poke: the kernel's trigger fills the buffer on its own.
    let heard = heard_while(&ground, || {}, |datum| !datum.is_empty());
    // Bytes, after the count of them, in groups of seven bits.
    let start = tcsprotocol::link_type::uncounted(
        &heard,
        tcsprotocol::link_type::Count::Groups,
    )
    .expect("counted");
    let got = &heard[start..];
    assert_eq!(
        got.len() % sensor.scan_bytes,
        0,
        "{} bytes is not a whole number of {}-byte sample sets",
        got.len(),
        sensor.scan_bytes
    );
}
