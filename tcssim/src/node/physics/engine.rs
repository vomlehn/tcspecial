//! An engine: thrust in proportion to its throttle, while its fuel lasts.
//!
//! Its terminals are `tcslibgs::node::physics::engine`'s: `throttle`, a ratio;
//! `force`, in newtons; and `flow`, the fuel it burns, in kilograms a
//! second.
//!
//! With its throttle held its fuel falls in a straight line, which is exact,
//! and reaches nought at a moment it can say: its next event. From then on
//! it gives out nothing.

use tcslibgs::node::physics::engine::Resolved;

use super::component::Component;

/// Its terminals' places, in `tcslibgs::node::physics::engine::TERMINALS`'s
/// order, which a test holds this to.
const THROTTLE: usize = 0;
const FORCE: usize = 1;
const FLOW: usize = 2;

/// Standard gravity, which turns a specific impulse in seconds into an
/// exhaust velocity: a definition, not the gravity of anywhere in
/// particular.
const STANDARD_GRAVITY: f64 = 9.80665;

/// An engine of the world.
#[derive(Debug)]
pub struct Engine {
    figures: Resolved,
    /// Its throttle, as last held, kept within nought and one.
    throttle: f64,
    /// The fuel it has left, in kilograms.
    fuel: f64,
}

impl Engine {
    /// Makes an engine with its fuel, its throttle shut.
    ///
    /// # Arguments
    ///
    /// * `figures` - What the model says of it.
    ///
    /// # Returns
    ///
    /// The engine.
    pub fn new(figures: Resolved) -> Engine {
        Engine {
            fuel: figures.fuel,
            throttle: 0.0,
            figures,
        }
    }

    /// The thrust it gives now, in newtons: nothing once its fuel is gone.
    ///
    /// # Returns
    ///
    /// Its thrust.
    fn force(&self) -> f64 {
        if self.fuel > 0.0 {
            self.figures.thrust * self.throttle
        } else {
            0.0
        }
    }

    /// The fuel it burns now, in kilograms a second.
    ///
    /// # Returns
    ///
    /// Its flow.
    fn flow(&self) -> f64 {
        self.force() / (self.figures.isp.as_secs_f64() * STANDARD_GRAVITY)
    }
}

impl Component for Engine {
    fn hold(&mut self, inputs: &[f64]) {
        // A throttle is a valve, which opens no further than fully and
        // closes no further than shut, whatever it is told.
        self.throttle = inputs[THROTTLE].clamp(0.0, 1.0);
    }

    fn advance(&mut self, from: f64, to: f64) {
        self.fuel = (self.fuel - self.flow() * (to - from)).max(0.0);
    }

    fn output(&self, terminal: usize) -> f64 {
        match terminal {
            FORCE => self.force(),
            FLOW => self.flow(),
            _ => 0.0,
        }
    }

    fn next_event(&self, now: f64) -> Option<f64> {
        let flow = self.flow();
        (flow > 0.0).then(|| now + self.fuel / flow)
    }

    fn event(&mut self, _at: f64) {
        // Out of fuel, give or take the last of the arithmetic.
        self.fuel = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tcslibgs::node::physics::engine::TERMINALS;

    /// The places here are the type's.
    #[test]
    fn the_terminals_are_the_types() {
        assert_eq!(TERMINALS[THROTTLE].name, "throttle");
        assert_eq!(TERMINALS[FORCE].name, "force");
        assert_eq!(TERMINALS[FLOW].name, "flow");
    }

    /// Half throttle is half its thrust and half its flow; its fuel lasts
    /// as long as the flow says, and then it gives nothing.
    #[test]
    fn it_burns_its_fuel_and_stops() {
        let mut engine = Engine::new(Resolved {
            thrust: 9806.65,
            isp: Duration::from_secs(100),
            fuel: 50.0,
        });
        engine.hold(&[0.5, 0.0, 0.0]);
        assert!((engine.output(FORCE) - 4903.325).abs() < 1e-9);
        assert!((engine.output(FLOW) - 5.0).abs() < 1e-12);
        let empty = engine.next_event(0.0).expect("it runs dry");
        assert!((empty - 10.0).abs() < 1e-9, "{empty}");
        engine.advance(0.0, empty);
        engine.event(empty);
        assert_eq!(engine.output(FORCE), 0.0);
        assert_eq!(engine.output(FLOW), 0.0);
        assert_eq!(engine.next_event(empty), None);
    }

    /// A throttle past its stops is held at them.
    #[test]
    fn its_throttle_is_held_within_its_stops() {
        let mut engine = Engine::new(Resolved {
            thrust: 1000.0,
            isp: Duration::from_secs(300),
            fuel: 1.0,
        });
        engine.hold(&[1.7, 0.0, 0.0]);
        assert_eq!(engine.output(FORCE), 1000.0);
        engine.hold(&[-0.2, 0.0, 0.0]);
        assert_eq!(engine.output(FORCE), 0.0);
    }
}
