//! The world's events, in the order they happen.
//!
//! An event is a moment at which a component's behaviour changes in a way an
//! advance cannot step across: a battery reaching full, a body reaching the
//! ground. Each component says when its next one is, as soon as it knows --
//! which may change whenever its inputs do -- and the queue keeps every
//! component's next one, soonest first. The world advances to the soonest,
//! lets it happen, and goes on from there, so nothing is stepped across.
//!
//! One event per component at most, because a component's next event is the
//! only one it can know: what happens after it depends on what it does. So
//! the queue's size is the number of components, fixed when the world is
//! made, and nothing is allocated once it is running.

/// One component's next event.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Due {
    /// Simulated seconds.
    at: f64,
    /// Which component, by its place in the world.
    component: usize,
}

/// Every component's next event, soonest first.
#[derive(Debug)]
pub struct Queue {
    /// Soonest first; of two at one moment, the component whose place is
    /// first, which is the component whose name sorts first: the world's
    /// components are in name order, as `tcslibgs::node::physics::Model` holds them,
    /// and never in the order a file wrote them. A name is one component's
    /// alone, so this is the whole of the order, and no second key is
    /// wanted.
    due: Vec<Due>,
}

impl Queue {
    /// Makes a queue with room for one event of each of `components`.
    ///
    /// # Arguments
    ///
    /// * `components` - How many components the world has.
    ///
    /// # Returns
    ///
    /// An empty queue.
    pub fn new(components: usize) -> Queue {
        Queue {
            due: Vec::with_capacity(components),
        }
    }

    /// Sets when a component's next event is, in place of whatever was
    /// there before.
    ///
    /// # Arguments
    ///
    /// * `component` - Which component.
    /// * `at`        - When, in simulated seconds, or `None` for none it
    ///                 knows of.
    pub fn set(&mut self, component: usize, at: Option<f64>) {
        // Within the room the queue was made with, so neither the removal
        // nor the insertion allocates: a component has one entry at most.
        self.due.retain(|due| due.component != component);
        let Some(at) = at.filter(|at| at.is_finite()) else {
            return;
        };
        let place = self
            .due
            .iter()
            .position(|due| (due.at, due.component) > (at, component))
            .unwrap_or(self.due.len());
        self.due.insert(place, Due { at, component });
    }

    /// The soonest event, if there is one.
    ///
    /// # Returns
    ///
    /// When it is, and whose.
    pub fn first(&self) -> Option<(f64, usize)> {
        self.due.first().map(|due| (due.at, due.component))
    }

    /// Takes the soonest event off the queue, if there is one.
    ///
    /// # Returns
    ///
    /// When it is, and whose.
    pub fn take(&mut self) -> Option<(f64, usize)> {
        if self.due.is_empty() {
            return None;
        }
        let due = self.due.remove(0);
        Some((due.at, due.component))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Events come out soonest first, a component's new event replaces its
    /// old one, and none removes it.
    #[test]
    fn events_come_soonest_first_one_for_each_component() {
        let mut queue = Queue::new(3);
        queue.set(0, Some(5.0));
        queue.set(1, Some(2.0));
        queue.set(2, Some(8.0));
        assert_eq!(queue.first(), Some((2.0, 1)));

        // Component 2 now knows its event is sooner, and component 1 that it
        // has none.
        queue.set(2, Some(1.0));
        queue.set(1, None);
        assert_eq!(queue.take(), Some((1.0, 2)));
        assert_eq!(queue.take(), Some((5.0, 0)));
        assert_eq!(queue.take(), None);
    }

    /// Two at one moment come in the order of their components' places --
    /// their names' order, in a world -- and a moment that is no moment is
    /// none.
    #[test]
    fn a_tie_goes_to_the_first_component() {
        let mut queue = Queue::new(3);
        queue.set(2, Some(4.0));
        queue.set(0, Some(4.0));
        queue.set(1, Some(f64::INFINITY));
        assert_eq!(queue.take(), Some((4.0, 0)));
        assert_eq!(queue.take(), Some((4.0, 2)));
        assert_eq!(queue.take(), None);
    }

    /// Setting one component's event again and again does not grow the
    /// queue past one entry for it, so the room it was made with holds.
    #[test]
    fn the_queue_stays_within_its_room() {
        let mut queue = Queue::new(2);
        let room = queue.due.capacity();
        for n in 0..1000 {
            queue.set(n % 2, Some(n as f64));
        }
        assert_eq!(queue.due.len(), 2);
        assert_eq!(queue.due.capacity(), room);
    }
}
