//! What every component of the world does, whatever its type.
//!
//! A component is a part simulated alone: it knows its own state and its
//! own arithmetic, and nothing of what it is joined to. The world joins
//! them: it gives each component what is at its inputs, advances them all
//! together, and reads their outputs for whatever they are joined to. So a
//! heater is written once, knowing only that it is switched and gives out
//! heat, and warms a zone, a tank or a battery by being joined to it.
//!
//! # How a component is advanced
//!
//! The world holds each component's inputs steady from one moment to the
//! next -- from one event, or one question a stand-in asks, to the next --
//! and a component advances across the interval as exactly as its arithmetic
//! allows: by a closed form where there is one, which is what makes the
//! answer independent of how often the world is looked at. Where a
//! component's behaviour changes within an interval -- a threshold crossed --
//! it says when, as its next event, and the world stops there.

/// The most terminals a component type has: the room each component's inputs
/// are given, which is fixed so that holding them allocates nothing.
pub const MOST_TERMINALS: usize = 8;

/// A component of the world.
///
/// Its terminals are its type's, in the order its type in
/// `tcslibgs::node::physics` lists them; every slice of values here is indexed
/// by them, an output's place holding nothing a component reads.
pub trait Component: Send + std::fmt::Debug {
    /// Takes what is at its inputs, which holds until it is given them
    /// again.
    ///
    /// # Arguments
    ///
    /// * `inputs` - One value for each of its type's terminals: what is at
    ///              each input, nought for an output. A value is a number
    ///              in its terminal's unit; a decision is one for on and
    ///              nought for off.
    fn hold(&mut self, inputs: &[f64]);

    /// Advances it across an interval, with the inputs it holds.
    ///
    /// # Arguments
    ///
    /// * `from` - Where it is, in simulated seconds.
    /// * `to`   - Where to bring it, no earlier than `from`.
    fn advance(&mut self, from: f64, to: f64);

    /// Says what one of its outputs is.
    ///
    /// # Arguments
    ///
    /// * `terminal` - Which, by its place among its type's terminals.
    ///
    /// # Returns
    ///
    /// Its value in its terminal's unit, as of the last advance and the
    /// inputs last held; nought for a terminal that is not an output.
    fn output(&self, terminal: usize) -> f64;

    /// Says when its behaviour next changes in a way an advance cannot step
    /// across, with the inputs it holds.
    ///
    /// # Arguments
    ///
    /// * `now` - Where it is, in simulated seconds.
    ///
    /// # Returns
    ///
    /// When, in simulated seconds, or `None` for nothing it can foresee.
    /// Most components have none: their arithmetic is one closed form
    /// whatever their state.
    fn next_event(&self, now: f64) -> Option<f64> {
        let _ = now;
        None
    }

    /// Lets the event it foresaw happen, having been advanced to it.
    ///
    /// # Arguments
    ///
    /// * `at` - When, in simulated seconds.
    fn event(&mut self, at: f64) {
        let _ = at;
    }
}
