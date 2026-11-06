//! A simulated payload that moves discrete lines, on a gpio-sim dummy
//! device.
//!
//! A supported type, flown against the kernel's `gpio-sim` by
//! `tcspecial/tests/kernel-dummies.rs`, which moves the lines itself rather
//! than through this; the scan below is what the tests here cover, as the
//! I2C simulation's is.
//!
//! A discrete line is the one type of device the simulator cannot simply be
//! the far end of. A serial line has a pty; a socket has another socket; a
//! line is a wire, and nothing in user space can drive one. What the kernel
//! offers instead is `gpio-sim`: a chip whose lines exist in memory, with a
//! file per line that sets what the line is being pulled to. So the simulator
//! writes that file and the handler, watching the same chip through its
//! character device, sees an edge.
//!
//! Like `i2c-stub`, the module is the kernel's and configuring it needs root,
//! so the simulator does not make a chip: it finds one and says how to make
//! one if there is none.
//!
//! What a simulated device decides here is not bytes but a pattern. Every
//! packet interval each of its lines is pulled the other way, so a device
//! watching one line produces one event an interval and a device watching
//! three produces three. The packet and segment sizes mean nothing to it --
//! an event is sixteen bytes whatever a file says -- and neither do the
//! faults, which alter bytes that do not exist here. Said plainly rather than
//! quietly ignored: a file that asks a set of discretes to drop a tenth of
//! its packets has asked for something this kind cannot do.
//!
//! A node that gives a `sim_address` needs no chip: in a run that simulates
//! it, tcspecial reaches its lines over UDP there, and this answers for them
//! -- see [`run_gpio_udp_node`]. One datagram is one state of the lines, an
//! ASCII `1` or `0` for each in the node's order, as
//! [`tcslibgs::node::payload::gpio::state_said`] says it, both ways: what
//! the vehicle drives its lines to, and what watched lines are. The lines are
//! moved and watched as a chip's are, by the same rules; only where a level
//! goes differs.
//!
//! See [`crate::payload`] for the pacing, which is common to every kind.

use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::payload::{
    could_not_simulate, wait_out_packet, Pacing, PayloadConfig, PayloadStats,
};

/// Where the kernel puts a simulated chip's lines: see
/// [`tcslibgs::node::payload::gpio::simulated_chips`].
const SIM_DEVICES: &str = tcslibgs::node::payload::gpio::SIM_DEVICES;

/// How many bytes one line event takes as device data, which is what this
/// counts as having sent.
const EVENT_BYTES: u64 = tcslibgs::node::payload::gpio::EVENT_BYTES as u64;

/// A simulated chip's line, as a path to write a pull to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SimulatedLine {
    /// The file that sets what the line is pulled to: `pull-up` or
    /// `pull-down`.
    pub(crate) pull: PathBuf,
}

impl SimulatedLine {
    /// The file that says what the line is: `1` or `0`, whichever drove it
    /// last -- this end's pull, or the vehicle driving it as an output.
    pub(crate) fn value(&self) -> PathBuf {
        self.pull.with_file_name("value")
    }
}

/// How often a driven chip's lines are looked at. Far quicker than any
/// decision a vehicle makes about a heater, and slow enough to cost nothing.
const WATCH_EVERY: std::time::Duration = std::time::Duration::from_millis(20);

/// Finds the simulated lines of a chip, where the kernel has any.
///
/// Where they are is tcslibgs's to say, once, for everything that looks:
/// see [`tcslibgs::node::payload::gpio::simulated_chips`]. The chip is
/// matched by its node's name, not its number, which is given out as chips
/// appear, exactly as an `i2c-stub` adapter's bus is.
///
/// # Arguments
///
/// * `devices` - Where to look, given so that a test can hand over a tree
///               of its own: the scan is the part of this that can be checked
///               without the module loaded.
/// * `chip`    - The character device a handler was told to open, such as
///               `/dev/gpiochip3`.
/// * `lines`   - The offsets of the lines wanted on it.
///
/// # Returns
///
/// The wanted lines of the matching chip, or `None` if no simulated chip is
/// that one or `devices` cannot be read.
pub(crate) fn lines_of(
    devices: &Path,
    chip: &str,
    lines: &[u32],
) -> Option<Vec<SimulatedLine>> {
    let dir = tcslibgs::node::payload::gpio::simulated_chip(devices, chip)?;
    Some(
        lines
            .iter()
            .map(|line| SimulatedLine {
                pull: dir.join(format!("sim_gpio{line}")).join("pull"),
            })
            .collect(),
    )
}

/// What to say when there is no simulated chip to be.
///
/// The two commands that make one, as the I2C simulation names its module: a
/// configuration that runs on one machine and not the next is a set whose
/// kernel was set up differently, and the message is the whole of what says
/// which.
pub(crate) fn how_to_make_one(chip: &str) -> String {
    format!(
        "no dummy GPIO chip is at {chip}. Load the module with \
         \"modprobe gpio-sim\" and make a chip through configfs under \
         /sys/kernel/config/gpio-sim, which needs root; the chip's node has to \
         be the one this device names"
    )
}

/// Runs a simulated device that moves discrete lines, or watches them
/// where the vehicle drives them.
///
/// # Arguments
///
/// * `config`  - The payload's configuration.
/// * `running` - Cleared to stop it; the function returns soon after.
/// * `stats`   - Where it counts what it sends and receives.
pub fn run_gpio_node(
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
) {
    let lines = match lines_of(
        Path::new(SIM_DEVICES),
        &config.address,
        &config.gpio_lines,
    ) {
        Some(lines) => lines,
        None => {
            could_not_simulate(&config, how_to_make_one(&config.address));
            return;
        }
    };
    config.standing.made();

    if config.driven {
        watch_driven_lines(&config, &running, &stats, &lines);
        return;
    }
    if let Some(world @ crate::node::physics::Attachment::Pulls { .. }) =
        &config.attachment
    {
        pull_to_the_world(&config, world, &running, &stats, &lines);
        return;
    }

    // What each line is being pulled to, which is what moving it means.
    let mut high = false;

    while running.load(Ordering::SeqCst) {
        let started = Instant::now();
        let pacing = Pacing::read(&config);

        // Nothing is read: these are inputs, which is why a GPIO device
        // cannot be triggered. A device whose interval is nought is a chip
        // whose lines do not move, as a device with no interval is silent
        // everywhere else.
        if pacing.sending && pacing.produces() {
            high = !high;
            let pull = if high { "pull-up" } else { "pull-down" };

            let mut moved = 0u64;
            for line in &lines {
                match std::fs::write(&line.pull, pull) {
                    Ok(()) => moved += 1,
                    Err(e) => {
                        eprintln!(
                            "{}: cannot move {}: {e}",
                            config.name,
                            line.pull.display()
                        );
                    }
                }
            }

            if moved > 0 {
                let mut guard = stats.lock().unwrap();
                // One event per line moved, and an event is its own size:
                // what this device produces is not bytes a file chose.
                guard.bytes_sent += moved * EVENT_BYTES;
                guard.a_packet_has_gone(pull.as_bytes());
            }
        }

        wait_out_packet(&pacing, started, &running);
    }
}

/// Answers for discrete lines over UDP, at their node's `sim_address`, with no
/// chip.
///
/// Lines the vehicle drives are taken from what it sends, as a chip's are
/// read from its values: where they all agree it is a decision, and the part
/// they are joined to in the world is driven by it; a change is counted as a
/// datum received. Lines the vehicle watches are said whenever they change --
/// to what the world says they sense, or every packet interval the other way
/// where they are joined to nothing -- and again to a vehicle that has just
/// said where it is, by an empty datagram.
///
/// # Arguments
///
/// * `config`  - The payload's configuration: its address and port are the
///               `sim_address`.
/// * `running` - Cleared to stop it; the function returns soon after.
/// * `stats`   - Where it counts what it sends and receives.
pub fn run_gpio_udp_node(
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
) {
    let at = format!("{}:{}", config.address, config.port);
    let socket = match UdpSocket::bind(&at).and_then(|socket| {
        socket.set_read_timeout(Some(WATCH_EVERY))?;
        Ok(socket)
    }) {
        Ok(socket) => socket,
        Err(e) => {
            could_not_simulate(
                &config,
                format!("cannot answer for its lines at {at}: {e}"),
            );
            return;
        }
    };
    config.standing.made();

    let count = config.gpio_lines.len();
    let mut vehicle: Option<SocketAddr> = None;
    let mut datagram = [0u8; tcslibgs::node::payload::gpio::MOST_LINES + 1];
    // What the vehicle last drove the lines to, and what watched lines were
    // last said to be: nothing yet, so the first of each counts.
    let mut driven: Option<u64> = None;
    let mut said: Option<u64> = None;
    // What watched lines are to be: from the world, or the pattern.
    let mut levels = vec![false; count];
    let mut high = false;
    let mut moved = Instant::now();

    while running.load(Ordering::SeqCst) {
        match socket.recv_from(&mut datagram) {
            Ok((0, from)) => {
                // A hello: say the lines again, to where it came from.
                vehicle = Some(from);
                said = None;
            }
            Ok((n, from)) => {
                vehicle = Some(from);
                if config.driven {
                    took_a_drive(
                        &config,
                        &stats,
                        count,
                        &datagram[..n],
                        &mut driven,
                    );
                }
            }
            Err(ref e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::ConnectionRefused
                ) => {}
            Err(e) => {
                eprintln!(
                    "{}: cannot hear the vehicle at {at}: {e}",
                    config.name
                );
                crate::payload::wait_while_running(WATCH_EVERY, &running);
            }
        }
        if config.driven {
            continue;
        }

        let pacing = Pacing::read(&config);
        let now = match &config.attachment {
            Some(world @ crate::node::physics::Attachment::Pulls { .. }) => {
                if pacing.sending && world.levels(&mut levels) {
                    Some(
                        levels.iter().enumerate().fold(0u64, |bits, (n, l)| {
                            bits | (u64::from(*l) << n)
                        }),
                    )
                } else {
                    said
                }
            }
            _ => {
                if pacing.sending
                    && pacing.produces()
                    && moved.elapsed() >= pacing.packet_interval
                {
                    high = !high;
                    moved = Instant::now();
                }
                Some(if high { (1u64 << count) - 1 } else { 0 })
            }
        };
        if let (Some(to), Some(now)) = (vehicle, now) {
            if said != Some(now) {
                let state =
                    tcslibgs::node::payload::gpio::state_said(count, now);
                match socket.send_to(&state, to) {
                    Ok(_) => {
                        let changed = said.map_or(count as u64, |was| {
                            u64::from((was ^ now).count_ones())
                        });
                        said = Some(now);
                        let mut guard = stats.lock().unwrap();
                        guard.bytes_sent += changed * EVENT_BYTES;
                        guard.a_packet_has_gone(&state);
                    }
                    Err(e) => eprintln!(
                        "{}: cannot tell the vehicle at {to} what its lines \
                         are: {e}",
                        config.name
                    ),
                }
            }
        }
    }
}

/// Takes what the vehicle drove its lines to, from a datagram.
///
/// Where every line agrees it is a decision, and the part the lines are
/// joined to in the world is driven by it, each time -- the part keeps the
/// latest, and an unpowered one stays off -- and a change is counted as a
/// datum received. A datagram that is not a state of the lines is said, and
/// taken for nothing.
///
/// # Arguments
///
/// * `config`   - The payload's configuration.
/// * `stats`    - Where a change is counted.
/// * `count`    - How many lines the node has.
/// * `datagram` - What arrived.
/// * `driven`   - What the lines were last driven to, which this updates.
fn took_a_drive(
    config: &PayloadConfig,
    stats: &std::sync::Mutex<PayloadStats>,
    count: usize,
    datagram: &[u8],
    driven: &mut Option<u64>,
) {
    let now = match tcslibgs::node::payload::gpio::state_read(count, datagram) {
        Ok(now) => now,
        Err(why) => {
            eprintln!("{}: the vehicle sent {why}", config.name);
            return;
        }
    };
    let all = (1u64 << count) - 1;
    if now == 0 || now == all {
        crate::node::physics::driven(
            &config.attachment,
            &config.name,
            &datagram[..1],
        );
    }
    if *driven != Some(now) {
        *driven = Some(now);
        let mut guard = stats.lock().unwrap();
        guard.a_packet_has_come(datagram);
        guard.bytes_recv += count as u64 * EVENT_BYTES;
    }
}

/// Pulls each line of a chip to what the world says it senses -- a line on a
/// heater, high while the heater is heating -- rather than to a pattern.
///
/// Looked at every packet interval, which is how often this payload looks
/// at the world, and a line is moved only when what it senses changes, so
/// the vehicle, polling or watching for edges, reads what the part is doing
/// within an interval of its doing it.
/// While the payload is disabled in its panel its lines are left where they
/// are, as a disabled payload sends nothing.
///
/// # Arguments
///
/// * `config`  - The payload's configuration.
/// * `world`   - What its lines are pulled to: an `Attachment::Pulls`.
/// * `running` - Cleared to stop it.
/// * `stats`   - Where each move is counted.
/// * `lines`   - The chip's lines, from [`lines_of`], in the node's order.
fn pull_to_the_world(
    config: &PayloadConfig,
    world: &crate::node::physics::Attachment,
    running: &AtomicBool,
    stats: &std::sync::Mutex<PayloadStats>,
    lines: &[SimulatedLine],
) {
    // Made here, before the loop, and reused: what each line is to be, and
    // what it was last pulled to -- nothing yet, so the first look pulls
    // every line.
    let mut levels = vec![false; lines.len()];
    let mut pulled: Vec<Option<bool>> = vec![None; lines.len()];
    while running.load(Ordering::SeqCst) {
        let started = Instant::now();
        let pacing = Pacing::read(config);
        if pacing.sending && world.levels(&mut levels) {
            let mut moved = 0u64;
            for ((line, &level), was) in
                lines.iter().zip(&levels).zip(pulled.iter_mut())
            {
                if *was == Some(level) {
                    continue;
                }
                let pull = if level { "pull-up" } else { "pull-down" };
                match std::fs::write(&line.pull, pull) {
                    Ok(()) => {
                        moved += 1;
                        *was = Some(level);
                    }
                    Err(e) => eprintln!(
                        "{}: cannot move {}: {e}",
                        config.name,
                        line.pull.display()
                    ),
                }
            }
            if moved > 0 {
                let mut guard = stats.lock().unwrap();
                guard.bytes_sent += moved * EVENT_BYTES;
                guard.a_packet_has_gone(b"pulled");
            }
        }
        wait_out_packet(&pacing, started, running);
    }
}

/// Watches the lines of a chip the vehicle drives -- a heater, a valve -- and
/// count each time it moves them, as a datum received.
///
/// The vehicle drives them as outputs, and gpio-sim keeps what each line is
/// in its `value`, so what the vehicle decided can be read back without being
/// in its way. A move is the lines' values, one byte each, `1` or `0`; where
/// they all agree it is a decision, and the heater it is joined to in the
/// world is driven by it, as one reached over a socket is.
///
/// Disabled -- its panel's button off -- the part is unpowered: a move is
/// counted, as every simulated payload counts what arrives, and kept as the
/// latest decision, and the part stays off until it is enabled. See
/// [`crate::node::physics::Power`].
///
/// # Arguments
///
/// * `config`  - The payload's configuration.
/// * `running` - Cleared to stop it.
/// * `stats`   - Where each move is counted.
/// * `lines`   - The chip's lines, from [`lines_of`].
fn watch_driven_lines(
    config: &PayloadConfig,
    running: &AtomicBool,
    stats: &std::sync::Mutex<PayloadStats>,
    lines: &[SimulatedLine],
) {
    use std::io::Read;

    // Made here, before the loop, and reused: each line's file, and one byte
    // a line for what it was and what it is.
    let values: Vec<PathBuf> = lines.iter().map(SimulatedLine::value).collect();
    let mut was: Vec<u8> = vec![0; lines.len()];
    let mut now: Vec<u8> = vec![0; lines.len()];
    let mut read = [0u8; 4];
    let mut first = true;
    while running.load(Ordering::SeqCst) {
        for (path, value) in values.iter().zip(now.iter_mut()) {
            // Opened each time, as a sysfs attribute is read: its value is
            // what it is when it is opened.
            let high = std::fs::File::open(path)
                .and_then(|mut file| file.read(&mut read))
                .is_ok_and(|n| n > 0 && read[0] == b'1');
            *value = if high { b'1' } else { b'0' };
        }
        if now != was && !first {
            if now.iter().all(|&v| v == now[0]) {
                crate::node::physics::driven(
                    &config.attachment,
                    &config.name,
                    &now[..1],
                );
            }
            let mut guard = stats.lock().unwrap();
            guard.a_packet_has_come(&now);
            guard.bytes_recv += lines.len() as u64 * EVENT_BYTES;
        }
        first = false;
        was.copy_from_slice(&now);
        crate::payload::wait_while_running(WATCH_EVERY, running);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A simulated chip is found by the node it became, not by its number.
    ///
    /// Which is the whole of the scan: a `gpio-sim` chip's node is numbered as
    /// chips appear, so the number a device set names is not an identity -- the
    /// same reason the I2C simulation scans for its adapter rather than
    /// assuming one.
    #[test]
    fn a_simulated_chip_is_found_by_the_node_it_became() {
        let tree = tempfile::tempdir().expect("a tree of our own");

        // Something that is not a simulated chip, to be passed over.
        std::fs::create_dir(tree.path().join("serial8250"))
            .expect("another device");

        // And one that is, gpiochip3, whose lines the kernel puts in a
        // directory of the chip's name under its gpio-sim device.
        let bank = tree.path().join("gpio-sim.1").join("gpiochip3");
        std::fs::create_dir_all(&bank).expect("the chip");
        // And another simulated chip, which is not the one wanted.
        std::fs::create_dir_all(tree.path().join("gpio-sim.0/gpiochip1"))
            .expect("another chip");

        let found = lines_of(tree.path(), "/dev/gpiochip3", &[0, 5])
            .expect("the dummy chip at gpiochip3");
        assert_eq!(
            found,
            vec![
                SimulatedLine {
                    pull: bank.join("sim_gpio0").join("pull")
                },
                SimulatedLine {
                    pull: bank.join("sim_gpio5").join("pull")
                },
            ]
        );

        // A chip with no dummy device behind it is not found, which is what
        // the message below is for.
        assert_eq!(lines_of(tree.path(), "/dev/gpiochip9", &[0]), None);
    }

    /// A chip the vehicle drives is watched: each time its lines move, a
    /// datum is counted as received, and nothing is counted while they stay.
    #[test]
    fn a_driven_chip_counts_each_move_of_its_lines() {
        use std::sync::atomic::{AtomicBool, AtomicU32};
        use std::sync::{Arc, Mutex};

        let tree = tempfile::tempdir().expect("a tree of our own");
        let lines: Vec<SimulatedLine> = [0]
            .iter()
            .map(|n| {
                let dir = tree.path().join(format!("sim_gpio{n}"));
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join("value"), "0\n").unwrap();
                SimulatedLine {
                    pull: dir.join("pull"),
                }
            })
            .collect();
        let config = PayloadConfig {
            attachment: None,
            thermostats: Vec::new(),
            name: "heaters".to_string(),
            protocol: crate::payload::PayloadProtocol::Gpio,
            address: "/dev/gpiochip3".to_string(),
            port: 0,
            bus_address: 0,
            can_id: 0,
            can_fd: false,
            gpio_lines: vec![0],
            triggered: false,
            faults: Default::default(),
            own_address: None,
            own_port: None,
            packet_size: Arc::new(AtomicU32::new(0)),
            segment_size: Arc::new(AtomicU32::new(0)),
            packet_interval_ms: Arc::new(AtomicU32::new(0)),
            segment_interval_ms: Arc::new(AtomicU32::new(0)),
            sending: Arc::new(AtomicBool::new(true)),
            standing: Default::default(),
            driven: true,
        };
        let running = Arc::new(AtomicBool::new(true));
        let stats = Arc::new(Mutex::new(PayloadStats::default()));
        let watching = {
            let (running, stats, lines) =
                (running.clone(), stats.clone(), lines.clone());
            std::thread::spawn(move || {
                watch_driven_lines(&config, &running, &stats, &lines)
            })
        };
        let settle = || std::thread::sleep(WATCH_EVERY * 5);

        settle();
        assert_eq!(stats.lock().unwrap().packets_recv, 0, "nothing has moved");
        for line in &lines {
            std::fs::write(line.value(), "1\n").unwrap();
        }
        settle();
        assert_eq!(stats.lock().unwrap().packets_recv, 1, "the line moved");
        settle();
        assert_eq!(stats.lock().unwrap().packets_recv, 1, "and stayed");

        running.store(false, std::sync::atomic::Ordering::SeqCst);
        watching.join().unwrap();
    }

    /// A payload of discrete lines answered for over UDP, for the tests
    /// below: no world, sending, at a port of its own on the loopback
    /// interface.
    ///
    /// # Arguments
    ///
    /// * `driven`   - Whether the vehicle drives the lines.
    /// * `lines`    - Their offsets.
    /// * `interval` - Its packet interval, in milliseconds.
    ///
    /// # Returns
    ///
    /// The configuration, and its address.
    fn over_udp(
        driven: bool,
        lines: Vec<u32>,
        interval: u32,
    ) -> (PayloadConfig, SocketAddr) {
        use std::sync::atomic::AtomicU32;

        // A port nothing has, found by asking for one and letting it go.
        let at = UdpSocket::bind("127.0.0.1:0")
            .and_then(|s| s.local_addr())
            .expect("a free port");
        let config = PayloadConfig {
            attachment: None,
            thermostats: Vec::new(),
            name: "lines".to_string(),
            protocol: crate::payload::PayloadProtocol::GpioUdp,
            address: at.ip().to_string(),
            port: at.port(),
            bus_address: 0,
            can_id: 0,
            can_fd: false,
            gpio_lines: lines,
            triggered: false,
            faults: Default::default(),
            own_address: None,
            own_port: None,
            packet_size: Arc::new(AtomicU32::new(0)),
            segment_size: Arc::new(AtomicU32::new(0)),
            packet_interval_ms: Arc::new(AtomicU32::new(interval)),
            segment_interval_ms: Arc::new(AtomicU32::new(0)),
            sending: Arc::new(AtomicBool::new(true)),
            standing: Default::default(),
            driven,
        };
        (config, at)
    }

    /// Starts a payload of lines answered for over UDP.
    ///
    /// # Arguments
    ///
    /// * `config` - Its configuration.
    ///
    /// # Returns
    ///
    /// The flag that stops it, its counts, and its thread.
    fn started(
        config: PayloadConfig,
    ) -> (
        Arc<AtomicBool>,
        Arc<std::sync::Mutex<PayloadStats>>,
        std::thread::JoinHandle<()>,
    ) {
        let running = Arc::new(AtomicBool::new(true));
        let stats = Arc::new(std::sync::Mutex::new(PayloadStats::default()));
        let thread = {
            let (running, stats) = (running.clone(), stats.clone());
            std::thread::spawn(move || {
                run_gpio_udp_node(config, running, stats)
            })
        };
        std::thread::sleep(WATCH_EVERY * 5);
        (running, stats, thread)
    }

    /// Lines the vehicle drives over UDP are taken from what it sends, a
    /// change counted as a datum received and a repeat not.
    #[test]
    fn lines_driven_over_udp_count_each_change() {
        let (config, at) = over_udp(true, vec![4, 5], 0);
        let (running, stats, thread) = started(config);
        let vehicle = UdpSocket::bind("127.0.0.1:0").expect("a socket");
        let received = || stats.lock().unwrap().packets_recv;

        for (state, count) in [(b"11", 1), (b"11", 1), (b"00", 2)] {
            vehicle.send_to(state, at).expect("sent");
            std::thread::sleep(WATCH_EVERY * 5);
            assert_eq!(received(), count, "after {state:?}");
        }
        // Not a state of the two lines: said, and taken for nothing.
        vehicle.send_to(b"1", at).expect("sent");
        std::thread::sleep(WATCH_EVERY * 5);
        assert_eq!(received(), 2);

        running.store(false, Ordering::SeqCst);
        thread.join().unwrap();
    }

    /// Lines the vehicle watches over UDP, joined to nothing, are said to the
    /// vehicle once it says where it is, and every packet interval the other
    /// way, every line together.
    #[test]
    fn lines_watched_over_udp_are_said_as_they_move() {
        let (config, at) = over_udp(false, vec![3, 7], 50);
        let (running, _stats, thread) = started(config);
        let vehicle = UdpSocket::bind("127.0.0.1:0").expect("a socket");
        vehicle
            .set_read_timeout(Some(std::time::Duration::from_secs(1)))
            .expect("a timeout");

        vehicle.send_to(&[], at).expect("the hello");
        let mut said = [0u8; 8];
        let mut states = Vec::new();
        for _ in 0..3 {
            let n = vehicle.recv(&mut said).expect("a state");
            states.push(said[..n].to_vec());
        }
        assert!(
            states.iter().all(|s| s == b"11" || s == b"00"),
            "{states:?}"
        );
        assert!(states.windows(2).all(|w| w[0] != w[1]), "{states:?}");

        running.store(false, Ordering::SeqCst);
        thread.join().unwrap();
    }

    /// With no simulated chip at all, nothing is found.
    #[test]
    fn with_no_simulated_chip_nothing_is_found() {
        let tree = tempfile::tempdir().expect("an empty tree");
        assert_eq!(lines_of(tree.path(), "/dev/gpiochip0", &[0]), None);
    }

    /// The message says how to make a chip, and which chip was wanted.
    ///
    /// A device set that runs on one machine and not the next is a set whose
    /// kernel was set up differently, and this is what says so rather than
    /// leaving a panel at nought with no reason.
    #[test]
    fn the_message_says_how_to_make_a_chip() {
        let said = how_to_make_one("/dev/gpiochip3");
        assert!(
            said.contains("/dev/gpiochip3")
                && said.contains("modprobe gpio-sim")
                && said.contains("root"),
            "the message should name the chip, the module and the privilege: \
             {said}"
        );
    }

    /// Moving a line writes what the kernel reads as a pull.
    ///
    /// Against files of our own, which is as far as this can go without the
    /// module: what the kernel does with `pull-up` is the kernel's, and what
    /// this is responsible for is writing the word to the right file.
    #[test]
    fn moving_a_line_writes_a_pull() {
        let tree = tempfile::tempdir().expect("a tree of our own");
        let bank = tree.path().join("gpio-sim.0").join("gpiochip0");
        std::fs::create_dir_all(bank.join("sim_gpio2")).expect("the line");

        let lines =
            lines_of(tree.path(), "/dev/gpiochip0", &[2]).expect("the line");
        std::fs::write(&lines[0].pull, "pull-up")
            .expect("the line is pulled up");

        assert_eq!(
            std::fs::read_to_string(&lines[0].pull)
                .expect("what it is pulled to"),
            "pull-up"
        );
    }
}
