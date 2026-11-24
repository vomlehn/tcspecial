//! A lander: flown in two dimensions over a landscape by thrust along each
//! axis, burning its fuel, until it crashes; then, after a pause, flown
//! again from where it started. Set down gently on the pad it has landed,
//! and stands there, still flyable: thrust greater than its weight lifts
//! it off again.
//!
//! Its terminals are `tcsprotocol::node::physical::lander`'s: `throttle_x`
//! and `throttle_y`, ratios from minus one to one; `x` and `y`, in metres;
//! `velocity_x` and `velocity_y`, in metres a second; `fuel`, the fraction
//! of a full tank left; `landed`, a decision asserted while it stands on
//! the pad, and `crashed`, one asserted from a crash until it starts again;
//! and `acceleration_x` and `acceleration_y`, in metres a second a second,
//! gravity's included, nought while it stands on the pad. Crashed, it
//! reports the velocity and acceleration it hit with until it starts
//! again, so that the last values sent down, and shown, are those.
//!
//! # The arithmetic
//!
//! With the throttles held, each axis's force `F` is held and the fuel
//! flows at `q = (|Fx| + |Fy|) / (isp g0)`, so the mass falls in a straight
//! line, `m - q t`, and each axis is the rocket equation [`super::body`]
//! advances by, with gravity on Y alone and a shared `u = q t / m`:
//!
//! ```text
//! v(t) = v - (F / q) ln(1 - u) - g t
//! p(t) = p + v t - g t^2 / 2 + (F m / q^2) h(u)
//! ```
//!
//! exact, and the constant acceleration's where no mass is lost. The tank
//! runs dry at a moment the flow says, and from then on there is no thrust.
//!
//! # Touchdown
//!
//! Where the lander's outline first meets the ground has no closed form, and
//! the ground is a line of segments, so it is found by looking ahead along
//! the path: every [`LOOK_EVERY`] seconds, as far as [`LOOK_AHEAD`], and then
//! by halving the interval it was found in until it is within
//! [`FOUND_WITHIN`]. A step is short enough that a lander going no faster
//! than a lunar landing goes cannot pass through a line of the ground
//! between two looks. Beyond the display's edges the ground is its bottom
//! line, going on indefinitely, so a lander flown off a side flies on out of
//! sight, and comes down if it drops below that line. Where
//! nothing is found ahead, the lander's next event is the end of the look,
//! where it looks again.

use tcsprotocol::node::physical::lander::{
    Point, RatingRecord, Resolved, RATINGS_KEPT, STAR_RECORD,
};

use super::component::Component;

/// Its terminals' places, in
/// `tcsprotocol::node::physical::lander::TERMINALS`'s order, which a test
/// holds this to.
const THROTTLE_X: usize = 0;
const THROTTLE_Y: usize = 1;
const X: usize = 2;
const Y: usize = 3;
const VELOCITY_X: usize = 4;
const VELOCITY_Y: usize = 5;
const FUEL: usize = 6;
const LANDED: usize = 7;
const CRASHED: usize = 8;
const ACCELERATION_X: usize = 9;
const ACCELERATION_Y: usize = 10;
const RESTART: usize = 11;
const SKY: usize = 12;
const SKY_QUENCH: usize = 13;
const RATING: usize = 14;
const RATING_AUTOLAND: usize = 15;
const RATED: usize = 16;
const RATINGS: usize = 17;

/// The generation of the lander's sky: made once, as it is started, and
/// never made again in a run, so always the first.
pub const SKY_GENERATION: u16 = 1;

/// Standard gravity, which turns a specific impulse in seconds into an
/// exhaust velocity: a definition, not the gravity of anywhere in
/// particular.
pub(crate) const STANDARD_GRAVITY: f64 = 9.80665;

/// How far apart, in seconds, the path is looked at for touchdown.
pub const LOOK_EVERY: f64 = 0.02;

/// How far ahead, in seconds, the path is looked at before the lander looks
/// again.
pub const LOOK_AHEAD: f64 = 0.5;

/// The least the bottom engine burns at, as a share of its full thrust, once
/// it burns at all: as the Apollo lunar module's descent engine could be
/// throttled no lower than about a tenth.
pub const LEAST_THROTTLE: f64 = 0.1;

/// The throttles a lander's engines burn at, from those it is given: across
/// within minus one and one; up within nought and one, the lander having
/// its engine on its bottom and nothing on its top to push it down, and,
/// lit, no lower than [`LEAST_THROTTLE`].
///
/// # Arguments
///
/// * `x` - The X throttle it is given.
/// * `y` - The Y throttle it is given.
///
/// # Returns
///
/// The X and Y throttles it burns at, each a share of that axis's full
/// thrust.
pub fn throttles(x: f64, y: f64) -> Point {
    let up = y.clamp(0.0, 1.0);
    let up = if up > 0.0 {
        up.max(LEAST_THROTTLE)
    } else {
        0.0
    };
    (x.clamp(-1.0, 1.0), up)
}

/// How close, in seconds, the moment of touchdown is found.
pub const FOUND_WITHIN: f64 = 1e-4;

/// How far, in metres, the lander may be into the pad at the moment found
/// and still be on it: what the moment, found to within [`FOUND_WITHIN`],
/// leaves of a landing faster than any the limits allow.
const INTO_THE_PAD: f64 = 0.01;

/// How it is.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Flight {
    /// Flying.
    Flying,
    /// Standing on the pad, landed since a moment: still, its engines
    /// burning as the throttles say, until thrust greater than its weight
    /// lifts it off, or its pad wait has run.
    Landed {
        /// When it landed, in simulated seconds.
        since: f64,
    },
    /// Crashed since a moment, until it starts again.
    Crashed {
        /// When it crashed, in simulated seconds.
        since: f64,
        /// How fast it was going as it hit, in metres a second: what it
        /// reports while it is down, so that the last values sent, and so
        /// shown, are what it crashed with.
        velocity: Point,
        /// Its acceleration as it hit, in metres a second a second,
        /// gravity's included, reported likewise.
        acceleration: Point,
    },
}

/// What happens next, and when.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Next {
    /// It touches the ground.
    Down,
    /// Its tank runs dry.
    Dry,
    /// It starts again.
    Again,
    /// Nothing within the look ahead: it looks again.
    Look,
}

/// A lander of the world.
#[derive(Debug)]
pub struct Lander {
    figures: Resolved,
    /// Where it is, the place between its feet, in metres.
    at: Point,
    /// How fast it is going, in metres a second.
    velocity: Point,
    /// The fuel it has left, in kilograms.
    fuel: f64,
    flight: Flight,
    /// Its outline's extent about the place between its feet, worked out
    /// once: see [`Resolved::extent`].
    extent: (f64, f64, f64, f64),
    /// Its throttles, as last held, each within minus one and one.
    throttle: Point,
    /// What its restart input was last held at: a new value other than
    /// nought starts it again.
    restart: f64,
    /// Its sky, as the records its `sky` output sends.
    sky: Vec<[u8; tcsprotocol::node::physical::lander::STAR_RECORD]>,
    /// The generation of its sky the ground last said it has whole: while
    /// it is [`SKY_GENERATION`], the sky is not sent.
    sky_quench: f64,
    /// What its `rated` input was last held at: a new value other than
    /// nought takes a rating.
    rated: f64,
    /// Its latest ratings, the latest first, no more than
    /// [`RATINGS_KEPT`], as the records its `ratings` output sends: see
    /// `tcsprotocol::node::physical::lander`'s "Ratings".
    ratings: std::collections::VecDeque<[u8; STAR_RECORD]>,
}

/// `h(u) = (1 - u) ln(1 - u) + u`, by its series where `u` is small: see
/// [`super::body`].
///
/// # Arguments
///
/// * `u` - The fraction of the mass lost, from nought up to but not one.
///
/// # Returns
///
/// `h(u)`.
fn h(u: f64) -> f64 {
    if u < 1e-4 {
        u * u * (0.5 + u * (1.0 / 6.0 + u / 12.0))
    } else {
        (1.0 - u) * (-u).ln_1p() + u
    }
}

/// Says whether two segments meet, touching at an end included.
///
/// # Arguments
///
/// * `a` - One segment's ends.
/// * `b` - The other's.
///
/// # Returns
///
/// Whether they meet.
fn meet(a: (Point, Point), b: (Point, Point)) -> bool {
    /// Which side of the line through `p` and `q` the point `r` is on:
    /// positive, negative, or nought on it.
    fn side(p: Point, q: Point, r: Point) -> f64 {
        (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
    }
    /// Whether `r`, on the line through `p` and `q`, is between them.
    ///
    /// # Arguments
    ///
    /// * `p` - One end of the line.
    /// * `q` - Its other end.
    /// * `r` - A point on the line through them.
    ///
    /// # Returns
    ///
    /// Whether `r` is inside the box `p` and `q` are corners of.
    fn within(p: Point, q: Point, r: Point) -> bool {
        r.0 >= p.0.min(q.0)
            && r.0 <= p.0.max(q.0)
            && r.1 >= p.1.min(q.1)
            && r.1 <= p.1.max(q.1)
    }
    let (p1, p2) = a;
    let (q1, q2) = b;
    let d1 = side(q1, q2, p1);
    let d2 = side(q1, q2, p2);
    let d3 = side(p1, p2, q1);
    let d4 = side(p1, p2, q2);
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    (d1 == 0.0 && within(q1, q2, p1))
        || (d2 == 0.0 && within(q1, q2, p2))
        || (d3 == 0.0 && within(p1, p2, q1))
        || (d4 == 0.0 && within(p1, p2, q2))
}

impl Lander {
    /// Makes a lander where its figures say it starts, flying, at rest,
    /// with a full tank and its throttles shut.
    ///
    /// # Arguments
    ///
    /// * `figures` - What the model says of it.
    ///
    /// # Returns
    ///
    /// The lander.
    pub fn new(figures: Resolved) -> Lander {
        Lander {
            at: figures.start,
            velocity: (0.0, 0.0),
            fuel: figures.fuel,
            flight: Flight::Flying,
            extent: figures.extent(),
            throttle: (0.0, 0.0),
            restart: 0.0,
            sky: super::sky::records(&figures, SKY_GENERATION),
            sky_quench: 0.0,
            rated: 0.0,
            ratings: std::collections::VecDeque::new(),
            figures,
        }
    }

    /// Starts it again: at its start, still, with a full tank, flying.
    fn start_again(&mut self) {
        self.at = self.figures.start;
        self.velocity = (0.0, 0.0);
        self.fuel = self.figures.fuel;
        self.flight = Flight::Flying;
    }

    /// Its thrust along each axis now, in newtons: nothing with an empty
    /// tank, or once it has crashed.
    ///
    /// # Returns
    ///
    /// The thrust along X and along Y.
    fn force(&self) -> Point {
        if self.fuel <= 0.0 || matches!(self.flight, Flight::Crashed { .. }) {
            return (0.0, 0.0);
        }
        (
            self.throttle.0 * self.figures.thrust_x,
            self.throttle.1 * self.figures.thrust_y,
        )
    }

    /// How fast it is speeding up now, along each axis: its thrust over its
    /// mass, and gravity on Y.
    ///
    /// # Returns
    ///
    /// Metres a second a second along X and along Y.
    fn acceleration(&self) -> Point {
        let mass = self.figures.dry_mass + self.fuel;
        let (fx, fy) = self.force();
        (fx / mass, fy / mass - self.figures.gravity)
    }

    /// The fuel it burns now, in kilograms a second.
    ///
    /// # Returns
    ///
    /// Its flow.
    fn flow(&self) -> f64 {
        let (fx, fy) = self.force();
        (fx.abs() + fy.abs())
            / (self.figures.isp.as_secs_f64() * STANDARD_GRAVITY)
    }

    /// Where it would be, and how fast, after a time in flight with the
    /// throttles it holds, its tank not running dry within the time.
    ///
    /// # Arguments
    ///
    /// * `t` - Seconds from now.
    ///
    /// # Returns
    ///
    /// Where it is and how fast it is going then.
    fn flown(&self, t: f64) -> (Point, Point) {
        let mass = self.figures.dry_mass + self.fuel;
        let q = self.flow();
        let u = q * t / mass;
        let g = self.figures.gravity;
        let (fx, fy) = self.force();
        // The push along one axis of a held force, on the falling mass.
        let pushed = |f: f64| -> (f64, f64) {
            if u < 1e-12 {
                (f * t * t / (2.0 * mass), f * t / mass)
            } else {
                (f * mass / (q * q) * h(u), -(f / q) * (-u).ln_1p())
            }
        };
        let (px, vx) = pushed(fx);
        let (py, vy) = pushed(fy);
        let ((x, y), (vx0, vy0)) = (self.at, self.velocity);
        (
            (x + vx0 * t + px, y + vy0 * t - g * t * t / 2.0 + py),
            (vx0 + vx, vy0 - g * t + vy),
        )
    }

    /// Its outline's segments, placed where it would be.
    ///
    /// # Arguments
    ///
    /// * `at` - Where the place between its feet would be.
    ///
    /// # Returns
    ///
    /// Each segment's ends.
    fn segments(&self, at: Point) -> Vec<(Point, Point)> {
        let placed = |(x, y): Point| (at.0 + x, at.1 + y);
        self.figures
            .outline
            .iter()
            .flat_map(|line| {
                line.windows(2)
                    .map(move |pair| (placed(pair[0]), placed(pair[1])))
            })
            .collect()
    }

    /// The ground's segments, by their place among them.
    ///
    /// # Returns
    ///
    /// Each segment's ends.
    fn ground(&self) -> impl Iterator<Item = (Point, Point)> + '_ {
        self.figures
            .ground
            .windows(2)
            .map(|pair| (pair[0], pair[1]))
    }

    /// The highest the ground is anywhere between two Xs, its bottom line
    /// counted beyond the display's edges.
    ///
    /// # Arguments
    ///
    /// * `from` - The left X, metres.
    /// * `to`   - The right X, metres.
    ///
    /// # Returns
    ///
    /// Metres.
    fn ground_top(&self, from: f64, to: f64) -> f64 {
        let ground = &self.figures.ground;
        let mut top = self.ground_at(from).max(self.ground_at(to));
        // The ground's own points between, found from the first at or past
        // `from`: the ground runs left to right.
        let first = ground.partition_point(|&(x, _)| x < from);
        for &(x, y) in &ground[first..] {
            if x > to {
                break;
            }
            top = top.max(y);
        }
        top
    }

    /// The height of the ground at an X: beyond either edge of the display,
    /// the display's bottom line, nought, which goes on indefinitely either
    /// way, so that a lander flown off a side comes down somewhere.
    ///
    /// # Arguments
    ///
    /// * `x` - Metres.
    ///
    /// # Returns
    ///
    /// Metres.
    fn ground_at(&self, x: f64) -> f64 {
        let ground = &self.figures.ground;
        if x < ground[0].0 || x > ground[ground.len() - 1].0 {
            return 0.0;
        }
        self.ground()
            .find(|&((x0, _), (x1, _))| x >= x0 && x <= x1)
            .map(|((x0, y0), (x1, y1))| {
                if x1 > x0 {
                    y0 + (y1 - y0) * (x - x0) / (x1 - x0)
                } else {
                    y0
                }
            })
            .unwrap_or(0.0)
    }

    /// Whether it would be down, where it would be: its outline meeting the
    /// ground, or any of it below the ground, which beyond the display's
    /// edges is its bottom line. Leaving the display is not coming down: it
    /// flies on out of sight, and may come back.
    ///
    /// # Arguments
    ///
    /// * `at` - Where the place between its feet would be.
    ///
    /// # Returns
    ///
    /// Whether it would be.
    fn down_at(&self, at: Point) -> bool {
        // Clear of the highest ground beneath it, as it is nearly all of a
        // flight, it is not down, and nothing more need be looked at.
        let (x0, x1, least_y, _) = self.extent;
        if at.1 + least_y > self.ground_top(at.0 + x0, at.0 + x1) {
            return false;
        }
        let segments = self.segments(at);
        let below = segments
            .iter()
            .flat_map(|&(p, q)| [p, q])
            .any(|(x, y)| y < self.ground_at(x));
        below || segments.iter().any(|&s| self.ground().any(|g| meet(s, g)))
    }

    /// Whether, coming down where it is now, it has landed: all of it over
    /// the pad, touching nothing but the pad, and slow enough.
    ///
    /// # Returns
    ///
    /// Whether it has.
    fn landed_now(&self) -> bool {
        let f = &self.figures;
        let (x0, x1, _, _) = f.extent();
        let ((pad0, pad_y), (pad1, _)) = f.pad_ends();
        let over = self.at.0 + x0 >= pad0 && self.at.0 + x1 <= pad1;
        let pad = (f.ground[f.pad], f.ground[f.pad + 1]);
        let segments = self.segments(self.at);
        let elsewhere = segments
            .iter()
            .any(|&s| self.ground().any(|g| g != pad && meet(s, g)));
        let below = segments
            .iter()
            .flat_map(|&(p, q)| [p, q])
            .any(|(_, y)| y < pad_y - INTO_THE_PAD);
        over && !elsewhere
            && !below
            && -self.velocity.1 <= f.landing_speed
            && self.velocity.0.abs() <= f.landing_drift
    }

    /// Whether, standing on the pad, its thrust up is greater than its
    /// weight, so that it lifts off.
    ///
    /// # Returns
    ///
    /// Whether it does.
    fn lifts(&self) -> bool {
        self.acceleration().1 > 0.0
    }

    /// Finds what happens next in flight, and when, with the throttles it
    /// holds.
    ///
    /// # Arguments
    ///
    /// * `now` - Where it is, in simulated seconds.
    ///
    /// # Returns
    ///
    /// When, in simulated seconds, and what.
    fn next(&self, now: f64) -> (f64, Next) {
        if let Flight::Crashed { since, .. } = self.flight {
            return (since + self.figures.pause.as_secs_f64(), Next::Again);
        }
        let flow = self.flow();
        let dry = (flow > 0.0).then(|| self.fuel / flow);
        let ahead = dry.map_or(LOOK_AHEAD, |dry| dry.min(LOOK_AHEAD));
        let mut before = 0.0;
        let mut t = LOOK_EVERY.min(ahead);
        loop {
            if self.down_at(self.flown(t).0) {
                // Somewhere in (before, t]: halve it until it is close.
                let (mut clear, mut hit) = (before, t);
                while hit - clear > FOUND_WITHIN {
                    let middle = (clear + hit) / 2.0;
                    if self.down_at(self.flown(middle).0) {
                        hit = middle;
                    } else {
                        clear = middle;
                    }
                }
                return (now + hit, Next::Down);
            }
            if t >= ahead {
                break;
            }
            before = t;
            t = (t + LOOK_EVERY).min(ahead);
        }
        match dry {
            Some(dry) if dry <= LOOK_AHEAD => (now + dry, Next::Dry),
            _ => (now + ahead, Next::Look),
        }
    }
}

impl Component for Lander {
    fn hold(&mut self, inputs: &[f64]) {
        self.throttle = throttles(inputs[THROTTLE_X], inputs[THROTTLE_Y]);
        // Driven to a new value other than nought, it starts again now,
        // however it is: the world has brought it to this moment first.
        let restart = inputs[RESTART];
        if restart != self.restart && restart != 0.0 {
            self.start_again();
        }
        self.restart = restart;
        self.sky_quench = inputs[SKY_QUENCH];
        // Driven to a new count, it takes the rating it is given, stamped
        // with the time of day, as the latest of those it sends.
        let rated = inputs[RATED];
        if rated != self.rated && rated != 0.0 {
            let when = tcsos::time::SystemTime::now()
                .duration_since(tcsos::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_millis() as u64);
            let record = RatingRecord {
                when,
                rating: inputs[RATING],
                autoland: inputs[RATING_AUTOLAND] > 0.5,
            };
            self.ratings.push_front(record.to_bytes());
            self.ratings.truncate(RATINGS_KEPT);
        }
        self.rated = rated;
    }

    fn advance(&mut self, from: f64, to: f64) {
        if matches!(self.flight, Flight::Crashed { .. }) || to <= from {
            return;
        }
        let t = to - from;
        let burnt = (self.flow() * t).min(self.fuel);
        // Standing on the pad it burns what the throttles say, held still
        // by the pad until it lifts off.
        if self.flight == Flight::Flying {
            let (at, velocity) = self.flown(t);
            self.at = at;
            self.velocity = velocity;
        }
        self.fuel = (self.fuel - burnt).max(0.0);
    }

    fn output(&self, terminal: usize) -> f64 {
        let decision = |b: bool| if b { 1.0 } else { 0.0 };
        match (terminal, self.flight) {
            (X, _) => self.at.0,
            (Y, _) => self.at.1,
            (VELOCITY_X, Flight::Crashed { velocity, .. }) => velocity.0,
            (VELOCITY_Y, Flight::Crashed { velocity, .. }) => velocity.1,
            (VELOCITY_X, _) => self.velocity.0,
            (VELOCITY_Y, _) => self.velocity.1,
            (FUEL, _) => self.fuel / self.figures.fuel,
            (LANDED, Flight::Landed { .. }) => decision(true),
            (CRASHED, Flight::Crashed { .. }) => decision(true),
            (ACCELERATION_X, Flight::Flying) => self.acceleration().0,
            (ACCELERATION_Y, Flight::Flying) => self.acceleration().1,
            (ACCELERATION_X, Flight::Crashed { acceleration, .. }) => {
                acceleration.0
            }
            (ACCELERATION_Y, Flight::Crashed { acceleration, .. }) => {
                acceleration.1
            }
            (SKY, _) => f64::from(SKY_GENERATION),
            _ => 0.0,
        }
    }

    fn records(
        &self,
        terminal: usize,
        from: usize,
        most: usize,
        out: &mut Vec<u8>,
    ) -> usize {
        if terminal == RATINGS {
            let total = self.ratings.len();
            for n in 0..most.min(total) {
                out.extend_from_slice(&self.ratings[(from + n) % total]);
            }
            return total;
        }
        let quenched = self.sky_quench == f64::from(SKY_GENERATION);
        if terminal != SKY || quenched || self.sky.is_empty() {
            return 0;
        }
        let total = self.sky.len();
        for n in 0..most.min(total) {
            out.extend_from_slice(&self.sky[(from + n) % total]);
        }
        total
    }

    fn next_event(&self, now: f64) -> Option<f64> {
        match self.flight {
            // On the pad, it lifts off now if its thrust is greater than its
            // weight, and otherwise stands until its throttles change or its
            // pad wait has run.
            Flight::Landed { since } => {
                if self.lifts() {
                    Some(now)
                } else {
                    Some(since + self.figures.pad_wait.as_secs_f64())
                }
            }
            _ => Some(self.next(now).0),
        }
    }

    fn event(&mut self, at: f64) {
        match self.flight {
            Flight::Crashed { since, .. }
                if at >= since + self.figures.pause.as_secs_f64() - 1e-9 =>
            {
                self.start_again();
            }
            Flight::Crashed { .. } => {}
            Flight::Landed { since } => {
                if self.lifts() {
                    self.flight = Flight::Flying;
                } else if at
                    >= since + self.figures.pad_wait.as_secs_f64() - 1e-9
                {
                    self.start_again();
                }
            }
            Flight::Flying => {
                if self.down_at(self.at) {
                    if self.landed_now() {
                        // Set down on the pad, standing on its lowest point.
                        let (_, _, least_y, _) = self.figures.extent();
                        let ((_, pad_y), _) = self.figures.pad_ends();
                        self.at.1 = pad_y - least_y;
                        self.flight = Flight::Landed { since: at };
                    } else {
                        self.flight = Flight::Crashed {
                            since: at,
                            velocity: self.velocity,
                            acceleration: self.acceleration(),
                        };
                    }
                    self.velocity = (0.0, 0.0);
                } else if self.fuel <= self.flow() * FOUND_WITHIN {
                    self.fuel = 0.0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcsos::time::Duration;
    use tcsprotocol::node::physical::lander::TERMINALS;

    /// A lander over flat ground with a pad in the middle: a square outline
    /// thirty metres a side.
    ///
    /// # Returns
    ///
    /// Its figures.
    fn figures() -> Resolved {
        Resolved {
            dry_mass: 6000.0,
            fuel: 2000.0,
            isp: Duration::from_secs(311),
            thrust_x: 12000.0,
            thrust_y: 24000.0,
            gravity: 1.62,
            start: (500.0, 300.0),
            width: 1000.0,
            height: 600.0,
            ground: vec![
                (0.0, 60.0),
                (470.0, 60.0),
                (530.0, 60.0),
                (1000.0, 60.0),
            ],
            pad: 1,
            outline: vec![vec![
                (-15.0, 0.0),
                (15.0, 0.0),
                (15.0, 30.0),
                (-15.0, 30.0),
                (-15.0, 0.0),
            ]],
            landing_speed: 3.05,
            landing_drift: 1.22,
            pause: Duration::from_secs(3),
            pad_wait: Duration::from_secs(10),
        }
    }

    /// Its inputs, as the world holds them: one for each terminal, the
    /// outputs' nought.
    ///
    /// # Arguments
    ///
    /// * `x`       - The X throttle.
    /// * `y`       - The Y throttle.
    /// * `restart` - The restart input.
    ///
    /// # Returns
    ///
    /// The inputs.
    fn held(x: f64, y: f64, restart: f64) -> Vec<f64> {
        let mut inputs = vec![0.0; TERMINALS.len()];
        inputs[THROTTLE_X] = x;
        inputs[THROTTLE_Y] = y;
        inputs[RESTART] = restart;
        inputs
    }

    /// A lander that has crashed stays down for its pause, and then starts
    /// again by itself; a restart starts it again at once, pause or not.
    #[test]
    fn a_crash_starts_again_after_its_pause() {
        let mut l = Lander::new(figures());
        let mut inputs = held(0.0, 0.0, 0.0);
        l.hold(&inputs);
        l.at = (100.0, 60.0);
        l.flight = Flight::Crashed {
            since: 1.0,
            velocity: (0.0, 0.0),
            acceleration: (0.0, 0.0),
        };
        assert_eq!(l.next_event(2.0), Some(4.0));
        l.event(3.9);
        assert!(matches!(l.flight, Flight::Crashed { .. }));
        l.event(4.0);
        assert_eq!(l.flight, Flight::Flying);
        assert_eq!(l.at, (500.0, 300.0));

        // Crashed again, a restart starts it at once.
        l.flight = Flight::Crashed {
            since: 200.0,
            velocity: (0.0, 0.0),
            acceleration: (0.0, 0.0),
        };
        inputs[RESTART] = 7.0;
        l.hold(&inputs);
        assert_eq!(l.flight, Flight::Flying);
    }

    /// The bottom engine, lit, burns at no less than its floor; off, it does
    /// not burn; and it pushes no lower than nought, nor beyond full.
    #[test]
    fn the_bottom_engine_has_a_floor() {
        let mut l = Lander::new(figures());
        for (asked, burns) in
            [(0.0, 0.0), (0.02, LEAST_THROTTLE), (0.5, 0.5), (2.0, 1.0)]
        {
            l.hold(&held(0.0, asked, 0.0));
            assert_eq!(l.throttle.1, burns, "{asked}");
        }
        l.hold(&held(0.0, -1.0, 0.0));
        assert_eq!(l.throttle.1, 0.0);
    }

    /// The sky goes a few records at a time, round and round, each record
    /// whole, until the ground quenches its generation; quenched with
    /// another generation, it goes on.
    #[test]
    fn the_sky_is_sent_until_it_is_quenched() {
        use tcsprotocol::node::physical::lander::StarRecord;
        let mut l = Lander::new(figures());
        let total = l.sky.len();
        assert!(total > 4);
        let mut out = Vec::new();
        assert_eq!(l.records(SKY, total - 2, 4, &mut out), total);
        assert_eq!(out.len(), 4 * STAR_RECORD);
        let ids: Vec<u16> = out
            .chunks(STAR_RECORD)
            .map(|r| StarRecord::from_bytes(r.try_into().unwrap()).id)
            .collect();
        let last = u16::try_from(total).unwrap();
        assert_eq!(ids, [last - 2, last - 1, 0, 1]);
        // No other terminal has records.
        assert_eq!(l.records(X, 0, 4, &mut Vec::new()), 0);
        let mut inputs = vec![0.0; TERMINALS.len()];
        inputs[SKY_QUENCH] = f64::from(SKY_GENERATION) + 1.0;
        l.hold(&inputs);
        assert_eq!(l.records(SKY, 0, 4, &mut Vec::new()), total);
        inputs[SKY_QUENCH] = f64::from(SKY_GENERATION);
        l.hold(&inputs);
        let mut out = Vec::new();
        assert_eq!(l.records(SKY, 0, 4, &mut out), 0);
        assert!(out.is_empty());
    }

    /// Driven to a new value other than nought, a lander starts again at
    /// once, however it is; held at the same value, or at nought, it does
    /// not.
    #[test]
    fn a_new_restart_starts_it_again() {
        let mut l = Lander::new(figures());
        // Down the full height and crashed beside the pad.
        l.at = (100.0, 60.0);
        l.velocity = (0.0, -30.0);
        l.fuel = 10.0;
        l.flight = Flight::Crashed {
            since: 1.0,
            velocity: (0.0, 0.0),
            acceleration: (0.0, 0.0),
        };
        l.hold(&held(0.0, 0.0, 1.0));
        assert_eq!(l.flight, Flight::Flying);
        assert_eq!(l.at, figures().start);
        assert_eq!(l.velocity, (0.0, 0.0));
        assert_eq!(l.fuel, figures().fuel);

        // The same value again, a throttle moving, starts nothing.
        l.at = (200.0, 200.0);
        l.hold(&held(0.5, 0.0, 1.0));
        assert_eq!(l.at, (200.0, 200.0));
        // Nor does nought.
        l.hold(&held(0.5, 0.0, 0.0));
        assert_eq!(l.at, (200.0, 200.0));
        // A new count does.
        l.hold(&held(0.0, 0.0, 2.0));
        assert_eq!(l.at, figures().start);
    }

    /// Runs a lander, as the world does, from one moment to another,
    /// letting each event it foresees happen.
    ///
    /// # Arguments
    ///
    /// * `lander` - It.
    /// * `from`   - Where it is, in simulated seconds.
    /// * `to`     - Where to bring it.
    fn run(lander: &mut Lander, from: f64, to: f64) {
        let mut now = from;
        while let Some(at) = lander.next_event(now).filter(|at| *at <= to) {
            lander.advance(now, at);
            lander.event(at);
            now = at;
        }
        lander.advance(now, to);
    }

    /// The places here are the type's.
    #[test]
    fn the_terminals_are_the_types() {
        let names: Vec<&str> = TERMINALS.iter().map(|t| t.name).collect();
        assert_eq!(names[THROTTLE_X], "throttle_x");
        assert_eq!(names[THROTTLE_Y], "throttle_y");
        assert_eq!(names[X], "x");
        assert_eq!(names[Y], "y");
        assert_eq!(names[VELOCITY_X], "velocity_x");
        assert_eq!(names[VELOCITY_Y], "velocity_y");
        assert_eq!(names[FUEL], "fuel");
        assert_eq!(names[LANDED], "landed");
        assert_eq!(names[CRASHED], "crashed");
        assert_eq!(names[ACCELERATION_X], "acceleration_x");
        assert_eq!(names[ACCELERATION_Y], "acceleration_y");
        assert_eq!(names[RESTART], "restart");
        assert_eq!(names[SKY], "sky");
        assert_eq!(names[SKY_QUENCH], "sky_quench");
        assert_eq!(names[RATING], "rating");
        assert_eq!(names[RATING_AUTOLAND], "rating_autoland");
        assert_eq!(names[RATED], "rated");
        assert_eq!(names[RATINGS], "ratings");
        assert_eq!(names.len(), RATINGS + 1);
    }

    /// Each new count driven to `rated` takes the rating and Autoland's
    /// help it is given, stamped with the time, as the latest of its
    /// ratings, which `ratings` sends the latest first; the same count
    /// again takes nothing, and it keeps no more than [`RATINGS_KEPT`].
    #[test]
    fn a_rating_is_kept_and_sent() {
        let mut l = Lander::new(figures());
        let mut inputs = held(0.0, 0.0, 0.0);
        let give = |l: &mut Lander, inputs: &mut Vec<f64>, rating, autoland| {
            inputs[RATING] = rating;
            inputs[RATING_AUTOLAND] = f64::from(u8::from(autoland));
            inputs[RATED] = inputs[RATED] % 255.0 + 1.0;
            l.hold(inputs);
        };
        assert_eq!(l.records(RATINGS, 0, 4, &mut Vec::new()), 0);
        give(&mut l, &mut inputs, 0.87, false);
        give(&mut l, &mut inputs, 0.52, true);
        let mut out = Vec::new();
        assert_eq!(l.records(RATINGS, 0, 4, &mut out), 2);
        let said: Vec<RatingRecord> = out
            .chunks(STAR_RECORD)
            .map(|c| RatingRecord::from_bytes(c.try_into().expect("whole")))
            .collect();
        assert_eq!((said[0].rating, said[0].autoland), (0.52, true));
        assert_eq!((said[1].rating, said[1].autoland), (0.87, false));
        assert!(said[0].when >= said[1].when && said[1].when > 0);

        // The same count again is no new rating.
        l.hold(&inputs);
        assert_eq!(l.ratings.len(), 2);
        for _ in 0..RATINGS_KEPT {
            give(&mut l, &mut inputs, 0.5, false);
        }
        assert_eq!(l.ratings.len(), RATINGS_KEPT);
    }

    /// Thrust accelerates it at its force over its mass, more as the tank
    /// empties, and burns fuel at the flow its specific impulse gives;
    /// gravity pulls it down whatever its mass.
    #[test]
    fn thrust_against_mass_and_gravity() {
        let mut l = Lander::new(figures());
        l.hold(&held(1.0, 0.0, 0.0));
        let flow = 12000.0 / (311.0 * STANDARD_GRAVITY);
        assert!((l.flow() - flow).abs() < 1e-12);
        l.advance(0.0, 1.0);
        // Faster than 12000 N on the full 8000 kg would make it, since the
        // mass fell.
        assert!(l.velocity.0 > 12000.0 / 8000.0, "{:?}", l.velocity);
        assert!(l.velocity.0 < 12000.0 / (8000.0 - flow), "{:?}", l.velocity);
        assert!((l.velocity.1 + 1.62).abs() < 1e-9, "{:?}", l.velocity);
        assert!((l.fuel - (2000.0 - flow)).abs() < 1e-9);
        assert!((l.output(FUEL) - l.fuel / 2000.0).abs() < 1e-12);
        // Its acceleration is its thrust over the mass left, and gravity.
        let mass = 6000.0 + l.fuel;
        assert!((l.output(ACCELERATION_X) - 12000.0 / mass).abs() < 1e-12);
        assert!((l.output(ACCELERATION_Y) + 1.62).abs() < 1e-12);
    }

    /// With the tank empty there is no thrust, whatever the throttles.
    #[test]
    fn an_empty_tank_gives_no_thrust() {
        let mut f = figures();
        f.fuel = 1.0;
        let mut l = Lander::new(f);
        l.hold(&held(0.0, 1.0, 0.0));
        run(&mut l, 0.0, 2.0);
        assert_eq!(l.fuel, 0.0);
        assert_eq!(l.force(), (0.0, 0.0));
        assert_eq!(l.flow(), 0.0);
    }

    /// Falling slowly onto the pad it lands, and stands there for its pad
    /// wait, and then starts again by itself, and not before.
    #[test]
    fn a_slow_touchdown_on_the_pad_lands() {
        let mut f = figures();
        f.start = (500.0, 62.0);
        let mut l = Lander::new(f);
        l.hold(&held(0.0, 0.0, 0.0));
        run(&mut l, 0.0, 2.0);
        assert_eq!(l.output(LANDED), 1.0);
        assert_eq!(l.output(CRASHED), 0.0);
        assert_eq!(l.at.1, 60.0);
        assert_eq!(l.output(FUEL), 1.0);

        // Landed again at 40 s: on the pad until 50 s, then started again.
        l.flight = Flight::Landed { since: 40.0 };
        assert_eq!(l.next_event(41.0), Some(50.0));
        run(&mut l, 41.0, 49.9);
        assert_eq!(l.output(LANDED), 1.0);
        run(&mut l, 49.9, 50.0);
        assert_eq!(l.flight, Flight::Flying);
        assert_eq!(l.at, (500.0, 62.0));
    }

    /// A lander that lifts off before its pad wait has run flies on, and is
    /// not started again; landing again begins a new wait.
    #[test]
    fn lifting_off_ends_the_pad_wait() {
        let mut l = Lander::new(figures());
        l.at = (500.0, 60.0);
        l.flight = Flight::Landed { since: 0.0 };
        let mut inputs = held(0.0, 1.0, 0.0);
        l.hold(&inputs);
        assert_eq!(l.next_event(5.0), Some(5.0));
        l.event(5.0);
        assert_eq!(l.flight, Flight::Flying);
        // Shut off at once, it settles back onto the pad, and waits again
        // from then.
        inputs[THROTTLE_Y] = 0.0;
        l.hold(&inputs);
        run(&mut l, 5.0, 12.0);
        let Flight::Landed { since } = l.flight else {
            panic!("{:?}", l.flight);
        };
        assert!(since > 5.0, "{since}");
        assert_eq!(l.next_event(12.0), Some(since + 10.0));
    }

    /// The quick look at the highest ground beneath it never says it is
    /// clear where the full test says it is down: compared at places all
    /// over and round the display, at the ground and just above it.
    #[test]
    fn the_quick_look_agrees_with_the_full_test() {
        let mut f = figures();
        f.ground = vec![
            (0.0, 60.0),
            (200.0, 140.0),
            (470.0, 60.0),
            (530.0, 60.0),
            (700.0, 220.0),
            (1000.0, 90.0),
        ];
        f.pad = 2;
        let l = Lander::new(f);
        let full = |at: Point| {
            let segments = l.segments(at);
            segments
                .iter()
                .flat_map(|&(p, q)| [p, q])
                .any(|(x, y)| y < l.ground_at(x))
                || segments.iter().any(|&s| l.ground().any(|g| meet(s, g)))
        };
        let mut x = -60.0;
        while x <= 1060.0 {
            let mut y = 0.0;
            while y <= 300.0 {
                assert_eq!(l.down_at((x, y)), full((x, y)), "at {x}, {y}");
                y += 2.5;
            }
            x += 7.0;
        }
    }

    /// Standing on the pad, thrust no greater than its weight leaves it
    /// there, burning fuel; thrust greater lifts it off, and it flies on,
    /// no longer landed, until it comes down again.
    #[test]
    fn a_landed_lander_lifts_off() {
        let mut l = Lander::new(figures());
        l.at = (500.0, 60.0);
        l.flight = Flight::Landed { since: 0.0 };
        // Half of 24 kN is less than 8000 kg's weight, 12.96 kN.
        l.hold(&held(0.0, 0.5, 0.0));
        // Not lifting, its next event is the end of its pad wait.
        assert_eq!(l.next_event(1.0), Some(10.0));
        l.advance(1.0, 2.0);
        assert_eq!(l.flight, Flight::Landed { since: 0.0 });
        assert_eq!(l.at, (500.0, 60.0));
        assert!(l.output(FUEL) < 1.0);
        assert_eq!(l.output(ACCELERATION_Y), 0.0);

        // Full thrust lifts it off, now.
        l.hold(&held(0.0, 1.0, 0.0));
        assert_eq!(l.next_event(2.0), Some(2.0));
        l.event(2.0);
        assert_eq!(l.flight, Flight::Flying);
        assert_eq!(l.output(LANDED), 0.0);
        run(&mut l, 2.0, 3.0);
        assert!(l.at.1 > 60.0, "{:?}", l.at);
        assert!(l.velocity.1 > 0.0, "{:?}", l.velocity);

        // Shut off, it falls back onto the pad, slowly enough to land.
        l.hold(&held(0.0, 0.0, 0.0));
        run(&mut l, 3.0, 10.0);
        assert!(matches!(l.flight, Flight::Landed { .. }));
        assert_eq!(l.output(CRASHED), 0.0);
    }

    /// Too fast down, it crashes even on the pad: falling 240 m it is
    /// going 27.9 m/s at 17.2 s.
    #[test]
    fn a_fast_touchdown_crashes() {
        let mut l = Lander::new(figures());
        run(&mut l, 0.0, 18.0);
        assert_eq!(l.output(CRASHED), 1.0);
        assert_eq!(l.output(LANDED), 0.0);
    }

    /// Crashed, it reports the velocity and acceleration it hit with until
    /// it starts again, so that the last values sent down are those: not
    /// the nought it stands still at.
    #[test]
    fn a_crash_reports_what_it_hit_with() {
        let mut l = Lander::new(figures());
        l.hold(&held(0.5, 0.0, 0.0));
        run(&mut l, 0.0, 18.0);
        assert_eq!(l.output(CRASHED), 1.0);
        let Flight::Crashed {
            velocity,
            acceleration,
            ..
        } = l.flight
        else {
            panic!("crashed, but not said to be")
        };
        assert!(velocity.1 < -20.0, "it fell, at {velocity:?}");
        assert!(velocity.0 > 0.0, "pushed across, at {velocity:?}");
        assert!(acceleration.0 > 0.0, "pushed across: {acceleration:?}");
        assert_eq!(acceleration.1, -l.figures.gravity);
        assert_eq!(l.velocity, (0.0, 0.0), "it stands still");
        let reported = [VELOCITY_X, VELOCITY_Y, ACCELERATION_X, ACCELERATION_Y]
            .map(|terminal| l.output(terminal));
        assert_eq!(
            reported,
            [velocity.0, velocity.1, acceleration.0, acceleration.1]
        );
        run(&mut l, 18.0, 18.5);
        assert_eq!(l.output(VELOCITY_Y), velocity.1, "still, while down");
    }

    /// Off the pad, it crashes however slowly it comes down.
    #[test]
    fn a_touchdown_off_the_pad_crashes() {
        let mut f = figures();
        f.start = (200.0, 62.0);
        let mut l = Lander::new(f);
        run(&mut l, 0.0, 2.0);
        assert_eq!(l.output(CRASHED), 1.0);
    }

    /// Drifting sideways faster than the limit, it crashes on the pad.
    #[test]
    fn drifting_too_fast_crashes() {
        let mut f = figures();
        f.start = (495.0, 62.0);
        let mut l = Lander::new(f);
        l.velocity = (2.0, 0.0);
        run(&mut l, 0.0, 2.0);
        assert_eq!(l.output(CRASHED), 1.0);
    }

    /// Flying off the display is not coming down: it flies on out of sight,
    /// and comes down where it drops below the display's bottom line,
    /// which goes on indefinitely beyond the edge.
    #[test]
    fn leaving_the_display_flies_on() {
        let mut f = figures();
        f.start = (980.0, 400.0);
        let mut l = Lander::new(f);
        l.velocity = (40.0, 0.0);
        run(&mut l, 0.0, 3.0);
        assert_eq!(l.output(CRASHED), 0.0);
        assert_eq!(l.flight, Flight::Flying);
        assert!(l.at.0 - 15.0 > 1000.0, "{:?}", l.at);
        // Out there it falls until it drops to the bottom line, and crashes:
        // looked at a step at a time, so as to see it before it starts
        // again.
        let mut now = 3.0;
        while l.output(CRASHED) == 0.0 && now < 60.0 {
            run(&mut l, now, now + 0.1);
            now += 0.1;
        }
        assert_eq!(l.output(CRASHED), 1.0, "{:?} at {now}", l.at);
        assert!(l.at.1.abs() < 0.5, "{:?}", l.at);
    }

    /// Two segments meet where they cross or one ends on the other, and
    /// not where they are apart or only in line.
    #[test]
    fn segments_meet_where_they_should() {
        let s = |a: Point, b: Point| (a, b);
        assert!(meet(s((0.0, 0.0), (2.0, 2.0)), s((0.0, 2.0), (2.0, 0.0))));
        assert!(meet(s((0.0, 0.0), (2.0, 0.0)), s((1.0, 0.0), (1.0, 3.0))));
        assert!(!meet(s((0.0, 0.0), (1.0, 0.0)), s((2.0, 0.0), (3.0, 0.0))));
        assert!(!meet(s((0.0, 0.0), (1.0, 1.0)), s((0.0, 1.0), (0.4, 0.9))));
    }
}
