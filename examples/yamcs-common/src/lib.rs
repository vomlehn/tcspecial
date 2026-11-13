//! What the tests of the examples through Yamcs share, ping-yamcs's and
//! thermostat-yamcs's: the mission database's wrapper `tcslibgs/yamcs.py`
//! generates for a configuration, read as XML, and worked as Yamcs would
//! work it on the packets the vehicle frames with `tcslib_protocol::ccsds`.
//!
//! * [`generated_mdb`] has the Python generate the wrapper, as the ground
//!   programs write it for Yamcs, and [`read_xml`] reads it;
//! * [`decode`] decodes a packet from the link by it, from its root
//!   container down through whichever container's restrictions hold, every
//!   parameter as its type says -- an integer, a string or bytes, a value
//!   calibrated into its unit, a decision as a boolean -- and [`encode`]
//!   encodes a command by it, as Yamcs fills it in;
//! * [`framed`] frames data as the vehicle does.
//!
//! The interpreter does what the wrapper uses of XTCE and no more: a wrapper
//! that grows past it fails here, saying what it does not know, rather than
//! passing unread.
//!
//! Not an example, though it is under `examples/`: it holds no
//! configuration, and nothing runs it but the examples' tests.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use quick_xml::escape::unescape;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use tcslib_protocol::ccsds::{PacketType, Sender, HEADER_BYTES};

/// One element of an XTCE document, with what is in it.
#[derive(Debug, Default)]
pub struct Element {
    /// Its name, as written, with any prefix.
    pub name: String,
    /// Its attributes, in order.
    pub attributes: Vec<(String, String)>,
    /// The elements in it, in order.
    pub children: Vec<Element>,
    /// Its text.
    pub text: String,
}

impl Element {
    /// One of its attributes.
    ///
    /// # Arguments
    ///
    /// * `name` - The attribute's name.
    ///
    /// # Returns
    ///
    /// Its value, or `None` where it has none of that name.
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(said, _)| said == name)
            .map(|(_, value)| value.as_str())
    }

    /// Its first child of a kind.
    ///
    /// # Arguments
    ///
    /// * `name` - The child's element name.
    ///
    /// # Returns
    ///
    /// The child, or `None`.
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|child| child.name == name)
    }

    /// Everything of a kind within it, at any depth, in document order.
    ///
    /// # Arguments
    ///
    /// * `name`  - The element name.
    /// * `found` - Where they are put.
    pub fn within<'a>(&'a self, name: &str, found: &mut Vec<&'a Element>) {
        for child in &self.children {
            if child.name == name {
                found.push(child);
            }
            child.within(name, found);
        }
    }

    /// The one element of a kind within it with a `name` attribute.
    ///
    /// # Arguments
    ///
    /// * `kind` - The element name.
    /// * `name` - Its `name` attribute.
    ///
    /// # Returns
    ///
    /// The element.
    ///
    /// # Panics
    ///
    /// There is none.
    pub fn named(&self, kind: &str, name: &str) -> &Element {
        let mut found = Vec::new();
        self.within(kind, &mut found);
        found
            .into_iter()
            .find(|element| element.attribute("name") == Some(name))
            .unwrap_or_else(|| panic!("the document has no {kind} {name}"))
    }
}

/// An element's name and attributes, as an [`Element`] with nothing in it.
///
/// # Arguments
///
/// * `start` - The element's start tag.
///
/// # Returns
///
/// The element, with no children or text.
fn opened(start: &BytesStart<'_>) -> Element {
    let text = |said: &str| unescape(said).expect("well escaped").into_owned();
    Element {
        name: text(start.name().as_ref()),
        attributes: start
            .attributes()
            .map(|attribute| {
                let attribute = attribute.expect("an attribute");
                (text(attribute.key.as_ref()), text(&attribute.value))
            })
            .collect(),
        ..Element::default()
    }
}

/// Reads an XTCE document.
///
/// # Arguments
///
/// * `text` - The document.
///
/// # Returns
///
/// Its root element.
///
/// # Panics
///
/// It is not XML.
pub fn read_xml(text: &str) -> Element {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut open = vec![Element::default()];
    loop {
        match reader.read_event().expect("the document is XML") {
            Event::Start(start) => open.push(opened(&start)),
            Event::Empty(start) => {
                let element = opened(&start);
                open.last_mut().expect("a parent").children.push(element);
            }
            Event::End(_) => {
                let element = open.pop().expect("an element");
                open.last_mut().expect("a parent").children.push(element);
            }
            Event::Text(said) => {
                let said: &str = said.as_ref();
                open.last_mut().expect("an element").text.push_str(said);
            }
            // An entity in text, `&lt;` in a comparison, comes apart from
            // the text around it.
            Event::GeneralRef(entity) => {
                let entity: &str = entity.as_ref();
                let said = unescape(&format!("&{entity};"))
                    .expect("an entity XML defines")
                    .into_owned();
                open.last_mut().expect("an element").text.push_str(&said);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut document = open.pop().expect("the document");
    assert!(open.is_empty(), "every element closed");
    document.children.pop().expect("a root element")
}

/// The tree's `python/` directory, which the Python is run with.
///
/// # Returns
///
/// Its path.
pub fn python_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../python")
}

/// Runs Python with the tree's `python/` and the given directories on its
/// path.
///
/// # Arguments
///
/// * `script`    - The program, which `sys.argv[1:]` gives `arguments` to.
/// * `path`      - Directories to put on its path after `python/`: an
///                 example's, whose own modules it imports.
/// * `arguments` - Its arguments.
///
/// # Returns
///
/// What it printed.
///
/// # Panics
///
/// python3 cannot be run, or the program fails.
pub fn python(script: &str, path: &[&Path], arguments: &[&Path]) -> String {
    let mut directories = vec![python_dir().display().to_string()];
    directories.extend(path.iter().map(|dir| dir.display().to_string()));
    let ran = Command::new("python3")
        .arg("-c")
        .arg(format!(
            "import sys\n\
             sys.path[:0] = {directories:?}\n\
             {script}"
        ))
        .args(arguments)
        .output()
        .expect("python3 runs");
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    String::from_utf8(ran.stdout).expect("text")
}

/// The mission database's wrapper `tcslibgs/yamcs.py` generates for a
/// configuration, as the ground programs write it for Yamcs.
///
/// # Arguments
///
/// * `configuration` - The configuration.
///
/// # Returns
///
/// The wrapper's root element.
///
/// # Panics
///
/// The Python fails, or what it writes is not XML.
pub fn generated_mdb(configuration: &Path) -> Element {
    let dir = tempfile::tempdir().expect("a directory");
    let written = dir.path().join("tcs-ccsds.xtce");
    python(
        "from pathlib import Path\n\
         from tcslibgs import configuration, yamcs\n\
         yamcs.write_mdb(configuration.load(sys.argv[1]), Path(sys.argv[2]))\n",
        &[],
        &[configuration, &written],
    );
    read_xml(&std::fs::read_to_string(&written).expect("it was written"))
}

/// `bits` bits of `packet` from bit `at`, most significant first.
///
/// # Arguments
///
/// * `packet` - The packet.
/// * `at`     - The first bit, counting from nought.
/// * `bits`   - How many.
///
/// # Returns
///
/// Their value.
fn bits(packet: &[u8], at: usize, bits: usize) -> u64 {
    assert!(at + bits <= packet.len() * 8, "{bits} bits at {at} overrun");
    (at..at + bits).fold(0, |value, bit| {
        let set = packet[bit / 8] >> (7 - bit % 8) & 1;
        value << 1 | u64::from(set)
    })
}

/// The bits a parameter's or argument's encoding takes, given the values
/// already decoded that a dynamic size may name.
///
/// # Arguments
///
/// * `kind`   - Its type.
/// * `values` - The integers known so far, by name.
///
/// # Returns
///
/// How many bits it takes.
///
/// # Panics
///
/// The type is not one of those the wrapper uses.
fn width(kind: &Element, values: &BTreeMap<String, u64>) -> usize {
    if let Some(encoding) = kind.child("IntegerDataEncoding") {
        return encoding
            .attribute("sizeInBits")
            .expect("an integer's size")
            .parse()
            .expect("a number");
    }
    let mut fixed = Vec::new();
    kind.within("FixedValue", &mut fixed);
    if let [fixed] = fixed.as_slice() {
        return fixed.text.trim().parse().expect("a number");
    }
    let mut dynamic = Vec::new();
    kind.within("DynamicValue", &mut dynamic);
    let [dynamic] = dynamic.as_slice() else {
        panic!("{} has no size this test reads", kind.name);
    };
    let reference = dynamic
        .child("ParameterInstanceRef")
        .and_then(|r| r.attribute("parameterRef"))
        .or_else(|| {
            dynamic
                .child("ArgumentInstanceRef")
                .and_then(|r| r.attribute("argumentRef"))
        })
        .expect("what the size is read from");
    let value = values[reference] as i64;
    let (slope, intercept) =
        dynamic
            .child("LinearAdjustment")
            .map_or((1, 0), |adjustment| {
                let number = |name| {
                    adjustment
                        .attribute(name)
                        .unwrap_or("0")
                        .parse::<i64>()
                        .expect("a whole number")
                };
                (number("slope"), number("intercept"))
            });
    (slope * value + intercept) as usize
}

/// Where an entry starts: where it says, from the start of the packet, or
/// where the entry before it ended.
///
/// # Arguments
///
/// * `entry` - The entry.
/// * `after` - Where the entry before it ended, or `None` where that is
///             not known.
///
/// # Returns
///
/// The bit it starts at.
fn start_of(entry: &Element, after: Option<usize>) -> usize {
    match entry.child("LocationInContainerInBits") {
        Some(location) => {
            assert_eq!(
                location.attribute("referenceLocation"),
                Some("containerStart"),
                "a location this test reads"
            );
            location
                .child("FixedValue")
                .expect("a fixed location")
                .text
                .trim()
                .parse()
                .expect("a number")
        }
        None => after.expect("an entry after a container placed nowhere"),
    }
}

/// A string's termination character, where its type ends it by one rather
/// than by a fixed size.
///
/// # Arguments
///
/// * `kind` - The string's type.
///
/// # Returns
///
/// The character's byte and the most bits the string takes, the character
/// counted; `None` for a string of a fixed size.
///
/// # Panics
///
/// The type is not one this test reads: a character of more than a byte.
fn terminated(kind: &Element) -> Option<(u8, usize)> {
    let variable = kind.child("StringDataEncoding")?.child("Variable")?;
    let most = variable
        .attribute("maxSizeInBits")
        .expect("a string's largest size")
        .parse()
        .expect("a number");
    let character = variable
        .child("TerminationChar")
        .expect("what ends the string")
        .text
        .trim();
    assert_eq!(character.len(), 2, "a one-byte termination character");
    let byte = u8::from_str_radix(character, 16).expect("hex");
    Some((byte, most))
}

/// Reads a string that a character ends: its bytes, up to the character.
///
/// # Arguments
///
/// * `packet` - The packet.
/// * `at`     - The bit it starts at, on an octet.
/// * `ended`  - Its termination character and the most bits it takes.
///
/// # Returns
///
/// The string's bytes, the character not among them, and the bits it
/// took, the character among them.
///
/// # Panics
///
/// The character is not there before the string's largest size, or the
/// packet's end.
fn read_terminated(
    packet: &[u8],
    at: usize,
    ended: (u8, usize),
) -> (Vec<u8>, usize) {
    let (character, most) = ended;
    assert!(at.is_multiple_of(8), "a string on an octet");
    let start = at / 8;
    let room = &packet[start..packet.len().min(start + most / 8)];
    let end = room
        .iter()
        .position(|&b| b == character)
        .unwrap_or_else(|| panic!("no {character:#04x} ends the string"));
    (room[..end].to_vec(), 8 * (end + 1))
}

/// What a packet decoded to.
#[derive(Debug, Default)]
pub struct Decoded {
    /// The container it ended in: the most derived whose restrictions held.
    pub container: String,
    /// Each integer parameter.
    pub integers: BTreeMap<String, u64>,
    /// Each string or binary parameter, as its bytes.
    pub bytes: BTreeMap<String, Vec<u8>>,
    /// Each float parameter, its raw value calibrated, with its units.
    pub floats: BTreeMap<String, (f64, Vec<String>)>,
    /// Each boolean parameter: its value, or, for what its type's strings
    /// do not name, the string that arrived, which Yamcs cannot read.
    pub booleans: BTreeMap<String, Result<bool, Vec<u8>>>,
    /// Each container of another document it refers to, and the bit it
    /// starts at.
    pub refers: Vec<(String, usize)>,
    /// Where its last entry ended, where that is known.
    pub end: Option<usize>,
}

/// Reads a float parameter from an integer encoding and calibrates it, as
/// Yamcs reads one: the raw integer, two's complement where its encoding
/// says, most significant byte first, through the polynomial of its
/// default calibrator.
///
/// # Arguments
///
/// * `kind` - Its `FloatParameterType`.
/// * `raw`  - The encoding's bits, as read.
/// * `size` - How many bits they are.
///
/// # Returns
///
/// The engineering value, and the type's units.
///
/// # Panics
///
/// The type is not one this reads: other than an integer encoding, most
/// significant byte first, with a polynomial calibrator or none.
fn calibrated(kind: &Element, raw: u64, size: usize) -> (f64, Vec<String>) {
    let encoding = kind
        .child("IntegerDataEncoding")
        .expect("a float read from an integer encoding");
    assert_eq!(
        encoding
            .attribute("byteOrder")
            .unwrap_or("mostSignificantByteFirst"),
        "mostSignificantByteFirst",
        "a byte order this reads"
    );
    let signed = match encoding.attribute("encoding").unwrap_or("unsigned") {
        "unsigned" => false,
        "twosComplement" => true,
        other => panic!("an integer encoding this does not read: {other}"),
    };
    let raw = if signed && size < 64 && raw >> (size - 1) & 1 == 1 {
        raw as i64 - (1i64 << size)
    } else {
        raw as i64
    } as f64;
    let value = match encoding
        .child("DefaultCalibrator")
        .map(|c| c.child("PolynomialCalibrator").expect("a polynomial"))
    {
        None => raw,
        Some(polynomial) => polynomial
            .children
            .iter()
            .map(|term| {
                let number = |name| {
                    term.attribute(name)
                        .expect("a term's coefficient and exponent")
                        .parse::<f64>()
                        .expect("a number")
                };
                number("coefficient") * raw.powf(number("exponent"))
            })
            .sum(),
    };
    let units = kind.child("UnitSet").map_or_else(Vec::new, |set| {
        set.children.iter().map(|unit| unit.text.clone()).collect()
    });
    (value, units)
}

/// Reads a boolean parameter from a string encoding of a fixed size, as
/// Yamcs reads one: the string against the type's strings for true and
/// false.
///
/// # Arguments
///
/// * `kind` - Its `BooleanParameterType`.
/// * `said` - The string's bytes.
///
/// # Returns
///
/// The value.
///
/// # Errors
///
/// The bytes, where they are neither string: what Yamcs cannot read.
///
/// # Panics
///
/// The type is not one this reads: other than a string encoding.
fn boolean(kind: &Element, said: &[u8]) -> Result<bool, Vec<u8>> {
    assert!(
        kind.child("StringDataEncoding").is_some(),
        "a boolean read from a string"
    );
    let one = kind.attribute("oneStringValue").unwrap_or("True");
    let zero = kind.attribute("zeroStringValue").unwrap_or("False");
    match said {
        _ if said == one.as_bytes() => Ok(true),
        _ if said == zero.as_bytes() => Ok(false),
        _ => Err(said.to_vec()),
    }
}

/// Whether a comparison holds.
///
/// # Arguments
///
/// * `left`     - The value compared.
/// * `operator` - The comparison, as XTCE writes it.
/// * `right`    - What it is compared with.
///
/// # Returns
///
/// Whether it holds.
fn holds(left: u64, operator: &str, right: u64) -> bool {
    match operator {
        "==" => left == right,
        "!=" => left != right,
        "<" => left < right,
        "<=" => left <= right,
        ">" => left > right,
        ">=" => left >= right,
        other => panic!("a comparison this test does not know: {other}"),
    }
}

/// Whether a container's restrictions hold of what has been decoded.
///
/// # Arguments
///
/// * `criteria` - Its `RestrictionCriteria`.
/// * `values`   - The integers decoded so far.
///
/// # Returns
///
/// Whether they hold.
fn restricted(criteria: &Element, values: &BTreeMap<String, u64>) -> bool {
    let compare = |comparison: &Element| {
        let name = comparison.attribute("parameterRef").expect("a parameter");
        let value = comparison
            .attribute("value")
            .expect("a value")
            .parse()
            .expect("a number");
        let operator =
            comparison.attribute("comparisonOperator").unwrap_or("==");
        holds(values[name], operator, value)
    };
    let condition = |condition: &Element| {
        let name = condition
            .child("ParameterInstanceRef")
            .and_then(|r| r.attribute("parameterRef"))
            .expect("a parameter");
        let operator = condition
            .child("ComparisonOperator")
            .expect("an operator")
            .text
            .trim();
        let value = condition
            .child("Value")
            .expect("a value")
            .text
            .trim()
            .parse()
            .expect("a number");
        holds(values[name], operator, value)
    };
    let [kind] = criteria.children.as_slice() else {
        panic!("one restriction at a time");
    };
    match kind.name.as_str() {
        "Comparison" => compare(kind),
        "ComparisonList" => kind.children.iter().all(compare),
        "BooleanExpression" => {
            let ored = kind.child("ORedConditions").expect("conditions");
            ored.children.iter().any(condition)
        }
        other => panic!("a restriction this test does not know: {other}"),
    }
}

/// Decodes a packet from the link as Yamcs would by the wrapper: from its
/// root container down through whichever container's restrictions hold.
///
/// # Arguments
///
/// * `xtce`   - The wrapper.
/// * `packet` - The packet as the vehicle sent it.
///
/// # Returns
///
/// What it decoded to.
///
/// # Panics
///
/// Two containers' restrictions both hold, or the wrapper uses what this
/// does not read.
pub fn decode(xtce: &Element, packet: &[u8]) -> Decoded {
    let mut containers = Vec::new();
    xtce.within("SequenceContainer", &mut containers);
    let mut decoded = Decoded {
        container: "CCSDSPacket".to_string(),
        ..Decoded::default()
    };
    let mut after = Some(0);
    loop {
        let container = xtce.named("SequenceContainer", &decoded.container);
        for entry in &container.child("EntryList").expect("entries").children {
            let at = start_of(entry, after);
            match entry.name.as_str() {
                "ParameterRefEntry" => {
                    let name =
                        entry.attribute("parameterRef").expect("a parameter");
                    let parameter = xtce.named("Parameter", name);
                    let kind_name = parameter
                        .attribute("parameterTypeRef")
                        .expect("a type");
                    let mut kinds = Vec::new();
                    xtce.within("ParameterTypeSet", &mut kinds);
                    let kind = kinds[0]
                        .children
                        .iter()
                        .find(|k| k.attribute("name") == Some(kind_name))
                        .expect("its type");
                    if let Some(ended) = terminated(kind) {
                        let (said, size) = read_terminated(packet, at, ended);
                        decoded.bytes.insert(name.to_string(), said);
                        after = Some(at + size);
                        continue;
                    }
                    let size = width(kind, &decoded.integers);
                    if kind.name == "IntegerParameterType" {
                        decoded
                            .integers
                            .insert(name.to_string(), bits(packet, at, size));
                    } else if kind.name == "FloatParameterType" {
                        let raw = bits(packet, at, size);
                        decoded.floats.insert(
                            name.to_string(),
                            calibrated(kind, raw, size),
                        );
                    } else if kind.name == "BooleanParameterType"
                        && kind.child("IntegerDataEncoding").is_some()
                    {
                        // An integer read as a boolean, as Yamcs reads one:
                        // nought is false and anything else true.
                        let raw = bits(packet, at, size);
                        decoded.booleans.insert(name.to_string(), Ok(raw != 0));
                    } else if kind.name == "BooleanParameterType" {
                        assert!(
                            at.is_multiple_of(8) && size.is_multiple_of(8),
                            "{name}: whole octets"
                        );
                        let said = &packet[at / 8..(at + size) / 8];
                        decoded
                            .booleans
                            .insert(name.to_string(), boolean(kind, said));
                    } else {
                        assert!(
                            at.is_multiple_of(8) && size.is_multiple_of(8),
                            "{name}: whole octets"
                        );
                        assert!(
                            (at + size) / 8 <= packet.len(),
                            "{name} runs past the packet"
                        );
                        decoded.bytes.insert(
                            name.to_string(),
                            packet[at / 8..(at + size) / 8].to_vec(),
                        );
                    }
                    after = Some(at + size);
                }
                "ContainerRefEntry" => {
                    let name =
                        entry.attribute("containerRef").expect("a container");
                    decoded.refers.push((name.to_string(), at));
                    // How long another document's container is, this does
                    // not read; whatever follows says where it is.
                    after = None;
                }
                other => panic!("an entry this test does not know: {other}"),
            }
        }
        let mut next = containers.iter().filter(|candidate| {
            candidate.child("BaseContainer").is_some_and(|base| {
                base.attribute("containerRef")
                    == Some(decoded.container.as_str())
                    && base
                        .child("RestrictionCriteria")
                        .is_some_and(|c| restricted(c, &decoded.integers))
            })
        });
        let Some(derived) = next.next() else {
            decoded.end = after;
            return decoded;
        };
        let name = derived.attribute("name").expect("a name").to_string();
        if let Some(also) = next.next() {
            panic!(
                "{name} and {} both match one packet",
                also.attribute("name").unwrap_or("?")
            );
        }
        decoded.container = name;
    }
}

/// What an argument is given.
pub enum Given<'a> {
    /// An integer.
    Integer(u64),
    /// Octets, as many as its type's size.
    Bytes(&'a [u8]),
    /// A string, which its type ends with a character or pads to its size.
    Text(&'a str),
}

/// Appends bytes to bits, most significant first.
///
/// # Arguments
///
/// * `bits`  - Where they go.
/// * `bytes` - The bytes.
fn push_bytes(bits: &mut Vec<bool>, bytes: &[u8]) {
    for byte in bytes {
        bits.extend((0..8).rev().map(|b| byte >> b & 1 == 1));
    }
}

/// Encodes a command as Yamcs would by the wrapper: its base command's
/// container, then its own, each argument as given or assigned or by its
/// initial value; and then the length field filled in, as Yamcs's command
/// postprocessor fills it in.
///
/// # Arguments
///
/// * `xtce`    - The wrapper.
/// * `command` - The command's name.
/// * `given`   - Its arguments, by name.
///
/// # Returns
///
/// The packet as it goes on the link.
pub fn encode(
    xtce: &Element,
    command: &str,
    given: &[(&str, Given)],
) -> Vec<u8> {
    // The command, and the commands it is built on, base first.
    let mut chain = vec![xtce.named("MetaCommand", command)];
    while let Some(base) = chain
        .last()
        .and_then(|c| c.child("BaseMetaCommand"))
        .and_then(|b| b.attribute("metaCommandRef"))
    {
        chain.push(xtce.named("MetaCommand", base));
    }
    chain.reverse();

    // Every argument's value, and its type.
    let mut integers: BTreeMap<String, u64> = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    for meta in &chain {
        for argument in meta
            .child("ArgumentList")
            .map_or(&[][..], |list| list.children.as_slice())
        {
            let name = argument.attribute("name").expect("a name");
            kinds.insert(
                name.to_string(),
                argument.attribute("argumentTypeRef").expect("a type"),
            );
            if let Some(initial) = argument.attribute("initialValue") {
                integers.insert(name.to_string(), initial.parse().unwrap());
            }
        }
        if let Some(assigned) = meta
            .child("BaseMetaCommand")
            .and_then(|b| b.child("ArgumentAssignmentList"))
        {
            for assignment in &assigned.children {
                let name = assignment.attribute("argumentName").unwrap();
                let value = assignment.attribute("argumentValue").unwrap();
                integers.insert(name.to_string(), value.parse().unwrap());
            }
        }
    }
    let mut bytes = BTreeMap::new();
    let mut texts = BTreeMap::new();
    for (name, value) in given {
        match value {
            Given::Integer(value) => {
                integers.insert(name.to_string(), *value);
            }
            Given::Bytes(value) => {
                bytes.insert(name.to_string(), *value);
            }
            Given::Text(value) => {
                texts.insert(name.to_string(), *value);
            }
        }
    }

    let mut types = Vec::new();
    xtce.within("ArgumentTypeSet", &mut types);
    let mut bits: Vec<bool> = Vec::new();
    for meta in &chain {
        let container = meta.child("CommandContainer").expect("a container");
        for entry in &container.child("EntryList").expect("entries").children {
            if entry.name == "FixedValueEntry" {
                let size: usize = entry
                    .attribute("sizeInBits")
                    .expect("a size")
                    .parse()
                    .expect("a number");
                let value = entry.attribute("binaryValue").expect("a value");
                assert_eq!(value.len() * 4, size, "a fixed value's size");
                let value: Vec<u8> = (0..value.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&value[i..i + 2], 16))
                    .collect::<Result<_, _>>()
                    .expect("hex");
                push_bytes(&mut bits, &value);
                continue;
            }
            assert_eq!(entry.name, "ArgumentRefEntry", "an entry this reads");
            let name = entry.attribute("argumentRef").expect("an argument");
            let kind = types[0]
                .children
                .iter()
                .find(|k| k.attribute("name") == Some(kinds[name]))
                .expect("its type");
            if let Some(text) = texts.get(name) {
                assert_eq!(kind.name, "StringArgumentType", "{name}: text");
                let mut said = text.as_bytes().to_vec();
                match terminated(kind) {
                    Some((character, most)) => {
                        assert!(!said.contains(&character), "{name} ends");
                        said.push(character);
                        assert!(said.len() * 8 <= most, "{name} fits");
                    }
                    None => {
                        let size = width(kind, &integers);
                        assert!(said.len() * 8 <= size, "{name} fits");
                        said.resize(size / 8, 0);
                    }
                }
                push_bytes(&mut bits, &said);
                continue;
            }
            let size = width(kind, &integers);
            match (integers.get(name), bytes.get(name)) {
                (Some(&value), _) if kind.name.starts_with("Integer") => {
                    assert!(size == 64 || value < 1 << size, "{name} fits");
                    bits.extend((0..size).rev().map(|b| value >> b & 1 == 1));
                }
                (_, Some(value)) => {
                    assert_eq!(value.len() * 8, size, "{name}'s size");
                    push_bytes(&mut bits, value);
                }
                _ => panic!("{name} is given no value"),
            }
        }
    }
    assert_eq!(bits.len() % 8, 0, "{command} is whole octets");
    let mut packet: Vec<u8> = bits
        .chunks(8)
        .map(|byte| byte.iter().fold(0, |b, &bit| b << 1 | u8::from(bit)))
        .collect();
    // The length field: what follows the header, less one.
    let length = (packet.len() - HEADER_BYTES - 1) as u16;
    packet[4..6].copy_from_slice(&length.to_be_bytes());
    packet
}

/// Frames a packet's data as the vehicle does, under an APID.
///
/// # Arguments
///
/// * `apid` - The APID.
/// * `data` - What follows the header.
///
/// # Returns
///
/// The packet.
pub fn framed(apid: u16, data: &[u8]) -> Vec<u8> {
    let mut sender =
        Sender::new(PacketType::Telemetry, apid).expect("a sender");
    let mut room = vec![0u8; HEADER_BYTES + data.len()];
    sender.frame(&mut room, data).expect("framed").to_vec()
}

/// A string or bytes as the vehicle sends it under Yamcs, before it is
/// framed: after the 32-bit count of its bytes, as XTCE encodes it.
///
/// # Arguments
///
/// * `data` - The datum.
///
/// # Returns
///
/// The count and the datum.
pub fn counted(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    tcslib_protocol::link_type::counted(
        data,
        tcslib_protocol::link_type::Count::Xtce,
        &mut out,
    )
    .expect("counted");
    out
}
