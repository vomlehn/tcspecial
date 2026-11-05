//! A component's terminals: the places it is joined to other components and to
//! the simulator's stand-ins.
//!
//! Each component type has a fixed list of them, stated in its module and
//! nowhere else. A terminal has a direction, an input or an output, and a
//! kind: a value in one unit, or a decision, on or off. A connection joins an
//! output to an input of the same kind, so a heater's heat, in watts, can
//! warm a zone, and cannot be read as its temperature.
//!
//! A terminal is written in a file as the component's name and its own,
//! joined by a dot: `zone.temperature`, `heater_1.on`.

use crate::unit::Unit;

/// Which way a terminal faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// The component is told something here: what it is driven to, or what
    /// another component gives it.
    In,
    /// The component says something here: what it is, or what it gives.
    Out,
}

/// What a terminal carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A number in a unit, which is what a sensor reports and a
    /// conversion node reads back.
    Value(Unit),
    /// On or off, which is what a thermostat node sends to drive a set of
    /// discrete lines.
    Decision,
}

impl Kind {
    /// Says what it is, as a sentence names it.
    ///
    /// # Returns
    ///
    /// Its unit's name, `kelvin`, or `a decision`.
    pub fn describe(self) -> &'static str {
        match self {
            Kind::Value(unit) => unit.name(),
            Kind::Decision => "a decision",
        }
    }
}

/// One terminal of a component type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Terminal {
    /// Its name, which a file writes after the component's and a dot.
    pub name: &'static str,
    pub direction: Direction,
    pub kind: Kind,
    /// For an input: whether it takes the sum of every output connected to
    /// it, as heat does -- two heaters on one zone warm it with both -- or
    /// one output at most.
    pub sums: bool,
    /// For an input: whether it must be connected or driven. A heater's
    /// `on` must, or the heater is one nothing can switch; a zone's `heat`
    /// need not, a zone nothing warms being a zone.
    pub required: bool,
}

impl Terminal {
    /// An input of this kind, taking one output at most, connected or not.
    ///
    /// # Arguments
    ///
    /// * `name` - Its name.
    /// * `kind` - What it carries.
    ///
    /// # Returns
    ///
    /// The terminal.
    pub const fn input(name: &'static str, kind: Kind) -> Terminal {
        Terminal {
            name,
            direction: Direction::In,
            kind,
            sums: false,
            required: false,
        }
    }

    /// An output of this kind.
    ///
    /// # Arguments
    ///
    /// * `name` - Its name.
    /// * `kind` - What it carries.
    ///
    /// # Returns
    ///
    /// The terminal.
    pub const fn output(name: &'static str, kind: Kind) -> Terminal {
        Terminal {
            name,
            direction: Direction::Out,
            kind,
            sums: false,
            required: false,
        }
    }

    /// This input, taking the sum of every output connected to it.
    ///
    /// # Returns
    ///
    /// The terminal, summing.
    pub const fn summing(self) -> Terminal {
        Terminal { sums: true, ..self }
    }

    /// This input, which must be connected or driven.
    ///
    /// # Returns
    ///
    /// The terminal, required.
    pub const fn must_be_joined(self) -> Terminal {
        Terminal {
            required: true,
            ..self
        }
    }
}

/// A terminal as a file names it: a component and one of its terminals.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct At {
    pub component: String,
    pub terminal: String,
}

impl At {
    /// Reads a terminal as a file writes it.
    ///
    /// # Arguments
    ///
    /// * `what` - The setting it is written in, for the complaint.
    /// * `said` - The terminal: `zone.temperature`.
    ///
    /// # Returns
    ///
    /// The component's name and the terminal's.
    ///
    /// # Errors
    ///
    /// A message for one that is not two names joined by one dot.
    pub fn parse(what: &str, said: &str) -> Result<At, String> {
        let said = said.trim();
        match said.split_once('.') {
            Some((component, terminal))
                if !component.trim().is_empty()
                    && !terminal.trim().is_empty()
                    && !terminal.contains('.') =>
            {
                Ok(At {
                    component: component.trim().to_string(),
                    terminal: terminal.trim().to_string(),
                })
            }
            _ => Err(format!(
                "{what}: \"{said}\" is not a terminal: write the component's \
                 name and the terminal's, joined by a dot, such as \
                 zone.temperature"
            )),
        }
    }
}

impl std::fmt::Display for At {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.component, self.terminal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A terminal is two names and a dot, and nothing else is.
    #[test]
    fn a_terminal_is_two_names_and_a_dot() {
        let at = At::parse("connects", " zone.temperature ").expect("reads");
        assert_eq!(at.component, "zone");
        assert_eq!(at.terminal, "temperature");
        assert_eq!(at.to_string(), "zone.temperature");
        for said in ["zone", ".on", "heater.", "a.b.c", ""] {
            let why = At::parse("connects", said).expect_err(said);
            assert!(why.contains("joined by a dot"), "{said}: {why}");
        }
    }
}
