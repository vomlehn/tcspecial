//! What a compute node may say, and what each computation must be told.
//!
//! The rules only -- what the attributes are, which are required, and what
//! each value may be. The arithmetic is `tcspecial::node::vehicle::compute`, which
//! holds the running computations and nothing about files.
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
    /// The format this name asks for.
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

    /// How many bytes a reading takes.
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
pub struct MeanParams {
    /// The inputs this is waiting for, as the node declares them.
    ///
    /// Declared and then checked against the links, which is the point of
    /// having them written down: a mean over three sensors whose third link
    /// was deleted has two inputs, each bound exactly once, and would quietly
    /// become a mean over two. The declaration is what makes that a
    /// configuration fault instead.
    pub inputs: Vec<String>,
}

/// A setpoint and the band either side of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HysteresisParams {
    pub setpoint: f64,
    pub above: f64,
    pub below: f64,
}

/// A computation that is a program of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessParams {
    pub program: String,
    pub timeout: Duration,
    pub restart: Restart,
}

/// What a compute node computes, settled.
#[derive(Debug, Clone, PartialEq)]
pub enum Computation {
    ToKelvin(ToKelvinParams),
    Mean(MeanParams),
    Hysteresis(HysteresisParams),
    Process(ProcessParams),
}

impl Computation {
    /// What this computation is called in a configuration file.
    pub fn function(&self) -> &'static str {
        match self {
            Computation::ToKelvin(_) => "to_kelvin",
            Computation::Mean(_) => "mean",
            Computation::Hysteresis(_) => "hysteresis",
            Computation::Process(_) => "process",
        }
    }
}

/// Every function there is, for an error to list.
pub fn functions() -> &'static str {
    "to_kelvin, mean, hysteresis, or process"
}

/// What a computation is told, by name.
///
/// The list every computation is held to: `params_of` refuses an attribute
/// that is not on it, and this is the same list said out loud so that a tool
/// can ask. tcsgui's description tree is what asks -- a description of a
/// computation names the attributes its panel offers, and a test holds those
/// against this -- because a description that asked for something else would
/// be a panel filling in a field the checking then refuses.
///
/// `function` is on every list, it being how a node says which computation it
/// is. `None` for a name that is not a computation.
pub fn attributes_of(function: &str) -> Option<&'static [&'static str]> {
    match function {
        "to_kelvin" => Some(&["function", "format", "scale", "offset"]),
        "mean" => Some(&["function", "inputs"]),
        "hysteresis" => Some(&["function", "setpoint", "above", "below"]),
        "process" => Some(&["function", "program", "timeout", "restart"]),
        _ => None,
    }
}

/// What this compute node computes, or what is wrong with what it says.
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
             built-in ones are to_kelvin, mean and hysteresis; \"process\" \
             runs a program of your own"
        ));
    };
    for attribute in attributes.keys() {
        if !own.contains(&attribute.as_str()) {
            return Err(named(format!(
                "there is nothing called \"{attribute}\" on this computation. It \
                 takes {}",
                own.join(", ")
            )));
        }
    }

    match function.as_str() {
        "to_kelvin" => {
            let format = attributes
                .get("format")
                .ok_or_else(|| {
                    named(
                        "format is not set: a sensor's bytes cannot be read without \
                         knowing how they are laid out"
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
                    "scale: nought would make every reading the same".to_string(),
                ));
            }

            Ok(Computation::ToKelvin(ToKelvinParams {
                format,
                scale,
                offset: number(attributes, "offset").map_err(named)?,
            }))
        }
        "mean" => {
            let inputs = attributes.get("inputs").ok_or_else(|| {
                named(
                    "inputs is not set: a mean says which inputs it is over, so \
                     that a link deleted from it is a fault and not a mean over \
                     fewer"
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

            if inputs.len() < 2 {
                return Err(named(format!(
                    "a mean is over at least two inputs, and this says {}: a mean \
                     over one is the number itself",
                    inputs.len()
                )));
            }
            let mut once = inputs.clone();
            once.sort();
            once.dedup();
            if once.len() != inputs.len() {
                return Err(named(
                    "an input is named twice: a mean over an input counted twice \
                     weights it twice"
                        .to_string(),
                ));
            }

            Ok(Computation::Mean(MeanParams { inputs }))
        }
        "hysteresis" => {
            let above = number(attributes, "above").map_err(named)?;
            let below = number(attributes, "below").map_err(named)?;
            if above <= 0.0 || below <= 0.0 {
                return Err(named(
                    "above and below are how far from the setpoint the decision \
                     changes, and must be more than nought: a band of nothing is \
                     a bare comparison, which cycles a relay on a wobble"
                        .to_string(),
                ));
            }

            Ok(Computation::Hysteresis(HysteresisParams {
                setpoint: number(attributes, "setpoint").map_err(named)?,
                above,
                below,
            }))
        }
        "process" => {
            let program = attributes
                .get("program")
                .map(|p| p.as_str().to_string())
                .ok_or_else(|| {
                    named(
                        "program is not set: a compute node whose function is \
                         \"process\" is the program it names"
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
                        "timeout is not set, and there is no default for it: how \
                         long a computation may take is a property of the \
                         computation"
                            .to_string(),
                    )
                })
                .and_then(|t| {
                    crate::graph::parse_interval(t.as_str())
                        .map_err(|why| named(format!("timeout: {why}")))
                })?;

            let restart = match attributes.get("restart").map(Scalar::as_str) {
                None | Some("never") => Restart::Never,
                Some(other) => crate::whole::whole_u32(other).map(Restart::UpTo).map_err(|_| {
                    named(format!(
                        "restart: \"{other}\" is neither \"never\" nor a number of \
                         times"
                    ))
                })?,
            };

            Ok(Computation::Process(ProcessParams {
                program,
                timeout,
                restart,
            }))
        }
        // Covered by the match above, which refuses a function this does not
        // know before any of its attributes are read.
        other => Err(format!("\"{what}\": there is no computation called \"{other}\"")),
    }
}

/// A number an attribute says, or what is wrong with it.
///
/// Required, because every parameter these computations take is one the
/// arithmetic cannot be done without. A scale of nought or an absent setpoint
/// is not a default worth inventing -- it is a configuration that has not said
/// what it wants, and saying so is better than averaging against zero.
fn number(attributes: &BTreeMap<String, Scalar>, name: &str) -> Result<f64, String> {
    let text = attributes
        .get(name)
        .ok_or_else(|| format!("{name} is not set, and there is no sensible default for it"))?;
    text.as_str()
        .parse()
        .map_err(|_| format!("{name}: \"{}\" is not a number", text.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attributes(pairs: &[(&str, &str)]) -> BTreeMap<String, Scalar> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), Scalar::of(*v)))
            .collect()
    }

    fn params(pairs: &[(&str, &str)]) -> Computation {
        params_of("node", &attributes(pairs)).unwrap_or_else(|why| panic!("{why}"))
    }

    fn refused(pairs: &[(&str, &str)]) -> String {
        params_of("node", &attributes(pairs)).expect_err("a computation that is wrong")
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
            params(&[("function", "mean"), ("inputs", "in0,in1,in2")]),
            Computation::Mean(MeanParams {
                inputs: vec!["in0".to_string(), "in1".to_string(), "in2".to_string()],
            })
        );

        assert_eq!(
            params(&[
                ("function", "hysteresis"),
                ("setpoint", "293.15"),
                ("above", "2.0"),
                ("below", "2.0"),
            ]),
            Computation::Hysteresis(HysteresisParams {
                setpoint: 293.15,
                above: 2.0,
                below: 2.0,
            })
        );

        assert_eq!(
            params(&[
                ("function", "process"),
                ("program", "/opt/flight/attitude"),
                ("timeout", "500ms"),
            ]),
            Computation::Process(ProcessParams {
                program: "/opt/flight/attitude".to_string(),
                timeout: Duration::from_millis(500),
                restart: Restart::Never,
            })
        );

        // And each says what it is called, which is what a complaint names.
        assert_eq!(params(&[("function", "mean"), ("inputs", "a,b")]).function(), "mean");
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
            (vec![("function", "to_kelvin"), ("scale", "1"), ("offset", "0")], "format"),
            (
                vec![("function", "to_kelvin"), ("format", "i16_be"), ("offset", "0")],
                "scale",
            ),
            (
                vec![("function", "to_kelvin"), ("format", "i16_be"), ("scale", "1")],
                "offset",
            ),
            (vec![("function", "mean")], "inputs is not set"),
            (
                vec![("function", "hysteresis"), ("setpoint", "293"), ("above", "2")],
                "below",
            ),
            (vec![("function", "process")], "program is not set"),
            (
                vec![("function", "process"), ("program", "/bin/cat")],
                "no default for it",
            ),
        ] {
            let said = refused(&pairs);
            assert!(said.contains(wanted), "{pairs:?}: {said}");
            assert!(said.contains("node"), "every complaint names the node: {said}");
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
                    ("above", "0"),
                    ("below", "2"),
                ],
                "more than nought",
            ),
            (
                vec![
                    ("function", "process"),
                    ("program", "/bin/cat"),
                    ("timeout", "soon"),
                ],
                "timeout",
            ),
            (
                vec![
                    ("function", "process"),
                    ("program", "/bin/cat"),
                    ("timeout", "1s"),
                    ("restart", "sometimes"),
                ],
                "sometimes",
            ),
        ] {
            let said = refused(&pairs);
            assert!(said.contains(wanted), "{pairs:?}: {said}");
        }
    }

    /// A mean is over at least two inputs, each named once.
    #[test]
    fn a_mean_is_over_several_inputs() {
        // Commas or spaces, as a file writes a list.
        assert_eq!(
            params(&[("function", "mean"), ("inputs", "in0 in1")]),
            params(&[("function", "mean"), ("inputs", "in0, in1")])
        );

        let said = refused(&[("function", "mean"), ("inputs", "in0")]);
        assert!(said.contains("at least two"), "{said}");

        let said = refused(&[("function", "mean"), ("inputs", "in0,in0")]);
        assert!(said.contains("twice"), "{said}");
    }

    /// An attribute nothing reads is refused, naming what the computation
    /// takes.
    #[test]
    fn an_attribute_nothing_reads_is_refused() {
        let said = refused(&[
            ("function", "hysteresis"),
            ("setpoint", "293.15"),
            ("setpiont", "300.0"),
            ("above", "2"),
            ("below", "2"),
        ]);
        assert!(
            said.contains("setpiont") && said.contains("setpoint, above, below"),
            "{said}"
        );

        // And one that belongs to another computation, which is the same
        // mistake from the other direction.
        let said = refused(&[
            ("function", "mean"),
            ("inputs", "in0,in1"),
            ("setpoint", "293.15"),
        ]);
        assert!(said.contains("setpoint"), "{said}");
    }

    /// A restart allowance is never, unless a number says otherwise.
    #[test]
    fn restarting_is_never_by_default() {
        let process = |pairs: &[(&str, &str)]| match params(pairs) {
            Computation::Process(process) => process,
            other => panic!("{other:?}"),
        };

        assert_eq!(
            process(&[
                ("function", "process"),
                ("program", "/bin/cat"),
                ("timeout", "1s"),
            ])
            .restart,
            Restart::Never
        );
        assert_eq!(
            process(&[
                ("function", "process"),
                ("program", "/bin/cat"),
                ("timeout", "1s"),
                ("restart", "never"),
            ])
            .restart,
            Restart::Never
        );
        assert_eq!(
            process(&[
                ("function", "process"),
                ("program", "/bin/cat"),
                ("timeout", "1s"),
                ("restart", "3"),
            ])
            .restart,
            Restart::UpTo(3)
        );
    }
}
