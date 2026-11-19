//! What a consolidator sends: a **consolidation**, several readings of
//! different things as one datum, and how it is laid out, packed and taken
//! apart. See "Compute elements" in `docs/tcspecial-design.rst`.
//!
//! The datum is a structure of the consolidator's inputs, in the order its
//! `world-in` lists them, every member on a byte boundary and encoded as its
//! type is on any link: a whole number as wide as its type, most significant
//! byte first; a value as its four bytes; a boolean as one byte, 1 or 0; a
//! structure as its fields. A member whose length varies -- a string or
//! bytes -- keeps its place but takes no bytes there: its count and its bytes
//! go after every member of a fixed width, in the order those that vary are
//! listed. So every member of a fixed width is at the same offset in every
//! datum. The count is one byte for a most up to 255 and two, most
//! significant first, up to 65,535.
//!
//! Each input has a default, which the datum carries until a reading has
//! arrived at it; after that, its latest reading. Nothing in the datum says
//! which: that is the user's to say, in values and in members of their own.
//!
//! What is laid out is worked out from the configuration alone -- the
//! inputs' order, each source's type and each input's default -- so the
//! vehicle and the ground, reading the same configuration, lay a datum out
//! the same way. Packing allocates nothing, since the vehicle packs in its
//! loop, and neither does taking a datum apart into buffers made before.

use std::fmt::Write as _;
use std::ops::Range;

use crate::graph::link_types::{carries, Carries};
use crate::graph::{Destination, Graph};
use crate::link_type::{Basic, LinkType};
use crate::node::vehicle::compute::{self, Computation, ConsolidatorParams};

/// The most bytes a string or bytes input may hold: what a count of two
/// bytes counts to.
pub const MOST_VARYING: usize = 65_535;

/// How one input is laid out in the datum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Of a fixed width, in place: this many bytes.
    Fixed {
        /// Its width.
        bytes: usize,
    },
    /// A string or bytes, at the datum's end: a count of `count_bytes`, and
    /// then up to `most` bytes.
    Varies {
        /// The most bytes it may hold.
        most: usize,
        /// The width of its count: one byte up to 255, two above.
        count_bytes: usize,
    },
}

impl Shape {
    /// How an input of a type is laid out.
    ///
    /// # Arguments
    ///
    /// * `kind` - The type.
    /// * `most` - The most bytes it may hold, where it is a string or
    ///            bytes; not read otherwise.
    ///
    /// # Returns
    ///
    /// Its shape.
    pub fn of(kind: &LinkType, most: usize) -> Shape {
        match kind.width() {
            Some(bytes) => Shape::Fixed { bytes },
            None => Shape::Varies {
                most,
                count_bytes: count_bytes(most),
            },
        }
    }
}

/// The width of the count before a string or bytes that holds at most
/// `most` bytes.
///
/// # Arguments
///
/// * `most` - The most it holds.
///
/// # Returns
///
/// One byte up to 255, and two above.
pub fn count_bytes(most: usize) -> usize {
    if most <= usize::from(u8::MAX) {
        1
    } else {
        2
    }
}

/// One input of a consolidator, as its datum lays it out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    /// Its name, as the consolidator's `world-in` lists it.
    pub name: String,
    /// The node whose readings it is.
    pub source: String,
    /// The type that node sends, which the input is.
    pub kind: LinkType,
    /// How it is laid out.
    pub shape: Shape,
    /// Its default, encoded as its type is on a link: what the datum
    /// carries until a reading has arrived.
    pub default: Vec<u8>,
}

impl Input {
    /// The most bytes a reading of it is, as it arrives on its link.
    ///
    /// # Returns
    ///
    /// Its width, or the most a string or bytes of it may be.
    pub fn largest_link_bytes(&self) -> usize {
        match self.shape {
            Shape::Fixed { bytes } => bytes,
            Shape::Varies { most, .. } => most,
        }
    }

    /// The most bytes it takes in the datum: its width, or its count and
    /// its most.
    ///
    /// # Returns
    ///
    /// Its width, or its count's bytes and its most together.
    pub fn largest_bytes(&self) -> usize {
        match self.shape {
            Shape::Fixed { bytes } => bytes,
            Shape::Varies { most, count_bytes } => count_bytes + most,
        }
    }

    /// Says whether a reading is one of the input's type: its width, a
    /// boolean 1 or 0 -- alone or as a structure's field -- and a string or
    /// bytes no longer than its most, a string being UTF-8.
    ///
    /// # Arguments
    ///
    /// * `reading` - The reading, as it arrived on its link.
    ///
    /// # Returns
    ///
    /// Whether it is.
    pub fn accepts(&self, reading: &[u8]) -> bool {
        match (&self.kind, self.shape) {
            (_, Shape::Varies { most, .. }) => {
                reading.len() <= most
                    && (!matches!(self.kind, LinkType::Basic(Basic::String(_)))
                        || std::str::from_utf8(reading).is_ok())
            }
            (LinkType::Basic(Basic::Boolean), Shape::Fixed { .. }) => {
                matches!(reading, [0] | [1])
            }
            (LinkType::Basic(_), Shape::Fixed { bytes }) => {
                reading.len() == bytes
            }
            (LinkType::Structure(fields), Shape::Fixed { bytes }) => {
                if reading.len() != bytes {
                    return false;
                }
                let mut at = 0;
                for (_, basic) in fields {
                    let wide = basic.width().unwrap_or(0);
                    if *basic == Basic::Boolean && reading[at] > 1 {
                        return false;
                    }
                    at += wide;
                }
                true
            }
        }
    }
}

/// How a consolidator's datum is laid out: its inputs, in the order its
/// `world-in` lists them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The inputs, in order.
    pub inputs: Vec<Input>,
}

/// Reads an input's default from its `defaults` entry, as a value of its
/// type is written: a number or a value of a unit in decimal, a boolean 1
/// or 0, a structure as `name=value` pairs, and a string or bytes as a
/// trigger is, in quotes or square brackets, `""` and `[]` being empty.
///
/// # Arguments
///
/// * `input` - The input's name, for what is said.
/// * `kind`  - Its type.
/// * `most`  - The most a string or bytes holds.
/// * `said`  - The default as written.
///
/// # Returns
///
/// The default, encoded as the type is on a link.
///
/// # Errors
///
/// A sentence naming the input where the default is not a value of its
/// type, or is longer than its most.
fn default_of(
    input: &str,
    kind: &LinkType,
    most: usize,
    said: &str,
) -> Result<Vec<u8>, String> {
    let what = format!("the default of input \"{input}\"");
    if kind.width().is_some() {
        let mut out = vec![0u8; kind.width().unwrap_or(0)];
        let used = crate::link_text::read_text(kind, said, &what, &mut out)?;
        out.truncate(used);
        return Ok(out);
    }
    let said = said.trim();
    let bytes = if said == "\"\"" || said == "[]" {
        Vec::new()
    } else {
        crate::trigger::bytes_of(&DEFAULT, said)
            .map_err(|why| format!("{what}: {why}"))?
    };
    if bytes.len() > most {
        return Err(format!(
            "{what} is {} bytes, and the input holds at most {most}",
            bytes.len()
        ));
    }
    if matches!(kind, LinkType::Basic(Basic::String(_)))
        && std::str::from_utf8(&bytes).is_err()
    {
        return Err(format!("{what} is not text, and the input is a string"));
    }
    Ok(bytes)
}

/// A consolidator's default for a string or bytes input, as
/// [`crate::trigger::bytes_of`] names it.
const DEFAULT: crate::trigger::Named = crate::trigger::Named {
    word: "default",
    a: "a default",
    empty: "an empty one is written \"\" or []",
    bare_was_decimal: false,
};

impl Layout {
    /// Lays out a consolidator's datum from the configuration: each input's
    /// source, found by the link that reaches it, the type that source
    /// sends, and the input's default.
    ///
    /// # Arguments
    ///
    /// * `graph`    - The configuration.
    /// * `node`     - The consolidator's name.
    /// * `inputs`   - Its inputs, in the order its `world-in` lists them.
    /// * `defaults` - Each input's default as written, in the same order.
    ///
    /// # Returns
    ///
    /// The layout.
    ///
    /// # Errors
    ///
    /// A sentence naming an input no link reaches, a source whose type
    /// cannot be read, a string or bytes that may hold more than
    /// [`MOST_VARYING`], or a default that is not a value of its input's
    /// type.
    pub fn of(
        graph: &Graph,
        node: &str,
        inputs: &[String],
        defaults: &[String],
    ) -> Result<Layout, String> {
        let mut laid = Vec::with_capacity(inputs.len());
        for (at, name) in inputs.iter().enumerate() {
            let source = graph
                .links
                .iter()
                .find(|link| {
                    link.to.each().iter().any(|to| {
                        let to = Destination::parse(to);
                        to.node == node && to.input.as_deref() == Some(name)
                    })
                })
                .and_then(|link| graph.node(&link.origin().node))
                .ok_or_else(|| {
                    format!("no link reaches its input \"{name}\"")
                })?;
            let kind = match carries(source, "world-out")
                .map_err(|why| format!("input \"{name}\": {why}"))?
            {
                Carries::Only(kind) => kind,
                // A source that sends anything: bytes, as long as it may be.
                Carries::Any => LinkType::Basic(Basic::Bytes(None)),
            };
            let most =
                crate::graph::largest::largest_datum(graph, source).bytes;
            if kind.width().is_none() && most > MOST_VARYING {
                return Err(format!(
                    "input \"{name}\" may hold {most} bytes, and a \
                     consolidator's string or bytes holds at most \
                     {MOST_VARYING}: give its source a smaller most"
                ));
            }
            let default = default_of(
                name,
                &kind,
                most,
                defaults.get(at).map_or("", String::as_str),
            )?;
            laid.push(Input {
                name: name.clone(),
                source: source.name.clone(),
                shape: Shape::of(&kind, most),
                kind,
                default,
            });
        }
        Ok(Layout { inputs: laid })
    }

    /// Lays out the datum of a node of the configuration where it is a
    /// consolidator: what the vehicle packs by and the ground takes apart
    /// by, so both find a consolidator, and its layout, alike.
    ///
    /// # Arguments
    ///
    /// * `graph` - The configuration.
    /// * `node`  - The node.
    ///
    /// # Returns
    ///
    /// `None` for a node that is not a consolidator; otherwise what its
    /// node says and its layout, or a sentence where it does not work out,
    /// which the check refuses.
    pub fn of_node(
        graph: &Graph,
        node: &crate::graph::Node,
    ) -> Option<Result<(ConsolidatorParams, Layout), String>> {
        if node.node_type != "compute" {
            return None;
        }
        let Ok(Computation::Consolidator(params)) =
            compute::params_of(&node.name, &node.attributes)
        else {
            return None;
        };
        Some(
            Layout::of(graph, &node.name, &params.inputs, &params.defaults)
                .map(|layout| (params, layout)),
        )
    }

    /// The bytes every member of a fixed width takes together: where the
    /// counts and bytes of those that vary begin.
    ///
    /// # Returns
    ///
    /// The sum of the widths of the members of a fixed width.
    pub fn fixed_part(&self) -> usize {
        self.inputs
            .iter()
            .map(|input| match input.shape {
                Shape::Fixed { bytes } => bytes,
                Shape::Varies { .. } => 0,
            })
            .sum()
    }

    /// Where a member of a fixed width is in every datum.
    ///
    /// # Arguments
    ///
    /// * `at` - The input's position.
    ///
    /// # Returns
    ///
    /// Its offset, or `None` for one whose length varies, or no such input.
    pub fn offset(&self, at: usize) -> Option<usize> {
        let input = self.inputs.get(at)?;
        if !matches!(input.shape, Shape::Fixed { .. }) {
            return None;
        }
        Some(
            self.inputs[..at]
                .iter()
                .map(|before| match before.shape {
                    Shape::Fixed { bytes } => bytes,
                    Shape::Varies { .. } => 0,
                })
                .sum(),
        )
    }

    /// The most bytes a datum may be: every member at its most.
    pub fn largest_bytes(&self) -> usize {
        self.inputs.iter().map(Input::largest_bytes).sum()
    }

    /// The fewest bytes a datum may be: every string and bytes empty.
    ///
    /// # Returns
    ///
    /// The widths of the members of a fixed width, and the counts of
    /// those that vary.
    pub fn smallest_bytes(&self) -> usize {
        self.inputs
            .iter()
            .map(|input| match input.shape {
                Shape::Fixed { bytes } => bytes,
                Shape::Varies { count_bytes, .. } => count_bytes,
            })
            .sum()
    }

    /// The width of every datum, where every input is of a fixed width and
    /// so every datum is as long as every other.
    ///
    /// # Returns
    ///
    /// The width, or `None` where a member's length varies.
    pub fn fixed_bytes(&self) -> Option<usize> {
        self.inputs
            .iter()
            .all(|input| matches!(input.shape, Shape::Fixed { .. }))
            .then(|| self.fixed_part())
    }

    /// The most bytes every input's value together may be, each as it
    /// arrives on its link: room for what [`Layout::unpack_into`] puts.
    pub fn largest_link_bytes(&self) -> usize {
        self.inputs.iter().map(Input::largest_link_bytes).sum()
    }

    /// Says whether a reading is one of an input's type, as it arrives:
    /// one that is not is counted and dropped, the value before it
    /// standing.
    ///
    /// # Arguments
    ///
    /// * `at`      - The input's position.
    /// * `reading` - The reading, as it arrived on its link.
    ///
    /// # Returns
    ///
    /// Whether it is.
    pub fn accepts(&self, at: usize, reading: &[u8]) -> bool {
        self.inputs
            .get(at)
            .is_some_and(|input| input.accepts(reading))
    }

    /// Packs a datum, allocating nothing.
    ///
    /// # Arguments
    ///
    /// * `values` - Each input's value -- its latest reading, or its
    ///              default -- as it is on its link, in the order of the
    ///              inputs.
    /// * `out`    - Where the datum is written: at least
    ///              [`Layout::largest_bytes`] long.
    ///
    /// # Returns
    ///
    /// How many bytes of `out` the datum is.
    ///
    /// # Errors
    ///
    /// The input, by position, whose value is not of its type, or that
    /// does not fit `out`. Callers check a reading as it arrives, with
    /// [`Layout::accepts`], so that a bad one never displaces a good.
    pub fn pack(
        &self,
        values: &[&[u8]],
        out: &mut [u8],
    ) -> Result<usize, usize> {
        let mut at = 0;
        // The members of a fixed width, in place.
        for (n, (input, value)) in self.inputs.iter().zip(values).enumerate() {
            if let Shape::Fixed { .. } = input.shape {
                if !input.accepts(value) {
                    return Err(n);
                }
                let room = out.get_mut(at..at + value.len()).ok_or(n)?;
                room.copy_from_slice(value);
                at += value.len();
            }
        }
        // And each that varies, its count and its bytes, after them.
        for (n, (input, value)) in self.inputs.iter().zip(values).enumerate() {
            if let Shape::Varies { count_bytes, .. } = input.shape {
                if !input.accepts(value) {
                    return Err(n);
                }
                let count = (value.len() as u16).to_be_bytes();
                let room = out.get_mut(at..at + count_bytes).ok_or(n)?;
                room.copy_from_slice(&count[2 - count_bytes..]);
                at += count_bytes;
                let room = out.get_mut(at..at + value.len()).ok_or(n)?;
                room.copy_from_slice(value);
                at += value.len();
            }
        }
        if values.len() != self.inputs.len() {
            return Err(values.len().min(self.inputs.len()));
        }
        Ok(at)
    }

    /// Takes a datum apart into each input's value, allocating nothing:
    /// each is put back in its member's place, in the order of the inputs,
    /// end to end in `out`, and `each` says where. A member that varies is
    /// read from the datum's end and put in its place among the rest.
    ///
    /// # Arguments
    ///
    /// * `datum` - The datum.
    /// * `out`   - Where the values are put: at least
    ///             [`Layout::largest_link_bytes`] long.
    /// * `each`  - Where each input's value is in `out`, in the order of
    ///             the inputs: at least as long as there are inputs.
    ///
    /// # Errors
    ///
    /// A sentence where the datum is shorter than its members say, has a
    /// count past an input's most, a string that is not text, a boolean
    /// that is neither 0 nor 1, or bytes after its last member; or where
    /// `out` or `each` is too short.
    pub fn unpack_into(
        &self,
        datum: &[u8],
        out: &mut [u8],
        each: &mut [Range<usize>],
    ) -> Result<(), String> {
        if each.len() < self.inputs.len() {
            return Err(format!(
                "room for {} inputs, and the consolidation has {}",
                each.len(),
                self.inputs.len()
            ));
        }
        // Where each value lands in `out`, in the order of the inputs.
        let mut put = 0;
        for (input, slot) in self.inputs.iter().zip(each.iter_mut()) {
            *slot = put..put;
            put += input.largest_link_bytes();
        }
        if put > out.len() {
            return Err(format!(
                "room for {} bytes of values, and they need {put}",
                out.len()
            ));
        }
        self.walk(datum, |n, found| {
            let slot = &mut each[n];
            slot.end = slot.start + found.len();
            out[slot.clone()].copy_from_slice(&datum[found]);
        })
    }

    /// Says whether a datum is one this layout can have made, allocating
    /// nothing and putting nothing anywhere: what the ground asks of a
    /// datum it hands on whole.
    ///
    /// # Arguments
    ///
    /// * `datum` - The datum.
    ///
    /// # Errors
    ///
    /// As [`Layout::unpack_into`], for the datum.
    pub fn check(&self, datum: &[u8]) -> Result<(), String> {
        self.walk(datum, |_, _| {})
    }

    /// Writes a datum as text, each member `input=value` in the order of the
    /// inputs, `between` between them, as `defaults` writes a default: a
    /// number in decimal, a value in its unit, a boolean `1` or `0`, a
    /// structure's fields `name=value` in parentheses, a string in quotes
    /// and bytes in hexadecimal in square brackets. For a ground station
    /// that shows a consolidation to a person.
    ///
    /// # Arguments
    ///
    /// * `datum`   - The datum.
    /// * `each`    - Room to find each input's value in: at least as long as
    ///               there are inputs.
    /// * `between` - What goes between one member and the next: a space,
    ///               for a line, or a newline, for a member to a line.
    /// * `out`     - Where the text goes, appended to; it allocates nothing
    ///               where it has room.
    ///
    /// # Errors
    ///
    /// As [`Layout::unpack_into`], for the datum; nothing is written then.
    pub fn write_text(
        &self,
        datum: &[u8],
        each: &mut [Range<usize>],
        between: &str,
        out: &mut String,
    ) -> Result<(), String> {
        if each.len() < self.inputs.len() {
            return Err(format!(
                "room for {} inputs, and the consolidation has {}",
                each.len(),
                self.inputs.len()
            ));
        }
        self.walk(datum, |n, found| each[n] = found)?;
        for (n, (input, found)) in
            self.inputs.iter().zip(each.iter()).enumerate()
        {
            let value = &datum[found.clone()];
            if n > 0 {
                out.push_str(between);
            }
            let _ = write!(out, "{}=", input.name);
            match &input.kind {
                LinkType::Basic(Basic::String(_)) => {
                    // Checked as text as the datum was read.
                    let _ = write!(
                        out,
                        "\"{}\"",
                        std::str::from_utf8(value).unwrap_or_default()
                    );
                }
                LinkType::Basic(Basic::Bytes(_)) => {
                    out.push('[');
                    for (k, byte) in value.iter().enumerate() {
                        if k > 0 {
                            out.push(' ');
                        }
                        let _ = write!(out, "{byte:02x}");
                    }
                    out.push(']');
                }
                LinkType::Structure(_) => {
                    out.push('(');
                    crate::link_text::write_text(&input.kind, value, out)?;
                    out.push(')');
                }
                kind => crate::link_text::write_text(kind, value, out)?,
            }
        }
        Ok(())
    }

    /// Finds where each input's value is in a datum, checking it as it
    /// goes, allocating nothing and copying nothing: the one statement of how
    /// a datum is read, which [`Layout::unpack_into`], [`Layout::check`] and
    /// [`Layout::write_text`] share.
    ///
    /// # Arguments
    ///
    /// * `datum` - The datum.
    /// * `found` - Given each input's position and where its value is in
    ///             the datum, the members of a fixed width first and then
    ///             those that vary, each once and only where it is of its
    ///             type.
    ///
    /// # Errors
    ///
    /// A sentence where the datum is shorter than its members say, has a
    /// count past an input's most, a value not of its input's type, or
    /// bytes after its last member.
    fn walk(
        &self,
        datum: &[u8],
        mut found: impl FnMut(usize, Range<usize>),
    ) -> Result<(), String> {
        let short = || format!("a datum of {} bytes ends early", datum.len());
        let damaged = |input: &Input| {
            format!(
                "input \"{}\" is not a {}: the datum is damaged",
                input.name, input.kind
            )
        };
        // The members of a fixed width, in place, then those that vary.
        let mut at = 0;
        for (n, input) in self.inputs.iter().enumerate() {
            if let Shape::Fixed { bytes } = input.shape {
                let value = datum.get(at..at + bytes).ok_or_else(short)?;
                if !input.accepts(value) {
                    return Err(damaged(input));
                }
                found(n, at..at + bytes);
                at += bytes;
            }
        }
        for (n, input) in self.inputs.iter().enumerate() {
            if let Shape::Varies { most, count_bytes } = input.shape {
                let count =
                    datum.get(at..at + count_bytes).ok_or_else(short)?;
                at += count_bytes;
                let length = count
                    .iter()
                    .fold(0usize, |sum, byte| sum << 8 | usize::from(*byte));
                if length > most {
                    return Err(format!(
                        "input \"{}\" says {length} bytes, and it holds at \
                         most {most}",
                        input.name
                    ));
                }
                let value = datum.get(at..at + length).ok_or_else(short)?;
                if !input.accepts(value) {
                    return Err(damaged(input));
                }
                found(n, at..at + length);
                at += length;
            }
        }
        if at != datum.len() {
            return Err(format!(
                "a datum of {} bytes holds {at} of a consolidation and more \
                 after it",
                datum.len()
            ));
        }
        Ok(())
    }

    /// [`Layout::unpack_into`], allocating the values: for the ground, and
    /// tests.
    ///
    /// # Arguments
    ///
    /// * `datum` - The datum.
    ///
    /// # Returns
    ///
    /// Each input's value, in the order of the inputs.
    ///
    /// # Errors
    ///
    /// As [`Layout::unpack_into`].
    pub fn unpack(&self, datum: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        let mut out = vec![0u8; self.largest_link_bytes()];
        let mut each = vec![0..0; self.inputs.len()];
        self.unpack_into(datum, &mut out, &mut each)?;
        Ok(each.into_iter().map(|span| out[span].to_vec()).collect())
    }
}

/// How many inputs one answer to QUERY_CONSOLIDATOR tells of.
///
/// A fixed number, so that the answer has one size and packs without
/// allocating, as [`crate::THREADS_PER_ANSWER`] is for the threads. A
/// consolidator with more inputs is read in turns, each QUERY_CONSOLIDATOR
/// naming the first input it asks about; the answer says how many there
/// are in all.
pub const INPUTS_PER_ANSWER: usize = 32;

/// What a consolidator has sent since the vehicle started, which
/// QUERY_CONSOLIDATOR answers with.
///
/// A datum sent in a pass in which two triggers fired is one datum, counted
/// once in `sent` and once under each trigger that fired, so the three
/// triggers' counts may add up to more than `sent`: what each says is how
/// often that trigger would have sent on its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConsolidatorCounts {
    /// The datums it has sent.
    pub sent: u64,
    /// Those sent with every input new, where `send` names `all_new`.
    pub sent_all_new: u64,
    /// Those sent as its interval came round.
    pub sent_interval: u64,
    /// Those sent because something arrived at its `moc-in`.
    pub sent_moc: u64,
    /// Readings refused for not being of their input's type, such as a
    /// boolean that is neither 0 nor 1.
    pub refused: u64,
}

/// How often one input went into a consolidator's datum other than new.
///
/// How often it went in new is the datums sent less these two, so it is not
/// sent: one count fewer for every input, on a link that may be slow.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputCounts {
    /// The datums it went into held: the reading before, again.
    pub held: u64,
    /// The datums it went into with its default: nothing had arrived from
    /// it yet.
    pub none: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A layout of these inputs, each of this type, its default nought or
    /// empty.
    ///
    /// # Arguments
    ///
    /// * `kinds` - Each input's name, type, and most bytes, the most
    ///             counting only for a string or bytes.
    ///
    /// # Returns
    ///
    /// The layout, each input its own source.
    fn layout(kinds: &[(&str, LinkType, usize)]) -> Layout {
        Layout {
            inputs: kinds
                .iter()
                .map(|(name, kind, most)| Input {
                    name: name.to_string(),
                    source: name.to_string(),
                    kind: kind.clone(),
                    shape: Shape::of(kind, *most),
                    default: vec![0; kind.width().unwrap_or(0)],
                })
                .collect(),
        }
    }

    /// A boolean, a u16, a string of at most 8, bytes of at most 300 and an
    /// i8: the members of a fixed width in place, in order, and then the
    /// string's one-byte count and bytes and the bytes' two-byte count and
    /// bytes, in the order they are listed.
    ///
    /// # Returns
    ///
    /// The layout of the five.
    fn sample() -> Layout {
        layout(&[
            ("flag", LinkType::Basic(Basic::Boolean), 0),
            ("count", LinkType::Basic(Basic::U16), 0),
            ("name", LinkType::Basic(Basic::String(Some(8))), 8),
            ("data", LinkType::Basic(Basic::Bytes(Some(300))), 300),
            ("level", LinkType::Basic(Basic::I8), 0),
        ])
    }

    /// Every member of a fixed width is at one offset; those that vary
    /// follow, each a count and its bytes; and the datum reads back as it
    /// was, each value in its member's place.
    #[test]
    fn a_datum_is_its_members_with_those_that_vary_at_the_end() {
        let laid = sample();
        assert_eq!(laid.fixed_part(), 1 + 2 + 1);
        assert_eq!(
            (0..5).map(|at| laid.offset(at)).collect::<Vec<_>>(),
            vec![Some(0), Some(1), None, None, Some(3)]
        );
        assert_eq!(laid.smallest_bytes(), 4 + 1 + 2);
        assert_eq!(laid.largest_bytes(), 4 + 1 + 8 + 2 + 300);
        assert_eq!(laid.fixed_bytes(), None);

        let mut out = [0xeeu8; 400];
        let values: [&[u8]; 5] =
            [&[1], &[0x12, 0x34], b"hi", &[9, 8, 7], &[0xfe]];
        let used = laid.pack(&values, &mut out).expect("it packs");
        assert_eq!(
            &out[..used],
            &[1, 0x12, 0x34, 0xfe, 2, b'h', b'i', 0, 3, 9, 8, 7]
        );
        let back = laid.unpack(&out[..used]).expect("it unpacks");
        assert_eq!(back, values.iter().map(|v| v.to_vec()).collect::<Vec<_>>());
    }

    /// A string at its most and one empty, and bytes empty, are what they
    /// were; and an input wholly of a fixed width is one length always.
    #[test]
    fn a_member_that_varies_may_be_empty_or_at_its_most() {
        let laid = sample();
        let mut out = [0u8; 400];
        for name in [&b""[..], &b"12345678"[..]] {
            let values: [&[u8]; 5] = [&[0], &[0, 0], name, &[], &[0]];
            let used = laid.pack(&values, &mut out).expect("it packs");
            assert_eq!(used, 4 + 1 + name.len() + 2);
            assert_eq!(laid.unpack(&out[..used]).expect("it reads")[2], name);
        }
        let fixed = layout(&[
            ("a", LinkType::Basic(Basic::U32), 0),
            ("b", LinkType::Basic(Basic::Boolean), 0),
        ]);
        assert_eq!(fixed.fixed_bytes(), Some(5));
    }

    /// A count is one byte for a most up to 255, and two above.
    #[test]
    fn a_count_is_one_byte_or_two() {
        assert_eq!(count_bytes(0), 1);
        assert_eq!(count_bytes(255), 1);
        assert_eq!(count_bytes(256), 2);
        assert_eq!(count_bytes(MOST_VARYING), 2);
    }

    /// A reading not of its input's type is not accepted, nor packed: a
    /// boolean that is not 0 or 1, alone or a structure's field; one of the
    /// wrong width; a string longer than its most, or not text.
    #[test]
    fn a_reading_not_of_its_type_is_refused() {
        let laid = sample();
        assert!(laid.accepts(0, &[1]));
        assert!(!laid.accepts(0, &[2]));
        assert!(!laid.accepts(1, &[1]));
        assert!(!laid.accepts(2, b"123456789"));
        assert!(!laid.accepts(2, &[0xff]));
        assert!(laid.accepts(3, &[0xff]));
        let structure = layout(&[
            (
                "s",
                LinkType::Structure(vec![
                    ("on".to_string(), Basic::Boolean),
                    ("n".to_string(), Basic::U8),
                ]),
                0,
            ),
            ("t", LinkType::Basic(Basic::U8), 0),
        ]);
        assert!(structure.accepts(0, &[1, 200]));
        assert!(!structure.accepts(0, &[2, 200]));
        let mut out = [0u8; 8];
        assert_eq!(structure.pack(&[&[2, 0], &[0]], &mut out), Err(0));
    }

    /// A datum that cannot be one is refused: cut short, a count past its
    /// input's most, bytes after its last member, a boolean neither 0 nor
    /// 1.
    #[test]
    fn a_damaged_datum_is_refused() {
        let laid = sample();
        assert!(laid.unpack(&[1, 0, 0, 0]).is_err(), "no counts");
        assert!(
            laid.unpack(&[1, 0, 0, 0, 9, 0, 0]).is_err(),
            "nine bytes in a string of eight"
        );
        assert!(laid.unpack(&[1, 0, 0, 0, 0, 0, 0, 0]).is_err(), "one over");
        assert!(laid.unpack(&[2, 0, 0, 0, 0, 0, 0]).is_err(), "boolean 2");
        assert!(laid.unpack(&[1, 0, 0, 0, 0, 0, 0]).is_ok());
        assert!(laid.check(&[1, 0, 0, 0, 0, 0, 0]).is_ok());
        assert!(laid.check(&[2, 0, 0, 0, 0, 0, 0]).is_err());
    }

    /// A datum is written as text in the order of the inputs, each as its
    /// default would be written: the string, which went at the end, back in
    /// its place, and an empty string and bytes as "" and []. A damaged
    /// datum is refused.
    #[test]
    fn a_datum_is_written_as_text_in_the_order_of_its_inputs() {
        let laid = sample();
        let mut out = [0u8; 400];
        let values: [&[u8]; 5] =
            [&[1], &[0x12, 0x34], b"hi", &[9, 0xab], &[0xfe]];
        let used = laid.pack(&values, &mut out).expect("it packs");
        let mut each = vec![0..0; 5];
        let mut text = String::new();
        laid.write_text(&out[..used], &mut each, " ", &mut text)
            .expect("it is written");
        assert_eq!(text, "flag=1 count=4660 name=\"hi\" data=[09 ab] level=-2");
        let values: [&[u8]; 5] = [&[0], &[0, 0], b"", &[], &[0]];
        let used = laid.pack(&values, &mut out).expect("it packs");
        text.clear();
        laid.write_text(&out[..used], &mut each, " ", &mut text)
            .expect("it is written");
        assert_eq!(text, "flag=0 count=0 name=\"\" data=[] level=0");
        text.clear();
        laid.write_text(&out[..used], &mut each, "\n", &mut text)
            .expect("it is written");
        assert_eq!(text, "flag=0\ncount=0\nname=\"\"\ndata=[]\nlevel=0");
        text.clear();
        assert!(laid
            .write_text(&[2, 0, 0, 0, 0, 0, 0], &mut each, " ", &mut text)
            .is_err());
        assert!(text.is_empty());
    }

    /// A default is read as a value of its input's type is written, and one
    /// that is not one is refused, naming the input.
    #[test]
    fn a_default_is_a_value_of_its_type() {
        let u8_ = LinkType::Basic(Basic::U8);
        assert_eq!(default_of("a", &u8_, 0, "7"), Ok(vec![7]));
        assert!(default_of("a", &u8_, 0, "300")
            .is_err_and(|why| why.contains("\"a\"")));
        let flag = LinkType::Basic(Basic::Boolean);
        assert_eq!(default_of("f", &flag, 0, "1"), Ok(vec![1]));
        assert!(default_of("f", &flag, 0, "yes").is_err());
        let i16_ = LinkType::Basic(Basic::I16);
        assert_eq!(default_of("t", &i16_, 0, "-1"), Ok(vec![0xff, 0xff]));
        let text = LinkType::Basic(Basic::String(Some(4)));
        assert_eq!(default_of("s", &text, 4, "\"\""), Ok(vec![]));
        assert_eq!(default_of("s", &text, 4, "\"none\""), Ok(b"none".to_vec()));
        assert!(default_of("s", &text, 4, "\"nothing\"").is_err());
        assert!(default_of("s", &text, 4, "[ff]").is_err(), "not text");
        let bytes = LinkType::Basic(Basic::Bytes(Some(4)));
        assert_eq!(default_of("b", &bytes, 4, "[]"), Ok(vec![]));
        assert_eq!(default_of("b", &bytes, 4, "[de ad]"), Ok(vec![0xde, 0xad]));
    }
}
