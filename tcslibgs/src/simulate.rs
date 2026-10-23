//! The simulation mode: how many of a run's payloads may be stand-ins.
//!
//! Given to tcspecial, tcssim and tcsmoc as `--simulate=<mode>` and never
//! written in a configuration, so that the file flown is the file tested,
//! byte for byte. Read here, once, so that the three programs take the flag
//! the same way and refuse the same mistakes in the same words. See
//! "Simulation Modes" in `docs/design.rst`.

use std::fmt;

/// The flag, as it is written before its `=`.
pub const FLAG: &str = "--simulate";

/// How many of a run's payloads may be stand-ins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// No payload may be a stand-in: the flight setting, and the setting a
    /// run gets when the flag is left out.
    None,
    /// At least one payload is a stand-in and at least one is not.
    Some,
    /// Every payload is a stand-in, so that no hardware is exercised at all.
    All,
}

impl Mode {
    /// Every mode, in the words the flag takes.
    pub const NAMES: [&'static str; 3] = ["none", "some", "all"];

    /// The word the flag takes for this mode.
    pub fn name(self) -> &'static str {
        match self {
            Mode::None => "none",
            Mode::Some => "some",
            Mode::All => "all",
        }
    }

    fn from_name(name: &str) -> Option<Mode> {
        match name {
            "none" => Some(Mode::None),
            "some" => Some(Mode::Some),
            "all" => Some(Mode::All),
            _ => None,
        }
    }

    /// Whether this is the setting a vehicle flies with.
    pub fn is_flight(self) -> bool {
        self == Mode::None
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Takes `--simulate=<mode>` out of a program's arguments, giving the mode and
/// what is left.
///
/// The mode is `none` when the flag is not there, so that a flight launch
/// that leaves it out gets the flight setting. A mode that is not one of the
/// three, a `--simulate` with no mode, and the flag given twice are each
/// refused, naming the three modes: a second `--simulate` is more likely a
/// mistake about which run this is than something meant to override the
/// first.
pub fn take_mode(args: Vec<String>) -> Result<(Mode, Vec<String>), String> {
    let modes = Mode::NAMES.join(", ");
    let mut mode = None;
    let mut rest = Vec::with_capacity(args.len());
    for arg in args {
        let Some(after) = arg.strip_prefix(FLAG) else {
            rest.push(arg);
            continue;
        };
        let Some(name) = after.strip_prefix('=') else {
            if after.is_empty() {
                return Err(format!(
                    "{FLAG} needs a mode, written {FLAG}=<mode>, \
                     where the mode is one of {modes}"
                ));
            }
            // Some other flag that begins the same way: not this one's to
            // judge.
            rest.push(arg);
            continue;
        };
        let Some(this) = Mode::from_name(name) else {
            return Err(format!(
                "{FLAG}={name} is not a simulation mode: give one of {modes}"
            ));
        };
        if mode.is_some() {
            return Err(format!(
                "{FLAG} is given twice: give it once, with one of {modes}"
            ));
        }
        mode = Some(this);
    }
    Ok((mode.unwrap_or(Mode::None), rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(of: &[&str]) -> Vec<String> {
        of.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn left_out_it_is_the_flight_setting() {
        let (mode, rest) = take_mode(args(&["tcspecial", "x.yaml"])).unwrap();
        assert_eq!(mode, Mode::None);
        assert!(mode.is_flight());
        assert_eq!(rest, args(&["tcspecial", "x.yaml"]));
    }

    #[test]
    fn each_mode_is_taken_out_wherever_it_is() {
        for name in Mode::NAMES {
            let flag = format!("--simulate={name}");
            let (mode, rest) = take_mode(args(&["tcspecial", &flag, "x.yaml"])).unwrap();
            assert_eq!(mode.name(), name);
            assert_eq!(rest, args(&["tcspecial", "x.yaml"]));
            let (after, _) = take_mode(args(&["tcspecial", "x.yaml", &flag])).unwrap();
            assert_eq!(after, mode);
        }
    }

    #[test]
    fn a_mistaken_mode_is_refused_naming_the_three() {
        for wrong in [
            &["p", "--simulate=most"][..],
            &["p", "--simulate"],
            &["p", "--simulate="],
            &["p", "--simulate=all", "--simulate=all"],
            &["p", "--simulate=all", "--simulate=none"],
        ] {
            let said = take_mode(args(wrong)).unwrap_err();
            assert!(said.contains("none, some, all"), "{said}");
        }
    }
}
