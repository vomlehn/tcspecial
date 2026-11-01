//! The world: every element of the physics model, joined as the model
//! joins them, and advanced together from event to event.
//!
//! # How time moves
//!
//! Nothing ticks. The world is at some moment of simulated time, and is
//! brought up to another only when something needs it there: a stand-in
//! reading an output, or driving an input. Between the two it advances
//! every element across the interval with its inputs held -- by each
//! element's closed form, so the answer does not depend on how often it is
//! asked -- except where an element has foreseen an event inside the
//! interval: then the world advances to the event, lets it happen, gives
//! every element its inputs again, and goes on. The events wait in a
//! [`Queue`], soonest first, and each element's is set again whenever its
//! inputs change, which is when what it foresees can change.
//!
//! So the world follows the wall clock -- each stand-in asks at the wall
//! clock's moment, through the [`Clock`] -- at no cost while nothing asks,
//! and at whatever rate the clock is given.
//!
//! # Inputs
//!
//! What is at an input is the sum of every output joined to it, for an
//! input that sums, or the one output joined to it, or what a stand-in last
//! drove it to: the model's check has made it one of those. They are worked
//! out from every element's outputs as they stand, and held, at the start
//! of each interval: an element's output is what the interval starts with.
//!
//! # Order
//!
//! Nothing the world does follows the order a model's file is written in.
//! Its elements are in name order and its connections in the order of the
//! names of what they join, as `tcslibgs::physics::Model` holds them, so the
//! outputs an input sums are added in one order, and of two events due at
//! one moment the element whose name sorts first has its event first: see
//! [`Queue`].
//!
//! # Sharing
//!
//! Every stand-in's thread asks the one world, so its state is behind a
//! lock, taken for an advance and never across anything that waits.
//! Nothing is allocated once it is made: the inputs, the driven values and
//! the queue have their room from the start.

use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Instant;

use tcslibgs::physics::terminal::{Direction, Terminal};
use tcslibgs::physics::{Joined, Model, Place, Resolution};

use super::battery::Battery;
use super::body::Body;
use super::bus::Bus;
use super::clock::Clock;
use super::element::{Element, MOST_TERMINALS};
use super::engine::Engine;
use super::heater::Heater;
use super::load::Load;
use super::queue::Queue;
use super::solar_array::SolarArray;
use super::sun::Sun;
use super::thermal_zone::ThermalZone;

/// The most events one advance lets happen before it stops, with a word in
/// the log.
///
/// An element that foresaw its next event at the moment it was already at,
/// time after time, would hold the world's lock for ever, and every
/// stand-in with it. That is a fault in the element, and the simulator
/// carrying on with the world a little behind is better than one that
/// stops answering: what the log says is where to look.
const MOST_EVENTS_AT_ONCE: usize = 10_000;

/// The world.
#[derive(Debug)]
pub struct World {
    clock: Clock,
    /// Each element's name, for what the log says.
    names: Vec<String>,
    /// Each element's type's terminals.
    terminals: Vec<&'static [Terminal]>,
    /// The model's connections.
    connections: Vec<Joined>,
    state: Mutex<State>,
    /// Told whenever a stand-in drives an input, which can bring an event
    /// sooner than a waiting stand-in was waiting for: see
    /// [`World::await_change`].
    driven: Condvar,
}

/// What changes as the world runs.
#[derive(Debug)]
struct State {
    /// Where the world is, in simulated seconds.
    now: f64,
    elements: Vec<Box<dyn Element>>,
    /// What each stand-in last drove each input to, by element and
    /// terminal: nought until it does.
    driven: Vec<[f64; MOST_TERMINALS]>,
    /// What is at each element's inputs, as last held.
    inputs: Vec<[f64; MOST_TERMINALS]>,
    /// What is at them now, worked out to be compared with what was held.
    given: Vec<[f64; MOST_TERMINALS]>,
    queue: Queue,
}

impl World {
    /// Makes the world of a model.
    ///
    /// # Arguments
    ///
    /// * `model` - The physics model, resolved and checked.
    /// * `clock` - How the wall clock maps to its time.
    /// * `seed`  - The run's seed, from which every element's draws are
    ///             seeded.
    ///
    /// # Returns
    ///
    /// The world at simulated time nought, every element as it starts and
    /// every input at nought.
    pub fn new(model: &Model, clock: Clock, seed: u64) -> World {
        let parts = model
            .elements
            .iter()
            .map(|element| {
                let made: Box<dyn Element> = match &element.what {
                    Resolution::Heater(figures) => {
                        Box::new(Heater::new(&element.name, *figures, seed))
                    }
                    Resolution::ThermalZone(figures) => Box::new(
                        ThermalZone::new(&element.name, *figures, seed),
                    ),
                    Resolution::Sun(figures) => Box::new(Sun::new(*figures)),
                    Resolution::SolarArray(figures) => {
                        Box::new(SolarArray::new(*figures))
                    }
                    Resolution::Load(figures) => Box::new(Load::new(*figures)),
                    Resolution::Bus => Box::new(Bus::default()),
                    Resolution::Battery(figures) => {
                        Box::new(Battery::new(*figures))
                    }
                    Resolution::Engine(figures) => {
                        Box::new(Engine::new(*figures))
                    }
                    Resolution::Body(figures) => Box::new(Body::new(*figures)),
                };
                (element.name.clone(), element.what.terminals(), made)
            })
            .collect();
        World::of(clock, parts, model.connections.clone())
    }

    /// Makes a world of elements already made.
    ///
    /// # Arguments
    ///
    /// * `clock`       - How the wall clock maps to its time.
    /// * `parts`       - Each element's name, its type's terminals and the
    ///                   element.
    /// * `connections` - How they are joined.
    ///
    /// # Returns
    ///
    /// The world at simulated time nought.
    ///
    /// # Panics
    ///
    /// If a type has more terminals than [`MOST_TERMINALS`], which a test
    /// holds every type to.
    fn of(
        clock: Clock,
        parts: Vec<(String, &'static [Terminal], Box<dyn Element>)>,
        connections: Vec<Joined>,
    ) -> World {
        let count = parts.len();
        let mut names = Vec::with_capacity(count);
        let mut terminals = Vec::with_capacity(count);
        let mut elements = Vec::with_capacity(count);
        for (name, of_type, element) in parts {
            assert!(of_type.len() <= MOST_TERMINALS, "{name}'s terminals");
            names.push(name);
            terminals.push(of_type);
            elements.push(element);
        }
        let world = World {
            clock,
            names,
            terminals,
            connections,
            driven: Condvar::new(),
            state: Mutex::new(State {
                now: 0.0,
                elements,
                driven: vec![[0.0; MOST_TERMINALS]; count],
                inputs: vec![[0.0; MOST_TERMINALS]; count],
                given: vec![[0.0; MOST_TERMINALS]; count],
                queue: Queue::new(count),
            }),
        };
        {
            // Every element given its inputs once, all nought, before they
            // are worked out from what it is joined to: `hold` gives them
            // again only where they change.
            let mut state = world.held();
            for (n, element) in state.elements.iter_mut().enumerate() {
                element
                    .hold(&[0.0; MOST_TERMINALS][..world.terminals[n].len()]);
            }
            world.hold(&mut state);
        }
        world
    }

    /// Says what simulated time a moment of the wall clock is, by this
    /// world's clock.
    pub fn seconds(&self, now: Instant) -> f64 {
        self.clock.at(now)
    }

    /// Says what a terminal is called.
    ///
    /// # Arguments
    ///
    /// * `place` - A terminal of this world.
    ///
    /// # Returns
    ///
    /// Its element's name and its own, as a file writes them.
    pub fn name_of(&self, place: Place) -> String {
        format!(
            "{}.{}",
            self.names[place.element],
            self.terminals[place.element][place.terminal].name
        )
    }

    /// Says what an output is at a moment of the wall clock, bringing the
    /// world up to it.
    pub fn read(&self, place: Place, now: Instant) -> f64 {
        self.read_at(place, self.clock.at(now))
    }

    /// Drives an input to a value at a moment of the wall clock, bringing
    /// the world up to it first.
    pub fn drive(&self, place: Place, value: f64, now: Instant) {
        self.drive_at(place, value, self.clock.at(now));
    }

    /// Says what an output is at a simulated time, bringing the world up to
    /// it.
    ///
    /// # Arguments
    ///
    /// * `place`   - The output.
    /// * `seconds` - The simulated time; one the world has passed -- a
    ///               stand-in whose moment was taken before another's, and
    ///               which reached the lock after it -- is answered as the
    ///               world is now.
    ///
    /// # Returns
    ///
    /// Its value, in its terminal's unit.
    pub fn read_at(&self, place: Place, seconds: f64) -> f64 {
        let mut state = self.held();
        self.advance_to(&mut state, seconds);
        state.elements[place.element].output(place.terminal)
    }

    /// Drives an input to a value at a simulated time, bringing the world
    /// up to it first.
    ///
    /// The world is advanced first, so what was held applies up to this
    /// moment and the new value from here: a heater switched on halfway
    /// through a second warms its zone for half of it.
    ///
    /// # Arguments
    ///
    /// * `place`   - The input, which no connection joins: the model's
    ///               check has made it a stand-in's.
    /// * `value`   - What it is driven to: one for on and nought for off,
    ///               for a decision.
    /// * `seconds` - The simulated time, as [`World::read_at`] takes it.
    pub fn drive_at(&self, place: Place, value: f64, seconds: f64) {
        debug_assert_eq!(
            self.terminals[place.element][place.terminal].direction,
            Direction::In
        );
        let mut state = self.held();
        self.advance_to(&mut state, seconds);
        state.driven[place.element][place.terminal] = value;
        self.hold(&mut state);
        drop(state);
        self.driven.notify_all();
    }

    /// Waits for an output to change from what it was, and says what it
    /// changed to.
    ///
    /// What a stand-in that signals a decision waits in, so that the vehicle
    /// hears of the change -- a lander's contact with the ground -- at the
    /// moment it happens, and not at the stand-in's next turn to send. The
    /// world is advanced to each event it foresees at that event's moment on
    /// the wall clock, unasked by anything else: a change comes at an event,
    /// or at a stand-in's drive, which wakes the wait too.
    ///
    /// # Arguments
    ///
    /// * `place` - The output.
    /// * `from`  - What it was, as the stand-in last said it.
    /// * `until` - When to stop waiting, on the wall clock.
    ///
    /// # Returns
    ///
    /// What it changed to, or `None` where it had not by `until`.
    pub fn await_change(
        &self,
        place: Place,
        from: f64,
        until: Instant,
    ) -> Option<f64> {
        let mut state = self.held();
        loop {
            let now = Instant::now();
            self.advance_to(&mut state, self.clock.at(now));
            let is = state.elements[place.element].output(place.terminal);
            if is != from {
                return Some(is);
            }
            if now >= until {
                return None;
            }
            let wake = state
                .queue
                .first()
                .map_or(until, |(at, _)| self.clock.instant_of(at).min(until));
            let (held, _) = self
                .driven
                .wait_timeout(state, wake.saturating_duration_since(now))
                .unwrap_or_else(|e| e.into_inner());
            state = held;
        }
    }

    /// The lock, taken through one place so a poisoned one is dealt with
    /// once.
    ///
    /// A poisoned lock means a thread panicked holding the state, which is
    /// a fault to find in the log rather than a reason for a simulator to
    /// stop answering: the worst in it is a world not advanced.
    fn held(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Gives every element what is at its inputs, as every output stands,
    /// and sets when each one's next event is.
    ///
    /// An element's outputs can follow from what it holds -- a heater's
    /// heat from whether it is on -- so what one is given changes what the
    /// next is given, and a heater switched on is a zone warmed from that
    /// moment and not from the next one. So inputs are given again until
    /// they settle: once more than the longest chain of elements there can
    /// be, which is the number of them, since the model's connections go
    /// from each element's outputs to another's inputs and a chain without a
    /// loop visits each element once.
    ///
    /// # Arguments
    ///
    /// * `state` - The world's state, held locked.
    fn hold(&self, state: &mut State) {
        let State {
            now,
            elements,
            driven,
            inputs,
            given,
            queue,
        } = state;
        for _ in 0..=elements.len() {
            given.copy_from_slice(driven);
            for joined in &self.connections {
                let out =
                    elements[joined.from.element].output(joined.from.terminal);
                given[joined.to.element][joined.to.terminal] += out;
            }
            if given == inputs {
                break;
            }
            inputs.copy_from_slice(given);
            for (n, element) in elements.iter_mut().enumerate() {
                element.hold(&inputs[n][..self.terminals[n].len()]);
            }
        }
        for (n, element) in elements.iter().enumerate() {
            queue.set(n, element.next_event(*now));
        }
    }

    /// Brings the world up to a simulated time, letting every event before
    /// it happen in its order.
    ///
    /// # Arguments
    ///
    /// * `state`   - The world's state, held locked.
    /// * `seconds` - Where to bring it; one it has passed changes nothing.
    fn advance_to(&self, state: &mut State, seconds: f64) {
        for _ in 0..MOST_EVENTS_AT_ONCE {
            let due = state.queue.first().filter(|(at, _)| *at <= seconds);
            let until = match due {
                Some((at, _)) => at.max(state.now),
                None => seconds,
            };
            if until > state.now {
                for element in &mut state.elements {
                    element.advance(state.now, until);
                }
                state.now = until;
            }
            match due {
                Some((_, n)) => {
                    state.queue.take();
                    state.elements[n].event(state.now);
                    self.hold(state);
                }
                None => {
                    self.hold(state);
                    return;
                }
            }
        }
        eprintln!(
            "the physics model let {MOST_EVENTS_AT_ONCE} events happen at \
             once at {:.6} s and stopped there: an element is foreseeing its \
             next event at the moment it is already at",
            state.now
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcslibgs::physics::terminal::{Kind, Terminal};
    use tcslibgs::physics::{settings_of, terminals_of, TYPES};
    use tcslibgs::unit::Unit;

    /// Every type's terminals fit the room an element's inputs are given,
    /// and every type is one the world can make.
    #[test]
    fn every_type_fits_the_world() {
        for name in TYPES {
            assert!(settings_of(name).is_some(), "{name}");
            let terminals = terminals_of(name).expect("its terminals");
            assert!(terminals.len() <= MOST_TERMINALS, "{name}");
        }
    }

    /// An element that records what the world does to it, and foresees an
    /// event at a time it is told, once.
    #[derive(Debug, Default)]
    struct Watched {
        /// What it was advanced across, and what happened at its events.
        seen: std::sync::Arc<Mutex<Vec<String>>>,
        /// When its event is, until it has happened.
        due: Option<f64>,
        /// Its input, as last held.
        input: f64,
    }

    const WATCHED: &[Terminal] = &[
        Terminal::input("in", Kind::Value(Unit::Watt)).summing(),
        Terminal::output("out", Kind::Value(Unit::Watt)),
    ];

    impl Element for Watched {
        fn hold(&mut self, inputs: &[f64]) {
            self.input = inputs[0];
        }
        fn advance(&mut self, from: f64, to: f64) {
            self.seen
                .lock()
                .unwrap()
                .push(format!("{from}..{to} with {}", self.input));
        }
        fn output(&self, terminal: usize) -> f64 {
            if terminal == 1 {
                1.0
            } else {
                0.0
            }
        }
        fn next_event(&self, _now: f64) -> Option<f64> {
            self.due
        }
        fn event(&mut self, at: f64) {
            self.seen.lock().unwrap().push(format!("event at {at}"));
            self.due = None;
        }
    }

    /// The world stops at a foreseen event, lets it happen and goes on,
    /// and a moment it has passed changes nothing.
    #[test]
    fn the_world_stops_at_each_event_in_its_order() {
        let early = Watched {
            due: Some(2.0),
            ..Default::default()
        };
        let late = Watched {
            due: Some(5.0),
            ..Default::default()
        };
        let seen = early.seen.clone();
        let world = World::of(
            Clock::keeping_pace(Instant::now()),
            vec![
                ("late".into(), WATCHED, Box::new(late)),
                ("early".into(), WATCHED, Box::new(early)),
            ],
            Vec::new(),
        );
        let out = Place {
            element: 1,
            terminal: 1,
        };
        assert_eq!(world.read_at(out, 7.0), 1.0);
        world.read_at(out, 3.0);
        assert_eq!(
            *seen.lock().unwrap(),
            ["0..2 with 0", "event at 2", "2..5 with 0", "5..7 with 0"]
        );
    }

    /// An input takes the sum of the outputs joined to it, held from the
    /// start of each interval, and a driven input what it was driven to.
    #[test]
    fn inputs_are_what_is_joined_to_them() {
        let target = Watched::default();
        let seen = target.seen.clone();
        let world = World::of(
            Clock::keeping_pace(Instant::now()),
            vec![
                ("a".into(), WATCHED, Box::new(Watched::default())),
                ("b".into(), WATCHED, Box::new(Watched::default())),
                ("target".into(), WATCHED, Box::new(target)),
            ],
            [0, 1]
                .map(|from| Joined {
                    from: Place {
                        element: from,
                        terminal: 1,
                    },
                    to: Place {
                        element: 2,
                        terminal: 0,
                    },
                })
                .to_vec(),
        );
        world.read_at(
            Place {
                element: 2,
                terminal: 1,
            },
            1.0,
        );
        assert_eq!(*seen.lock().unwrap(), ["0..1 with 2"]);

        let a = Place {
            element: 0,
            terminal: 0,
        };
        world.drive_at(a, 3.0, 1.5);
        assert_eq!(world.held().inputs[0][0], 3.0);
        assert_eq!(world.name_of(a), "a.in");
    }

    /// An element that foresees an event at the moment it is at, every
    /// time, holds the world only as long as the limit.
    #[derive(Debug)]
    struct Stuck;

    impl Element for Stuck {
        fn hold(&mut self, _inputs: &[f64]) {}
        fn advance(&mut self, _from: f64, _to: f64) {}
        fn output(&self, _terminal: usize) -> f64 {
            0.0
        }
        fn next_event(&self, now: f64) -> Option<f64> {
            Some(now)
        }
    }

    #[test]
    fn an_element_stuck_at_one_moment_does_not_hold_the_world() {
        let world = World::of(
            Clock::keeping_pace(Instant::now()),
            vec![("stuck".into(), WATCHED, Box::new(Stuck))],
            Vec::new(),
        );
        assert_eq!(
            world.read_at(
                Place {
                    element: 0,
                    terminal: 1
                },
                1.0
            ),
            0.0
        );
    }
}
