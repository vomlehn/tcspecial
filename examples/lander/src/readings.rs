//! What the two vehicles last said, as the station shows it: each value by
//! the node that sent it, where the lander is by dead reckoning between
//! them, whether it has landed or crashed, and its sky.

use std::collections::{BTreeMap, HashMap};

use tcsos::time::{Duration, Instant, SystemTime};
use tcsprotocol::node::physical::lander::{StarRecord, STAR_RECORD};
use tcsprotocol::node::vehicle::compute::from_value;
use tcsprotocol::unit::Unit;

/// How far ahead of the last position the lander is carried by dead
/// reckoning, at most: so that a link gone quiet does not carry it off.
pub const RECKON_FOR: Duration = Duration::from_millis(500);

/// What the station says of a value that has not arrived.
pub const NOTHING_YET: &str = "--";

/// A star as the sky is drawn from it: its place, in metres; its radius, in
/// the scene's pixels; and its brightness, from nought to one.
pub type Star = ((f64, f64), f64, f32);

/// What the lander is doing, as the values sent down say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Neither landed nor crashed.
    Flying,
    /// Standing on the pad.
    Landed,
    /// Down, and not on its feet on the pad.
    Crashed,
}

/// One value as it arrived.
#[derive(Debug, Clone, PartialEq)]
enum Value {
    /// A number, in its node's unit.
    Number(f64),
    /// Anything else: a signal's text, a record.
    Bytes(Vec<u8>),
}

/// What the vehicles last said.
#[derive(Debug, Default)]
pub struct Readings {
    /// Each node's unit, for the nodes that send numbers.
    units: HashMap<String, Unit>,
    /// Each value, by the name of the node that sent it.
    values: HashMap<String, Value>,
    /// When each value last changed, on this station's clock.
    changed: HashMap<String, Instant>,
    /// When each value last changed, by the wall clock, which its node's
    /// panel says as a time of day.
    arrived: HashMap<String, SystemTime>,
    /// The last number sent up to each payload the station commands, and
    /// when, by the payload's name.
    sent: HashMap<String, (SystemTime, u64)>,
}

impl Readings {
    /// Makes an empty set of readings.
    ///
    /// # Arguments
    ///
    /// * `units` - The unit of each node that sends a number; any other
    ///             node's datum is kept as it came.
    ///
    /// # Returns
    ///
    /// The readings, with nothing arrived.
    pub fn new(units: HashMap<String, Unit>) -> Readings {
        Readings {
            units,
            ..Readings::default()
        }
    }

    /// Takes a datum that arrived.
    ///
    /// # Arguments
    ///
    /// * `source` - The node that sent it.
    /// * `data`   - What it sent.
    /// * `at`     - When it is taken to have arrived.
    pub fn take(&mut self, source: &str, data: &[u8], at: Instant) {
        let value = match self.units.get(source) {
            Some(&unit) => match from_value(data, unit, source) {
                Ok(number) => Value::Number(number),
                Err(_) => return,
            },
            None => Value::Bytes(data.to_vec()),
        };
        if self.values.get(source) != Some(&value) {
            self.changed.insert(source.to_string(), at);
            self.arrived.insert(source.to_string(), SystemTime::now());
        }
        self.values.insert(source.to_string(), value);
    }

    /// A node's latest value as a number.
    ///
    /// # Arguments
    ///
    /// * `node` - The node's name.
    ///
    /// # Returns
    ///
    /// The value, or `None` before one has arrived.
    pub fn number(&self, node: &str) -> Option<f64> {
        match self.values.get(node) {
            Some(Value::Number(number)) => Some(*number),
            _ => None,
        }
    }

    /// Whether a signal's latest value says yes: the text `1`.
    ///
    /// # Arguments
    ///
    /// * `node` - The signal's name.
    ///
    /// # Returns
    ///
    /// True where it said `1`; false where it said anything else, or
    /// nothing yet.
    pub fn said(&self, node: &str) -> bool {
        match self.values.get(node) {
            Some(Value::Bytes(bytes)) => {
                String::from_utf8_lossy(bytes).trim() == "1"
            }
            Some(Value::Number(number)) => *number == 1.0,
            None => false,
        }
    }

    /// What the lander is doing.
    ///
    /// # Returns
    ///
    /// Landed or crashed as the signals say, flying otherwise.
    pub fn state(&self) -> State {
        if self.said("landed") {
            State::Landed
        } else if self.said("crashed") {
            State::Crashed
        } else {
            State::Flying
        }
    }

    /// Where the lander is along an axis now, by dead reckoning: its last
    /// reported position, carried on by its last reported velocity and
    /// acceleration for the time since that position arrived, at most
    /// [`RECKON_FOR`], and not at all while it stands or has crashed.
    ///
    /// # Arguments
    ///
    /// * `axis` - `"x"` or `"y"`.
    /// * `now`  - The moment.
    ///
    /// # Returns
    ///
    /// The position, in metres, or `None` before one has arrived.
    pub fn estimate(&self, axis: &str, now: Instant) -> Option<f64> {
        let node = format!("to_{axis}");
        let at = self.number(&node)?;
        if self.state() != State::Flying {
            return Some(at);
        }
        let Some(since) = self.changed.get(&node) else {
            return Some(at);
        };
        let t = now
            .saturating_duration_since(*since)
            .min(RECKON_FOR)
            .as_secs_f64();
        let speed = self.number(&format!("to_velocity_{axis}")).unwrap_or(0.0);
        let push = self.number(&format!("to_accel_{axis}")).unwrap_or(0.0);
        Some(at + speed * t + 0.5 * push * t * t)
    }

    /// A node's latest value, as the station shows it.
    ///
    /// # Arguments
    ///
    /// * `node`   - The node's name.
    /// * `places` - How many places after the point.
    ///
    /// # Returns
    ///
    /// The value, or [`NOTHING_YET`].
    pub fn text(&self, node: &str, places: usize) -> String {
        match self.number(node) {
            Some(number) => format!("{number:.places$}"),
            None => NOTHING_YET.to_string(),
        }
    }

    /// Notes that something arrived from a node the readings do not keep
    /// the value of -- the sky, which the window keeps -- for its panel.
    ///
    /// # Arguments
    ///
    /// * `node` - The node's name.
    pub fn heard(&mut self, node: &str) {
        self.arrived.insert(node.to_string(), SystemTime::now());
    }

    /// Notes a number sent up to a payload, for its panel.
    ///
    /// # Arguments
    ///
    /// * `node`   - The payload's name.
    /// * `number` - What was sent.
    pub fn note_sent(&mut self, node: &str, number: u64) {
        self.sent
            .insert(node.to_string(), (SystemTime::now(), number));
    }

    /// A node's latest value in the words its panel says it in: a decision
    /// by its name -- `landed` or `not landed` -- the latest rating as
    /// tcs-moc's panels say one, and any other value as a number.
    ///
    /// # Arguments
    ///
    /// * `node` - The node's name.
    ///
    /// # Returns
    ///
    /// The words, or `None` before anything has arrived.
    pub fn node_said(&self, node: &str) -> Option<String> {
        let value = self.values.get(node)?;
        Some(match (node, value) {
            ("landed" | "crashed", _) if self.said(node) => node.to_string(),
            ("landed" | "crashed", _) => format!("not {node}"),
            ("ratings", Value::Bytes(bytes)) => {
                match tcsprotocol::node::physical::lander::ratings_said(bytes) {
                    Some(all) => {
                        all.split("; ").next().unwrap_or("").to_string()
                    }
                    None => "none yet".to_string(),
                }
            }
            (_, Value::Number(_)) => self.text(node, 2),
            (_, Value::Bytes(bytes)) => format!("{} bytes", bytes.len()),
        })
    }

    /// What a node's panel says of it: when its value last arrived and what
    /// it was, or for a payload the station commands, when it was last sent
    /// a number and what; each time of day in UTC, as every panel's is.
    ///
    /// # Arguments
    ///
    /// * `node`      - The node's name.
    /// * `commanded` - Whether the station sends it numbers, rather than
    ///                 hearing from it.
    /// * `said`      - What to say of its value, where the readings do not
    ///                 keep it: the sky's stars, which the window keeps.
    ///
    /// # Returns
    ///
    /// The line: `Last recv: 10:03:12: 450.00`, `Last sent: 10:03:12: 3`,
    /// or the time as dashes before there is one.
    pub fn panel_line(
        &self,
        node: &str,
        commanded: bool,
        said: Option<String>,
    ) -> String {
        let clock =
            |when: SystemTime| tcsprotocol::Timestamp::at(when).time_of_day();
        if commanded {
            return match self.sent.get(node) {
                Some(&(when, number)) => {
                    format!("Last sent: {}: {number}", clock(when))
                }
                None => "Last sent: --:--:--".to_string(),
            };
        }
        match (
            self.arrived.get(node),
            said.or_else(|| self.node_said(node)),
        ) {
            (Some(&when), Some(said)) => {
                format!("Last recv: {}: {said}", clock(when))
            }
            _ => "Last recv: --:--:--".to_string(),
        }
    }
}

/// The lander's sky, as its records come down: no stars until the first
/// arrives, and each star kept as its record arrives.
///
/// tcs-sim sends the stars round and round, four to a packet, each record
/// whole. Once every star of a generation is here, each packet more of it
/// is answered with a quench, the generation sent up to `sky_quench`, which
/// stops the sending; a lost quench is answered again by the next packet. A
/// new generation starts the sky afresh.
#[derive(Debug, Default)]
pub struct Sky {
    /// The generation being gathered, `None` before any.
    generation: Option<u16>,
    /// How many stars it has.
    total: u16,
    /// Each star had, by its number.
    stars: BTreeMap<u16, StarRecord>,
    /// Whether a star has come since the sky was last drawn.
    fresh: bool,
}

impl Sky {
    /// What the sky's panel says of it: the stars come down so far.
    ///
    /// # Returns
    ///
    /// The words: `57 of 120 stars`.
    pub fn said(&self) -> String {
        format!("{} of {} stars", self.stars.len(), self.total)
    }

    /// Takes a packet of records.
    ///
    /// # Arguments
    ///
    /// * `data` - The packet's bytes: whole records, a trailing part of one
    ///            left alone.
    ///
    /// # Returns
    ///
    /// The generation to quench, where the sky is whole.
    pub fn take(&mut self, data: &[u8]) -> Option<u16> {
        let mut whole = None;
        for record in data.as_chunks::<STAR_RECORD>().0 {
            let star = StarRecord::from_bytes(record);
            if Some(star.generation) != self.generation {
                self.generation = Some(star.generation);
                self.total = star.total;
                self.stars.clear();
                self.fresh = true;
            }
            if star.id < star.total && !self.stars.contains_key(&star.id) {
                self.stars.insert(star.id, star);
                self.fresh = true;
            }
            if self.stars.len() >= usize::from(self.total) {
                whole = self.generation;
            }
        }
        whole
    }

    /// The stars, if any has come since this was last asked.
    ///
    /// # Returns
    ///
    /// Each star had, or `None` where none is new.
    pub fn fresh(&mut self) -> Option<Vec<Star>> {
        if !std::mem::take(&mut self.fresh) {
            return None;
        }
        Some(
            self.stars
                .values()
                .map(|s| ((s.x, s.y), s.radius, s.brightness as f32))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A value's bytes, as a compute node sends it.
    ///
    /// # Arguments
    ///
    /// * `value` - The number.
    /// * `unit`  - Its unit.
    ///
    /// # Returns
    ///
    /// Its bytes.
    fn bytes_of(value: f64, unit: Unit) -> Vec<u8> {
        ((value * unit.per_unit()).round() as i32)
            .to_be_bytes()
            .to_vec()
    }

    /// Readings of the lander's position and velocity along X.
    ///
    /// # Returns
    ///
    /// The readings.
    fn readings() -> Readings {
        Readings::new(HashMap::from([
            ("to_x".to_string(), Unit::Metre),
            ("to_velocity_x".to_string(), Unit::MetrePerSecond),
            ("to_accel_x".to_string(), Unit::MetrePerSecondSquared),
        ]))
    }

    /// The lander is carried on from where it was said to be, for no more
    /// than the time dead reckoning is allowed.
    #[test]
    fn the_lander_is_carried_on_by_dead_reckoning() {
        let mut r = readings();
        let t0 = Instant::now();
        assert_eq!(r.estimate("x", t0), None);
        r.take("to_x", &bytes_of(100.0, Unit::Metre), t0);
        r.take("to_velocity_x", &bytes_of(10.0, Unit::MetrePerSecond), t0);
        let later = t0 + Duration::from_millis(200);
        let x = r.estimate("x", later).expect("a position");
        assert!((x - 102.0).abs() < 1e-6, "{x}");
        let much_later = t0 + Duration::from_secs(10);
        let x = r.estimate("x", much_later).expect("a position");
        assert!((x - 105.0).abs() < 1e-6, "{x}");
        assert_eq!(r.text("to_x", 1), "100.0");
        assert_eq!(r.text("to_y", 1), NOTHING_YET);
    }

    /// Landed and crashed are said by their signals' text; landed stops
    /// the reckoning.
    #[test]
    fn the_signals_say_landed_and_crashed() {
        let mut r = readings();
        let t0 = Instant::now();
        assert_eq!(r.state(), State::Flying);
        r.take("crashed", b"1", t0);
        assert_eq!(r.state(), State::Crashed);
        r.take("crashed", b"0", t0);
        r.take("landed", b"1\n", t0);
        assert_eq!(r.state(), State::Landed);
        r.take("to_x", &bytes_of(100.0, Unit::Metre), t0);
        r.take("to_velocity_x", &bytes_of(10.0, Unit::MetrePerSecond), t0);
        let x = r.estimate("x", t0 + Duration::from_millis(300));
        assert_eq!(x, Some(100.0));
    }

    /// The sky is gathered a packet at a time, quenched once it is whole,
    /// and started afresh by a new generation.
    #[test]
    fn the_sky_is_gathered_and_quenched() {
        let star = |generation, id| StarRecord {
            generation,
            total: 3,
            id,
            radius: 1.5,
            brightness: 0.5,
            x: 10.0,
            y: 20.0,
        };
        let packet = |stars: &[StarRecord]| -> Vec<u8> {
            stars.iter().flat_map(|s| s.to_bytes()).collect()
        };
        let mut sky = Sky::default();
        assert_eq!(sky.fresh(), None);
        assert_eq!(sky.take(&packet(&[star(4, 0), star(4, 1)])), None);
        assert_eq!(sky.fresh().map(|s| s.len()), Some(2));
        assert_eq!(sky.fresh(), None);
        assert_eq!(sky.take(&packet(&[star(4, 2), star(4, 0)])), Some(4));
        assert_eq!(sky.take(&packet(&[star(4, 1)])), Some(4));
        assert_eq!(sky.fresh().map(|s| s.len()), Some(3));
        assert_eq!(sky.take(&packet(&[star(5, 0)])), None);
        assert_eq!(sky.fresh().map(|s| s.len()), Some(1));
    }

    /// A panel says when a value came and what it was, a decision in words,
    /// what was last sent to a payload the station commands, and dashes
    /// before anything.
    #[test]
    fn a_panel_says_what_arrived_and_what_was_sent() {
        let mut readings = readings();
        assert_eq!(
            readings.panel_line("to_x", false, None),
            "Last recv: --:--:--"
        );
        assert_eq!(
            readings.panel_line("restart", true, None),
            "Last sent: --:--:--"
        );
        readings.take("to_x", &bytes_of(450.0, Unit::Metre), Instant::now());
        readings.take("landed", b"0", Instant::now());
        let line = readings.panel_line("to_x", false, None);
        assert!(line.starts_with("Last recv: "), "{line}");
        assert!(line.ends_with(": 450.00"), "{line}");
        let line = readings.panel_line("landed", false, None);
        assert!(line.ends_with(": not landed"), "{line}");
        readings.note_sent("restart", 3);
        let line = readings.panel_line("restart", true, None);
        assert!(line.starts_with("Last sent: "), "{line}");
        assert!(line.ends_with(": 3"), "{line}");
        readings.heard("sky");
        let line =
            readings.panel_line("sky", false, Some("4 of 120 stars".into()));
        assert!(line.ends_with(": 4 of 120 stars"), "{line}");
    }
}
