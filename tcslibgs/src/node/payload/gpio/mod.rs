//! GPIO payloads: discrete lines, as one whole number.
//!
//! **Supported**, as flown against the kernel's simulated chip, `gpio-sim`, by
//! `tcspecial/tests/kernel-dummies.rs`; nothing has been run against hardware.
//! See "What Is Supported and What Is Experimental" in `docs/design.rst`.
//!
//! A payload is not always a stream of data. A deployment switch, an arming
//! line, a thermostat's output, a payload's own "I am alive" line: each is one
//! wire that is high or low. Linux offers those through a GPIO chip's
//! character device, where a program asks for a set of lines and reads or
//! sets their values together.
//!
//! So this is the one kind whose payload data is made rather than carried: a
//! set of lines is one unsigned whole number, the narrowest of `u8`, `u16`,
//! `u32` and `u64` with a bit for each line, bit 0 the first line the node
//! lists. It is what a set of lines is driven with and what it reports, every
//! `state_interval`: see "Typed links" in the design. It states no
//! `read_size`, its width being its lines'.
//!
//! Three node types, one for each thing a set of lines can be to the graph,
//! told apart by the terminals each has -- see [`Kind`]:
//!
//! * `gpio_in`, lines watched. What they are is world data, leaving by
//!   `world-out` towards the ground every `state_interval`; what arrives at
//!   `world-in` asks for a report of what the lines are now.
//! * `gpio_out`, lines driven. What arrives at `moc-in` sets them -- a
//!   thermostat's decision, a command -- and is passed on out of
//!   `moc-out`, towards whatever the lines drive.
//! * `gpio_inout`, driven as a `gpio_out`'s are, and saying so: what arrives
//!   at `moc-in` drives the lines and goes on by `moc-out` where it changes
//!   them, and every `state_interval` the last value the lines were told goes
//!   out by `world-out` towards the ground. A heater a thermostat switches,
//!   whose state the ground follows on a beat.
//!
//! The type says which, and nothing else does. A single `gpio` with a
//! `direction` said it once, and drew a set of driven lines with a payload's
//! terminals -- a sink for moc data and a source for world data -- when what
//! goes through them is moc data both ways. A file still saying `gpio` is
//! told the new words: see `crate::graph::renamed`.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::endpoint_config::{
    bad, parse_u32, reject_foreign_fields, reject_stream, require,
    EndpointConfigResult, GroupKind, LinkTerms, Scalar,
};

/// Says a state of a node's lines as a datagram, as tcspecial and tcssim
/// send one another simulated lines' states over UDP: an ASCII `1` or `0` for
/// each line, in the order of the node's lines, lowest offset first -- `10`
/// for lines 4 and 5 with 4 asserted.
///
/// # Arguments
///
/// * `count` - How many lines the node has.
/// * `bits`  - A bit per line by its place, set where it is asserted.
///
/// # Returns
///
/// The datagram.
pub fn state_said(count: usize, bits: u64) -> Vec<u8> {
    (0..count)
        .map(|place| {
            if bits & (1u64 << place) != 0 {
                b'1'
            } else {
                b'0'
            }
        })
        .collect()
}

/// Reads a state of a node's lines from a datagram: see [`state_said`].
///
/// # Arguments
///
/// * `count`    - How many lines the node has.
/// * `datagram` - What arrived.
///
/// # Returns
///
/// A bit per line by its place, set where it is asserted.
///
/// # Errors
///
/// What is wrong, if the datagram is not one `1` or `0` for each line: a
/// state that cannot be read is not guessed at.
pub fn state_read(count: usize, datagram: &[u8]) -> Result<u64, String> {
    let refused = || {
        format!(
            "{:?} is not a 1 or a 0 for each of the {count} lines",
            String::from_utf8_lossy(datagram)
        )
    };
    if datagram.len() != count {
        return Err(refused());
    }
    datagram
        .iter()
        .enumerate()
        .try_fold(0u64, |bits, (place, said)| match said {
            b'1' => Ok(bits | (1u64 << place)),
            b'0' => Ok(bits),
            _ => Err(refused()),
        })
}

/// The greatest number of lines one request may name, which is the kernel's
/// own limit on a line request.
pub const MOST_LINES: usize = 64;

/// What holds a line that nothing is driving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bias {
    /// Nothing: the line is left as the board wired it, which is the default
    /// and what a line with its own pull resistor wants.
    None,
    /// Held high until something pulls it down.
    PullUp,
    /// Held low until something pulls it up.
    PullDown,
}

/// Which of the three GPIO node types a node is: whether its lines are
/// watched, driven, or some of each.
///
/// The one place the three words are, and what each means, so that the
/// reader, the terminals, the drawing and the vehicle cannot disagree about
/// which is which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// `gpio_in`: watched.
    In,
    /// `gpio_out`: driven.
    Out,
    /// `gpio_inout`: driven, and reporting what it was told.
    Inout,
}

impl Kind {
    /// Every kind, in the order the words are listed.
    pub const ALL: [Kind; 3] = [Kind::In, Kind::Out, Kind::Inout];

    /// The node type a configuration writes for this kind.
    ///
    /// # Returns
    ///
    /// `gpio_in`, `gpio_out` or `gpio_inout`.
    pub fn type_name(self) -> &'static str {
        match self {
            Kind::In => "gpio_in",
            Kind::Out => "gpio_out",
            Kind::Inout => "gpio_inout",
        }
    }

    /// The kind a node type is, if it is one of the three.
    ///
    /// # Arguments
    ///
    /// * `node_type` - The node's `type`.
    ///
    /// # Returns
    ///
    /// The kind, or `None` for a type that is not a set of lines.
    pub fn of(node_type: &str) -> Option<Kind> {
        Kind::ALL
            .into_iter()
            .find(|kind| kind.type_name() == node_type)
    }

    /// Whether lines of this kind are watched.
    pub fn watches(self) -> bool {
        self == Kind::In
    }

    /// Whether lines of this kind are driven.
    pub fn drives(self) -> bool {
        self != Kind::In
    }

    /// Whether lines of this kind report the last value they were told to
    /// be, by `world-out`, every `state_interval`.
    pub fn reports(self) -> bool {
        self == Kind::Inout
    }
}

/// What a GPIO node's lines are set to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpioParams {
    /// Which of the three types it is.
    pub kind: Kind,
    /// The offsets of the lines watched, lowest first: see [`lines_of`].
    /// None but for a `gpio_in`.
    pub inputs: Vec<u32>,
    /// And of the lines driven, lowest first. None for a `gpio_in`.
    pub outputs: Vec<u32>,
    /// What holds the lines.
    pub bias: Bias,
    /// How long a line must be steady before a change counts, where the file
    /// asks for it.
    ///
    /// Applied by the kernel, which does the debouncing: a mechanical switch
    /// closes several times in a few milliseconds, and a handler that
    /// reported each of them would be reporting the switch's bounce as the
    /// payload's state.
    pub debounce: Option<Duration>,
    /// How often what the lines are is reported, where the file asks for it.
    ///
    /// Edges say when a line changed and this says what it is now: the two
    /// answer different questions, and a set of lines may ask for either or
    /// both.
    pub state_interval: Option<Duration>,
}

/// A length of time a set of lines states, or what is wrong with it.
///
/// Nought is refused by the grammar itself -- a line that need not be steady
/// at all is a line with no debouncing, and a report every no time at all is
/// a line read as fast as the chip will answer -- so neither needs a rule of
/// its own here.
fn length_of_time(
    name: &str,
    term: &str,
    stated: &Scalar,
) -> EndpointConfigResult<Duration> {
    crate::graph::parse_interval(stated.as_str())
        .map_err(|why| bad(name, &format!("{term}: {why}")))
}

/// Reads a list of line offsets as a file writes one: `0, 1, 5` or `0 1 5`.
///
/// Commas or spaces, decimal or hexadecimal, because a file written for a
/// board commonly copies the offsets out of a schematic in whichever form the
/// schematic used.
///
/// The order they are written in means nothing, as no order in a
/// configuration does: they are held lowest first, so that the first line of
/// a report, its lowest bit, is the lowest offset however the file listed
/// them.
///
/// # Arguments
///
/// * `name`   - The node's name, for the messages.
/// * `term`   - Which attribute it is, which is `lines`.
/// * `stated` - The attribute as the file wrote it.
///
/// # Returns
///
/// The offsets, lowest first.
///
/// # Errors
///
/// An offset that is not a whole number or is named twice, and a list that
/// names no line or more than `MOST_LINES`.
pub fn lines_of(
    name: &str,
    term: &str,
    stated: &Scalar,
) -> EndpointConfigResult<Vec<u32>> {
    let mut lines = Vec::new();

    for word in stated
        .as_str()
        .split([',', ' ', '\t'])
        .filter(|word| !word.trim().is_empty())
    {
        let offset = parse_u32(name, term, &Scalar::of(word.trim()))?;
        if lines.contains(&offset) {
            return Err(bad(
                name,
                &format!(
                    "{term} names offset {offset} twice: a line watched twice \
                     would report every change of it twice, and one driven \
                     twice is one line said twice"
                ),
            ));
        }
        lines.push(offset);
    }

    if lines.is_empty() {
        return Err(bad(
            name,
            &format!(
                "{term} must name at least one line: a node with no lines has \
                 nothing to watch and nothing to drive"
            ),
        ));
    }

    if lines.len() > MOST_LINES {
        return Err(bad(
            name,
            &format!(
                "{term} names {} lines, and one request may name {MOST_LINES}",
                lines.len()
            ),
        ));
    }

    lines.sort_unstable();
    Ok(lines)
}

/// Reads what a GPIO node says about its lines, and what it must say.
///
/// Each type states its `lines` -- a `gpio_in`'s watched, a `gpio_out`'s and
/// a `gpio_inout`'s driven. A `gpio_in` must state the `state_interval` it
/// reports its lines' levels on and may state their `bias` and `debounce`,
/// and a `gpio_inout` must state the `state_interval` it reports on. There
/// is no `edge`: what a set of lines sends is what its lines are, as one
/// whole number, on a beat (see "Typed links" in the design).
///
/// # Arguments
///
/// * `name` - The node's name, for the messages.
/// * `kind` - Which of the three types it is.
/// * `g`    - The node's terms as the file states them.
///
/// # Returns
///
/// The lines, and how the watched ones are watched.
///
/// # Errors
///
/// A term that does not belong to the type, `direction`, `edge`, a
/// `stream`, missing or bad lines, a value that does not read, and lines
/// that report at no interval.
pub(crate) fn group_kind_of(
    name: &str,
    kind: Kind,
    g: LinkTerms,
) -> EndpointConfigResult<GroupKind> {
    let type_name = kind.type_name();

    // The word the type took over, refused with what to do: it is a file
    // written for the one `gpio` there was, and reading it as foreign would
    // say what is wrong and not why.
    if g.direction.is_some() {
        return Err(bad(
            name,
            &format!(
                "direction is said by the type now: a {type_name} {}, so take \
                 direction out -- and where these lines are not that, the type \
                 to write is gpio_in for watched lines, gpio_out for driven \
                 ones, or gpio_inout for driven lines the ground is told of",
                match kind {
                    Kind::In => "watches its lines",
                    Kind::Out => "drives its lines",
                    Kind::Inout => "drives its lines and reports them",
                }
            ),
        ));
    }

    // What a gpio_in sent once: a record of each change of a line. It sends
    // what its lines are, as one number, every state_interval, so a file
    // asking for edges is told so rather than that the word is foreign.
    if g.edge.is_some() {
        return Err(bad(
            name,
            "edge is gone: a set of lines sends what its lines are, as one \
             whole number with a bit for each, every state_interval, and \
             nothing when one changes in between. Take edge out, and give \
             a state_interval",
        ));
    }

    const WATCHING: [&str; 3] = ["bias", "debounce", "state_interval"];
    let allowed: Vec<&str> = match kind {
        Kind::In => ["lines"].into_iter().chain(WATCHING).collect(),
        Kind::Out => vec!["lines"],
        Kind::Inout => vec!["lines", "state_interval"],
    };

    // The watching terms on a type that only drives, said apart from any
    // other foreign word: the usual cause is a set of lines turned into a
    // gpio_out with the rest of its words left as they were.
    if !kind.watches() {
        for (term, given) in [
            ("bias", g.bias.is_some()),
            ("debounce", g.debounce.is_some()),
            (
                "state_interval",
                g.state_interval.is_some() && !kind.reports(),
            ),
        ] {
            if given {
                return Err(bad(
                    name,
                    &format!(
                        "{term} is about watching a line, and a {type_name} \
                         drives its lines: an output has nothing to hold or \
                         debounce"
                    ),
                ));
            }
        }
    }

    reject_foreign_fields(name, type_name, &g, &allowed)?;

    // An event is its own length, so there is nothing for a rule about where
    // a read ends to say.
    reject_stream(name, type_name, g.stream.is_some())?;

    let lines = require(name, type_name, "lines", g.lines.as_ref())?;
    let lines = lines_of(name, "lines", lines)?;
    let (inputs, outputs) = match kind {
        Kind::In => (lines, Vec::new()),
        Kind::Out | Kind::Inout => (Vec::new(), lines),
    };

    let bias = match g.bias.as_ref() {
        Some(stated) => match stated.as_str().to_ascii_lowercase().as_str() {
            "none" => Bias::None,
            "pull_up" => Bias::PullUp,
            "pull_down" => Bias::PullDown,
            other => {
                return Err(bad(
                    name,
                    &format!(
                        "bias: \"{other}\" is not none, pull_up or pull_down"
                    ),
                ))
            }
        },
        None => Bias::None,
    };

    let debounce = match g.debounce.as_ref() {
        Some(stated) => Some(length_of_time(name, "debounce", stated)?),
        None => None,
    };

    let state_interval = match g.state_interval.as_ref() {
        Some(stated) => Some(length_of_time(name, "state_interval", stated)?),
        None => None,
    };

    // Lines that report report on a beat, which is the only thing that sends
    // anything from them groundward: what a gpio_in's lines are, and what a
    // gpio_inout's were told.
    if kind.reports() && state_interval.is_none() {
        return Err(bad(
            name,
            "a gpio_inout reports the last value its lines were told every \
             state_interval, and says none: say how often, as 1 s",
        ));
    }

    if kind.watches() && state_interval.is_none() {
        return Err(bad(
            name,
            "a gpio_in reports what its lines are every state_interval, and \
             says none: say how often, as 1 s",
        ));
    }

    Ok(GroupKind::Gpio(GpioParams {
        kind,
        inputs,
        outputs,
        bias,
        debounce,
        state_interval,
    }))
}

/// What `--simulate` asks to change on a GPIO node that is a real device.
const TO_SIMULATE: &str =
    "point path at the gpio-sim chip, which bin/mkgpio makes";

/// Judges whether a GPIO chip belongs to the kernel's `gpio-sim`, from where
/// the kernel puts it. See [`crate::simulate`].
///
/// A simulated chip is a child of a `gpio-sim.<n>` platform device, and a
/// real one the child of whatever controller it is on; the chip's number is
/// given out as chips appear and says nothing.
///
/// # Arguments
///
/// * `host` - The host, whose `/sys` is read.
/// * `path` - The chip's device path, such as `/dev/gpiochip0`.
///
/// # Returns
///
/// The judgement, with what to change if it is a real device.
pub fn judged(
    host: &crate::simulate::Host,
    path: &str,
) -> crate::simulate::Judged {
    match host.device_of(path) {
        Ok(device) => chip_judged(path, &device),
        Err(found) => found.judged("path", TO_SIMULATE),
    }
}

/// Where the kernel puts the chips of its `gpio-sim`, under `/sys`.
pub const SIM_DEVICES: &str = "/sys/devices/platform";

/// Lists the chips of the kernel's `gpio-sim`, with the directory each one's
/// lines are in.
///
/// The one statement of where those are, which tcssim, the tests and the
/// scripts in `bin/` all follow. A `gpio-sim` device is a platform device,
/// `gpio-sim.<n>`; under it is one directory per bank, named after the chip
/// that bank became -- `gpiochip1` for `/dev/gpiochip1` -- and in that one
/// directory per line, `sim_gpio<offset>`, holding `pull`, which sets what
/// the line is pulled to, and `value`, which says what it is. The bank's own
/// name, `bank0`, and its `chip_name` are in configfs, not here. The chip's
/// number is given out as chips appear and says nothing.
///
/// # Arguments
///
/// * `devices` - Where the platform devices are: [`SIM_DEVICES`], or a tree
///               a test made to look like it.
///
/// # Returns
///
/// Each chip's node name, such as `gpiochip1`, and its lines' directory,
/// sorted by name; none where `devices` cannot be read.
pub fn simulated_chips(
    devices: &std::path::Path,
) -> Vec<(String, std::path::PathBuf)> {
    let mut chips = Vec::new();
    let Ok(entries) = std::fs::read_dir(devices) else {
        return chips;
    };
    for device in entries.flatten() {
        if !device
            .file_name()
            .to_string_lossy()
            .starts_with("gpio-sim.")
        {
            continue;
        }
        let Ok(banks) = std::fs::read_dir(device.path()) else {
            continue;
        };
        for bank in banks.flatten() {
            let name = bank.file_name().to_string_lossy().into_owned();
            if name.starts_with("gpiochip") && bank.path().is_dir() {
                chips.push((name, bank.path()));
            }
        }
    }
    chips.sort();
    chips
}

/// Finds the directory a `gpio-sim` chip's lines are in, by the chip's
/// device path: see [`simulated_chips`] for where that is.
///
/// # Arguments
///
/// * `devices` - Where the platform devices are, as for
///               [`simulated_chips`].
/// * `chip`    - The chip's device path, such as `/dev/gpiochip1`.
///
/// # Returns
///
/// The directory, holding a `sim_gpio<offset>` per line; `None` where no
/// simulated chip is that one.
pub fn simulated_chip(
    devices: &std::path::Path,
    chip: &str,
) -> Option<std::path::PathBuf> {
    let node = std::path::Path::new(chip).file_name()?.to_string_lossy();
    simulated_chips(devices)
        .into_iter()
        .find(|(name, _)| *name == node)
        .map(|(_, lines)| lines)
}

/// Judges a chip by where its kernel directory is.
///
/// # Arguments
///
/// * `path`   - The chip's device path, for the messages.
/// * `device` - Its kernel directory, with links resolved.
///
/// # Returns
///
/// A dummy device when the directory is under a `gpio-sim` device, and hardware
/// otherwise.
fn chip_judged(
    path: &str,
    device: &std::path::Path,
) -> crate::simulate::Judged {
    use crate::simulate::Judged;
    let simulated = device.components().any(|part| {
        part.as_os_str()
            .to_str()
            .is_some_and(|part| part.starts_with("gpio-sim."))
    });
    if simulated {
        return Judged::dummy(
            format!("{path} is a chip of the kernel's gpio-sim"),
            "path",
            TO_SIMULATE,
        );
    }
    let on = device
        .parent()
        .and_then(|parent| parent.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| device.display().to_string());
    Judged::hardware(
        format!("{path} is a real chip, on {on}"),
        "path",
        TO_SIMULATE,
    )
}

#[cfg(test)]
mod judged_tests {
    use super::*;
    use crate::simulate::Verdict;
    use std::path::Path;

    #[test]
    fn a_chip_under_gpio_sim_is_a_dummy_and_another_is_not() {
        let simulated = Path::new("/sys/devices/platform/gpio-sim.0/gpiochip5");
        assert_eq!(
            chip_judged("/dev/gpiochip5", simulated).verdict,
            Verdict::Dummy
        );

        let real = Path::new("/sys/devices/pci0000:00/INTC1056:00/gpiochip0");
        let judged = chip_judged("/dev/gpiochip0", real);
        assert_eq!(judged.verdict, Verdict::Hardware);
        assert_eq!(
            judged.found,
            "/dev/gpiochip0 is a real chip, on INTC1056:00"
        );
    }

    /// A simulated chip's lines are found where the kernel puts them: in a
    /// directory named after the chip, under its `gpio-sim` device, with
    /// no bank directory between -- the layout a live chip has.
    #[test]
    fn a_simulated_chips_lines_are_under_the_chips_own_name() {
        let tree = tempfile::tempdir().unwrap();
        let chip = tree.path().join("gpio-sim.0").join("gpiochip1");
        std::fs::create_dir_all(chip.join("sim_gpio0")).unwrap();
        // Another platform device, and a file beside the chip, passed over.
        std::fs::create_dir_all(tree.path().join("serial8250")).unwrap();
        std::fs::write(tree.path().join("gpio-sim.0").join("uevent"), "")
            .unwrap();

        assert_eq!(
            simulated_chips(tree.path()),
            vec![("gpiochip1".to_string(), chip.clone())]
        );
        assert_eq!(simulated_chip(tree.path(), "/dev/gpiochip1"), Some(chip));
        assert_eq!(simulated_chip(tree.path(), "/dev/gpiochip0"), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Makes terms as a file states them, with everything else absent.
    ///
    /// # Arguments
    ///
    /// * `stated` - Each term stated, with its value as written.
    ///
    /// # Returns
    ///
    /// The terms.
    ///
    /// # Panics
    ///
    /// If a term is not one of GPIO's or `datarate`.
    fn terms(stated: &[(&str, &str)]) -> LinkTerms {
        let mut g = LinkTerms::default();
        for (field, value) in stated {
            let value = Some(Scalar::of(*value));
            match *field {
                "lines" => g.lines = value,
                "edge" => g.edge = value,
                "bias" => g.bias = value,
                "debounce" => g.debounce = value,
                "state_interval" => g.state_interval = value,
                "direction" => g.direction = value,
                "datarate" => g.datarate = value,
                other => panic!("{other} is not a term this test states"),
            }
        }
        g
    }

    /// Reads the lines a node named `discretes` states.
    ///
    /// # Arguments
    ///
    /// * `kind`   - Which of the three types it is.
    /// * `stated` - Each term stated, with its value as written.
    ///
    /// # Returns
    ///
    /// The lines' parameters.
    ///
    /// # Panics
    ///
    /// If the terms are refused, or are not GPIO lines.
    fn params(kind: Kind, stated: &[(&str, &str)]) -> GpioParams {
        // A gpio_in reports on a beat, which these tests are seldom about.
        let mut stated = stated.to_vec();
        if kind == Kind::In
            && !stated.iter().any(|(f, _)| *f == "state_interval")
        {
            stated.push(("state_interval", "1 s"));
        }
        match group_kind_of("discretes", kind, terms(&stated)).expect("lines") {
            GroupKind::Gpio(gpio) => gpio,
            other => panic!("expected GPIO lines, got {other:?}"),
        }
    }

    /// A payload watches the lines it names, nothing holding them, and
    /// reports them on its beat.
    #[test]
    fn a_payload_watches_the_lines_it_names() {
        assert_eq!(
            params(
                Kind::In,
                &[("lines", "0, 1, 5"), ("state_interval", "1 s")]
            ),
            GpioParams {
                kind: Kind::In,
                inputs: vec![0, 1, 5],
                outputs: Vec::new(),
                bias: Bias::None,
                debounce: None,
                state_interval: Some(Duration::from_secs(1)),
            }
        );
    }

    /// A set of lines watched reports what it is every `state_interval`,
    /// which it has to say, and nothing else: a file asking for edges is
    /// told they are gone.
    #[test]
    fn lines_are_reported_on_a_beat() {
        let said =
            params(Kind::In, &[("lines", "0"), ("state_interval", "5 s")]);
        assert_eq!(said.state_interval, Some(Duration::from_secs(5)));

        let why =
            group_kind_of("discretes", Kind::In, terms(&[("lines", "0")]))
                .expect_err("lines never reported");
        assert!(format!("{why}").contains("state_interval"), "{why}");

        for edge in ["both", "none", "rising"] {
            let why = group_kind_of(
                "discretes",
                Kind::In,
                terms(&[
                    ("lines", "0"),
                    ("edge", edge),
                    ("state_interval", "1 s"),
                ]),
            )
            .expect_err("an edge");
            let why = format!("{why}");
            assert!(
                why.contains("edge is gone") && why.contains("state_interval"),
                "{why}"
            );
        }

        // A report of no time at all is refused by the grammar itself: a
        // length of time is never nought, here or anywhere else.
        let why = group_kind_of(
            "discretes",
            Kind::In,
            terms(&[("lines", "0"), ("state_interval", "0 s")]),
        )
        .expect_err("a report every no time");
        assert!(format!("{why}").contains("no interval at all"), "{why}");

        // And a driven line says nothing about reports.
        let why = group_kind_of(
            "discretes",
            Kind::Out,
            terms(&[("lines", "17"), ("state_interval", "1 s")]),
        )
        .expect_err("a report of a line this node drives");
        assert!(format!("{why}").contains("state_interval"), "{why}");
    }

    /// Offsets may be written as a schematic writes them.
    #[test]
    fn offsets_are_read_in_either_notation() {
        assert_eq!(
            params(Kind::In, &[("lines", "0 1 17")]).inputs,
            vec![0, 1, 17]
        );
        assert_eq!(
            params(Kind::In, &[("lines", "0x0a,0x0b")]).inputs,
            vec![10, 11]
        );
        assert_eq!(params(Kind::In, &[("lines", " 3 ")]).inputs, vec![3]);
        // In no order but the offsets' own.
        assert_eq!(
            params(Kind::In, &[("lines", "17, 0 1")]).inputs,
            vec![0, 1, 17]
        );
    }

    /// A payload that names no lines, or one twice, is refused.
    #[test]
    fn a_payload_names_lines_once_each_and_at_least_one() {
        let said =
            group_kind_of("discretes", Kind::In, terms(&[("lines", "")]))
                .expect_err("a payload watching nothing");
        assert!(
            format!("{said}").contains("at least one"),
            "the error should say what is wanted: {said}"
        );

        let said =
            group_kind_of("discretes", Kind::In, terms(&[("lines", "4, 4")]))
                .expect_err("one line watched twice");
        assert!(
            format!("{said}").contains("twice"),
            "the error should say what is wrong: {said}"
        );

        let many: Vec<String> =
            (0..=MOST_LINES as u32).map(|n| n.to_string()).collect();
        let said = group_kind_of(
            "discretes",
            Kind::In,
            terms(&[("lines", &many.join(","))]),
        )
        .expect_err("more lines than a request may name");
        assert!(
            format!("{said}").contains("64"),
            "the error should say the limit: {said}"
        );
    }

    /// The bias and the debounce are read, and a word that is neither is
    /// refused.
    #[test]
    fn what_a_payload_may_say_about_its_lines() {
        let said = params(
            Kind::In,
            &[
                ("lines", "2"),
                ("bias", "pull_up"),
                ("debounce", "5 ms"),
                ("state_interval", "1 s"),
            ],
        );
        assert_eq!(said.bias, Bias::PullUp);
        assert_eq!(said.debounce, Some(Duration::from_micros(5_000)));

        for (field, value, expected) in
            [("bias", "pull_sideways", "none, pull_up or pull_down")]
        {
            let said = group_kind_of(
                "discretes",
                Kind::In,
                terms(&[
                    ("lines", "0"),
                    (field, value),
                    ("state_interval", "1 s"),
                ]),
            )
            .expect_err("a word that is not a value");
            assert!(
                format!("{said}").contains(expected),
                "the error should list the values: {said}"
            );
        }

        let said = group_kind_of(
            "discretes",
            Kind::In,
            terms(&[
                ("lines", "0"),
                ("debounce", "0 s"),
                ("state_interval", "1 s"),
            ]),
        )
        .expect_err("no debouncing at all");
        assert!(
            format!("{said}").contains("debounce")
                && format!("{said}").contains("no interval"),
            "the error should name the term: {said}"
        );
    }

    /// A term of another kind of link is refused.
    #[test]
    fn a_term_of_another_link_is_refused() {
        let said = group_kind_of(
            "discretes",
            Kind::In,
            terms(&[("lines", "0"), ("datarate", "9600")]),
        )
        .expect_err("a line's rate on a set of discretes");
        assert!(
            format!("{said}").contains("datarate"),
            "the error should name the term: {said}"
        );
    }

    /// The type says whether the lines are watched or driven, and a file
    /// that still says `direction` is told to take it out and which type to
    /// write.
    #[test]
    fn the_type_says_whether_lines_are_watched_or_driven() {
        let watched = params(Kind::In, &[("lines", "0")]);
        assert_eq!((watched.inputs, watched.outputs), (vec![0], vec![]));
        let driven = params(Kind::Out, &[("lines", "17, 18, 19")]);
        assert_eq!(driven.kind, Kind::Out);
        assert_eq!((driven.inputs, driven.outputs), (vec![], vec![17, 18, 19]));

        for kind in Kind::ALL {
            assert_eq!(Kind::of(kind.type_name()), Some(kind));
            let said = group_kind_of(
                "discretes",
                kind,
                terms(&[("lines", "0"), ("direction", "output")]),
            )
            .expect_err("a direction the type says now");
            let said = format!("{said}");
            assert!(
                said.contains("take direction out")
                    && said.contains("gpio_inout"),
                "the error should say what to do: {said}"
            );
        }
        assert_eq!(Kind::of("gpio"), None, "the old word is no type");
    }

    /// A `gpio_inout` drives its `lines` as a `gpio_out` does, reports them
    /// every `state_interval`, which it has to say, and says nothing about
    /// watching them.
    #[test]
    fn a_gpio_inout_drives_its_lines() {
        let told =
            params(Kind::Inout, &[("lines", "5 4"), ("state_interval", "1 s")]);
        assert_eq!(told.kind, Kind::Inout);
        assert_eq!((told.inputs, told.outputs), (vec![], vec![4, 5]));
        assert_eq!(told.state_interval, Some(Duration::from_secs(1)));

        // Reported on a beat, and so saying how often.
        let said =
            group_kind_of("discretes", Kind::Inout, terms(&[("lines", "4")]))
                .expect_err("a report never made");
        assert!(format!("{said}").contains("state_interval"), "{said}");
        assert!(Kind::Inout.drives() && Kind::Inout.reports());
        assert!(!Kind::Inout.watches());

        let said = group_kind_of(
            "discretes",
            Kind::Inout,
            terms(&[
                ("lines", "4"),
                ("state_interval", "1 s"),
                ("edge", "rising"),
            ]),
        )
        .expect_err("a watching term on driven lines");
        assert!(format!("{said}").contains("edge"), "{said}");
    }

    /// A driven line may not also say how it is watched.
    ///
    /// Each of the watching terms separately, because the usual cause is a
    /// set of lines turned into outputs with the rest of its words left as
    /// they were, and the one that was missed is the one worth naming.
    #[test]
    fn a_driven_line_says_nothing_about_watching() {
        for (term, value) in [("bias", "pull_up"), ("debounce", "5 ms")] {
            let said = group_kind_of(
                "heaters",
                Kind::Out,
                terms(&[("lines", "17"), (term, value)]),
            )
            .expect_err("a watching term on a driven line");
            let said = format!("{said}");
            assert!(
                said.contains(term) && said.contains("drives"),
                "the error should name the term and say why: {said}"
            );
        }

        // And an output that says nothing else is what a heater looks like.
        assert_eq!(
            params(Kind::Out, &[("lines", "17, 18, 19")]),
            GpioParams {
                kind: Kind::Out,
                inputs: Vec::new(),
                outputs: vec![17, 18, 19],
                bias: Bias::None,
                debounce: None,
                state_interval: None,
            }
        );
    }
}
