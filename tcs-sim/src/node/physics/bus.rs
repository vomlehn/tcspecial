//! A power bus: what the sources supply less what the loads demand.
//!
//! Its terminals are `tcsprotocol::node::physics::bus`'s: `supply` and
//! `demand`, each the sum of what is joined to it, and `current`, what is left
//! over, all in amperes.

use super::component::Component;

/// Its terminals' places, in `tcsprotocol::node::physics::bus::TERMINALS`'s
/// order, which a test holds this to.
const SUPPLY: usize = 0;
const DEMAND: usize = 1;
const CURRENT: usize = 2;

/// A bus of the world.
#[derive(Debug, Default)]
pub struct Bus {
    /// What its sources supply and its loads demand, as last held.
    supply: f64,
    demand: f64,
}

impl Component for Bus {
    fn hold(&mut self, inputs: &[f64]) {
        self.supply = inputs[SUPPLY];
        self.demand = inputs[DEMAND];
    }

    fn advance(&mut self, _from: f64, _to: f64) {}

    fn output(&self, terminal: usize) -> f64 {
        match terminal {
            CURRENT => self.supply - self.demand,
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcsprotocol::node::physics::bus::TERMINALS;

    /// The places here are the type's; what is left over is the supply
    /// less the demand, either way.
    #[test]
    fn what_is_left_over_is_supply_less_demand() {
        assert_eq!(TERMINALS[SUPPLY].name, "supply");
        assert_eq!(TERMINALS[DEMAND].name, "demand");
        assert_eq!(TERMINALS[CURRENT].name, "current");
        let mut bus = Bus::default();
        bus.hold(&[7.0, 4.0, 0.0]);
        assert_eq!(bus.output(CURRENT), 3.0);
        bus.hold(&[0.0, 4.0, 0.0]);
        assert_eq!(bus.output(CURRENT), -4.0);
    }
}
