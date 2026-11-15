//! A consolidator aboard: several readings of different things, sent to the
//! ground as one datum. See "Compute elements" in the design, and
//! `tcsprotocol::consolidation`, which lays the datum out.
//!
//! It holds, for each input, the latest reading and whether it is new,
//! held or none, in storage sized when the configuration is loaded. A
//! reading that arrives replaces the one held; one that is not of its
//! input's type -- a boolean that is neither 0 nor 1 among them -- is
//! refused and counted, and the reading held before it stands. What arrives
//! at its `moc-in` asks for a datum.
//!
//! It sends at the end of a pass, never at the arrival that fired a
//! trigger, so that a datum holds every reading of the pass: see
//! [`Port::end_of_pass`]. Once a datum is sent every new input becomes
//! held. An `interval` that comes round with nothing new sends nothing; a
//! datum asked for at `moc-in` is always sent.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tcsprotocol::consolidation::{Layout, Shape, State};
use tcsprotocol::node::vehicle::compute::{ConsolidatorParams, Triggers};

use crate::runtime::fabric::{Arriving, Port, Taken};
use crate::runtime::node_state::ConsolidatorCounters;

/// One input's latest reading.
struct Held {
    /// The reading, in storage as long as the input's most.
    bytes: Vec<u8>,
    /// How many of `bytes` it is.
    len: usize,
    /// Whether it is new, held, or none.
    state: State,
}

/// A consolidator, as the fabric holds it.
pub struct ConsolidatorPort {
    /// Its name, which every complaint names.
    what: String,
    /// How its datum is laid out, and its inputs' names and order.
    layout: Layout,
    /// When it sends.
    send: Triggers,
    /// How often it sends where `send` names `interval`, which the ground
    /// may set.
    interval: Option<Duration>,
    /// When the interval next comes round.
    due: Option<Instant>,
    /// Each input's latest reading, in the order of the inputs.
    held: Vec<Held>,
    /// Whether something has arrived at its `moc-in` since the last datum.
    asked: bool,
    /// What it has sent and refused, which its node's state shares for
    /// QUERY_CONSOLIDATOR.
    counters: Arc<ConsolidatorCounters>,
}

impl ConsolidatorPort {
    /// Makes a consolidator with nothing arrived: every input none.
    ///
    /// # Arguments
    ///
    /// * `what`   - Its name.
    /// * `params` - What its node says.
    /// * `layout` - How its datum is laid out, from the configuration.
    /// * `now`    - When the vehicle starts, from which the interval counts.
    ///
    /// # Returns
    ///
    /// The port, every reading's storage made.
    ///
    /// # Errors
    ///
    /// A sentence where it has more inputs than [`MOST_INPUTS`].
    pub fn of(
        what: &str,
        params: &ConsolidatorParams,
        layout: Layout,
        now: Instant,
    ) -> Result<ConsolidatorPort, String> {
        if layout.inputs.len() > MOST_INPUTS {
            return Err(format!(
                "\"{what}\" (consolidator) has {} inputs, and tcspecial packs \
                 at most {MOST_INPUTS}: lower max_consolidator_inputs and use \
                 a second consolidator",
                layout.inputs.len()
            ));
        }
        let counters = Arc::new(ConsolidatorCounters::new(layout.inputs.len()));
        let held = layout
            .inputs
            .iter()
            .map(|input| {
                let most = match input.shape {
                    Shape::Counted { most, .. } => most,
                    Shape::Fixed(_) => input.fixed_link_bytes().unwrap_or(0),
                };
                Held {
                    bytes: vec![0; most],
                    len: 0,
                    state: State::None,
                }
            })
            .collect();
        Ok(ConsolidatorPort {
            what: what.to_string(),
            layout,
            send: params.send,
            interval: params.interval,
            due: params.interval.map(|every| now + every),
            held,
            asked: false,
            counters,
        })
    }

    /// What it has sent and refused.
    pub fn counters(&self) -> &ConsolidatorCounters {
        &self.counters
    }

    /// Says which triggers have fired, and moves the interval on where it
    /// has come round.
    ///
    /// # Arguments
    ///
    /// * `now` - The time of the end of the pass.
    ///
    /// # Returns
    ///
    /// The triggers that fired: a datum is sent where any did.
    fn fires(&mut self, now: Instant) -> Triggers {
        let any_new = self.held.iter().any(|held| held.state == State::New);
        let all_new = self.held.iter().all(|held| held.state == State::New);
        let mut fired = Triggers {
            all_new: self.send.all_new && all_new,
            interval: false,
            moc: self.send.moc && self.asked,
        };
        if let (Some(every), Some(due)) = (self.interval, self.due) {
            if now >= due {
                // From when it was due, so that it does not drift; one so
                // far behind that the next is already past is put an
                // interval from now.
                let next = due + every;
                self.due = Some(if next <= now { now + every } else { next });
                fired.interval = self.send.interval && any_new;
            }
        }
        fired
    }
}

impl Port for ConsolidatorPort {
    /// Nothing: what it sends is handed over at the end of a pass.
    fn take(&mut self, _buffer: &mut [u8]) -> Result<Taken, String> {
        Ok(Taken::NOTHING)
    }

    /// Holds a reading at the input it names, or takes what names no input
    /// -- what arrives at its `moc-in` -- as asking for a datum.
    fn give(
        &mut self,
        arriving: Arriving<'_>,
        data: &[u8],
    ) -> Result<(), String> {
        let Some(name) = arriving.input else {
            self.asked = true;
            return Ok(());
        };
        let Some(at) = self.layout.inputs.iter().position(|i| i.name == name)
        else {
            return Err(format!(
                "\"{}\" has no input called \"{name}\"",
                self.what
            ));
        };
        if !self.layout.accepts(at, data) {
            self.counters.refused();
            return Err(format!(
                "\"{}\": what arrived at \"{name}\" is not of its type, {}, \
                 and the reading held before it stands",
                self.what, self.layout.inputs[at].kind
            ));
        }
        let held = &mut self.held[at];
        held.bytes[..data.len()].copy_from_slice(data);
        held.len = data.len();
        held.state = State::New;
        Ok(())
    }

    /// Nothing is taken from it as a device's reading is: what it sends is
    /// handed over at the end of a pass.
    fn produces(&self) -> bool {
        false
    }

    /// Sets its `interval`, the one attribute the ground may set: `send`
    /// is the shape of the node.
    fn set(&mut self, attribute: &str, value: &[u8]) -> Result<(), String> {
        if attribute != "interval" {
            return Err(format!(
                "\"{}\" (consolidator): the ground may set its interval, and \
                 nothing called \"{attribute}\"",
                self.what
            ));
        }
        if !self.send.interval {
            return Err(format!(
                "\"{}\" (consolidator): its send does not name interval, so \
                 there is no interval to set",
                self.what
            ));
        }
        let said = std::str::from_utf8(value).map_err(|_| {
            format!("\"{}\": the interval is not text", self.what)
        })?;
        let every = tcsprotocol::graph::parse_interval(said.trim())
            .map_err(|why| format!("\"{}\": interval: {why}", self.what))?;
        self.interval = Some(every);
        self.due = Some(Instant::now() + every);
        Ok(())
    }

    /// Packs a datum where a trigger has fired, and makes every new input
    /// held once it has: allocating nothing.
    fn end_of_pass(
        &mut self,
        now: Instant,
        out: &mut [u8],
    ) -> Result<usize, String> {
        let fired = self.fires(now);
        if !(fired.all_new || fired.interval || fired.moc) {
            return Ok(0);
        }
        // Each input's state and reading, on the stack: at most as many as
        // a configuration lets a consolidator have.
        let mut readings: [(State, &[u8]); MOST_INPUTS] =
            [(State::None, &[]); MOST_INPUTS];
        for (slot, held) in readings.iter_mut().zip(&self.held) {
            *slot = (held.state, &held.bytes[..held.len]);
        }
        let used = self
            .layout
            .pack(&readings[..self.held.len()], out)
            .map_err(|at| {
                format!(
                    "\"{}\": input \"{}\" could not be packed",
                    self.what, self.layout.inputs[at].name
                )
            })?;
        self.counters
            .sent(fired, self.held.iter().map(|held| held.state));
        for held in &mut self.held {
            if held.state == State::New {
                held.state = State::Held;
            }
        }
        self.asked = false;
        Ok(used)
    }

    /// When its interval next comes round, so that a pass is made then.
    fn due(&self) -> Option<Instant> {
        self.due
    }

    /// Its counters, for its node's state to share.
    fn consolidator_counters(&self) -> Option<Arc<ConsolidatorCounters>> {
        Some(Arc::clone(&self.counters))
    }
}

/// The most inputs a consolidator aboard may have: the stack array its
/// readings are gathered in for packing is this long. A configuration's
/// `max_consolidator_inputs` above it is refused where the port is made.
pub const MOST_INPUTS: usize = 256;

#[cfg(test)]
mod tests {
    use super::*;
    use tcsprotocol::consolidation::Input;
    use tcsprotocol::link_type::{Basic, LinkType};

    /// A consolidator of two `u8`s, sending as `send` says, every second
    /// where it names interval.
    fn port(send: &str, now: Instant) -> ConsolidatorPort {
        let input = |name: &str| Input {
            name: name.to_string(),
            source: name.to_string(),
            kind: LinkType::Basic(Basic::U8),
            shape: Shape::of_basic(Basic::U8, 0),
        };
        let triggers = Triggers::of(send).expect("triggers");
        ConsolidatorPort::of(
            "pack",
            &ConsolidatorParams {
                inputs: vec!["a".to_string(), "b".to_string()],
                send: triggers,
                interval: triggers.interval.then_some(Duration::from_secs(1)),
            },
            Layout {
                inputs: vec![input("a"), input("b")],
            },
            now,
        )
        .expect("a port")
    }

    /// What arrives at its moc-in asks for a datum, which is sent whether
    /// or not anything is new; with nothing asked, nothing is.
    #[test]
    fn a_datum_asked_for_is_sent_new_or_not() {
        let now = Instant::now();
        let mut pack = port("moc", now);
        let mut out = [0u8; 8];
        assert_eq!(pack.end_of_pass(now, &mut out), Ok(0));
        pack.give(Arriving::ANYWHERE, b"please").expect("asked");
        // Nothing has arrived from either: both none, every bit nought.
        assert_eq!(pack.end_of_pass(now, &mut out), Ok(3));
        assert_eq!(&out[..3], &[0, 0, 0]);
        // Asked once, sent once.
        assert_eq!(pack.end_of_pass(now, &mut out), Ok(0));
    }

    /// The ground may set the interval, and nothing else; and not where
    /// send does not name interval.
    #[test]
    fn the_interval_may_be_set_from_the_ground() {
        let now = Instant::now();
        let mut pack = port("all_new, interval", now);
        assert!(pack.set("interval", b"250 ms").is_ok());
        assert!(pack.due().is_some_and(|due| due > now));
        assert!(pack.set("interval", b"soon").is_err());
        assert!(pack.set("send", b"moc").is_err());
        let mut asked = port("moc", now);
        assert!(asked.set("interval", b"1 s").is_err());
    }
}
