//! A load: a current drawn while it is on, nothing while it is shed.
//!
//! Its terminals are `tcslibgs::physics::load`'s: `shed`, a decision, and
//! `current`, in amperes.

use tcslibgs::physics::load::Resolved;

use super::element::Element;

/// Its terminals' places, in `tcslibgs::physics::load::TERMINALS`'s order,
/// which a test holds this to.
const SHED: usize = 0;
const CURRENT: usize = 1;

/// A load of the world.
#[derive(Debug)]
pub struct Load {
    figures: Resolved,
    /// Whether it is shed, as last held.
    shed: bool,
}

impl Load {
    /// Makes a load, on.
    ///
    /// # Arguments
    ///
    /// * `figures` - What the model says of it.
    ///
    /// # Returns
    ///
    /// The load.
    pub fn new(figures: Resolved) -> Load {
        Load {
            figures,
            shed: false,
        }
    }
}

impl Element for Load {
    fn hold(&mut self, inputs: &[f64]) {
        self.shed = inputs[SHED] > 0.5;
    }

    fn advance(&mut self, _from: f64, _to: f64) {}

    fn output(&self, terminal: usize) -> f64 {
        match terminal {
            CURRENT if !self.shed => self.figures.current,
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::physics::load::TERMINALS;

    /// The places here are the type's; it draws its current until it is
    /// shed.
    #[test]
    fn it_draws_until_it_is_shed() {
        assert_eq!(TERMINALS[SHED].name, "shed");
        assert_eq!(TERMINALS[CURRENT].name, "current");
        let mut load = Load::new(Resolved { current: 2.0 });
        assert_eq!(load.output(CURRENT), 2.0);
        load.hold(&[1.0, 0.0]);
        assert_eq!(load.output(CURRENT), 0.0);
        load.hold(&[0.0, 0.0]);
        assert_eq!(load.output(CURRENT), 2.0);
    }
}
