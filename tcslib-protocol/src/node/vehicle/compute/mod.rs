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
//! on a desk by `tcs-verify` and must therefore be stated on this side.
//!
//! They are here for one reason: `tcs-verify` checks a configuration on a desk,
//! and it cannot check what it cannot see. The rules used to live beside the
//! arithmetic, inside the flight program, so a thermostat with no `below` or a
//! program with no `timeout` passed the ground check in silence and was
//! refused when the vehicle started. That is the wrong end of the trip. Every
//! other node type keeps its rules in this crate -- see [`crate::node`] -- and
//! the log node's move set the pattern.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::link_type::{Basic, LinkType};
use crate::unit::Unit;
use crate::Scalar;

/// How wide a value is on a link: a 32-bit integer.
pub const VALUE_BYTES: usize = 4;

/// Turns a number in a unit into the count of that unit's steps a value
/// carries.
///
/// The one place the range of a value is decided, and everything that turns a
/// number into a value comes here: a reading on its way onto a link, a number
/// read from text -- which is a program's answer or an operator's command --
/// and a number written in a configuration. How fine a step is, and so how
/// far a value reaches, is the unit's: see [`crate::unit`]. A temperature is
/// counted in micro-kelvin and reaches a little past 2147 K either way.
///
/// A number outside the range is refused and named rather than wrapped or
/// pinned to the end of it: wrapping turns 2200 K into a zone that is 42 K
/// below zero, pinning turns a runaway into a reading that looks like a hot
/// but surviving part, and either is a decision taken confidently on a number
/// nobody produced. A quantity that must carry more wants a unit with a
/// coarser step, and that is a change to make deliberately.
///
/// A NaN or an infinity is refused here too, and it is the comparison that
/// makes it matter: a vote sorts its readings, and NaN compares as neither
/// less nor greater, so a sort that met one would order the readings by where
/// they happened to lie and the middle one would be the middle of nothing.
///
/// # Arguments
///
/// * `number` - The number, in `unit`.
/// * `unit`   - What it is a number of.
///
/// # Returns
///
/// The count of the unit's steps, rounded to the nearest.
///
/// # Errors
///
/// A message naming the number for a NaN or an infinity, or for a number
/// outside the unit's range.
pub fn value_count(number: f64, unit: Unit) -> Result<i32, String> {
    if !number.is_finite() {
        return Err(format!(
            "{number} is not a number anything can be decided on"
        ));
    }

    // Rounded to the nearest step rather than truncated: the conversions
    // land a hair either side of the figure the part reported -- 289.15 K
    // arrives from a count as 289.14999999999998 -- and truncating would
    // take a step off half of them.
    let count = (number * unit.per_unit()).round();
    if count < f64::from(i32::MIN) || count > f64::from(i32::MAX) {
        let (least, most) = unit.range();
        return Err(format!(
            "{number} {unit} is outside what a value carries, which is \
             {least} {unit} to {most} {unit} in {}",
            unit.step()
        ));
    }
    Ok(count as i32)
}

/// A value as it travels: four bytes of a signed count of its unit's steps,
/// most significant first.
///
/// A number is carried as a whole number of steps rather than as the double
/// the arithmetic produced. A step is finer than any part here resolves, so
/// nothing a part reported is lost by it, and what is gained is that the
/// number on the link is exact: a conversion, a vote and a thermostat all see
/// the same integer, where three doubles of the same temperature can differ
/// in their last bit and a comparison between them turns on dust. It halves
/// what a reading costs in a log and on a link as well. The unit is not on
/// the link: the configuration says what each node produces.
///
/// Signed, although a temperature in kelvin is not, for two reasons. The
/// arithmetic that produces one is signed -- a conversion is `counts * scale +
/// offset` and a wrong scale produces a negative, which must arrive as a
/// negative to be seen for what it is rather than as a temperature near
/// 4295 K. And a difference of two values is a value's width, which a band
/// is, and that is signed by nature.
///
/// Most significant first because everything this software puts on the
/// network is, unless the protocol it goes in says otherwise: a timestamp's
/// microseconds, a node's id, a beacon interval, every field of a command and
/// an answer. A sensor's own bytes are the exception and say which way round
/// they are -- `i16_le`, `u24_be` -- because the part decides that and the
/// file states it.
///
/// What it costs is the range, which is where this is refused rather than
/// wrapped: see [`value_count`]. And a value in a telemetry log is four bytes
/// rather than a readable number, as is the datum a panel shows for a source.
/// A decision is still text, `1` or `0`, because what it drives is a set of
/// discrete lines and that is the value they take.
///
/// # Arguments
///
/// * `number` - The number, in `unit`.
/// * `unit`   - What it is a number of.
///
/// # Returns
///
/// The value's bytes.
///
/// # Errors
///
/// What [`value_count`] refuses.
pub fn as_value(number: f64, unit: Unit) -> Result<Vec<u8>, String> {
    Ok(value_bytes(number, unit)?.to_vec())
}

/// Makes a value as [`as_value`] does, as the four bytes themselves,
/// allocating nothing: what a computation aboard makes a value with, once the
/// vehicle is running.
///
/// # Arguments
///
/// * `number` - The number, in `unit`.
/// * `unit`   - What it is a number of.
///
/// # Returns
///
/// The value's bytes, most significant first.
///
/// # Errors
///
/// What [`value_count`] refuses.
pub fn value_bytes(
    number: f64,
    unit: Unit,
) -> Result<[u8; VALUE_BYTES], String> {
    Ok(value_count(number, unit)?.to_be_bytes())
}

/// Reads a value that arrived as a number in a unit.
///
/// Exactly four bytes, and every four bytes are a value: a count of steps is
/// a number whatever the bits say, so the width is the whole of what there is
/// to check here. What cannot be represented is refused where a value is made
/// instead, by [`value_count`], which is where the complaint can name the
/// number that would not fit. The unit is the configuration's, which says
/// what the node that sent it produces; the bytes do not say.
///
/// A width that is not four is a link carrying something that is not a value
/// -- a sensor's raw bytes delivered where a conversion's output was meant, or
/// a node whose output is a decision -- and taking four of them would be a
/// number made of whatever was there.
///
/// # Arguments
///
/// * `data` - The value's bytes as they arrived.
/// * `unit` - What it is a number of.
/// * `what` - What the value is, for the message.
///
/// # Returns
///
/// The number, in `unit`.
///
/// # Errors
///
/// A message when `data` is not [`VALUE_BYTES`] long.
pub fn from_value(data: &[u8], unit: Unit, what: &str) -> Result<f64, String> {
    let bytes: [u8; VALUE_BYTES] = data.try_into().map_err(|_| {
        format!(
            "{what}: a value is {VALUE_BYTES} bytes of {}, and {} arrived",
            unit.step(),
            data.len()
        )
    })?;

    Ok(f64::from(i32::from_be_bytes(bytes)) / unit.per_unit())
}

/// What a computation produces, as the ground reads it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Produces {
    /// A value: four bytes of micro-kelvin, which [`from_value`] reads.
    Value,
    /// A decision: a `boolean`, one byte, 1 or 0, which [`from_decision`]
    /// reads.
    Decision,
}

/// Tells what this computation produces.
///
/// Three of the four make a value: a conversion makes one out of a part's
/// counts, a vote makes one out of several, and a program's answer is read as
/// one. A thermostat is the exception -- what it produces is a decision, 1 or
/// 0, as the type it states it sends: see [`node_produces`].
///
/// Asked by whoever has to read a datum back: the ground shows a reading as a
/// temperature and a decision as on or off, and the only thing that can tell
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
/// [`Produces::Value`] for `to_kelvin`, `vote` and `process`,
/// [`Produces::Decision`] for `thermostat`, and `None` for a name that is not
/// a computation.
pub fn produces(function: &str) -> Option<Produces> {
    match function {
        "to_kelvin" | "to_value" | "vote" | "process" => Some(Produces::Value),
        "thermostat" => Some(Produces::Decision),
        // Not a computation at all. Said as nothing rather than refused: this
        // answers how to show a datum, and a node nobody can name does not
        // have its data read as a temperature or a decision on the strength
        // of a typo.
        _ => None,
    }
}

/// Tells whether what this computation produces is a value: see
/// [`produces`].
///
/// # Arguments
///
/// * `function` - The computation's `function`.
///
/// # Returns
///
/// `true` for `to_kelvin`, `vote` and `process`; `false` for `thermostat`
/// and for a name that is not a computation.
pub fn result_is_a_value(function: &str) -> bool {
    produces(function) == Some(Produces::Value)
}

/// Tells what a node sends, where it is a value or a decision.
///
/// Only a computation makes either: see [`produces`]. Everything else sends
/// what it was given or what its part produced -- a sensor's own bytes, a log
/// record, whatever a program wrote -- and none of that is a number in this
/// form or a decision.
///
/// # Arguments
///
/// * `node_type`  - The node's type.
/// * `attributes` - The node's attributes, which name its function.
///
/// # Returns
///
/// What a compute node's function produces, and `None` for every other
/// node.
pub fn node_produces(
    node_type: &str,
    attributes: &BTreeMap<String, Scalar>,
) -> Option<Produces> {
    if node_type != "compute" {
        return None;
    }
    match attributes
        .get("function")
        .and_then(|function| produces(function.as_str()))
    {
        // A decision only where it is sent as one, a `boolean`: sent as a
        // whole number, it is that number, which is no decision to read
        // back as on or off.
        Some(Produces::Decision) => match stated_type(attributes, "type-out") {
            Ok(LinkType::Basic(Basic::Boolean)) => Some(Produces::Decision),
            _ => None,
        },
        other => other,
    }
}

/// Tells whether what this node sends is a value: see [`node_produces`].
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
    node_produces(node_type, attributes) == Some(Produces::Value)
}

/// Tells whether what this node sends is a decision: see
/// [`node_produces`].
///
/// # Arguments
///
/// * `node_type`  - The node's type.
/// * `attributes` - The node's attributes, which name its function.
///
/// # Returns
///
/// `true` for a compute node whose function makes a decision.
pub fn node_sends_a_decision(
    node_type: &str,
    attributes: &BTreeMap<String, Scalar>,
) -> bool {
    node_produces(node_type, attributes) == Some(Produces::Decision)
}

/// A decision to assert, as it travels: a `boolean`, the one byte 1.
pub const DECISION_ON: u8 = 1;

/// A decision to unassert, as it travels: the one byte 0.
pub const DECISION_OFF: u8 = 0;

/// Reads a decision: a `boolean`, the one byte 1 or 0.
///
/// Anything else is refused, because what reads a decision drives something
/// with it, and a decision that cannot be read is not guessed at. A
/// decision a program writes is text, and is read as text where the program
/// is, at a `process` node's pipe.
///
/// The one statement of what a decision is, read by the ground that shows
/// one.
///
/// # Arguments
///
/// * `data` - The decision's bytes as they arrived.
/// * `what` - Who is reading it, for the message.
///
/// # Returns
///
/// True to assert, false to unassert.
///
/// # Errors
///
/// A message when `data` is not the one byte 1 or 0.
pub fn from_decision(data: &[u8], what: &str) -> Result<bool, String> {
    match data {
        [DECISION_ON] => Ok(true),
        [DECISION_OFF] => Ok(false),
        _ => Err(format!(
            "{what}: {data:?} is not a decision, which is the one byte 1 or \
             0, and one that cannot be read is not guessed at"
        )),
    }
}

/// A value as a program reads and writes one: decimal text.
///
/// The one place a value is text, because the far end is a program reading
/// lines -- see `tcspecial::node::vehicle::compute::process` -- and a line is
/// text. Written out of the number the value carried, which is a whole number
/// of its unit's steps, so a program is handed no more places than there are
/// and what it reads back is what arrived.
pub fn as_text(number: f64) -> String {
    format!("{number}")
}

/// Reads a number a program or an operator wrote, in a unit.
///
/// Held to the same range as a value in that unit, by [`value_count`],
/// because what is read here becomes one: a program's answer goes straight
/// onto a link, and an operator's setpoint or band is compared against
/// readings that cannot leave that range. A number outside it is told so
/// when it is sent rather than carried as far as the arithmetic and lost
/// there.
///
/// # Arguments
///
/// * `text` - The number as written.
/// * `unit` - What it is a number of.
/// * `what` - What the number is, for the messages.
///
/// # Returns
///
/// The number, in `unit`.
///
/// # Errors
///
/// A message for text that is not a number, and for what [`value_count`]
/// refuses.
pub fn from_text(text: &str, unit: Unit, what: &str) -> Result<f64, String> {
    let said = text.trim();
    let number: f64 = said
        .parse()
        .map_err(|_| format!("{what}: \"{said}\" is not a number"))?;
    value_count(number, unit).map_err(|why| format!("{what}: {why}"))?;
    Ok(number)
}

/// How a whole number's bytes are laid out.
///
/// What a part puts on a bus is a number in its own units and its own order,
/// and the datasheet is the only statement of which. A device states it as
/// its `format` where its bytes are not its type's own layout -- most
/// significant byte first, as wide as the type -- and the vehicle lays them
/// out that way as it reads them, so that every node after it sees one
/// layout for each type: see "Typed links" in `docs/tcspecial-design.rst`. A
/// conversion reads that layout of its `type-in`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    /// One byte, unsigned.
    U8,
    /// One byte, signed.
    I8,
    /// Two bytes, signed, most significant first.
    I16Be,
    /// Two bytes, unsigned, most significant first.
    U16Be,
    /// Three bytes, unsigned, most significant first.
    U24Be,
    /// Two bytes, signed, least significant first.
    I16Le,
    /// Four bytes, signed, most significant first.
    I32Be,
    /// Four bytes, unsigned, most significant first.
    U32Be,
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
            "u8" => Ok(Format::U8),
            "i8" => Ok(Format::I8),
            "i16_be" => Ok(Format::I16Be),
            "u16_be" => Ok(Format::U16Be),
            "u24_be" => Ok(Format::U24Be),
            "i16_le" => Ok(Format::I16Le),
            "i32_be" => Ok(Format::I32Be),
            "u32_be" => Ok(Format::U32Be),
            other => Err(format!(
                "format: \"{other}\" is not one this knows. There are u8, \
                 i8, i16_be, u16_be, u24_be, i16_le, i32_be and u32_be"
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
            Format::U8 | Format::I8 => 1,
            Format::U24Be => 3,
            Format::I32Be | Format::U32Be => 4,
            _ => 2,
        }
    }

    /// Says which type a number laid out this way is.
    ///
    /// # Returns
    ///
    /// The whole number it is: a `u24_be` reading is a `u32`, the narrowest
    /// type with room for it.
    pub fn basic(self) -> Basic {
        match self {
            Format::U8 => Basic::U8,
            Format::I8 => Basic::I8,
            Format::I16Be | Format::I16Le => Basic::I16,
            Format::U16Be => Basic::U16,
            Format::U24Be | Format::U32Be => Basic::U32,
            Format::I32Be => Basic::I32,
        }
    }

    /// Finds a type's own layout: most significant byte first, as wide as the
    /// type.
    ///
    /// # Arguments
    ///
    /// * `basic` - The type.
    ///
    /// # Returns
    ///
    /// Its layout, or `None` for a type that is not a whole number of at
    /// most 32 bits, which no conversion reads.
    pub fn standard(basic: Basic) -> Option<Format> {
        match basic {
            Basic::U8 => Some(Format::U8),
            Basic::I8 => Some(Format::I8),
            Basic::I16 => Some(Format::I16Be),
            Basic::U16 => Some(Format::U16Be),
            Basic::I32 => Some(Format::I32Be),
            Basic::U32 => Some(Format::U32Be),
            _ => None,
        }
    }

    /// Lays a reading of this format out as its type's own layout, as the
    /// vehicle does where it reads a device.
    ///
    /// # Arguments
    ///
    /// * `data` - The reading, as the part laid it out.
    /// * `out`  - Where its type's layout is written: at least four bytes.
    ///
    /// # Returns
    ///
    /// How many bytes of `out` were written.
    ///
    /// # Errors
    ///
    /// A message when `data` is not exactly this format's width.
    pub fn to_standard(
        self,
        data: &[u8],
        out: &mut [u8],
    ) -> Result<usize, String> {
        let counts = self.counts(data)? as i64;
        let width = Format::standard(self.basic())
            .expect("a format's type has a layout")
            .width();
        let bytes = counts.to_be_bytes();
        out[..width].copy_from_slice(&bytes[8 - width..]);
        Ok(width)
    }

    /// Reads the number of counts these bytes hold.
    ///
    /// Exactly the width and no more: a reading longer than the format is a
    /// sensor that was read wrongly or a bus that returned something else,
    /// and taking the first two bytes of it would turn that into a plausible
    /// temperature. Shorter is the same mistake the other way.
    ///
    /// Here rather than with the conversion that uses it, so that the
    /// simulator, which simulates that conversion's thermostat, reads a
    /// sensor's bytes as the vehicle does.
    ///
    /// # Arguments
    ///
    /// * `data` - The reading, which must be exactly this format's width.
    ///
    /// # Returns
    ///
    /// The counts.
    ///
    /// # Errors
    ///
    /// A message saying how long the reading should be if `data` is longer
    /// or shorter.
    pub fn counts(self, data: &[u8]) -> Result<f64, String> {
        if data.len() != self.width() {
            return Err(format!(
                "a {:?} reading is {} bytes and {} arrived",
                self,
                self.width(),
                data.len()
            ));
        }

        Ok(match self {
            Format::U8 => data[0] as f64,
            Format::I8 => data[0] as i8 as f64,
            Format::I32Be => {
                i32::from_be_bytes([data[0], data[1], data[2], data[3]]) as f64
            }
            Format::U32Be => {
                u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as f64
            }
            Format::I16Be => i16::from_be_bytes([data[0], data[1]]) as f64,
            Format::U16Be => u16::from_be_bytes([data[0], data[1]]) as f64,
            Format::I16Le => i16::from_le_bytes([data[0], data[1]]) as f64,
            Format::U24Be => {
                u32::from_be_bytes([0, data[0], data[1], data[2]]) as f64
            }
        })
    }
}

/// What to do when an external program stops.
///
/// Declared by the family rather than by this module, two of its node types
/// running a program of their own: a computation whose function is `process`,
/// and an app node. Named from here as well, because this is where the
/// attribute was first written and where every caller of it has looked since.
pub use super::Restart;

/// Raw counts to a value: read the bytes, scale, add an offset, in a unit.
///
/// What every conversion is, `to_kelvin` and `to_value` alike: a part's
/// counts made a number in a unit. `to_kelvin` is the one whose unit is the
/// kelvin, kept by that name because a configuration that says it says what
/// its part measures.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToValueParams {
    pub format: Format,
    /// How much of `unit` one count is.
    pub scale: f64,
    /// What a count of nought is, in `unit`.
    pub offset: f64,
    /// What the value made is a number of.
    pub unit: Unit,
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
    /// How old a reading may be and still be voted on: one older is left
    /// out, and a vote left with fewer than a majority says so rather than
    /// voting. `None`, unstated, is no limit -- a reading stands until the
    /// same input sends another.
    pub age: Option<Duration>,
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
pub struct ThermostatParams {
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
    /// What it sends its decision as: its `type-out`, a `boolean` or a whole
    /// number, 1 when asserted and 0 when not.
    pub sends: Basic,
}

/// A computation that is a program of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessParams {
    pub executable: String,
    pub timeout: Duration,
    pub restart: Restart,
    /// What the program is handed, as a line of text: its `type-in`.
    pub takes: LinkType,
    /// What its answers are, which nothing else can say -- it is a program,
    /// and what it computes is its own business: its `type-out`.
    pub sends: LinkType,
}

/// What a compute node computes, settled.
#[derive(Debug, Clone, PartialEq)]
pub enum Computation {
    ToKelvin(ToValueParams),
    ToValue(ToValueParams),
    Vote(VoteParams),
    Thermostat(ThermostatParams),
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
            Computation::ToValue(_) => "to_value",
            Computation::Vote(_) => "vote",
            Computation::Thermostat(_) => "thermostat",
            Computation::Process(_) => "process",
        }
    }
}

/// Every function there is, for an error to list.
pub fn functions() -> &'static str {
    "to_kelvin, to_value, vote, thermostat, or process"
}

/// Lists what a computation is told, by name.
///
/// The list every computation is held to: `params_of` refuses an attribute
/// that is not on it, and this is the same list said out loud so that a tool
/// can ask. tcs-gui's description tree is what asks -- a description of a
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
        "to_kelvin" => Some(&["function", "type-in", "scale", "offset"]),
        "to_value" => {
            Some(&["function", "type-in", "scale", "offset", "type-out"])
        }
        "vote" => Some(&["function", "inputs", "algorithm", "age", "carries"]),
        "thermostat" => {
            Some(&["function", "setpoint", "half_band", "type-in", "type-out"])
        }
        "process" => Some(&[
            "function",
            "executable",
            "timeout",
            "restart",
            "type-in",
            "type-out",
        ]),
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
    // The thermostat was called hysteresis once, after how it switches
    // rather than what it is for. A file still saying so is told the new
    // word, rather than that no such computation exists.
    if function == "hysteresis" {
        return Err(format!(
            "\"{what}\": the function \"hysteresis\" is now called \
             \"thermostat\"; change the function to thermostat"
        ));
    }
    let Some(own) = attributes_of(function.as_str()) else {
        return Err(format!(
            "\"{what}\": there is no computation called \"{function}\". The \
             built-in ones are to_kelvin, vote and thermostat; \"process\" \
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
        "to_kelvin" | "to_value" => {
            // A to_kelvin's unit is in its name; a to_value states its own.
            let unit = if function == "to_kelvin" {
                Unit::Kelvin
            } else {
                value_type(attributes, "type-out").map_err(named)?
            };

            // What it reads is the whole number its sensor sends, laid out
            // as that type's own: the sensor's `format` is the sensor's.
            let format =
                match stated_type(attributes, "type-in").map_err(named)? {
                    LinkType::Basic(basic) => Format::standard(basic),
                    LinkType::Structure(_) => None,
                }
                .ok_or_else(|| {
                    named(
                    "type-in: a conversion reads a whole number of at most \
                     32 bits -- u8, i8, u16, i16, u32 or i32 -- the type its \
                     sensor sends"
                        .to_string(),
                )
                })?;

            let scale = number(attributes, "scale").map_err(named)?;
            if scale == 0.0 {
                // Every reading would become the offset, which is a sensor
                // whose value never changes -- and that looks like a working
                // sensor reporting a steady reading.
                return Err(named(
                    "scale: nought would make every reading the same"
                        .to_string(),
                ));
            }

            // A value: it is what a count of nought converts to, so an
            // offset outside the range is every reading outside it.
            let offset = in_unit(attributes, "offset", unit).map_err(named)?;
            let params = ToValueParams {
                format,
                scale,
                offset,
                unit,
            };
            Ok(if function == "to_kelvin" {
                Computation::ToKelvin(params)
            } else {
                Computation::ToValue(params)
            })
        }
        "vote" => {
            let inputs = attributes.get("inputs").ok_or_else(|| {
                named(
                    "its world-in lists no inputs: a vote names the inputs it \
                     is over, as `world-in: [in0, in1, in2]`, so that a link \
                     deleted from it is a fault and not a vote over fewer"
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

            // How old a reading may be. A length of time, as a timeout is,
            // and optional: unstated is no limit, which is what a vote was
            // before there was one.
            let age = attributes
                .get("age")
                .map(|said| {
                    crate::graph::parse_interval(said.as_str())
                        .map_err(|why| named(format!("age: {why}")))
                })
                .transpose()?;

            value_type(attributes, "carries").map_err(named)?;

            Ok(Computation::Vote(VoteParams {
                inputs,
                algorithm,
                age,
            }))
        }
        "thermostat" => {
            // Both are numbers in the unit of what the thermostat reads -- a
            // goal and the width of a band around it -- which this node
            // alone cannot say: whether each is in that unit's range is the
            // graph's to check, knowing what feeds it. See
            // `crate::graph::units`.
            let half_band = number(attributes, "half_band").map_err(named)?;
            if half_band <= 0.0 {
                return Err(named(
                    "half_band is how far from the setpoint the decision \
                     changes, and must be more than nought: a band of nothing \
                     is a bare comparison, which cycles a relay on a wobble"
                        .to_string(),
                ));
            }

            value_type(attributes, "type-in").map_err(named)?;
            let sends =
                match stated_type(attributes, "type-out").map_err(named)? {
                    LinkType::Basic(basic)
                        if basic == Basic::Boolean || basic.is_whole() =>
                    {
                        basic
                    }
                    other => {
                        return Err(named(format!(
                            "type-out is {other}, and a thermostat sends its \
                         decision as a boolean or as a whole number, 1 or 0"
                        )))
                    }
                };

            Ok(Computation::Thermostat(ThermostatParams {
                setpoint: number(attributes, "setpoint").map_err(named)?,
                half_band,
                sends,
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

            // Any type with a text form, which a program reads and writes:
            // see `crate::link_text`.
            let texted = |name: &str| {
                let kind = stated_type(attributes, name).map_err(named)?;
                if crate::link_text::has_text(&kind) {
                    Ok(kind)
                } else {
                    Err(named(format!(
                        "{name} is {kind}, which has no text form for a \
                         program to read or write: a value, a whole number, \
                         a boolean or a structure of them"
                    )))
                }
            };
            let takes = texted("type-in")?;
            let sends = texted("type-out")?;

            Ok(Computation::Process(ProcessParams {
                executable,
                timeout,
                restart,
                takes,
                sends,
            }))
        }
        // Covered by the match above, which refuses a function this does not
        // know before any of its attributes are read.
        other => Err(format!(
            "\"{what}\": there is no computation called \"{other}\""
        )),
    }
}

/// Reads a type an attribute states: see [`crate::link_type`].
///
/// Required, because a type is what the link to or from the node is checked
/// against, and nothing is inferred.
///
/// # Arguments
///
/// * `attributes` - The node's attributes.
/// * `name`       - The attribute's name: `carries`, `type-in` or
///                  `type-out`.
///
/// # Returns
///
/// The type.
///
/// # Errors
///
/// A message when the attribute is not set or names no type.
pub fn stated_type(
    attributes: &BTreeMap<String, Scalar>,
    name: &str,
) -> Result<LinkType, String> {
    let said = attributes.get(name).ok_or_else(|| {
        format!(
            "{name} is not set: each end of a link states the type it \
             carries, and this one names the type it {}",
            match name {
                "type-out" => "sends",
                "carries" => "takes and sends",
                _ => "takes",
            }
        )
    })?;
    LinkType::of(said.as_str()).map_err(|why| format!("{name}: {why}"))
}

/// Reads a value type an attribute states: one of the units' types.
///
/// # Arguments
///
/// * `attributes` - The node's attributes.
/// * `name`       - The attribute's name.
///
/// # Returns
///
/// The value type's unit.
///
/// # Errors
///
/// A message when the attribute is not set, names no type, or names one
/// that is not a value in a unit.
pub fn value_type(
    attributes: &BTreeMap<String, Scalar>,
    name: &str,
) -> Result<Unit, String> {
    let stated = stated_type(attributes, name)?;
    stated.unit().ok_or_else(|| {
        let units: Vec<&str> =
            Unit::ALL.iter().map(|unit| unit.type_name()).collect();
        format!(
            "{name} is {stated}, and this computation works on values in a \
             unit: one of {}",
            units.join(", ")
        )
    })
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

/// A number a file states in a unit, held to what a value in that unit
/// carries.
///
/// The same range a reading is held to, and for the same reason it is a rule
/// here rather than aboard: an offset of 3000 K is a conversion whose every
/// reading is out of range, and is one line to change on a desk.
///
/// # Arguments
///
/// * `attributes` - The node's attributes.
/// * `name`       - The attribute.
/// * `unit`       - What it is a number of.
///
/// # Returns
///
/// The number.
///
/// # Errors
///
/// A message when it is not set, not a number, or outside the unit's range.
fn in_unit(
    attributes: &BTreeMap<String, Scalar>,
    name: &str,
    unit: Unit,
) -> Result<f64, String> {
    let said = number(attributes, name)?;
    value_count(said, unit).map_err(|why| format!("{name}: {why}"))?;
    Ok(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A value survives the trip there and back.
    #[test]
    fn a_value_reads_back_as_itself() {
        for number in [0.0, 1.0, 273.15, 293.15, -40.0, 0.0625, 1000.0] {
            let sent = as_value(number, Unit::Kelvin)
                .expect("a temperature a value carries");
            let back = from_value(&sent, Unit::Kelvin, "a value")
                .expect("it reads back");
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
            as_value(293.15, Unit::Kelvin).expect("a temperature"),
            [0x11, 0x79, 0x1d, 0x30]
        );
        assert_eq!(
            as_value(0.0, Unit::Kelvin).expect("nought").len(),
            VALUE_BYTES
        );
        // Below zero, which is what the sign is for: a conversion with the
        // wrong offset produces one, and it must arrive as what it is.
        assert_eq!(
            as_value(-40.0, Unit::Kelvin).expect("below zero"),
            [0xfd, 0x9d, 0xa6, 0x00]
        );

        // The trip is exact for every temperature that is a whole number of
        // micro-kelvin, which is every temperature a value carries.
        for number in [0.0, 273.15, 293.15, -40.0, 0.0625, 1000.0, 0.000_001] {
            assert_eq!(
                from_value(
                    &as_value(number, Unit::Kelvin).expect("a temperature"),
                    Unit::Kelvin,
                    "a value"
                )
                .expect("it reads back"),
                number
            );
        }

        // And what is finer than the unit is rounded to it, rather than
        // truncated: a third of a kelvin is 333_333 micro-kelvin and a hair.
        let third = from_value(
            &as_value(1.0 / 3.0, Unit::Kelvin).expect("a third"),
            Unit::Kelvin,
            "a third",
        )
        .expect("it reads back");
        assert_eq!(third, 0.333_333);
    }

    /// A temperature a value cannot carry is refused, and named.
    ///
    /// Refused where it is made rather than where it is read, because this is
    /// where the number is still in hand to put in the complaint. Wrapping it
    /// would turn 2200 K into a zone 42 K below zero and a thermostat would
    /// act on that.
    #[test]
    fn a_temperature_outside_the_range_is_refused() {
        for wrong in [2200.0, -2200.0, 1.0e9, f64::NAN, f64::INFINITY] {
            let said = as_value(wrong, Unit::Kelvin)
                .expect_err("outside what a value carries");
            assert!(
                said.contains("2147.483647") || said.contains("number"),
                "the complaint should say what the range is: {said}"
            );
        }

        // The ends themselves are carried, and are what they say.
        assert_eq!(
            from_value(
                &as_value(2147.483647, Unit::Kelvin).expect("the top"),
                Unit::Kelvin,
                "the top"
            ),
            Ok(2147.483647)
        );
        assert_eq!(
            from_value(
                &as_value(-2147.483648, Unit::Kelvin).expect("the bottom"),
                Unit::Kelvin,
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
        let said = from_value(b"293.15", Unit::Kelvin, "the fore sensor")
            .expect_err("not a value");
        assert!(said.contains("fore sensor"), "{said}");
        assert!(said.contains('6'), "the width is worth saying: {said}");
        assert!(from_value(b"1", Unit::Kelvin, "a decision").is_err());

        // Four bytes are a value whatever they hold, there being no bit
        // pattern of a 32-bit integer that is not a number of micro-kelvin.
        // What a sensor cannot produce is refused where a value is made.
        assert_eq!(
            from_value(&[0x7F, 0xFF, 0xFF, 0xFF], Unit::Kelvin, "the top"),
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
            !result_is_a_value("thermostat"),
            "a decision is not a value"
        );
        assert!(!result_is_a_value("to_kelvan"), "nor is a misspelling");
        assert_eq!(produces("thermostat"), Some(Produces::Decision));
        assert_eq!(produces("to_kelvan"), None);

        let node = |pairs: &[(&str, &str)]| attributes(pairs);
        assert!(node_sends_a_value(
            "compute",
            &node(&[("function", "vote")])
        ));
        assert!(!node_sends_a_value(
            "compute",
            &node(&[("function", "thermostat")])
        ));
        // A compute node with no function is a node that will be refused;
        // until then it does not have its data read as a temperature.
        assert!(!node_sends_a_value("compute", &node(&[])));
        // A thermostat's decision is one where it is sent as a boolean,
        // and a whole number where it is sent as one.
        assert!(node_sends_a_decision(
            "compute",
            &node(&[("function", "thermostat"), ("type-out", "boolean")])
        ));
        assert!(!node_sends_a_decision(
            "compute",
            &node(&[("function", "thermostat"), ("type-out", "u8")])
        ));
        assert!(!node_sends_a_decision(
            "compute",
            &node(&[("function", "vote")])
        ));
        assert!(!node_sends_a_decision("compute", &node(&[])));
        // And nothing else makes one, whatever it says.
        for node_type in
            ["i2c", "gpio", "network", "log", "gateway_radio", "app_pl"]
        {
            assert!(
                !node_sends_a_value(node_type, &node(&[("function", "vote")])),
                "{node_type}"
            );
            assert!(
                !node_sends_a_decision(
                    node_type,
                    &node(&[("function", "thermostat")])
                ),
                "{node_type}"
            );
        }
    }

    /// A file still calling the thermostat by its old name is told the new
    /// one.
    #[test]
    fn the_old_name_of_the_thermostat_is_refused_with_the_new_one() {
        let why = params_of(
            "thermostat",
            &attributes(&[
                ("function", "hysteresis"),
                ("setpoint", "293.15"),
                ("half_band", "2.0"),
            ]),
        )
        .expect_err("refused");
        assert!(why.contains("change the function to thermostat"), "{why}");
    }

    /// A decision is a boolean, the one byte 1 or 0, and nothing else: not
    /// a value, not the text a person types, not two bytes.
    #[test]
    fn a_decision_is_1_or_0() {
        assert_eq!(from_decision(&[1], "x"), Ok(true));
        assert_eq!(from_decision(&[0], "x"), Ok(false));
        assert_eq!(from_decision(&[DECISION_OFF], "x"), Ok(false));
        for bad in [&b""[..], &[2], b"1", b"0", &[0, 1], b"\x00\x00\x00\x01"] {
            let why = from_decision(bad, "heater_wanted")
                .expect_err("not a decision");
            assert!(why.contains("heater_wanted"), "{why}");
            assert!(why.contains("not a decision"), "{why}");
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
                from_text(&as_text(number), Unit::Kelvin, "a program")
                    .expect("it reads back"),
                number
            );
        }

        assert!(from_text("nearly", Unit::Kelvin, "a program").is_err());
        assert!(
            from_text("NaN", Unit::Kelvin, "a program").is_err(),
            "a program may not answer with a number nothing can be decided on"
        );
        // And the range a reading is held to, because what a program answers
        // with becomes a reading: see `from_text`.
        let said = from_text("3000", Unit::Kelvin, "a program")
            .expect_err("outside the range");
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
    /// Makes a node's attributes, with the types a vote, a thermostat or a
    /// program states where the test does not state them: what each is
    /// told besides is what these tests are about.
    ///
    /// # Arguments
    ///
    /// * `pairs` - Each attribute and its value.
    ///
    /// # Returns
    ///
    /// The attributes.
    fn attributes(pairs: &[(&str, &str)]) -> BTreeMap<String, Scalar> {
        let mut said = bare(pairs);
        let function = said
            .get("function")
            .map(|f| f.as_str().to_string())
            .unwrap_or_default();
        let types: &[(&str, &str)] = match function.as_str() {
            "vote" => &[("carries", "kelvin")],
            "thermostat" => &[("type-in", "kelvin"), ("type-out", "u8")],
            "process" => &[("type-in", "kelvin"), ("type-out", "kelvin")],
            _ => &[],
        };
        for (name, value) in types {
            said.entry(name.to_string())
                .or_insert_with(|| Scalar::of(*value));
        }
        said
    }

    /// Makes a node's attributes as the pairs say and no others.
    ///
    /// # Arguments
    ///
    /// * `pairs` - Each attribute and its value.
    ///
    /// # Returns
    ///
    /// The attributes.
    fn bare(pairs: &[(&str, &str)]) -> BTreeMap<String, Scalar> {
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

    /// An offset a value in the conversion's unit could not carry is
    /// refused in the file.
    ///
    /// The same range a reading is held to, checked where the file is read:
    /// an offset is what a count of nought converts to, so an offset outside
    /// the range is every reading outside it -- and on a desk that is one
    /// line to change. A thermostat's setpoint and band are held to the
    /// range of what it reads, which only the graph knows: see
    /// `crate::graph::units`.
    #[test]
    fn a_temperature_no_value_could_carry_is_refused() {
        let (coldest, hottest) = Unit::Kelvin.range();

        for (pairs, attribute) in [
            (
                vec![
                    ("function", "to_kelvin"),
                    ("type-in", "i16"),
                    ("scale", "0.0625"),
                    ("offset", "-2500"),
                ],
                "offset",
            ),
            (
                vec![
                    ("function", "to_value"),
                    ("type-in", "i16"),
                    ("scale", "0.0625"),
                    ("offset", "-2500"),
                    ("type-out", "kelvin"),
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
                ("function", "thermostat"),
                ("setpoint", &format!("{setpoint}")),
                ("half_band", "2.0"),
            ]);
        }

        // And a scale is not a temperature: it is kelvin per count, and what
        // it may be depends on counts no file states.
        params(&[
            ("function", "to_kelvin"),
            ("type-in", "i16"),
            ("scale", "100000"),
            ("offset", "0"),
        ]);
    }

    /// A to_value states the value type it sends and makes values in its
    /// unit; one that states none, or one that is not a value type, is
    /// refused, saying what to write. A to_kelvin is one in kelvin, and a
    /// process states its type-out too.
    #[test]
    fn a_conversion_states_its_unit() {
        let conversion = |kind: Option<&str>| {
            let mut pairs = vec![
                ("function", "to_value"),
                ("type-in", "u16"),
                ("scale", "0.001"),
                ("offset", "0"),
            ];
            if let Some(kind) = kind {
                pairs.push(("type-out", kind));
            }
            params_of("bus", &attributes(&pairs))
        };
        match conversion(Some("volt")) {
            Ok(Computation::ToValue(params)) => {
                assert_eq!(params.unit, Unit::Volt);
                assert_eq!(params.format, Format::U16Be);
            }
            other => panic!("{other:?}"),
        }
        let why = conversion(None).expect_err("no type-out");
        assert!(why.contains("type-out is not set"), "{why}");
        let why = conversion(Some("furlong")).expect_err("not a type");
        assert!(why.contains("furlong") && why.contains("kelvin"), "{why}");
        let why = conversion(Some("u8")).expect_err("not a value");
        assert!(why.contains("u8") && why.contains("unit"), "{why}");
        assert_eq!(produces("to_value"), Some(Produces::Value));

        let why = params_of(
            "node",
            &bare(&[
                ("function", "process"),
                ("executable", "/bin/cat"),
                ("timeout", "1 s"),
                ("type-in", "kelvin"),
            ]),
        )
        .expect_err("no type-out");
        assert!(why.contains("type-out is not set"), "{why}");
    }

    /// A value is counted in its unit's steps, so the same four bytes are a
    /// different number in each, and each reads back as itself.
    #[test]
    fn a_value_is_counted_in_its_units_steps() {
        let volts = as_value(28.5, Unit::Volt).expect("a voltage");
        assert_eq!(from_value(&volts, Unit::Volt, "v"), Ok(28.5));
        let metres = as_value(15_000.25, Unit::Metre).expect("an altitude");
        assert_eq!(from_value(&metres, Unit::Metre, "m"), Ok(15_000.25));
        // 15 km is past what micro-units reach, which is what a coarser
        // step for metres is for.
        assert!(as_value(15_000.0, Unit::Volt).is_err());
    }

    /// Each computation is built from what its node says.
    #[test]
    fn each_computation_is_what_its_node_says() {
        assert_eq!(
            params(&[
                ("function", "to_kelvin"),
                ("type-in", "i16"),
                ("scale", "0.0625"),
                ("offset", "273.15"),
            ]),
            Computation::ToKelvin(ToValueParams {
                format: Format::I16Be,
                scale: 0.0625,
                offset: 273.15,
                unit: Unit::Kelvin,
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
                age: None,
            })
        );

        assert_eq!(
            params(&[
                ("function", "thermostat"),
                ("setpoint", "293.15"),
                ("half_band", "2.0"),
            ]),
            Computation::Thermostat(ThermostatParams {
                setpoint: 293.15,
                half_band: 2.0,
                sends: Basic::U8,
            })
        );

        assert_eq!(
            params(&[
                ("function", "process"),
                ("executable", "/opt/flight/attitude"),
                ("timeout", "500 ms"),
                ("type-out", "ratio"),
            ]),
            Computation::Process(ProcessParams {
                executable: "/opt/flight/attitude".to_string(),
                timeout: Duration::from_millis(500),
                restart: Restart::Never,
                takes: LinkType::Basic(Basic::Value(Unit::Kelvin)),
                sends: LinkType::Basic(Basic::Value(Unit::Ratio)),
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
                "type-in",
            ),
            (
                vec![
                    ("function", "to_kelvin"),
                    ("type-in", "i16"),
                    ("offset", "0"),
                ],
                "scale",
            ),
            (
                vec![
                    ("function", "to_kelvin"),
                    ("type-in", "i16"),
                    ("scale", "1"),
                ],
                "offset",
            ),
            (vec![("function", "vote")], "its world-in lists no inputs"),
            (
                vec![("function", "vote"), ("inputs", "in0,in1,in2")],
                "algorithm is not set",
            ),
            (
                vec![("function", "thermostat"), ("setpoint", "293")],
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
                    ("type-in", "f32"),
                    ("scale", "1"),
                    ("offset", "0"),
                ],
                "is not a type",
            ),
            (
                vec![
                    ("function", "to_kelvin"),
                    ("type-in", "i16"),
                    ("scale", "0"),
                    ("offset", "0"),
                ],
                "every reading the same",
            ),
            (
                vec![
                    ("function", "to_kelvin"),
                    ("type-in", "i16"),
                    ("scale", "fast"),
                    ("offset", "0"),
                ],
                "is not a number",
            ),
            (
                vec![
                    ("function", "thermostat"),
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
            ("function", "thermostat"),
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
            ("function", "thermostat"),
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
                ("type-out", "kelvin"),
            ])
            .restart,
            Restart::Never
        );
        assert_eq!(
            process(&[
                ("function", "process"),
                ("executable", "/bin/cat"),
                ("timeout", "1 s"),
                ("type-out", "kelvin"),
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
                ("type-out", "kelvin"),
                ("restart", "true"),
            ])
            .restart,
            Restart::WhenItStops
        );
    }
}
