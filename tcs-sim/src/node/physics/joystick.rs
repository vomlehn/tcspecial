//! A joystick: two axes, set by hand from tcs-sim's window, given out for
//! the vehicle to read.
//!
//! Its terminals are `tcsprotocol::node::physics::joystick`'s: `set_x` and
//! `set_y`, which the window drives as it drives any input, and `x` and
//! `y`, which give them out, each held within minus one and one.

use super::component::Component;

/// Its terminals' places, in
/// `tcsprotocol::node::physics::joystick::TERMINALS`'s order, which a test
/// holds this to.
const SET_X: usize = 0;
const SET_Y: usize = 1;
const X: usize = 2;
const Y: usize = 3;

/// A joystick of the world.
#[derive(Debug, Default)]
pub struct Joystick {
    /// Its axes, as last set, each within minus one and one.
    x: f64,
    y: f64,
}

impl Component for Joystick {
    fn hold(&mut self, inputs: &[f64]) {
        self.x = inputs[SET_X].clamp(-1.0, 1.0);
        self.y = inputs[SET_Y].clamp(-1.0, 1.0);
    }

    fn advance(&mut self, _from: f64, _to: f64) {}

    fn output(&self, terminal: usize) -> f64 {
        match terminal {
            X => self.x,
            Y => self.y,
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcsprotocol::node::physics::joystick::TERMINALS;

    /// The places here are the type's.
    #[test]
    fn the_terminals_are_the_types() {
        assert_eq!(TERMINALS[SET_X].name, "set_x");
        assert_eq!(TERMINALS[SET_Y].name, "set_y");
        assert_eq!(TERMINALS[X].name, "x");
        assert_eq!(TERMINALS[Y].name, "y");
    }

    /// What it is set to is what it gives out, held within its stops.
    #[test]
    fn it_gives_out_what_it_is_set_to() {
        let mut stick = Joystick::default();
        stick.hold(&[0.25, -1.5, 0.0, 0.0]);
        assert_eq!(stick.output(X), 0.25);
        assert_eq!(stick.output(Y), -1.0);
    }
}
