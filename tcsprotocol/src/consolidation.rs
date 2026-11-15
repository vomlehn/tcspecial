//! What a consolidator sends: a **consolidation**, several readings of
//! different things as one datum, and how it is laid out, packed and taken
//! apart. See "Compute elements" in `docs/tcspecial-design.rst`.
//!
//! The datum is a block of states, two bits for each input in the order its
//! `world-in` lists them, then each input's **bit string**, packed end to end
//! with no padding between them, the whole padded with zero bits to a whole
//! byte. Every field is most significant bit first, so the datum's first bit
//! is the top bit of its first byte. An input of a fixed width -- a number, a
//! value of a unit, a boolean, a structure of those -- is a bit string of its
//! width; a boolean is one bit in it, [`BOOLEAN_BITS`], where it is a byte
//! on a link. A string or bytes is a length, a count of bytes in a field just
//! wide enough for the most the input may hold, and then its bytes.
//!
//! What is laid out is worked out from the configuration alone -- the
//! inputs' order and each source's type -- so the vehicle and the ground,
//! reading the same configuration, lay a datum out the same way. Packing
//! allocates nothing, since the vehicle packs in its loop; taking apart is
//! the ground's, which may.

use crate::graph::link_types::{carries, Carries};
use crate::graph::{Destination, Graph};
use crate::link_type::{Basic, LinkType};

/// How many bits a boolean is in a consolidation: one, where it is a byte
/// on a link. `types.xtce` states it, as `boolean_bit`, beside the byte.
pub const BOOLEAN_BITS: usize = 1;

/// How many bits each input's state takes, in the block of states.
pub const STATE_BITS: usize = 2;

/// What a consolidation says of one input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Nothing has arrived from it since tcspecial started: its bit string
    /// is zeros of its width, or an empty one.
    None,
    /// No reading has arrived since the last datum was sent: the value is
    /// the latest tcspecial has.
    Held,
    /// A reading has arrived since the last datum was sent.
    New,
}

impl State {
    /// The state's two bits: `00` for none, so that a datum of zeros says
    /// nothing has arrived, `01` for held and `10` for new.
    pub fn bits(self) -> u8 {
        match self {
            State::None => 0b00,
            State::Held => 0b01,
            State::New => 0b10,
        }
    }

    /// The state two bits say.
    ///
    /// # Arguments
    ///
    /// * `bits` - The two bits.
    ///
    /// # Returns
    ///
    /// The state, or `None` for `11`, which no state is.
    pub fn of(bits: u8) -> Option<State> {
        match bits & 0b11 {
            0b00 => Some(State::None),
            0b01 => Some(State::Held),
            0b10 => Some(State::New),
            _ => None,
        }
    }

    /// What the state is called where it is shown.
    pub fn as_str(self) -> &'static str {
        match self {
            State::None => "none",
            State::Held => "held",
            State::New => "new",
        }
    }
}

/// One field of a fixed-width input's bit string, as it is laid out in the
/// datum and as it is on the link it arrives on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    /// A whole number or a value of a unit: its bytes on the link, every
    /// bit of them, most significant first.
    Bits(usize),
    /// A boolean: one byte on the link, 1 or 0, and [`BOOLEAN_BITS`] in the
    /// datum.
    Boolean,
}

impl Field {
    /// The field of one basic type of a fixed width.
    ///
    /// # Arguments
    ///
    /// * `basic` - The type.
    ///
    /// # Returns
    ///
    /// The field, or `None` for a string or bytes.
    fn of(basic: Basic) -> Option<Field> {
        match basic {
            Basic::Boolean => Some(Field::Boolean),
            other => other.width().map(|bytes| Field::Bits(bytes * 8)),
        }
    }

    /// How many bytes it is on the link.
    fn link_bytes(self) -> usize {
        match self {
            Field::Bits(bits) => bits / 8,
            Field::Boolean => 1,
        }
    }

    /// How many bits it is in the datum.
    fn datum_bits(self) -> usize {
        match self {
            Field::Bits(bits) => bits,
            Field::Boolean => BOOLEAN_BITS,
        }
    }
}

/// How one input is laid out in the datum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shape {
    /// Of a fixed width: these fields, end to end.
    Fixed(Vec<FixedField>),
    /// A string or bytes: a length in `length_bits`, a count of bytes up to
    /// `most`, then the bytes.
    Counted {
        /// The most bytes it may hold.
        most: usize,
        /// The width of its length, just enough to count to `most`.
        length_bits: usize,
    },
}

impl Shape {
    /// How an input of one basic type is laid out.
    ///
    /// # Arguments
    ///
    /// * `basic` - The type.
    /// * `most`  - The most bytes it may hold, where it is a string or
    ///             bytes; not read otherwise.
    ///
    /// # Returns
    ///
    /// Its shape: its one field, or a length and bytes.
    pub fn of_basic(basic: Basic, most: usize) -> Shape {
        match Field::of(basic) {
            Some(field) => Shape::Fixed(vec![FixedField(field)]),
            None => Shape::Counted {
                most,
                length_bits: length_bits(most),
            },
        }
    }
}

/// A field of a fixed-width input, as [`Shape::Fixed`] lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedField(Field);

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
}

impl Input {
    /// The most bits its bit string may be: its width, or its length and
    /// its most bytes.
    pub fn largest_bits(&self) -> usize {
        match &self.shape {
            Shape::Fixed(fields) => {
                fields.iter().map(|f| f.0.datum_bits()).sum()
            }
            Shape::Counted { most, length_bits } => length_bits + most * 8,
        }
    }

    /// How many bytes a reading of it is on the link, where every reading
    /// is as long as every other.
    pub fn fixed_link_bytes(&self) -> Option<usize> {
        match &self.shape {
            Shape::Fixed(fields) => {
                Some(fields.iter().map(|f| f.0.link_bytes()).sum())
            }
            Shape::Counted { .. } => None,
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

/// The width of a field that counts from nought to `most`.
///
/// # Arguments
///
/// * `most` - The most it counts to.
///
/// # Returns
///
/// The fewest bits that hold `most`: seven for 64, thirteen for 4,096, and
/// one for nought or one.
pub fn length_bits(most: usize) -> usize {
    (usize::BITS - most.leading_zeros()).max(1) as usize
}

impl Layout {
    /// Lays out a consolidator's datum from the configuration: each input's
    /// source, found by the link that reaches it, and the type that source
    /// sends.
    ///
    /// # Arguments
    ///
    /// * `graph`  - The configuration.
    /// * `node`   - The consolidator's name.
    /// * `inputs` - Its inputs, in the order its `world-in` lists them.
    ///
    /// # Returns
    ///
    /// The layout.
    ///
    /// # Errors
    ///
    /// A sentence naming an input no link reaches, or a source whose type
    /// cannot be read.
    pub fn of(
        graph: &Graph,
        node: &str,
        inputs: &[String],
    ) -> Result<Layout, String> {
        let mut laid = Vec::with_capacity(inputs.len());
        for name in inputs {
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
            let shape = match &kind {
                LinkType::Basic(basic) => Shape::of_basic(
                    *basic,
                    crate::graph::largest::largest_datum(graph, source).bytes,
                ),
                LinkType::Structure(fields) => Shape::Fixed(
                    fields
                        .iter()
                        .map(|(field, basic)| {
                            Field::of(*basic).map(FixedField).ok_or_else(|| {
                                format!(
                                    "input \"{name}\": its field \"{field}\" \
                                     has no fixed width"
                                )
                            })
                        })
                        .collect::<Result<_, _>>()?,
                ),
            };
            laid.push(Input {
                name: name.clone(),
                source: source.name.clone(),
                kind,
                shape,
            });
        }
        Ok(Layout { inputs: laid })
    }

    /// The most bits a datum may be: the states and every input at its
    /// most, before the datum is padded to a whole byte.
    pub fn largest_bits(&self) -> usize {
        STATE_BITS * self.inputs.len()
            + self.inputs.iter().map(Input::largest_bits).sum::<usize>()
    }

    /// The most bytes a datum may be.
    pub fn largest_bytes(&self) -> usize {
        self.largest_bits().div_ceil(8)
    }

    /// The width of every datum, where every input is of a fixed width and
    /// so every datum is as long as every other.
    pub fn fixed_bytes(&self) -> Option<usize> {
        self.inputs
            .iter()
            .all(|input| matches!(input.shape, Shape::Fixed(_)))
            .then(|| self.largest_bytes())
    }

    /// Packs a datum, allocating nothing.
    ///
    /// # Arguments
    ///
    /// * `readings` - Each input's state and its latest reading, as it
    ///                arrived on its link, in the order of the inputs; a
    ///                reading of an input whose state is none is not read.
    /// * `out`      - Where the datum is written: at least
    ///                [`Layout::largest_bytes`] long.
    ///
    /// # Returns
    ///
    /// How many bytes of `out` the datum is.
    ///
    /// # Errors
    ///
    /// The input, by position, whose reading is not of its type: a reading
    /// of the wrong length, a boolean that is neither 0 nor 1, a string or
    /// bytes longer than its most. Callers check a reading as it arrives,
    /// with [`Layout::accepts`], so that a bad one never displaces a good.
    pub fn pack(
        &self,
        readings: &[(State, &[u8])],
        out: &mut [u8],
    ) -> Result<usize, usize> {
        let mut bits = Bits::writer(out);
        for (at, (state, _)) in readings.iter().enumerate() {
            bits.put(u64::from(state.bits()), STATE_BITS)
                .map_err(|_| at)?;
        }
        for (at, (input, (state, reading))) in
            self.inputs.iter().zip(readings).enumerate()
        {
            let none = *state == State::None;
            match &input.shape {
                Shape::Fixed(fields) => {
                    let mut from = 0;
                    for field in fields {
                        let field = field.0;
                        let bytes = field.link_bytes();
                        if none {
                            bits.zeros(field.datum_bits()).map_err(|_| at)?;
                            continue;
                        }
                        let here = reading.get(from..from + bytes).ok_or(at)?;
                        match field {
                            Field::Boolean => {
                                if here[0] > 1 {
                                    return Err(at);
                                }
                                bits.put(u64::from(here[0]), BOOLEAN_BITS)
                                    .map_err(|_| at)?;
                            }
                            Field::Bits(_) => {
                                for byte in here {
                                    bits.put(u64::from(*byte), 8)
                                        .map_err(|_| at)?;
                                }
                            }
                        }
                        from += bytes;
                    }
                    if !none && from != reading.len() {
                        return Err(at);
                    }
                }
                Shape::Counted { most, length_bits } => {
                    let reading: &[u8] = if none { &[] } else { reading };
                    if reading.len() > *most {
                        return Err(at);
                    }
                    bits.put(reading.len() as u64, *length_bits)
                        .map_err(|_| at)?;
                    for byte in reading {
                        bits.put(u64::from(*byte), 8).map_err(|_| at)?;
                    }
                }
            }
        }
        Ok(bits.bytes())
    }

    /// Says whether a reading is one of an input's type, as it arrives:
    /// one that is not is counted and dropped, the reading held before it
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
        let Some(input) = self.inputs.get(at) else {
            return false;
        };
        match &input.shape {
            Shape::Fixed(fields) => {
                let mut from = 0;
                for field in fields {
                    let bytes = field.0.link_bytes();
                    let Some(here) = reading.get(from..from + bytes) else {
                        return false;
                    };
                    if field.0 == Field::Boolean && here[0] > 1 {
                        return false;
                    }
                    from += bytes;
                }
                from == reading.len()
            }
            Shape::Counted { most, .. } => reading.len() <= *most,
        }
    }

    /// Takes a datum apart into each input's state and reading, each
    /// reading as it arrived on its link, so that what reads a datum of the
    /// source's type reads it.
    ///
    /// # Arguments
    ///
    /// * `datum` - The datum.
    ///
    /// # Returns
    ///
    /// Each input's state and reading, in the order of the inputs; the
    /// reading of an input whose state is none is empty.
    ///
    /// # Errors
    ///
    /// A sentence where the datum is shorter than it says it is, says a
    /// state no state is, has a length more than its input's most, or has
    /// bits left over past a byte's padding.
    pub fn unpack(
        &self,
        datum: &[u8],
    ) -> Result<Vec<(State, Vec<u8>)>, String> {
        let mut bits = Bits::reader(datum);
        let short = |_| format!("a datum of {} bytes ends early", datum.len());
        let mut states = Vec::with_capacity(self.inputs.len());
        for input in &self.inputs {
            let said = bits.take(STATE_BITS).map_err(short)? as u8;
            states.push(State::of(said).ok_or_else(|| {
                format!(
                    "input \"{}\" has the state 11, which no state is: the \
                     datum is damaged",
                    input.name
                )
            })?);
        }
        let mut readings = Vec::with_capacity(self.inputs.len());
        for (input, state) in self.inputs.iter().zip(&states) {
            let mut reading = Vec::new();
            match &input.shape {
                Shape::Fixed(fields) => {
                    for field in fields {
                        match field.0 {
                            Field::Boolean => {
                                reading.push(
                                    bits.take(BOOLEAN_BITS).map_err(short)?
                                        as u8,
                                );
                            }
                            Field::Bits(width) => {
                                for _ in 0..width / 8 {
                                    reading.push(
                                        bits.take(8).map_err(short)? as u8
                                    );
                                }
                            }
                        }
                    }
                }
                Shape::Counted { most, length_bits } => {
                    let length =
                        bits.take(*length_bits).map_err(short)? as usize;
                    if length > *most {
                        return Err(format!(
                            "input \"{}\" says {length} bytes, and it holds at \
                             most {most}",
                            input.name
                        ));
                    }
                    for _ in 0..length {
                        reading.push(bits.take(8).map_err(short)? as u8);
                    }
                }
            }
            if *state == State::None {
                reading.clear();
            }
            readings.push((*state, reading));
        }
        if bits.bytes() != datum.len() {
            return Err(format!(
                "a datum of {} bytes holds {} of a consolidation and more \
                 after it",
                datum.len(),
                bits.bytes()
            ));
        }
        Ok(readings)
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
    /// The datums it went into none: nothing has arrived from it yet.
    pub none: u64,
}

/// Bits written into, or read out of, a run of bytes, most significant bit
/// first.
struct Bits<'a> {
    /// The bytes written into; empty for a reader.
    out: &'a mut [u8],
    /// The bytes read; empty for a writer.
    from: &'a [u8],
    /// How many bits have been written or read.
    at: usize,
}

/// The run of bytes is too short for what is written or read.
#[derive(Debug)]
struct Short;

impl<'a> Bits<'a> {
    /// Writes into `out`, which is zeroed first.
    fn writer(out: &'a mut [u8]) -> Bits<'a> {
        out.fill(0);
        Bits {
            out,
            from: &[],
            at: 0,
        }
    }

    /// Reads from `from`.
    fn reader(from: &'a [u8]) -> Bits<'a> {
        Bits {
            out: &mut [],
            from,
            at: 0,
        }
    }

    /// Writes the low `width` bits of `value`, most significant first.
    fn put(&mut self, value: u64, width: usize) -> Result<(), Short> {
        if self.at + width > self.out.len() * 8 {
            return Err(Short);
        }
        for bit in (0..width).rev() {
            if value >> bit & 1 == 1 {
                self.out[self.at / 8] |= 0x80 >> (self.at % 8);
            }
            self.at += 1;
        }
        Ok(())
    }

    /// Writes `width` zero bits.
    fn zeros(&mut self, width: usize) -> Result<(), Short> {
        if self.at + width > self.out.len() * 8 {
            return Err(Short);
        }
        self.at += width;
        Ok(())
    }

    /// Reads `width` bits, at most 64, most significant first.
    fn take(&mut self, width: usize) -> Result<u64, Short> {
        if self.at + width > self.from.len() * 8 {
            return Err(Short);
        }
        let mut value = 0u64;
        for _ in 0..width {
            let bit = self.from[self.at / 8] >> (7 - self.at % 8) & 1;
            value = value << 1 | u64::from(bit);
            self.at += 1;
        }
        Ok(value)
    }

    /// How many whole bytes the bits written or read take, the last padded.
    fn bytes(&self) -> usize {
        self.at.div_ceil(8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A layout of these inputs, each of this type.
    fn layout(kinds: &[(&str, LinkType, Option<usize>)]) -> Layout {
        Layout {
            inputs: kinds
                .iter()
                .map(|(name, kind, most)| Input {
                    name: name.to_string(),
                    source: name.to_string(),
                    kind: kind.clone(),
                    shape: match kind {
                        LinkType::Basic(basic) => match Field::of(*basic) {
                            Some(field) => {
                                Shape::Fixed(vec![FixedField(field)])
                            }
                            None => Shape::Counted {
                                most: most.expect("a most"),
                                length_bits: length_bits(most.unwrap()),
                            },
                        },
                        LinkType::Structure(fields) => Shape::Fixed(
                            fields
                                .iter()
                                .map(|(_, b)| {
                                    FixedField(Field::of(*b).unwrap())
                                })
                                .collect(),
                        ),
                    },
                })
                .collect(),
        }
    }

    /// A boolean is as many bits in a consolidation as `types.xtce` says
    /// `boolean_bit` is, the one statement of it.
    #[test]
    fn a_booleans_bits_are_types_xtces() {
        let types = include_str!("../xtce/types.xtce");
        let at = types
            .find("<BooleanParameterType name=\"boolean_bit\">")
            .expect("types.xtce states boolean_bit");
        let encoding = &types[at..];
        let said = encoding
            .split("sizeInBits=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("its width");
        assert_eq!(said.parse::<usize>(), Ok(BOOLEAN_BITS));
    }

    /// A length field is just wide enough for its most.
    #[test]
    fn a_length_is_just_wide_enough() {
        assert_eq!(length_bits(64), 7);
        assert_eq!(length_bits(63), 6);
        assert_eq!(length_bits(4096), 13);
        assert_eq!(length_bits(1), 1);
        assert_eq!(length_bits(0), 1);
    }

    /// The states come first, two bits each, then each input's bits, end to
    /// end and most significant first: a boolean is one bit, and the datum
    /// is padded only at its end.
    #[test]
    fn a_datum_is_packed_bit_by_bit() {
        let laid = layout(&[
            ("flag", LinkType::Basic(Basic::Boolean), None),
            ("count", LinkType::Basic(Basic::U8), None),
            ("name", LinkType::Basic(Basic::String(Some(3))), Some(3)),
        ]);
        // 3 states, 6 bits; flag 1; count 8; name 2 + 16: 33 bits, 5 bytes.
        assert_eq!(laid.largest_bits(), 6 + 1 + 8 + 2 + 3 * 8);
        let mut out = [0xffu8; 8];
        let used = laid
            .pack(
                &[
                    (State::New, &[1]),
                    (State::Held, &[0xa5]),
                    (State::New, b"hi"),
                ],
                &mut out,
            )
            .expect("it packs");
        // 10 01 10 | 1 | 1010_0101 | 10 | 0110_1000 0110_1001, padded.
        let mut want = String::from("100110");
        want += "1";
        want += "10100101";
        want += "10";
        want += "0110100001101001";
        while want.len() % 8 != 0 {
            want.push('0');
        }
        let got: String = out[..used]
            .iter()
            .map(|byte| format!("{byte:08b}"))
            .collect();
        assert_eq!(got, want);

        let back = laid.unpack(&out[..used]).expect("it unpacks");
        assert_eq!(
            back,
            vec![
                (State::New, vec![1]),
                (State::Held, vec![0xa5]),
                (State::New, b"hi".to_vec()),
            ]
        );
    }

    /// An input that has never arrived is zeros of its width, or an empty
    /// string, and is read back as nothing; a datum of zeros is every input
    /// none.
    #[test]
    fn an_input_that_never_arrived_is_zeros() {
        let laid = layout(&[
            ("a", LinkType::Basic(Basic::I16), None),
            ("b", LinkType::Basic(Basic::Bytes(Some(10))), Some(10)),
        ]);
        let mut out = [0u8; 8];
        let used = laid
            .pack(&[(State::None, &[1, 2]), (State::None, b"xyz")], &mut out)
            .expect("it packs");
        assert!(out[..used].iter().all(|byte| *byte == 0));
        let back = laid.unpack(&out[..used]).expect("it unpacks");
        assert_eq!(back, vec![(State::None, vec![]), (State::None, vec![])]);
    }

    /// A boolean that is not 0 or 1, a reading of the wrong width and one
    /// longer than its most are not accepted, nor packed.
    #[test]
    fn a_reading_not_of_its_type_is_refused() {
        let laid = layout(&[
            ("flag", LinkType::Basic(Basic::Boolean), None),
            ("word", LinkType::Basic(Basic::U16), None),
            ("text", LinkType::Basic(Basic::String(Some(2))), Some(2)),
        ]);
        assert!(laid.accepts(0, &[1]));
        assert!(!laid.accepts(0, &[2]));
        assert!(!laid.accepts(1, &[1]));
        assert!(laid.accepts(2, b"ok"));
        assert!(!laid.accepts(2, b"too"));
        let mut out = [0u8; 8];
        assert_eq!(
            laid.pack(
                &[(State::New, &[2]), (State::New, &[0, 1]), (State::New, b"")],
                &mut out
            ),
            Err(0)
        );
    }

    /// A structure's booleans are one bit each among its other fields.
    #[test]
    fn a_structures_booleans_are_one_bit_each() {
        let laid = layout(&[
            (
                "status",
                LinkType::Structure(vec![
                    ("on".to_string(), Basic::Boolean),
                    ("level".to_string(), Basic::U8),
                    ("fault".to_string(), Basic::Boolean),
                ]),
                None,
            ),
            ("other", LinkType::Basic(Basic::Boolean), None),
        ]);
        assert_eq!(
            laid.fixed_bytes(),
            Some((4 + 1 + 8 + 1 + 1usize).div_ceil(8))
        );
        let mut out = [0u8; 4];
        let used = laid
            .pack(&[(State::New, &[1, 7, 0]), (State::Held, &[1])], &mut out)
            .expect("it packs");
        assert_eq!(
            laid.unpack(&out[..used]).expect("it unpacks"),
            vec![(State::New, vec![1, 7, 0]), (State::Held, vec![1])]
        );
    }

    /// A state of 11 is a damaged datum, and so is one with bytes past its
    /// end or one that ends early.
    #[test]
    fn a_damaged_datum_is_refused() {
        let laid = layout(&[
            ("a", LinkType::Basic(Basic::U8), None),
            ("b", LinkType::Basic(Basic::U8), None),
        ]);
        assert!(laid.unpack(&[0b1100_0000, 0, 0]).is_err());
        assert!(laid.unpack(&[0b1010_0000, 0, 0, 0]).is_err());
        assert!(laid.unpack(&[0b1010_0000]).is_err());
    }
}
