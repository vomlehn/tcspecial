//! RS-485 on a serial line: the half-duplex turnaround.
//!
//! **Experimental, and not yet supported.** The terms parse and reach a
//! handler, and tcspecial asks the port for them; no configuration is shipped
//! that uses them and nothing has been run against real RS-485 hardware.
//!
//! RS-485 is not another kind of link. It is the electrical layer under a
//! start-stop line: the same termios settings, the same framing, the same
//! device node -- and one line driven by several transmitters, so a
//! transmitter has to stop driving the bus when it has finished. Linux does
//! that turnaround in the UART driver, told through `TIOCSRS485`, and what it
//! wants to be told is exactly what a file states here: whether the mode is
//! on at all, which way RTS is driven while sending, and how long to hold the
//! bus either side of a transmission.
//!
//! So these are terms of the `serial` kind rather than a kind of their own.
//! A device on a four-wire RS-422 line states none of them and is unchanged;
//! one on a two-wire RS-485 bus states `rs485: true` and as much of the rest
//! as its wiring needs.
//!
//! Asynchronous lines only. The kernel's RS-485 support is part of its
//! start-stop drivers, where a synchronous line is driven through generic
//! HDLC and a network interface; a synchronous line stating these would be
//! describing hardware that cannot be told about them.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::endpoint_config::{
    bad, parse_flag, EndpointConfigResult, LinkTerms,
};

/// What a line's RS-485 turnaround is set to.
///
/// Carried to the handler, where `tcspecial::node::payload::serial::rs485` asks
/// the port for it. Absent -- `None` on a [`crate::SerialConfig`] -- is a line
/// that is not an RS-485 bus and is left exactly as it was before any of this
/// existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rs485Params {
    /// The logical level of RTS while the line is transmitting.
    ///
    /// Which level asserts the driver is a property of the board, not of the
    /// protocol: the signal commonly goes through an inverter to a
    /// transceiver's DE pin, and on half the boards that means RTS high while
    /// sending and on the other half RTS low. A file that got it the wrong
    /// way round has a line that drives the bus exactly when it should not,
    /// which is why this is stated rather than assumed.
    pub rts_on_send: bool,
    /// How long to hold the bus before the first bit.
    ///
    /// Time for every other transceiver on the bus to turn its driver off.
    /// Nothing is right wherever the transceivers are quick enough, which is
    /// the usual case, and is the default.
    pub delay_before_send: Duration,
    /// How long to hold the bus after the last bit.
    ///
    /// Time for the last stop bit to be clear of the wire before this
    /// transmitter lets go of it. Nought by default, for the reason above.
    pub delay_after_send: Duration,
}

/// The longest delay a file may ask for.
///
/// A turnaround is measured in bit times: a millisecond is already a
/// generous one at 9600 baud and an age at anything faster. A file asking for
/// seconds of it has mistaken the unit -- microseconds, most likely -- and a
/// line that held the bus for that long would look like a line that had
/// failed.
pub const LONGEST_DELAY: Duration = Duration::from_millis(1_000);

/// Reads what a serial device said about RS-485, and what it must say to say
/// anything at all.
///
/// The details are refused without `rs485: true`, rather than kept against
/// the day it is turned on: a file that set a turnaround delay and left the
/// mode off has described a bus whose driver is never released, and reading
/// it as a plain line would be reading a different file from the one that was
/// written.
///
/// # Arguments
///
/// * `name`         - The node's name, for the messages.
/// * `g`            - The line's terms as the file states them.
/// * `asynchronous` - Whether the line is start-stop rather than clocked.
///
/// # Returns
///
/// The line's RS-485 settings, or `None` when the line is not an RS-485 bus,
/// which is a line stating nothing here or stating `rs485: false`.
///
/// # Errors
///
/// Any RS-485 term on a synchronous line; a detail without `rs485: true`;
/// an RS-485 line without `rs485_rts_on_send`; a flag that is not a boolean;
/// and a delay that does not read as an interval or is longer than
/// `LONGEST_DELAY`.
pub(crate) fn rs485_of(
    name: &str,
    g: &LinkTerms,
    asynchronous: bool,
) -> EndpointConfigResult<Option<Rs485Params>> {
    let details = [
        ("rs485_rts_on_send", g.rs485_rts_on_send.is_some()),
        (
            "rs485_delay_before_send",
            g.rs485_delay_before_send.is_some(),
        ),
        ("rs485_delay_after_send", g.rs485_delay_after_send.is_some()),
    ];

    // A synchronous line is driven through generic HDLC rather than through a
    // UART's own settings, so there is nothing to tell about a turnaround.
    if !asynchronous {
        for (field, stated) in
            details.iter().chain([("rs485", g.rs485.is_some())].iter())
        {
            if *stated {
                return Err(bad(
                    name,
                    &format!(
                        "{field} does not apply to a synchronous line: RS-485 \
                         is the electrical layer under a start-stop line, and \
                         a line whose bits are on a clock is driven through \
                         generic HDLC instead"
                    ),
                ));
            }
        }
        return Ok(None);
    }

    let on = parse_flag(name, "rs485", g.rs485.as_ref())?.unwrap_or(false);

    if !on {
        if let Some((field, _)) = details.iter().find(|(_, stated)| *stated) {
            return Err(bad(
                name,
                &format!(
                    "{field} applies only to a line that states rs485: true: a \
                     turnaround on a line that is not an RS-485 bus is a \
                     driver nothing ever releases"
                ),
            ));
        }
        return Ok(None);
    }

    // Which level drives the transceiver, and no default for it: see
    // Rs485Params::rts_on_send.
    let rts_on_send = match g.rs485_rts_on_send.as_ref() {
        Some(stated) => parse_flag(name, "rs485_rts_on_send", Some(stated))?
            .expect("a value was given"),
        None => {
            return Err(bad(
                name,
                "an RS-485 line states rs485_rts_on_send: which level of RTS \
                 drives the transceiver is a property of the board, and a line \
                 that drove the bus the wrong way round would transmit exactly \
                 when it should be listening",
            ))
        }
    };

    let delay =
        |field: &str, stated: Option<&crate::endpoint_config::Scalar>| {
            match stated {
                Some(s) => {
                    let held = crate::graph::parse_interval(s.as_str())
                        .map_err(|why| bad(name, &format!("{field}: {why}")))?;
                    if held > LONGEST_DELAY {
                        return Err(bad(
                            name,
                            &format!(
                        "{field} is longer than {} ms: a turnaround is a few \
                         bit times, and a line that held the bus for this long \
                         would look like one that had failed",
                        LONGEST_DELAY.as_millis()
                    ),
                        ));
                    }
                    Ok(held)
                }
                None => Ok(Duration::ZERO),
            }
        };

    Ok(Some(Rs485Params {
        rts_on_send,
        delay_before_send: delay(
            "rs485_delay_before_send",
            g.rs485_delay_before_send.as_ref(),
        )?,
        delay_after_send: delay(
            "rs485_delay_after_send",
            g.rs485_delay_after_send.as_ref(),
        )?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint_config::Scalar;

    /// Makes terms as a file states them, with everything else absent.
    ///
    /// # Arguments
    ///
    /// * `stated` - Each RS-485 term stated, with its value as written.
    ///
    /// # Returns
    ///
    /// The terms.
    ///
    /// # Panics
    ///
    /// If a term is not one of the four RS-485 terms.
    fn terms(stated: &[(&str, &str)]) -> LinkTerms {
        let mut g = LinkTerms::default();
        for (field, value) in stated {
            let value = Some(Scalar::of(*value));
            match *field {
                "rs485" => g.rs485 = value,
                "rs485_rts_on_send" => g.rs485_rts_on_send = value,
                "rs485_delay_before_send" => g.rs485_delay_before_send = value,
                "rs485_delay_after_send" => g.rs485_delay_after_send = value,
                other => panic!("{other} is not an RS-485 term"),
            }
        }
        g
    }

    /// A line that says nothing is not an RS-485 bus.
    ///
    /// Which is what every configuration written before this existed says, and
    /// what a four-wire line says: the mode is off and the port is left as it
    /// was.
    #[test]
    fn a_line_that_says_nothing_is_not_an_rs485_bus() {
        assert_eq!(
            rs485_of("line", &terms(&[]), true).expect("a plain line"),
            None
        );
        assert_eq!(
            rs485_of("line", &terms(&[("rs485", "false")]), true)
                .expect("a plain line"),
            None
        );
    }

    /// An RS-485 line states which level drives its transceiver.
    #[test]
    fn an_rs485_line_says_which_way_rts_goes() {
        let said = rs485_of("line", &terms(&[("rs485", "true")]), true)
            .expect_err("a line with no level stated");
        assert!(
            format!("{said}").contains("rs485_rts_on_send"),
            "the error should name what is missing: {said}"
        );

        assert_eq!(
            rs485_of(
                "line",
                &terms(&[("rs485", "true"), ("rs485_rts_on_send", "true")]),
                true
            )
            .expect("a line with its level stated"),
            Some(Rs485Params {
                rts_on_send: true,
                delay_before_send: Duration::from_millis(0),
                delay_after_send: Duration::from_millis(0),
            }),
            "and the delays default to nought, which is right for a quick \
             transceiver"
        );
    }

    /// The delays are read, and a delay nobody meant is refused.
    #[test]
    fn the_turnaround_is_a_few_milliseconds_at_most() {
        let good = rs485_of(
            "line",
            &terms(&[
                ("rs485", "true"),
                ("rs485_rts_on_send", "false"),
                ("rs485_delay_before_send", "2 ms"),
                ("rs485_delay_after_send", "3 ms"),
            ]),
            true,
        )
        .expect("a line with its turnaround stated");
        assert_eq!(
            good,
            Some(Rs485Params {
                rts_on_send: false,
                delay_before_send: Duration::from_millis(2),
                delay_after_send: Duration::from_millis(3),
            })
        );

        // Microseconds mistaken for milliseconds, which is the way this is
        // got wrong.
        let said = rs485_of(
            "line",
            &terms(&[
                ("rs485", "true"),
                ("rs485_rts_on_send", "true"),
                ("rs485_delay_before_send", "5 s"),
            ]),
            true,
        )
        .expect_err("five seconds of turnaround");
        assert!(
            format!("{said}").contains("1000 ms"),
            "the error should say the limit: {said}"
        );
    }

    /// A turnaround without the mode is refused rather than kept for later.
    #[test]
    fn a_turnaround_on_a_line_that_is_not_a_bus_is_refused() {
        for field in [
            "rs485_rts_on_send",
            "rs485_delay_before_send",
            "rs485_delay_after_send",
        ] {
            let said = rs485_of("line", &terms(&[(field, "1")]), true)
                .expect_err("a turnaround with the mode off");
            assert!(
                format!("{said}").contains(field)
                    && format!("{said}").contains("rs485: true"),
                "the error should name the field and what it needs: {said}"
            );
        }
    }

    /// A synchronous line is told that none of this is about it.
    #[test]
    fn a_synchronous_line_has_no_turnaround() {
        for field in ["rs485", "rs485_rts_on_send", "rs485_delay_after_send"] {
            let said = rs485_of("line", &terms(&[(field, "true")]), false)
                .expect_err("RS-485 on a synchronous line");
            assert!(
                format!("{said}").contains("synchronous"),
                "the error should say why it does not apply: {said}"
            );
        }

        // And a synchronous line that says nothing is simply not one.
        assert_eq!(
            rs485_of("line", &terms(&[]), false).expect("a synchronous line"),
            None
        );
    }
}
