//! A component's terminals: the places it is joined to other components and to
//! the simulator's simulated payloads.
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
    /// Records, which a simulated payload sends a few at a time, each whole
    /// in its packet, round and round: a lander's sky. What it reads as a
    /// number is which generation of records it is sending.
    Records,
}

impl Kind {
    /// Says what it is, as a sentence names it.
    ///
    /// # Returns
    ///
    /// Its unit's name, `kelvin`, `a decision`, or `records`.
    pub fn describe(self) -> &'static str {
        match self {
            Kind::Value(unit) => unit.name(),
            Kind::Decision => "a decision",
            Kind::Records => "records",
        }
    }
}

/// How a panel says what an output sent, where it says more than the
/// bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Said {
    /// As its kind says it: a decision by the terminal's name, `landed` or
    /// `not landed`; records as their bytes.
    ByKind,
    /// A decision in these words, on and then off, where its name alone
    /// does not read as a state: a body's `contact`.
    Words(&'static str, &'static str),
    /// A lander's ratings, each as its percentage, who flew it and when:
    /// see [`super::lander::ratings_said`].
    Ratings,
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
    /// For an output: how a panel says what it sent.
    pub said: Said,
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
            said: Said::ByKind,
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
            said: Said::ByKind,
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

    /// This decision output, said in these words rather than by its name.
    ///
    /// # Arguments
    ///
    /// * `on`  - What a panel says while it is on.
    /// * `off` - And while it is off.
    ///
    /// # Returns
    ///
    /// The terminal.
    pub const fn said_as(
        self,
        on: &'static str,
        off: &'static str,
    ) -> Terminal {
        Terminal {
            said: Said::Words(on, off),
            ..self
        }
    }

    /// This records output, its records a lander's ratings.
    ///
    /// # Returns
    ///
    /// The terminal.
    pub const fn said_as_ratings(self) -> Terminal {
        Terminal {
            said: Said::Ratings,
            ..self
        }
    }

    /// What a panel says this output sent, in words, where its kind and
    /// [`Said`] have words for it: a decision as its state, `landed` or
    /// `not landed`, and a lander's ratings as they read.
    ///
    /// The one place this is decided, so that the simulator and the ground
    /// say the same thing of the same bytes.
    ///
    /// # Arguments
    ///
    /// * `data` - What it sent: a decision as the text `1` or `0`.
    ///
    /// # Returns
    ///
    /// The words, or `None` where there are none, or the bytes are not
    /// what the output sends: bytes that are wrong are what says they are.
    pub fn said(&self, data: &[u8]) -> Option<String> {
        let mut said = String::new();
        self.write_said(data, &mut said).then_some(said)
    }

    /// Writes what [`Terminal::said`] says, into text a caller keeps, so
    /// that a receiving loop allocates nothing once its text has room.
    ///
    /// # Arguments
    ///
    /// * `data` - What it sent.
    /// * `out`  - Where the words are written, after what is there.
    ///
    /// # Returns
    ///
    /// Whether there were words, and they were written.
    pub fn write_said(
        &self,
        data: &[u8],
        out: &mut impl std::fmt::Write,
    ) -> bool {
        match (self.kind, self.said) {
            (Kind::Decision, said) => {
                let on = match data {
                    b"1" => true,
                    b"0" => false,
                    _ => return false,
                };
                let written = match (said, on) {
                    (Said::Words(on, _), true) => out.write_str(on),
                    (Said::Words(_, off), false) => out.write_str(off),
                    (_, true) => out.write_str(self.name),
                    (_, false) => write!(out, "not {}", self.name),
                };
                written.is_ok()
            }
            (Kind::Records, Said::Ratings) => {
                super::lander::write_ratings_said(data, out)
            }
            _ => false,
        }
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

#[cfg(test)]
mod said_tests {
    use super::*;

    /// A decision is said by its name, or in its own words where it has
    /// them, and anything but `1` or `0` is not said at all.
    #[test]
    fn a_decision_is_said_as_its_state() {
        let landed = Terminal::output("landed", Kind::Decision);
        assert_eq!(landed.said(b"1").as_deref(), Some("landed"));
        assert_eq!(landed.said(b"0").as_deref(), Some("not landed"));
        assert_eq!(landed.said(b"2"), None);
        assert_eq!(landed.said(b"10"), None);
        let contact = landed.said_as("contact", "no contact");
        assert_eq!(contact.said(b"1").as_deref(), Some("contact"));
        assert_eq!(contact.said(b"0").as_deref(), Some("no contact"));
    }

    /// A value, and records with no words of their own, are left to the
    /// panel.
    #[test]
    fn values_and_plain_records_are_not_said() {
        let value = Terminal::output("x", Kind::Value(Unit::Metre));
        assert_eq!(value.said(b"1"), None);
        let sky = Terminal::output("sky", Kind::Records);
        assert_eq!(sky.said(&[0; 16]), None);
    }
}
