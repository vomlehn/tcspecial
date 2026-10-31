//! What a compute node may say, what each computation must be told, and the
//! form a value takes on a link.
//!
//! The rules -- what the attributes are, which are required, and what each
//! value may be. The arithmetic is `tcspecial::node::vehicle::compute`, which
//! holds the running computations and nothing about files.
//!
//! The form of a value is here for a second reason: it is one fact that the
//! vehicle writes and the ground reads back, so it belongs where the two share
//! what they agree about, beside the protocol. And the range it can carry is a
//! rule about what a configuration may say -- a setpoint no reading can reach
//! is a thermostat that will hold its heaters on for ever -- which is checked
//! on a desk by `tcsverify` and must therefore be stated on this side.
//!
//! They are here for one reason: `tcsverify` checks a configuration on a desk,
//! and it cannot check what it cannot see. The rules used to live beside the
//! arithmetic, inside the flight program, so a thermostat with no `below` or a
//! program with no `timeout` passed the ground check in silence and was
//! refused when the vehicle started. That is the wrong end of the trip. Every
//! other node type keeps its rules in this crate -- see [`crate::node`] -- and
//! the log node's move set the pattern.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::Scalar;

/// How wide a value is on a link: a 32-bit integer.
pub const VALUE_BYTES: usize = 4;

/// How many of them are in a kelvin.
///
/// A micro-kelvin is finer than anything a spacecraft's thermometry resolves
/// -- the parts the examples use report in sixteenths and hundredths of a
/// kelvin -- so the unit is below the noise of every sensor that will be read
/// through it, which is what a unit has to be if rounding to it is to be
/// nothing at all.
pub const MICRO_KELVIN: f64 = 1_000_000.0;

/// Gives the coldest and the hottest a value can say, in kelvin.
///
/// Said out loud so that a message can name it and a tool can ask: these are
/// the ends of [`micro_kelvin`], which is the one place the range is decided.
///
/// # Returns
///
/// The coldest, then the hottest.
pub fn kelvin_range() -> (f64, f64) {
    (
        f64::from(i32::MIN) / MICRO_KELVIN,
        f64::from(i32::MAX) / MICRO_KELVIN,
    )
}

/// Turns a temperature into the count of micro-kelvin a value carries.
///
/// The one place the range of a value is decided, and everything that turns a
/// number into a temperature comes here: a reading on its way onto a link, a
/// number read from text -- which is a program's answer or an operator's
/// command -- and a temperature written in a configuration.
///
/// A signed count of micro-kelvin reaches a little past 2147 K either way,
/// which is above anything these nodes are pointed at and below a great deal
/// that exists -- a thruster's throat is hotter. A temperature outside it is
/// refused and named rather than wrapped or pinned to the end of the range:
/// wrapping turns 2200 K into a cabin that is 42 K below zero, pinning turns
/// a runaway into a reading that looks like a hot but surviving part, and
/// either is a thermostat acting confidently on a number nobody produced. A
/// node that must carry more than this wants a unit of its own, and that is a
/// change to make deliberately.
///
/// A NaN or an infinity is refused here too, and it is the comparison that
/// makes it matter: a vote sorts its readings, and NaN compares as neither
/// less nor greater, so a sort that met one would order the readings by where
/// they happened to lie and the middle one would be the middle of nothing.
///
/// # Arguments
///
/// * `number` - The temperature in kelvin.
///
/// # Returns
///
/// The count of micro-kelvin, rounded to the nearest.
///
/// # Errors
///
/// A message naming the number for a NaN or an infinity, or a temperature
/// outside [`kelvin_range`].
pub fn micro_kelvin(number: f64) -> Result<i32, String> {
    if !number.is_finite() {
        return Err(format!(
            "{number} is not a number anything can be decided on"
        ));
    }

    // Rounded to the nearest micro-kelvin rather than truncated: the
    // conversions land a hair either side of the figure the part reported --
    // 289.15 K arrives from a count as 289.14999999999998 -- and truncating
    // would take a micro-kelvin off half of them.
    let micro = (number * MICRO_KELVIN).round();
    let (coldest, hottest) = kelvin_range();
    if micro < f64::from(i32::MIN) || micro > f64::from(i32::MAX) {
        return Err(format!(
            "{number} K is outside what a value carries, which is \
             {coldest:.6} K to {hottest:.6} K in micro-kelvin"
        ));
    }
    Ok(micro as i32)
}

/// A value as it travels: four bytes of a signed count of micro-kelvin,
/// most significant first.
///
/// A temperature is carried as a whole number of micro-kelvin rather than as
/// the double the arithmetic produced. The unit is finer than any sensor here
/// resolves, so nothing a part reported is lost by it, and what is gained is
/// that the number on the link is exact: a conversion, a vote and a thermostat
/// all see the same integer, where three doubles of the same temperature can
/// differ in their last bit and a comparison between them turns on dust. It
/// halves what a reading costs in a log and on a link as well.
///
/// Signed, although a temperature in kelvin is not, for two reasons. The
/// arithmetic that produces one is signed -- a conversion is `counts * scale +
/// offset` and a wrong scale produces a negative, which must arrive as a
/// negative to be seen for what it is rather than as a temperature near
/// 4295 K. And a difference of two temperatures is a temperature's width,
/// which a band is, and that is signed by nature.
///
/// Most significant first because everything this software puts on the
/// network is, unless the protocol it goes in says otherwise: a timestamp's
/// microseconds, a node's id, a beacon interval, every field of a command and
/// an answer. A sensor's own bytes are the exception and say which way round
/// they are -- `i16_le`, `u24_be` -- because the part decides that and the
/// file states it.
///
/// What it costs is the range, which is where this is refused rather than
/// wrapped: see [`micro_kelvin`]. And a value in a telemetry log is four bytes
/// rather than a readable number, as is the datum a panel shows for a source.
/// A decision is still text, `1` or `0`, because what it drives is a set of
/// discrete lines and that is the value they take.
pub fn as_value(number: f64) -> Result<Vec<u8>, String> {
    Ok(value_bytes(number)?.to_vec())
}

/// Makes a value as [`as_value`] does, as the four bytes themselves,
/// allocating nothing: what a computation aboard makes a value with, once the
/// vehicle is running.
///
/// # Arguments
///
/// * `number` - The temperature in kelvin.
///
/// # Returns
///
/// The value's bytes, most significant first.
///
/// # Errors
///
/// What [`micro_kelvin`] refuses.
pub fn value_bytes(number: f64) -> Result<[u8; VALUE_BYTES], String> {
    Ok(micro_kelvin(number)?.to_be_bytes())
}

/// Reads a value that arrived as a number of kelvin.
///
/// Exactly four bytes, and every four bytes are a value: a count of
/// micro-kelvin is a number whatever the bits say, so the width is the whole
/// of what there is to check here. What cannot be represented is refused
/// where a value is made instead, by [`micro_kelvin`], which is where the
/// complaint can name the number that would not fit.
///
/// A width that is not four is a link carrying something that is not a value
/// -- a sensor's raw bytes delivered where a conversion's output was meant, or
/// a node whose output is a decision -- and taking four of them would be a
/// temperature made of whatever was there.
///
/// # Arguments
///
/// * `data` - The value's bytes as they arrived.
/// * `what` - What the value is, for the message.
///
/// # Returns
///
/// The temperature in kelvin.
///
/// # Errors
///
/// A message when `data` is not [`VALUE_BYTES`] long.
pub fn from_value(data: &[u8], what: &str) -> Result<f64, String> {
    let bytes: [u8; VALUE_BYTES] = data.try_into().map_err(|_| {
        format!(
            "{what}: a value is {VALUE_BYTES} bytes of micro-kelvin, and {} \
             arrived",
            data.len()
        )
    })?;

    Ok(f64::from(i32::from_be_bytes(bytes)) / MICRO_KELVIN)
}

/// Tells whether what this computation produces is a value.
///
/// Three of the four are: a conversion makes one out of a part's counts, a
/// vote makes one out of several, and a program's answer is read as one. A
/// hysteresis is the exception -- what it produces is a decision, the text
/// `1` or `0`, because what it drives is a set of discrete lines.
///
/// Asked by whoever has to read a datum back: the ground shows a reading as a
/// temperature and a decision as what it is, and the only thing that can tell
/// them apart is the configuration. Not the width -- every four bytes are a
/// value, so a decision of four characters would read as a temperature, which
/// is the one guess this exists to avoid.
///
/// # Arguments
///
/// * `function` - The computation's `function`.
///
/// # Returns
///
/// `true` for `to_kelvin`, `vote` and `process`; `false` for `hysteresis`
/// and for a name that is not a computation.
pub fn result_is_a_value(function: &str) -> bool {
    match function {
        "to_kelvin" | "vote" | "process" => true,
        "hysteresis" => false,
        // Not a computation at all. Said as "no" rather than refused: this
        // answers how to show a datum, and a node nobody can name does not
        // have its data read as a temperature on the strength of a typo.
        _ => false,
    }
}

/// Tells whether what this node sends is a value.
///
/// Only a computation makes one, and only three of the four: see
/// [`result_is_a_value`]. Everything else sends what it was given or what its
/// part produced -- a sensor's own bytes, a log record, whatever a program
/// wrote -- and none of that is a number in this form.
///
/// # Arguments
///
/// * `node_type`  - The node's type.
/// * `attributes` - The node's attributes, which name its function.
///
/// # Returns
///
/// `true` for a compute node whose function makes a value.
pub fn node_sends_a_value(
    node_type: &str,
    attributes: &BTreeMap<String, Scalar>,
) -> bool {
    node_type == "compute"
        && attributes
            .get("function")
            .is_some_and(|function| result_is_a_value(function.as_str()))
}

/// A value as a program reads and writes one: decimal text.
///
/// The one place a value is text, because the far end is a program reading
/// lines -- see `tcspecial::node::vehicle::compute::process` -- and a line is
/// text. Written out of the kelvin the value carried, which is a whole number
/// of micro-kelvin, so a program is handed at most six decimal places and what
/// it reads back is what arrived.
pub fn as_text(number: f64) -> String {
    format!("{number}")
}

/// Reads a number a program wrote as a number of kelvin.
///
/// Held to the same range as a reading, by [`micro_kelvin`], because what is
/// read here becomes one: a program's answer goes straight onto a link, and
/// an operator's setpoint or band is compared against readings that cannot
/// leave that range. A number outside it is told so when it is sent rather
/// than carried as far as the arithmetic and lost there.
///
/// # Arguments
///
/// * `text` - The number as written.
/// * `what` - What the number is, for the messages.
///
/// # Returns
///
/// The temperature in kelvin.
///
/// # Errors
///
/// A message for text that is not a number, and for what
/// [`micro_kelvin`] refuses.
pub fn from_text(text: &str, what: &str) -> Result<f64, String> {
    let said = text.trim();
    let number: f64 = said
        .parse()
        .map_err(|_| format!("{what}: \"{said}\" is not a number"))?;
    micro_kelvin(number).map_err(|why| format!("{what}: {why}"))?;
    Ok(number)
}

/// How a sensor's bytes are laid out.
///
/// What a part puts on a bus is a number in its own units and its own order,
/// and the datasheet is the only statement of which. Four so far, which are
/// the four the parts this has met use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    /// Two bytes, signed, most significant first.
    I16Be,
    /// Two bytes, unsigned, most significant first.
    U16Be,
    /// Three bytes, unsigned, most significant first.
    U24Be,
    /// Two bytes, signed, least significant first.
    I16Le,
}

impl Format {
    /// Reads the format a `format` attribute names.
    ///
    /// # Arguments
    ///
    /// * `name` - The attribute as the file wrote it.
    ///
    /// # Returns
    ///
    /// The format.
    ///
    /// # Errors
    ///
    /// A message listing the formats, for a name that is none of them.
    pub fn of(name: &str) -> Result<Format, String> {
        match name {
            "i16_be" => Ok(Format::I16Be),
            "u16_be" => Ok(Format::U16Be),
            "u24_be" => Ok(Format::U24Be),
            "i16_le" => Ok(Format::I16Le),
            other => Err(format!(
                "format: \"{other}\" is not one this knows. There are i16_be, \
                 u16_be, u24_be and i16_le"
            )),
        }
    }

    /// Says how many bytes a reading takes.
    ///
    /// # Returns
    ///
    /// The reading's width in bytes.
    pub fn width(self) -> usize {
        match self {
            Format::U24Be => 3,
            _ => 2,
        }
    }
}

/// What to do when an external program stops.
///
/// Declared by the family rather than by this module, two of its node types
/// running a program of their own: a computation whose function is `process`,
/// and an app node. Named from here as well, because this is where the
/// attribute was first written and where every caller of it has looked since.
pub use super::Restart;

/// Raw counts to kelvin: read the bytes, scale, add an offset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToKelvinParams {
    pub format: Format,
    pub scale: f64,
    pub offset: f64,
}

/// The mean over a named set of inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoteParams {
    /// The inputs it votes over, as the node declares them.
    ///
    /// Declared and then checked against the links, which is the point of
    /// having them written down: a vote over three sensors whose third link
    /// was deleted has two inputs, each bound exactly once, and would quietly
    /// become a vote over two -- which is no vote at all, two readings that
    /// disagree having no majority between them. The declaration is what
    /// makes that a configuration fault instead.
    pub inputs: Vec<String>,
    /// How the one value is picked out of them.
    pub algorithm: Algorithm,
}

/// How a vote turns several readings of one thing into the value to use.
///
/// Both of these reject an outlier, which is the whole point: the readings
/// are of one quantity, so they disagree only because a sensor is wrong, and
/// a mean over all of them would carry the wrong one into the answer in
/// proportion to how wrong it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// The middle reading, which an outlier cannot be.
    ///
    /// For an even count, the mean of the two middle ones; an odd count is
    /// what a vote wants, so that there is a middle to take.
    Median,
    /// The mean of the two readings closest to each other.
    ///
    /// Where the median takes one sensor's word, this takes the two that
    /// agree -- so a third that has drifted is left out of the answer
    /// altogether rather than being the one consulted.
    ClosestMean,
}

impl Algorithm {
    /// Names the algorithm as a file does.
    ///
    /// # Returns
    ///
    /// `median` or `closest_mean`.
    pub fn spelling(&self) -> &'static str {
        match self {
            Algorithm::Median => "median",
            Algorithm::ClosestMean => "closest_mean",
        }
    }

    /// Reads the algorithm a file's `algorithm` names.
    ///
    /// # Arguments
    ///
    /// * `said` - The attribute as the file wrote it.
    ///
    /// # Returns
    ///
    /// The algorithm.
    ///
    /// # Errors
    ///
    /// A message describing the two algorithms, for a word that is neither.
    pub fn of(said: &str) -> Result<Algorithm, String> {
        match said.trim() {
            "median" => Ok(Algorithm::Median),
            "closest_mean" => Ok(Algorithm::ClosestMean),
            other => Err(format!(
                "algorithm: \"{other}\" is not one this knows. There are \
                 median, which takes the middle reading, and closest_mean, \
                 which takes the mean of the two that agree best"
            )),
        }
    }
}

/// A setpoint and how far either side of it the decision changes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HysteresisParams {
    /// What the reading is held at.
    pub setpoint: f64,
    /// How far from the setpoint the decision changes, each way.
    ///
    /// One width and not two. It was `above` and `below`, which could differ
    /// -- and a band wider on one side than the other is a setpoint that is
    /// not in the middle of its own band, which is a different thing said in
    /// a way nobody reading the file would notice. What is wanted is a narrow
    /// band around a goal, so the goal and the half-width are the two numbers
    /// there are.
    pub half_band: f64,
}

/// A computation that is a program of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessParams {
    pub executable: String,
    pub timeout: Duration,
    pub restart: Restart,
}

/// What a compute node computes, settled.
#[derive(Debug, Clone, PartialEq)]
pub enum Computation {
    ToKelvin(ToKelvinParams),
    Vote(VoteParams),
    Hysteresis(HysteresisParams),
    Process(ProcessParams),
}

impl Computation {
    /// Names this computation as a configuration file does.
    ///
    /// # Returns
    ///
    /// Its `function`, such as `to_kelvin`.
    pub fn function(&self) -> &'static str {
        match self {
            Computation::ToKelvin(_) => "to_kelvin",
            Computation::Vote(_) => "vote",
            Computation::Hysteresis(_) => "hysteresis",
            Computation::Process(_) => "process",
        }
    }
}

/// Every function there is, for an error to list.
pub fn functions() -> &'static str {
    "to_kelvin, vote, hysteresis, or process"
}

/// Lists what a computation is told, by name.
///
/// The list every computation is held to: `params_of` refuses an attribute
/// that is not on it, and this is the same list said out loud so that a tool
/// can ask. tcsgui's description tree is what asks -- a description of a
/// computation names the attributes its panel offers, and a test holds those
/// against this -- because a description that asked for something else would
/// be a panel filling in a field the checking then refuses.
///
/// `function` is on every list, it being how a node says which computation it
/// is.
///
/// # Arguments
///
/// * `function` - The computation's `function`.
///
/// # Returns
///
/// The attributes it takes, or `None` for a name that is not a computation.
pub fn attributes_of(function: &str) -> Option<&'static [&'static str]> {
    match function {
        "to_kelvin" => Some(&["function", "format", "scale", "offset"]),
        "vote" => Some(&["function", "inputs", "algorithm"]),
        "hysteresis" => Some(&["function", "setpoint", "half_band"]),
        "process" => Some(&["function", "executable", "timeout", "restart"]),
        _ => None,
    }
}

/// Reads what this compute node computes, or what is wrong with what it says.
///
/// # Arguments
///
/// * `what`       - The node's name, for the messages.
/// * `attributes` - The node's attributes as the file states them.
///
/// # Returns
///
/// The computation and its parameters.
///
/// # Errors
///
/// A message naming the node for a missing or unknown `function`, an
/// attribute the computation does not take, and a parameter that is missing
/// or wrong for it: a format or algorithm it does not know, a scale of
/// nought, a vote over fewer than three inputs or over one twice, a
/// half-band that is not positive, or a timeout that is not an interval.
pub fn params_of(
    what: &str,
    attributes: &BTreeMap<String, Scalar>,
) -> Result<Computation, String> {
    let function = attributes
        .get("function")
        .map(|f| f.as_str().to_string())
        .ok_or_else(|| {
        format!(
            "\"{what}\" is a compute node and says no function: one of {}",
            functions()
        )
    })?;

    // Every attribute, so that one nothing reads is refused rather than
    // ignored: a thermostat whose `setpiont` was misspelled would be a
    // thermostat with no setpoint, which is refused for the absence and never
    // for the typo -- and a reader comparing the two words is the only way
    // anybody would find it.
    let named = |why: String| format!("\"{what}\" ({function}): {why}");
    // One list, read here and said out loud by `attributes_of`: two would be
    // two places for a parameter to be added to.
    let Some(own) = attributes_of(function.as_str()) else {
        return Err(format!(
            "\"{what}\": there is no computation called \"{function}\". The \
             built-in ones are to_kelvin, vote and hysteresis; \"process\" \
             runs a program of your own"
        ));
    };
    for attribute in attributes.keys() {
        if !own.contains(&attribute.as_str()) {
            return Err(named(
                match crate::graph::spelled(attribute, |other| {
                    own.contains(&other)
                }) {
                    Some(hint) => {
                        format!(
                            "there is nothing called \"{attribute}\" on this \
                             computation: {hint}"
                        )
                    }
                    None => format!(
                        "there is nothing called \"{attribute}\" on this \
                         computation. It takes {}",
                        own.join(", ")
                    ),
                },
            ));
        }
    }

    match function.as_str() {
        "to_kelvin" => {
            let format = attributes
                .get("format")
                .ok_or_else(|| {
                    named(
                        "format is not set: a sensor's bytes cannot be read \
                         without knowing how they are laid out"
                            .to_string(),
                    )
                })
                .and_then(|f| Format::of(f.as_str()).map_err(named))?;

            let scale = number(attributes, "scale").map_err(named)?;
            if scale == 0.0 {
                // Every reading would become the offset, which is a sensor
                // whose value never changes -- and that looks like a working
                // sensor reporting a steady temperature.
                return Err(named(
                    "scale: nought would make every reading the same"
                        .to_string(),
                ));
            }

            Ok(Computation::ToKelvin(ToKelvinParams {
                format,
                scale,
                // A temperature: it is what a count of nought converts to, so
                // an offset outside the range is every reading outside it.
                offset: temperature(attributes, "offset").map_err(named)?,
            }))
        }
        "vote" => {
            let inputs = attributes.get("inputs").ok_or_else(|| {
                named(
                    "inputs is not set: a vote says which inputs it is over, \
                     so that a link deleted from it is a fault and not a vote \
                     over fewer"
                        .to_string(),
                )
            })?;
            let inputs: Vec<String> = inputs
                .as_str()
                .split([',', ' '])
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect();

            if inputs.len() < 3 {
                return Err(named(format!(
                    "a vote is over at least three inputs, and this says {}: \
                     two readings that disagree have no majority between them, \
                     and one is the number itself",
                    inputs.len()
                )));
            }
            let mut once = inputs.clone();
            once.sort();
            once.dedup();
            if once.len() != inputs.len() {
                return Err(named(
                    "an input is named twice: a vote over an input counted \
                     twice gives that sensor two votes"
                        .to_string(),
                ));
            }

            let algorithm = attributes
                .get("algorithm")
                .ok_or_else(|| {
                    named(
                        "algorithm is not set: a vote says how it picks the \
                         one value out of its readings, there being more than \
                         one way to and no reason to prefer either by default"
                            .to_string(),
                    )
                })
                .and_then(|said| Algorithm::of(said.as_str()).map_err(named))?;

            Ok(Computation::Vote(VoteParams { inputs, algorithm }))
        }
        "hysteresis" => {
            // Both are temperatures -- a goal and the width of a band around
            // it -- so both are held to what a value carries.
            let half_band =
                temperature(attributes, "half_band").map_err(named)?;
            if half_band <= 0.0 {
                return Err(named(
                    "half_band is how far from the setpoint the decision \
                     changes, and must be more than nought: a band of nothing \
                     is a bare comparison, which cycles a relay on a wobble"
                        .to_string(),
                ));
            }

            Ok(Computation::Hysteresis(HysteresisParams {
                setpoint: temperature(attributes, "setpoint").map_err(named)?,
                half_band,
            }))
        }
        "process" => {
            let executable = attributes
                .get("executable")
                .map(|p| p.as_str().to_string())
                .ok_or_else(|| {
                    named(
                        "executable is not set: a compute node whose function \
                         is \"process\" is the program it names"
                            .to_string(),
                    )
                })?;

            // Required, with no default. How long a computation may take is a
            // property of the computation, and a default would be this
            // software guessing at somebody else's arithmetic -- then treating
            // a slow answer as a failure on the strength of the guess.
            let timeout = attributes
                .get("timeout")
                .ok_or_else(|| {
                    named(
                        "timeout is not set, and there is no default for it: \
                         how long a computation may take is a property of the \
                         computation"
                            .to_string(),
                    )
                })
                .and_then(|t| {
                    crate::graph::parse_interval(t.as_str())
                        .map_err(|why| named(format!("timeout: {why}")))
                })?;

            // The one place the two spellings are read, an app node taking the
            // same attribute: see `Restart::of`.
            let restart =
                Restart::of(attributes.get("restart").map(Scalar::as_str))
                    .map_err(named)?;

            Ok(Computation::Process(ProcessParams {
                executable,
                timeout,
                restart,
            }))
        }
        // Covered by the match above, which refuses a function this does not
        // know before any of its attributes are read.
        other => Err(format!(
            "\"{what}\": there is no computation called \"{other}\""
        )),
    }
}

/// Reads a number an attribute says.
///
/// Required, because every parameter these computations take is one the
/// arithmetic cannot be done without. A scale of nought or an absent setpoint
/// is not a default worth inventing -- it is a configuration that has not said
/// what it wants, and saying so is better than averaging against zero.
///
/// # Arguments
///
/// * `attributes` - The node's attributes.
/// * `name`       - The attribute's name.
///
/// # Returns
///
/// The number.
///
/// # Errors
///
/// A message when the attribute is not set or is not a number.
fn number(
    attributes: &BTreeMap<String, Scalar>,
    name: &str,
) -> Result<f64, String> {
    let text = attributes.get(name).ok_or_else(|| {
        format!("{name} is not set, and there is no sensible default for it")
    })?;
    text.as_str()
        .parse()
        .map_err(|_| format!("{name}: \"{}\" is not a number", text.as_str()))
}

/// A temperature a file states, held to what a value carries.
///
/// The same range a reading is held to, and for the same reason it is a rule
/// here rather than aboard: a setpoint of 3000 K is a thermostat whose
/// readings can never reach it, so the heaters would be driven on and held
/// there for the whole flight. A file is checked on a desk, where that is one
/// line to change.
///
/// Only the attributes that are temperatures. A `scale` goes through
/// [`number`] and not through this: it is a number of kelvin per count, not a
/// temperature, and what it may be depends on the part's counts, which no
/// file states.
fn temperature(
    attributes: &BTreeMap<String, Scalar>,
    name: &str,
) -> Result<f64, String> {
    let said = number(attributes, name)?;
    micro_kelvin(said).map_err(|why| format!("{name}: {why}"))?;
    Ok(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A value survives the trip there and back.
    #[test]
    fn a_value_reads_back_as_itself() {
        for number in [0.0, 1.0, 273.15, 293.15, -40.0, 0.0625, 1000.0] {
            let sent = as_value(number).expect("a temperature a value carries");
            let back = from_value(&sent, "a value").expect("it reads back");
            assert!(
                (back - number).abs() < 0.0001,
                "{number} came back as {back}"
            );
        }
    }

    /// A value is four bytes of micro-kelvin, most significant first, written
    /// out by hand so that the test states the order rather than taking it
    /// from the code it checks.
    ///
    /// Rounded to the unit and exact in it: what goes on the link is a whole
    /// number of micro-kelvin, so every node that reads it reads the same
    /// number rather than its own last bit of a double.
    #[test]
    fn a_value_is_a_count_of_micro_kelvin() {
        assert_eq!(
            as_value(293.15).expect("a temperature"),
            [0x11, 0x79, 0x1d, 0x30]
        );
        assert_eq!(as_value(0.0).expect("nought").len(), VALUE_BYTES);
        // Below zero, which is what the sign is for: a conversion with the
        // wrong offset produces one, and it must arrive as what it is.
        assert_eq!(
            as_value(-40.0).expect("below zero"),
            [0xfd, 0x9d, 0xa6, 0x00]
        );

        // The trip is exact for every temperature that is a whole number of
        // micro-kelvin, which is every temperature a value carries.
        for number in [0.0, 273.15, 293.15, -40.0, 0.0625, 1000.0, 0.000_001] {
            assert_eq!(
                from_value(
                    &as_value(number).expect("a temperature"),
                    "a value"
                )
                .expect("it reads back"),
                number
            );
        }

        // And what is finer than the unit is rounded to it, rather than
        // truncated: a third of a kelvin is 333_333 micro-kelvin and a hair.
        let third =
            from_value(&as_value(1.0 / 3.0).expect("a third"), "a third")
                .expect("it reads back");
        assert_eq!(third, 0.333_333);
    }

    /// A temperature a value cannot carry is refused, and named.
    ///
    /// Refused where it is made rather than where it is read, because this is
    /// where the number is still in hand to put in the complaint. Wrapping it
    /// would turn 2200 K into a cabin 42 K below zero and a thermostat would
    /// act on that.
    #[test]
    fn a_temperature_outside_the_range_is_refused() {
        for wrong in [2200.0, -2200.0, 1.0e9, f64::NAN, f64::INFINITY] {
            let said =
                as_value(wrong).expect_err("outside what a value carries");
            assert!(
                said.contains("2147.483647") || said.contains("number"),
                "the complaint should say what the range is: {said}"
            );
        }

        // The ends themselves are carried, and are what they say.
        assert_eq!(
            from_value(&as_value(2147.483647).expect("the top"), "the top"),
            Ok(2147.483647)
        );
        assert_eq!(
            from_value(
                &as_value(-2147.483648).expect("the bottom"),
                "the bottom"
            ),
            Ok(-2147.483648)
        );
    }

    /// What is not a value is reported rather than read as one.
    #[test]
    fn a_datum_that_is_not_a_value_is_reported() {
        // A width that is not four: a sensor's own bytes delivered where a
        // conversion's output was meant, or a decision, which is text.
        let said =
            from_value(b"293.15", "the fore sensor").expect_err("not a value");
        assert!(said.contains("fore sensor"), "{said}");
        assert!(said.contains('6'), "the width is worth saying: {said}");
        assert!(from_value(b"1", "a decision").is_err());

        // Four bytes are a value whatever they hold, there being no bit
        // pattern of a 32-bit integer that is not a number of micro-kelvin.
        // What a sensor cannot produce is refused where a value is made.
        assert_eq!(
            from_value(&[0x7F, 0xFF, 0xFF, 0xFF], "the top"),
            Ok(2147.483647)
        );
    }

    /// Which nodes send values, and which send something else.
    ///
    /// The question the ground asks before it shows a datum as a
    /// temperature. A thermostat is the computation that does not: what it
    /// sends is a decision.
    #[test]
    fn a_configuration_says_which_nodes_send_values() {
        assert!(result_is_a_value("to_kelvin"));
        assert!(result_is_a_value("vote"));
        assert!(result_is_a_value("process"));
        assert!(
            !result_is_a_value("hysteresis"),
            "a decision is not a value"
        );
        assert!(!result_is_a_value("to_kelvan"), "nor is a misspelling");

        let node = |pairs: &[(&str, &str)]| attributes(pairs);
        assert!(node_sends_a_value(
            "compute",
            &node(&[("function", "vote")])
        ));
        assert!(!node_sends_a_value(
            "compute",
            &node(&[("function", "hysteresis")])
        ));
        // A compute node with no function is a node that will be refused;
        // until then it does not have its data read as a temperature.
        assert!(!node_sends_a_value("compute", &node(&[])));
        // And nothing else makes one, whatever it says.
        for node_type in
            ["i2c", "gpio", "network", "log", "gateway_radio", "app_pl"]
        {
            assert!(
                !node_sends_a_value(node_type, &node(&[("function", "vote")])),
                "{node_type}"
            );
        }
    }

    /// A program's pipe carries a decimal, and it round-trips.
    ///
    /// The one place a value is written out and read back, by the node that
    /// runs a program of its own: see
    /// `tcspecial::node::vehicle::compute::process`. A value is a whole
    /// number of micro-kelvin, so six decimal places say all of it and
    /// nothing drifts by going through a program.
    #[test]
    fn a_program_reads_and_writes_a_decimal() {
        assert_eq!(as_text(293.15), "293.15");
        assert_eq!(as_text(-40.0), "-40");

        for number in [0.0, 293.15, 0.333_333, -40.0, 1.0e-9] {
            assert_eq!(
                from_text(&as_text(number), "a program")
                    .expect("it reads back"),
                number
            );
        }

        assert!(from_text("nearly", "a program").is_err());
        assert!(
            from_text("NaN", "a program").is_err(),
            "a program may not answer with a number nothing can be decided on"
        );
        // And the range a reading is held to, because what a program answers
        // with becomes a reading: see `from_text`.
        let said =
            from_text("3000", "a program").expect_err("outside the range");
        assert!(
            said.contains("a program") && said.contains("2147.483647"),
            "the refusal should name the program and the range: {said}"
        );
    }

    /// Makes a node's attributes as a file states them.
    ///
    /// # Arguments
    ///
    /// * `pairs` - Each attribute, with its value as written.
    ///
    /// # Returns
    ///
    /// The attributes, by name.
    fn attributes(pairs: &[(&str, &str)]) -> BTreeMap<String, Scalar> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), Scalar::of(*v)))
            .collect()
    }

    fn params(pairs: &[(&str, &str)]) -> Computation {
        params_of("node", &attributes(pairs))
            .unwrap_or_else(|why| panic!("{why}"))
    }

    fn refused(pairs: &[(&str, &str)]) -> String {
        params_of("node", &attributes(pairs))
            .expect_err("a computation that is wrong")
    }

    /// A temperature a value could not carry is refused in the file.
    ///
    /// The same range a reading is held to, checked where the file is read:
    /// a setpoint of 3000 K is a thermostat whose readings can never reach
    /// it, so the heaters would be driven on and held there for the whole
    /// flight -- and on a desk that is one line to change. An offset is the
    /// temperature a count of nought converts to, so an offset outside the
    /// range is every reading outside it.
    #[test]
    fn a_temperature_no_value_could_carry_is_refused() {
        let (coldest, hottest) = kelvin_range();

        for (pairs, attribute) in [
            (
                vec![
                    ("function", "hysteresis"),
                    ("setpoint", "3000"),
                    ("half_band", "2.0"),
                ],
                "setpoint",
            ),
            (
                vec![
                    ("function", "hysteresis"),
                    ("setpoint", "293.15"),
                    ("half_band", "4000"),
                ],
                "half_band",
            ),
            (
                vec![
                    ("function", "to_kelvin"),
                    ("format", "i16_be"),
                    ("scale", "0.0625"),
                    ("offset", "-2500"),
                ],
                "offset",
            ),
        ] {
            let said = refused(&pairs);
            assert!(
                said.contains(attribute) && said.contains("2147.483647"),
                "the refusal should name {attribute} and the range: {said}"
            );
        }

        // The ends themselves are accepted: a rule that refused what it
        // allows would be a rule nobody could satisfy at the edge.
        for setpoint in [coldest, hottest] {
            params(&[
                ("function", "hysteresis"),
                ("setpoint", &format!("{setpoint}")),
                ("half_band", "2.0"),
            ]);
        }

        // And a scale is not a temperature: it is kelvin per count, and what
        // it may be depends on counts no file states.
        params(&[
            ("function", "to_kelvin"),
            ("format", "i16_be"),
            ("scale", "100000"),
            ("offset", "0"),
        ]);
    }

    /// Each computation is built from what its node says.
    #[test]
    fn each_computation_is_what_its_node_says() {
        assert_eq!(
            params(&[
                ("function", "to_kelvin"),
                ("format", "i16_be"),
                ("scale", "0.0625"),
                ("offset", "273.15"),
            ]),
            Computation::ToKelvin(ToKelvinParams {
                format: Format::I16Be,
                scale: 0.0625,
                offset: 273.15,
            })
        );

        assert_eq!(
            params(&[
                ("function", "vote"),
                ("inputs", "in0,in1,in2"),
                ("algorithm", "median"),
            ]),
            Computation::Vote(VoteParams {
                inputs: vec![
                    "in0".to_string(),
                    "in1".to_string(),
                    "in2".to_string()
                ],
                algorithm: Algorithm::Median,
            })
        );

        assert_eq!(
            params(&[
                ("function", "hysteresis"),
                ("setpoint", "293.15"),
                ("half_band", "2.0"),
            ]),
            Computation::Hysteresis(HysteresisParams {
                setpoint: 293.15,
                half_band: 2.0,
            })
        );

        assert_eq!(
            params(&[
                ("function", "process"),
                ("executable", "/opt/flight/attitude"),
                ("timeout", "500 ms"),
            ]),
            Computation::Process(ProcessParams {
                executable: "/opt/flight/attitude".to_string(),
                timeout: Duration::from_millis(500),
                restart: Restart::Never,
            })
        );

        // And each says what it is called, which is what a complaint names.
        assert_eq!(
            params(&[
                ("function", "vote"),
                ("inputs", "a,b,c"),
                ("algorithm", "median"),
            ])
            .function(),
            "vote"
        );
    }

    /// A function nobody has is refused, listing the ones there are.
    #[test]
    fn a_function_that_is_not_one_is_refused() {
        let said = refused(&[("function", "levitate")]);
        assert!(
            said.contains("levitate") && said.contains("to_kelvin"),
            "{said}"
        );

        let said = refused(&[("scale", "1.0")]);
        assert!(said.contains("says no function"), "{said}");
    }

    /// What each computation cannot do without.
    #[test]
    fn a_computation_that_says_too_little_is_refused() {
        for (pairs, wanted) in [
            (
                vec![
                    ("function", "to_kelvin"),
                    ("scale", "1"),
                    ("offset", "0"),
                ],
                "format",
            ),
            (
                vec![
                    ("function", "to_kelvin"),
                    ("format", "i16_be"),
                    ("offset", "0"),
                ],
                "scale",
            ),
            (
                vec![
                    ("function", "to_kelvin"),
                    ("format", "i16_be"),
                    ("scale", "1"),
                ],
                "offset",
            ),
            (vec![("function", "vote")], "inputs is not set"),
            (
                vec![("function", "vote"), ("inputs", "in0,in1,in2")],
                "algorithm is not set",
            ),
            (
                vec![("function", "hysteresis"), ("setpoint", "293")],
                "half_band",
            ),
            (vec![("function", "process")], "executable is not set"),
            (
                vec![("function", "process"), ("executable", "/bin/cat")],
                "no default for it",
            ),
        ] {
            let said = refused(&pairs);
            assert!(said.contains(wanted), "{pairs:?}: {said}");
            assert!(
                said.contains("node"),
                "every complaint names the node: {said}"
            );
        }
    }

    /// A value that is not a value is refused, by name.
    #[test]
    fn a_value_that_is_not_one_is_refused() {
        for (pairs, wanted) in [
            (
                vec![
                    ("function", "to_kelvin"),
                    ("format", "f32_be"),
                    ("scale", "1"),
                    ("offset", "0"),
                ],
                "u24_be",
            ),
            (
                vec![
                    ("function", "to_kelvin"),
                    ("format", "i16_be"),
                    ("scale", "0"),
                    ("offset", "0"),
                ],
                "every reading the same",
            ),
            (
                vec![
                    ("function", "to_kelvin"),
                    ("format", "i16_be"),
                    ("scale", "fast"),
                    ("offset", "0"),
                ],
                "is not a number",
            ),
            (
                vec![
                    ("function", "hysteresis"),
                    ("setpoint", "293"),
                    ("half_band", "0"),
                ],
                "more than nought",
            ),
            (
                vec![
                    ("function", "process"),
                    ("executable", "/bin/cat"),
                    ("timeout", "soon"),
                ],
                "timeout",
            ),
            (
                vec![
                    ("function", "process"),
                    ("executable", "/bin/cat"),
                    ("timeout", "1 s"),
                    ("restart", "sometimes"),
                ],
                "sometimes",
            ),
        ] {
            let said = refused(&pairs);
            assert!(said.contains(wanted), "{pairs:?}: {said}");
        }
    }

    /// A vote is over at least three inputs, each named once, by an
    /// algorithm it knows.
    ///
    /// Three and not two: two readings that disagree have no majority between
    /// them, so a vote over two is a coin toss dressed as a decision.
    #[test]
    fn a_vote_is_over_at_least_three_inputs() {
        // Commas or spaces, as a file writes a list.
        assert_eq!(
            params(&[
                ("function", "vote"),
                ("inputs", "in0 in1 in2"),
                ("algorithm", "median"),
            ]),
            params(&[
                ("function", "vote"),
                ("inputs", "in0, in1, in2"),
                ("algorithm", "median"),
            ])
        );

        let said = refused(&[
            ("function", "vote"),
            ("inputs", "in0,in1"),
            ("algorithm", "median"),
        ]);
        assert!(said.contains("at least three"), "{said}");

        let said = refused(&[
            ("function", "vote"),
            ("inputs", "in0,in0,in1"),
            ("algorithm", "median"),
        ]);
        assert!(said.contains("twice"), "{said}");

        // And an algorithm it does not have, which names the ones it does.
        let said = refused(&[
            ("function", "vote"),
            ("inputs", "in0,in1,in2"),
            ("algorithm", "average"),
        ]);
        assert!(
            said.contains("average")
                && said.contains("median")
                && said.contains("closest_mean"),
            "{said}"
        );
    }

    /// An attribute nothing reads is refused, naming what the computation
    /// takes.
    #[test]
    fn an_attribute_nothing_reads_is_refused() {
        let said = refused(&[
            ("function", "hysteresis"),
            ("setpoint", "293.15"),
            ("setpiont", "300.0"),
            ("half_band", "2"),
        ]);
        assert!(
            said.contains("setpiont") && said.contains("setpoint, half_band"),
            "{said}"
        );

        // A parameter written with a hyphen for its underscore is told the
        // spelling it has.
        let said = refused(&[
            ("function", "hysteresis"),
            ("setpoint", "293.15"),
            ("half-band", "2"),
        ]);
        assert!(said.contains("it is spelled half_band"), "{said}");

        // And one that belongs to another computation, which is the same
        // mistake from the other direction.
        let said = refused(&[
            ("function", "vote"),
            ("inputs", "in0,in1,in2"),
            ("algorithm", "median"),
            ("setpoint", "293.15"),
        ]);
        assert!(said.contains("setpoint"), "{said}");
    }

    /// Restarting is never, unless the configuration says `true`.
    #[test]
    fn restarting_is_never_by_default() {
        let process = |pairs: &[(&str, &str)]| match params(pairs) {
            Computation::Process(process) => process,
            other => panic!("{other:?}"),
        };

        assert_eq!(
            process(&[
                ("function", "process"),
                ("executable", "/bin/cat"),
                ("timeout", "1 s"),
            ])
            .restart,
            Restart::Never
        );
        assert_eq!(
            process(&[
                ("function", "process"),
                ("executable", "/bin/cat"),
                ("timeout", "1 s"),
                ("restart", "false"),
            ])
            .restart,
            Restart::Never
        );
        assert_eq!(
            process(&[
                ("function", "process"),
                ("executable", "/bin/cat"),
                ("timeout", "1 s"),
                ("restart", "true"),
            ])
            .restart,
            Restart::WhenItStops
        );
    }
}
