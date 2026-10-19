//! A simulated device that moves discrete lines.
//!
//! **Experimental, and not yet supported.** Written against the kernel's
//! `gpio-sim` and run against no hardware; the scan below is what the tests
//! here cover, as the I2C stand-in's is.
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
//! See [`crate::payload`] for the pacing, which is common to every kind.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::payload::{wait_out_packet, Pacing, PayloadConfig, PayloadStats};

/// Where the kernel puts a simulated chip's lines.
///
/// A `gpio-sim` chip appears as a platform device, and under it one directory
/// per bank with one per line: `pull` is what the line is being pulled to,
/// which is what a simulator writes.
const SIM_DEVICES: &str = "/sys/devices/platform";

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

/// Find the simulated lines of `chip`, where the kernel has any.
///
/// `devices` is where to look, given so that a test can hand over a tree of
/// its own: the scan is the part of this that can be checked without the
/// module loaded.
///
/// `chip` is the character device a handler was told to open --
/// `/dev/gpiochip3` -- and what matches it is a simulated bank whose
/// `chip_name` file holds that node's name. The number is not an identity: a
/// simulated chip's node is numbered as chips appear, exactly as an
/// `i2c-stub` adapter's bus is.
pub(crate) fn lines_of(devices: &Path, chip: &str, lines: &[u32]) -> Option<Vec<SimulatedLine>> {
    let node = Path::new(chip).file_name()?.to_string_lossy().into_owned();

    for device in std::fs::read_dir(devices).ok()? {
        let Ok(device) = device else { continue };
        if !device
            .file_name()
            .to_string_lossy()
            .starts_with("gpio-sim.")
        {
            continue;
        }

        // One directory per bank, each with the node it became.
        let Ok(banks) = std::fs::read_dir(device.path()) else {
            continue;
        };
        for bank in banks {
            let Ok(bank) = bank else { continue };
            let named = bank.path().join("chip_name");
            let Ok(name) = std::fs::read_to_string(&named) else {
                continue;
            };
            if name.trim() != node {
                continue;
            }

            // The bank is this chip, so its line directories are the lines.
            return Some(
                lines
                    .iter()
                    .map(|line| SimulatedLine {
                        pull: bank.path().join(format!("sim_gpio{line}")).join("pull"),
                    })
                    .collect(),
            );
        }
    }

    None
}

/// What to say when there is no simulated chip to be.
///
/// The two commands that make one, as the I2C stand-in names its module: a
/// configuration that runs on one machine and not the next is a set whose
/// kernel was set up differently, and the message is the whole of what says
/// which.
pub(crate) fn how_to_make_one(chip: &str) -> String {
    format!(
        "no simulated GPIO chip stands in for {chip}. Load the module with \
         \"modprobe gpio-sim\" and make a chip through configfs under \
         /sys/kernel/config/gpio-sim, which needs root; the chip's node has to be \
         the one this device names"
    )
}

/// Run a simulated device that moves discrete lines.
pub fn run_gpio_node(
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
) {
    let lines = match lines_of(Path::new(SIM_DEVICES), &config.address, &config.gpio_lines) {
        Some(lines) => lines,
        None => {
            eprintln!("{}: {}", config.name, how_to_make_one(&config.address));
            return;
        }
    };

    // What each line is being pulled to, which is what moving it means.
    let mut high = false;

    while running.load(Ordering::SeqCst) {
        let started = Instant::now();
        let pacing = Pacing::read(&config);

        // Nothing is read: these are inputs, which is why a GPIO device
        // cannot be triggered. A device whose interval is nought is a chip
        // whose lines do not move, as a device with no interval is silent
        // everywhere else.
        if pacing.produces() {
            high = !high;
            let pull = if high { "pull-up" } else { "pull-down" };

            let mut moved = 0u64;
            for line in &lines {
                match std::fs::write(&line.pull, pull) {
                    Ok(()) => moved += 1,
                    Err(e) => {
                        eprintln!("{}: cannot move {}: {e}", config.name, line.pull.display());
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A simulated chip is found by the node it became, not by its number.
    ///
    /// Which is the whole of the scan: a `gpio-sim` chip's node is numbered
    /// as chips appear, so the number a device set names is not an identity
    /// -- the same reason the I2C stand-in scans for its adapter rather than
    /// assuming one.
    #[test]
    fn a_simulated_chip_is_found_by_the_node_it_became() {
        let tree = tempfile::tempdir().expect("a tree of our own");

        // Something that is not a simulated chip, to be passed over.
        std::fs::create_dir(tree.path().join("serial8250")).expect("another device");

        // And one that is, whose bank became gpiochip3.
        let bank = tree.path().join("gpio-sim.0").join("gpiochip1");
        std::fs::create_dir_all(&bank).expect("the bank");
        std::fs::write(bank.join("chip_name"), "gpiochip3\n").expect("what it became");

        let found = lines_of(tree.path(), "/dev/gpiochip3", &[0, 5])
            .expect("the chip standing in for gpiochip3");
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

        // A chip nothing stands in for is not found, which is what the
        // message below is for.
        assert_eq!(lines_of(tree.path(), "/dev/gpiochip9", &[0]), None);
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
            "the message should name the chip, the module and the privilege: {said}"
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
        std::fs::write(bank.join("chip_name"), "gpiochip0").expect("what it became");

        let lines = lines_of(tree.path(), "/dev/gpiochip0", &[2]).expect("the line");
        std::fs::write(&lines[0].pull, "pull-up").expect("the line is pulled up");

        assert_eq!(
            std::fs::read_to_string(&lines[0].pull).expect("what it is pulled to"),
            "pull-up"
        );
    }
}
