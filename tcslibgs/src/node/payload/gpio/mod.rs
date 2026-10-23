//! GPIO payloads: discrete lines, as events.
//!
//! **Experimental, and not yet supported.** The terms parse and reach a
//! handler, and the handler watches the lines; nothing has been run against
//! hardware, only against the kernel's simulated chip.
//!
//! A payload is not always a stream of data. A deployment switch, an arming
//! line, a thermostat's output, a payload's own "I am alive" line: each is one
//! wire that is high or low, and what the ground wants to know is when it
//! changed. Linux offers those through a GPIO chip's character device, where a
//! program asks for a set of lines with edge detection and then reads events.
//!
//! So this is the one kind whose payload data is made rather than carried. A
//! conduit moves bytes, and a line's change is not bytes, so each event
//! becomes a record of a fixed size: when it happened, which line, and what it
//! became. The size is [`EVENT_BYTES`], and a payload's `read_size` must be
//! a whole number of them -- a read takes whole records only, so one smaller
//! than a record would never deliver anything. `graph::reach` holds a node
//! to it.
//!
//! What a GPIO payload cannot be is triggered. These are inputs: there is
//! nothing to write a request to, and writing would mean driving an output,
//! which is commanding a payload rather than asking it for data.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::endpoint_config::{
    bad, parse_u32, reject_foreign_fields, reject_stream, require, EndpointConfigResult, GroupKind,
    LinkTerms, Scalar,
};

/// How many bytes one line event takes as payload data.
///
/// Sixteen: the time as eight, the line's offset as four, and what it became
/// as four. The time is a [`crate::Timestamp`] -- microseconds since the
/// epoch, least significant byte first -- which is what every other time in
/// this project is, so a line's change and a telemetry response can be read
/// against each other; the offset is there because one
/// payload may watch several lines, and without it an event says only that
/// something moved.
pub const EVENT_BYTES: usize = 16;

/// The greatest number of lines one request may name, which is the kernel's
/// own limit on a line request.
pub const MOST_LINES: usize = 64;

/// Which changes of a line are worth an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Edge {
    /// Low to high only.
    Rising,
    /// High to low only.
    Falling,
    /// Both, which is the default: a discrete that is read for its state is
    /// read for both of the states it has.
    Both,
    /// None: the lines are not watched for changes at all.
    ///
    /// For a set of lines read for what they are rather than for when they
    /// changed, which is `state_interval` and nothing else. The kernel is
    /// asked for the lines with no edge detection, so there are no events to
    /// read and the levels are taken when the report is due.
    None,
}

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

/// Whether the lines are read or driven.
///
/// Not a thing a set of lines can be both of. A line requested as an input
/// and written to is a line the kernel refuses to write, and one requested as
/// an output produces no edges to read -- so which it is has to be stated,
/// and stating it is what lets a configuration be refused for driving
/// something it declared it would watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Watched: changes on these lines are events. The default, because a set
    /// of discretes is more often something read -- a deployment switch, an
    /// arming line -- than something driven.
    #[default]
    Input,
    /// Driven: what arrives sets these lines.
    Output,
}

impl Direction {
    /// The direction this word asks for.
    pub fn of(name: &str) -> Result<Direction, String> {
        match name {
            "input" => Ok(Direction::Input),
            "output" => Ok(Direction::Output),
            other => Err(format!(
                "direction: \"{other}\" is neither input nor output"
            )),
        }
    }

    /// Whether these lines are driven.
    pub fn is_output(self) -> bool {
        self == Direction::Output
    }
}

/// What a GPIO node's lines are set to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpioParams {
    /// The offsets of the lines, in the order the file gave them.
    pub lines: Vec<u32>,
    /// Whether they are read or driven.
    pub direction: Direction,
    /// Which changes are events.
    pub edge: Edge,
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
fn length_of_time(name: &str, term: &str, stated: &Scalar) -> EndpointConfigResult<Duration> {
    crate::graph::parse_interval(stated.as_str())
        .map_err(|why| bad(name, &format!("{term}: {why}")))
}

/// Read a list of line offsets as a file writes one: `0, 1, 5` or `0 1 5`.
///
/// Commas or spaces, decimal or hexadecimal, because a file written for a
/// board commonly copies the offsets out of a schematic in whichever form the
/// schematic used.
pub fn lines_of(name: &str, stated: &Scalar) -> EndpointConfigResult<Vec<u32>> {
    let mut lines = Vec::new();

    for word in stated
        .as_str()
        .split([',', ' ', '\t'])
        .filter(|word| !word.trim().is_empty())
    {
        let offset = parse_u32(name, "lines", &Scalar::of(word.trim()))?;
        if lines.contains(&offset) {
            return Err(bad(
                name,
                &format!(
                    "lines names offset {offset} twice: a line watched twice would \
                     report every change of it twice"
                ),
            ));
        }
        lines.push(offset);
    }

    if lines.is_empty() {
        return Err(bad(
            name,
            "lines must name at least one line: a payload watching no lines has \
             nothing to report",
        ));
    }

    if lines.len() > MOST_LINES {
        return Err(bad(
            name,
            &format!(
                "lines names {} lines, and one request may name {MOST_LINES}",
                lines.len()
            ),
        ));
    }

    Ok(lines)
}

/// What a GPIO payload says about its lines, and what it must say.
pub(crate) fn group_kind_of(name: &str, g: LinkTerms) -> EndpointConfigResult<GroupKind> {
    reject_foreign_fields(
        name,
        "gpio",
        &g,
        &[
            "lines",
            "edge",
            "bias",
            "debounce",
            "state_interval",
            "direction",
        ],
    )?;

    // An event is its own length, so there is nothing for a rule about where
    // a read ends to say.
    reject_stream(name, "gpio", g.stream.is_some())?;

    let lines = require(name, "gpio", "lines", g.lines.as_ref())?;
    let lines = lines_of(name, lines)?;

    let edge = match g.edge.as_ref() {
        Some(stated) => match stated.as_str().to_ascii_lowercase().as_str() {
            "rising" => Edge::Rising,
            "falling" => Edge::Falling,
            "both" => Edge::Both,
            "none" => Edge::None,
            other => {
                return Err(bad(
                    name,
                    &format!("edge: \"{other}\" is not rising, falling, both or none"),
                ))
            }
        },
        None => Edge::Both,
    };

    let bias = match g.bias.as_ref() {
        Some(stated) => match stated.as_str().to_ascii_lowercase().as_str() {
            "none" => Bias::None,
            "pull_up" => Bias::PullUp,
            "pull_down" => Bias::PullDown,
            other => {
                return Err(bad(
                    name,
                    &format!("bias: \"{other}\" is not none, pull_up or pull_down"),
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

    let direction = match g.direction.as_ref() {
        Some(stated) => {
            Direction::of(&stated.as_str().to_ascii_lowercase()).map_err(|why| bad(name, &why))?
        }
        None => Direction::Input,
    };

    // Terms that only mean something for lines being watched. Refused rather
    // than ignored on an output: an edge or a debounce on a line nothing reads
    // is a file saying something it does not mean, and the usual cause is a
    // set of lines whose direction was changed and whose other terms were left
    // as they were.
    if direction.is_output() {
        for (term, given) in [
            ("edge", g.edge.is_some()),
            ("bias", g.bias.is_some()),
            ("debounce", g.debounce.is_some()),
            ("state_interval", g.state_interval.is_some()),
        ] {
            if given {
                return Err(bad(
                    name,
                    &format!(
                        "{term} is about watching a line, and these lines are \
                         driven: an output has no edges to report and nothing \
                         to debounce"
                    ),
                ));
            }
        }
    }

    // Lines that are watched are watched somehow. Edges are the default, so
    // the only way to ask for neither is to say so -- `edge: none` with no
    // report -- and that is a payload that would send nothing at all: the
    // configuration asked for the lines and then asked for nothing about
    // them.
    if !direction.is_output() && edge == Edge::None && state_interval.is_none() {
        return Err(bad(
            name,
            "these lines are watched for no edges and reported at no interval, \
             so nothing about them would ever be sent: say an edge, or a \
             state_interval, or both",
        ));
    }

    Ok(GroupKind::Gpio(GpioParams {
        lines,
        direction,
        edge,
        bias,
        debounce,
        state_interval,
    }))
}

/// What `--simulate` asks to change on a GPIO node that is not a stand-in.
const TO_STAND_IN: &str = "point path at the gpio-sim chip, which bin/mkgpio makes";

/// Whether the chip at `path` belongs to the kernel's `gpio-sim`, from where
/// the kernel puts it. See [`crate::simulate`].
///
/// A simulated chip is a child of a `gpio-sim.<n>` platform device, and a
/// real one the child of whatever controller it is on; the chip's number is
/// given out as chips appear and says nothing.
pub fn judged(host: &crate::simulate::Host, path: &str) -> crate::simulate::Judged {
    match host.device_of(path) {
        Ok(device) => chip_judged(path, &device),
        Err(found) => crate::simulate::Judged::cannot_tell(found, "path", TO_STAND_IN),
    }
}

/// The judgement of a chip whose kernel directory is `device`.
fn chip_judged(path: &str, device: &std::path::Path) -> crate::simulate::Judged {
    use crate::simulate::Judged;
    let simulated = device.components().any(|part| {
        part.as_os_str()
            .to_str()
            .is_some_and(|part| part.starts_with("gpio-sim."))
    });
    if simulated {
        return Judged::stand_in(
            format!("{path} is a chip of the kernel's gpio-sim"),
            "path",
            TO_STAND_IN,
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
        TO_STAND_IN,
    )
}

#[cfg(test)]
mod stand_in_tests {
    use super::*;
    use crate::simulate::Verdict;
    use std::path::Path;

    #[test]
    fn a_chip_under_gpio_sim_is_a_stand_in_and_another_is_not() {
        let simulated = Path::new("/sys/devices/platform/gpio-sim.0/gpiochip5");
        assert_eq!(
            chip_judged("/dev/gpiochip5", simulated).verdict,
            Verdict::StandIn
        );

        let real = Path::new("/sys/devices/pci0000:00/INTC1056:00/gpiochip0");
        let judged = chip_judged("/dev/gpiochip0", real);
        assert_eq!(judged.verdict, Verdict::Hardware);
        assert_eq!(
            judged.found,
            "/dev/gpiochip0 is a real chip, on INTC1056:00"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn params(stated: &[(&str, &str)]) -> GpioParams {
        match group_kind_of("discretes", terms(stated)).expect("lines") {
            GroupKind::Gpio(gpio) => gpio,
            other => panic!("expected GPIO lines, got {other:?}"),
        }
    }

    /// A payload watches the lines it names, both edges, nothing holding
    /// them.
    #[test]
    fn a_payload_watches_the_lines_it_names() {
        assert_eq!(
            params(&[("lines", "0, 1, 5")]),
            GpioParams {
                lines: vec![0, 1, 5],
                direction: Direction::Input,
                edge: Edge::Both,
                bias: Bias::None,
                debounce: None,
                state_interval: None,
            }
        );
    }

    /// A set of lines says how it is watched: by its edges, by a report of
    /// what it is, or by both.
    ///
    /// Edges are the default, so a payload that says nothing gets them and
    /// every file written before the report existed still means what it
    /// meant. Asking for neither takes saying so, and is refused: lines
    /// watched for no edges and reported at no interval would send nothing.
    #[test]
    fn lines_are_watched_by_edges_or_by_report_or_by_both() {
        // The default, unchanged: edges, and no report.
        let said = params(&[("lines", "0")]);
        assert_eq!(said.edge, Edge::Both);
        assert_eq!(said.state_interval, None);

        // A report and no edges, which is a set of lines read for what they
        // are rather than for when they changed.
        let said = params(&[("lines", "0"), ("edge", "none"), ("state_interval", "5 s")]);
        assert_eq!(said.edge, Edge::None);
        assert_eq!(said.state_interval, Some(Duration::from_secs(5)));

        // And both, which is edges with a refresh behind them.
        let said = params(&[("lines", "0"), ("state_interval", "1 s")]);
        assert_eq!(said.edge, Edge::Both);
        assert_eq!(said.state_interval, Some(Duration::from_secs(1)));

        // Neither is refused, and the complaint says both ways out.
        let why = group_kind_of("discretes", terms(&[("lines", "0"), ("edge", "none")]))
            .expect_err("lines nobody watches");
        let why = format!("{why}");
        assert!(
            why.contains("state_interval") && why.contains("edge"),
            "{why}"
        );

        // A report of no time at all is refused by the grammar itself: a
        // length of time is never nought, here or anywhere else.
        let why = group_kind_of(
            "discretes",
            terms(&[("lines", "0"), ("state_interval", "0 s")]),
        )
        .expect_err("a report every no time");
        assert!(format!("{why}").contains("no interval at all"), "{why}");

        // And a driven line says nothing about reports, as it says nothing
        // about edges.
        let why = group_kind_of(
            "discretes",
            terms(&[
                ("lines", "17"),
                ("direction", "output"),
                ("state_interval", "1 s"),
            ]),
        )
        .expect_err("a report of a line this node drives");
        assert!(format!("{why}").contains("state_interval"), "{why}");
    }

    /// Offsets may be written as a schematic writes them.
    #[test]
    fn offsets_are_read_in_either_notation() {
        assert_eq!(params(&[("lines", "0 1 17")]).lines, vec![0, 1, 17]);
        assert_eq!(params(&[("lines", "0x0a,0x0b")]).lines, vec![10, 11]);
        assert_eq!(params(&[("lines", " 3 ")]).lines, vec![3]);
    }

    /// A payload that names no lines, or one twice, is refused.
    #[test]
    fn a_payload_names_lines_once_each_and_at_least_one() {
        let said = group_kind_of("discretes", terms(&[("lines", "")]))
            .expect_err("a payload watching nothing");
        assert!(
            format!("{said}").contains("at least one"),
            "the error should say what is wanted: {said}"
        );

        let said = group_kind_of("discretes", terms(&[("lines", "4, 4")]))
            .expect_err("one line watched twice");
        assert!(
            format!("{said}").contains("twice"),
            "the error should say what is wrong: {said}"
        );

        let many: Vec<String> = (0..=MOST_LINES as u32).map(|n| n.to_string()).collect();
        let said = group_kind_of("discretes", terms(&[("lines", &many.join(","))]))
            .expect_err("more lines than a request may name");
        assert!(
            format!("{said}").contains("64"),
            "the error should say the limit: {said}"
        );
    }

    /// The edge, the bias and the debounce are read, and a word that is
    /// neither is refused.
    #[test]
    fn what_a_payload_may_say_about_its_lines() {
        let said = params(&[
            ("lines", "2"),
            ("edge", "falling"),
            ("bias", "pull_up"),
            ("debounce", "5 ms"),
        ]);
        assert_eq!(said.edge, Edge::Falling);
        assert_eq!(said.bias, Bias::PullUp);
        assert_eq!(said.debounce, Some(Duration::from_micros(5_000)));

        for (field, value, expected) in [
            ("edge", "sideways", "rising, falling, both or none"),
            ("bias", "pull_sideways", "none, pull_up or pull_down"),
        ] {
            let said = group_kind_of("discretes", terms(&[("lines", "0"), (field, value)]))
                .expect_err("a word that is not a value");
            assert!(
                format!("{said}").contains(expected),
                "the error should list the values: {said}"
            );
        }

        let said = group_kind_of("discretes", terms(&[("lines", "0"), ("debounce", "0 s")]))
            .expect_err("no debouncing at all");
        assert!(
            format!("{said}").contains("debounce") && format!("{said}").contains("no interval"),
            "the error should name the term: {said}"
        );
    }

    /// A term of another kind of link is refused.
    #[test]
    fn a_term_of_another_link_is_refused() {
        let said = group_kind_of("discretes", terms(&[("lines", "0"), ("datarate", "9600")]))
            .expect_err("a line's rate on a set of discretes");
        assert!(
            format!("{said}").contains("datarate"),
            "the error should name the term: {said}"
        );
    }

    /// Lines are watched unless the file says they are driven.
    #[test]
    fn lines_are_watched_unless_the_file_says_driven() {
        assert_eq!(params(&[("lines", "0")]).direction, Direction::Input);
        assert_eq!(
            params(&[("lines", "0"), ("direction", "input")]).direction,
            Direction::Input
        );
        assert_eq!(
            params(&[("lines", "17, 18, 19"), ("direction", "output")]).direction,
            Direction::Output
        );

        // However the file capitalised it, as the other words of a link are
        // read.
        assert_eq!(
            params(&[("lines", "0"), ("direction", "OUTPUT")]).direction,
            Direction::Output
        );

        let said = group_kind_of("discretes", terms(&[("lines", "0"), ("direction", "both")]))
            .expect_err("a direction that is neither");
        assert!(
            format!("{said}").contains("neither input nor output"),
            "the error should say what the choices are: {said}"
        );
    }

    /// A driven line may not also say how it is watched.
    ///
    /// Each of the watching terms separately, because the usual cause is a
    /// set of lines turned into outputs with the rest of its words left as
    /// they were, and the one that was missed is the one worth naming.
    #[test]
    fn a_driven_line_says_nothing_about_watching() {
        for (term, value) in [
            ("edge", "rising"),
            ("bias", "pull_up"),
            ("debounce", "5 ms"),
        ] {
            let said = group_kind_of(
                "heaters",
                terms(&[("lines", "17"), ("direction", "output"), (term, value)]),
            )
            .expect_err("a watching term on a driven line");
            let said = format!("{said}");
            assert!(
                said.contains(term) && said.contains("driven"),
                "the error should name the term and say why: {said}"
            );
        }

        // And an output that says nothing else is what a heater looks like.
        assert_eq!(
            params(&[("lines", "17, 18, 19"), ("direction", "output")]),
            GpioParams {
                lines: vec![17, 18, 19],
                direction: Direction::Output,
                edge: Edge::Both,
                bias: Bias::None,
                debounce: None,
                state_interval: None,
            }
        );
    }

    /// An event is sixteen bytes: a time, a line and a value.
    #[test]
    fn an_event_is_a_time_a_line_and_a_value() {
        assert_eq!(EVENT_BYTES, 8 + 4 + 4);
    }
}
