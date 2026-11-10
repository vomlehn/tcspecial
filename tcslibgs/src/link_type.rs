//! The type a link between two nodes carries.
//!
//! Every type a link can carry is stated once, in `tcslibgs/xtce/types.xtce`:
//! the whole numbers, `boolean`, one value type per unit, and bounded strings
//! and bytes. A terminal of a node carries one of them, or a structure of them
//! that its configuration writes, and a link is valid only where the types its
//! two ends state are the same. This is where a configuration's name for a
//! type is read and written; [`crate::graph::link_types`] says which type each
//! terminal of each node carries, and checks each link. See "Typed links" in
//! `docs/design.rst`.

use crate::unit::Unit;
use std::fmt;

/// The most fields a structure may have, so that a datum of one is of a size
/// known before anything runs.
pub const MOST_FIELDS: usize = 16;

/// One of the types `types.xtce` states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Basic {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    Boolean,
    /// A value in a unit: a 32-bit count of the unit's step.
    Value(Unit),
    /// Text, at most 4096 bytes of it.
    String,
    /// Opaque data, at most 4096 bytes of it.
    Bytes,
}

/// The whole numbers, with their names, in the order they are listed.
const WHOLE: [(Basic, &str); 8] = [
    (Basic::U8, "u8"),
    (Basic::U16, "u16"),
    (Basic::U32, "u32"),
    (Basic::U64, "u64"),
    (Basic::I8, "i8"),
    (Basic::I16, "i16"),
    (Basic::I32, "i32"),
    (Basic::I64, "i64"),
];

impl Basic {
    /// Reads a type's name as a configuration writes it.
    ///
    /// # Arguments
    ///
    /// * `said` - The name: `u8`, `boolean`, `kelvin`, `string`.
    ///
    /// # Returns
    ///
    /// The type, or `None` for a name that is no type's.
    pub fn of(said: &str) -> Option<Basic> {
        match said {
            "boolean" => Some(Basic::Boolean),
            "string" => Some(Basic::String),
            "bytes" => Some(Basic::Bytes),
            _ => WHOLE
                .iter()
                .find(|(_, name)| *name == said)
                .map(|(basic, _)| *basic)
                .or_else(|| Unit::of_type_name(said).map(Basic::Value)),
        }
    }

    /// Names this type as a configuration writes it.
    pub fn name(self) -> &'static str {
        match self {
            Basic::Boolean => "boolean",
            Basic::String => "string",
            Basic::Bytes => "bytes",
            Basic::Value(unit) => unit.type_name(),
            whole => {
                WHOLE
                    .iter()
                    .find(|(basic, _)| *basic == whole)
                    .expect("every whole number is listed")
                    .1
            }
        }
    }

    /// Says whether this is a whole number.
    pub fn is_whole(self) -> bool {
        WHOLE.iter().any(|(basic, _)| *basic == self)
    }

    /// Says how many bits a whole number of this type holds.
    ///
    /// # Returns
    ///
    /// The bits, or `None` for a type that is not a whole number.
    pub fn bits(self) -> Option<u32> {
        match self {
            Basic::U8 | Basic::I8 => Some(8),
            Basic::U16 | Basic::I16 => Some(16),
            Basic::U32 | Basic::I32 => Some(32),
            Basic::U64 | Basic::I64 => Some(64),
            _ => None,
        }
    }

    /// The type of a set of GPIO lines: the narrowest unsigned whole number
    /// with a bit for each.
    ///
    /// # Arguments
    ///
    /// * `count` - How many lines, one to 64.
    ///
    /// # Returns
    ///
    /// `u8`, `u16`, `u32` or `u64`.
    pub fn lines(count: usize) -> Basic {
        match count {
            0..=8 => Basic::U8,
            9..=16 => Basic::U16,
            17..=32 => Basic::U32,
            _ => Basic::U64,
        }
    }

    /// Lists every type's name, for a complaint.
    pub fn every_name() -> String {
        let mut names: Vec<&str> =
            WHOLE.iter().map(|(_, name)| *name).collect();
        names.push("boolean");
        names.extend(Unit::ALL.iter().map(|unit| unit.type_name()));
        names.push("string");
        names.push("bytes");
        names.join(", ")
    }
}

/// The type a terminal carries: one of `types.xtce`'s, or a structure of
/// them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LinkType {
    Basic(Basic),
    /// Named fields, in the order they are written, at most [`MOST_FIELDS`]
    /// of them.
    Structure(Vec<(String, Basic)>),
}

impl LinkType {
    /// Reads a type as a configuration writes it: a type's name, or a
    /// structure's fields, `x: i16, y: i16, z: i16`.
    ///
    /// # Arguments
    ///
    /// * `said` - What the configuration wrote.
    ///
    /// # Returns
    ///
    /// The type.
    ///
    /// # Errors
    ///
    /// A message for a name that is no type's, a field with no name or a
    /// name used twice, and a structure of more than [`MOST_FIELDS`].
    pub fn of(said: &str) -> Result<LinkType, String> {
        let said = said.trim();
        if !said.contains(':') {
            return Basic::of(said).map(LinkType::Basic).ok_or_else(|| {
                format!(
                    "\"{said}\" is not a type. The types are {}, or a \
                     structure of them written as name: type, name: type",
                    Basic::every_name()
                )
            });
        }
        let mut fields: Vec<(String, Basic)> = Vec::new();
        for field in said.split(',') {
            let Some((name, kind)) = field.split_once(':') else {
                return Err(format!(
                    "\"{}\" is not a field of a structure: write it as \
                     name: type",
                    field.trim()
                ));
            };
            let (name, kind) = (name.trim(), kind.trim());
            if name.is_empty()
                || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                return Err(format!(
                    "\"{name}\" is not a field's name: a name is letters, \
                     digits and underscores"
                ));
            }
            if fields.iter().any(|(seen, _)| seen == name) {
                return Err(format!(
                    "the structure names \"{name}\" twice: each field has a \
                     name of its own"
                ));
            }
            let Some(basic) = Basic::of(kind) else {
                return Err(format!(
                    "field \"{name}\" is of \"{kind}\", which is not a type. \
                     The types are {}",
                    Basic::every_name()
                ));
            };
            fields.push((name.to_string(), basic));
        }
        if fields.len() > MOST_FIELDS {
            return Err(format!(
                "the structure has {} fields, and one may have at most \
                 {MOST_FIELDS}",
                fields.len()
            ));
        }
        Ok(LinkType::Structure(fields))
    }

    /// Says what unit a datum of this type is in.
    ///
    /// # Returns
    ///
    /// The unit, for a value type; `None` for anything else.
    pub fn unit(&self) -> Option<Unit> {
        match self {
            LinkType::Basic(Basic::Value(unit)) => Some(*unit),
            _ => None,
        }
    }
}

impl fmt::Display for LinkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkType::Basic(basic) => f.write_str(basic.name()),
            LinkType::Structure(fields) => {
                let said: Vec<String> = fields
                    .iter()
                    .map(|(name, basic)| format!("{name}: {}", basic.name()))
                    .collect();
                f.write_str(&said.join(", "))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every type's name reads back as that type.
    #[test]
    fn every_name_reads_back() {
        for name in Basic::every_name().split(", ") {
            let basic = Basic::of(name).expect(name);
            assert_eq!(basic.name(), name);
        }
        assert_eq!(Basic::of("kelvin"), Some(Basic::Value(Unit::Kelvin)));
        assert_eq!(Basic::of("K"), None);
    }

    /// A structure is read field by field, and said back the same.
    #[test]
    fn a_structure_reads_and_says_itself() {
        let imu = LinkType::of("x: i16, y: i16,z:i16").expect("a structure");
        assert_eq!(imu.to_string(), "x: i16, y: i16, z: i16");
        for bad in ["x: furlong", "x: i16, x: u8", ": u8", "x y: u8"] {
            LinkType::of(bad).expect_err(bad);
        }
        let many: Vec<String> =
            (0..=MOST_FIELDS).map(|n| format!("f{n}: u8")).collect();
        LinkType::of(&many.join(", ")).expect_err("seventeen fields");
    }

    /// A set of lines is the narrowest whole number with room for them.
    #[test]
    fn lines_take_the_narrowest_number() {
        assert_eq!(Basic::lines(1), Basic::U8);
        assert_eq!(Basic::lines(8), Basic::U8);
        assert_eq!(Basic::lines(9), Basic::U16);
        assert_eq!(Basic::lines(33), Basic::U64);
    }
}
