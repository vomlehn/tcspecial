//! When each group of devices is read.
//!
//! A group exists so that readings which will be compared with one another
//! are taken as close to together as the hardware allows. That is the whole
//! of its purpose, and it decides the shape of this: the unit of work is a
//! group, not a device. When a group comes due, every device in it is read
//! before anything else is done, so three sensors in one group are three
//! reads in a row rather than three reads a third of an interval apart.
//!
//! A device in no group is read as it becomes readable, which is what every
//! payload did before groups existed. Those are not scheduled here at all --
//! they are polled -- so this is only about the ones that are.
//!
//! The clock is monotonic and the next time is computed from the last *due*
//! time rather than from the last time the work finished. A group read every
//! three seconds whose reads take a hundred milliseconds is still read every
//! three seconds, where adding the interval to the finish time would make it
//! three-point-one and drift.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use tcslibgs::graph::Graph;

/// A group and when it is next due.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Due {
    pub group: String,
    pub at: Instant,
}

/// The groups of a configuration and their intervals.
///
/// Kept by position, in the order the configuration writes them, so that the
/// pass can name a group by its place -- [`Schedule::most_overdue`],
/// [`Schedule::members_at`], [`Schedule::read_at`] -- and read what is due
/// without copying a name: see `System::pass`. The by-name calls are the same
/// questions, for a caller that has a name.
#[derive(Debug, Clone)]
pub struct Schedule {
    /// Each group's name, interval, members and when it is next due.
    groups: Vec<Group>,
    /// Each group's place in `groups`, by name.
    index: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
struct Group {
    name: String,
    interval: Duration,
    members: Vec<String>,
    next: Instant,
}

impl Schedule {
    /// Build the schedule for a configuration, starting now.
    ///
    /// Every group is due immediately, so a vehicle that has just started
    /// reads everything once before settling into its intervals rather than
    /// waiting out the longest of them with nothing to show.
    ///
    /// The graph is assumed checked: `Graph::check` has said that every
    /// group's interval reads and that every node's group exists, so an
    /// interval that will not parse cannot arrive here. One that does anyway
    /// is left out, with the group, rather than panicking in flight -- a
    /// group that does not run is better than a vehicle that stops.
    pub fn of(graph: &Graph, now: Instant) -> Schedule {
        let mut groups: Vec<Group> = Vec::new();
        let mut index: BTreeMap<String, usize> = BTreeMap::new();

        for group in &graph.groups {
            let Ok(interval) = group.interval() else {
                continue;
            };
            index.insert(group.name.clone(), groups.len());
            groups.push(Group {
                name: group.name.clone(),
                interval,
                members: Vec::new(),
                next: now,
            });
        }

        for node in &graph.nodes {
            let Some(name) = &node.group else { continue };
            if let Some(&at) = index.get(name) {
                groups[at].members.push(node.name.clone());
            }
        }

        Schedule { groups, index }
    }

    /// The group that has waited longest of those due at `now`, by its
    /// place, or `None` where none is due.
    ///
    /// Several may be due at once -- two groups on the same interval, or a
    /// run that fell behind -- and the one that has waited longest is read
    /// first; a tie goes to the one written first. A group once read is not
    /// due again until its interval has passed (see [`Schedule::read_at`]),
    /// so asking again after each read goes through all that are due, each
    /// once. Allocates nothing.
    pub fn most_overdue(&self, now: Instant) -> Option<usize> {
        self.groups
            .iter()
            .enumerate()
            .filter(|(_, group)| group.next <= now)
            .min_by_key(|(at, group)| (group.next, *at))
            .map(|(at, _)| at)
    }

    /// Which groups are due at `now`, soonest first.
    ///
    /// The same answer as asking [`Schedule::most_overdue`] after each read,
    /// all at once and as a list -- which allocates, so the pass does not ask
    /// it this way.
    pub fn due(&self, now: Instant) -> Vec<Due> {
        let mut due: Vec<Due> = self
            .groups
            .iter()
            .filter(|group| group.next <= now)
            .map(|group| Due {
                group: group.name.clone(),
                at: group.next,
            })
            .collect();
        due.sort_by_key(|d| d.at);
        due
    }

    /// Record that the group at `at` has been read, and say when it is next
    /// due.
    ///
    /// From the time it was *due* and not from now: a group read every three
    /// seconds whose reads take a moment is still read every three seconds.
    /// A group so far behind that the next time is already past is pushed to
    /// now rather than to a time in the past -- otherwise one slow pass would
    /// leave it due for ever and starve everything else.
    pub fn read_at(&mut self, at: usize, now: Instant) -> Option<Instant> {
        let group = self.groups.get_mut(at)?;
        let then = group.next.checked_add(group.interval)?;
        let then = if then <= now {
            now + group.interval
        } else {
            then
        };
        group.next = then;
        Some(then)
    }

    /// The same, for the group called `group`.
    pub fn read(&mut self, group: &str, now: Instant) -> Option<Instant> {
        let at = *self.index.get(group)?;
        self.read_at(at, now)
    }

    /// The nodes in the group at `at`, in the order the configuration names
    /// them.
    pub fn members_at(&self, at: usize) -> &[String] {
        self.groups
            .get(at)
            .map_or(&[], |group| group.members.as_slice())
    }

    /// The same, for the group called `group`.
    pub fn members(&self, group: &str) -> &[String] {
        self.index.get(group).map_or(&[], |&at| self.members_at(at))
    }

    /// How long until something is due, or nothing if there is nothing to
    /// wait for.
    ///
    /// What the run loop waits on: a poll with this timeout wakes either
    /// because a device had something to say or because a group came due.
    /// `None` is a configuration with no groups in it, where the loop waits
    /// on its devices alone.
    pub fn until_next(&self, now: Instant) -> Option<Duration> {
        self.groups
            .iter()
            .map(|group| group.next)
            .min()
            .map(|at| at.saturating_duration_since(now))
    }

    /// Every group, by name.
    pub fn groups(&self) -> Vec<&str> {
        self.groups
            .iter()
            .map(|group| group.name.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn thermostat() -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Graph::load(root.join("tests/manual/thermostat.yaml")).expect("the example reads")
    }

    /// The three sensors of the example are read together, on one interval.
    ///
    /// Which is the point of a group: readings that will be averaged must be
    /// taken as close to together as the hardware allows.
    #[test]
    fn a_groups_devices_are_read_together() {
        let start = Instant::now();
        let schedule = Schedule::of(&thermostat(), start);

        assert_eq!(schedule.groups(), vec!["cabin_temps"]);
        let members = schedule.members("cabin_temps");
        assert_eq!(members.len(), 3, "{members:?}");
        for sensor in ["temp_1", "temp_2", "temp_3"] {
            assert!(
                members.iter().any(|m| m == sensor),
                "{sensor} is not in the group: {members:?}"
            );
        }
    }

    /// Everything is due at once when a vehicle starts.
    ///
    /// A vehicle that waited out its longest interval before reading anything
    /// would come up with nothing to show for three seconds, which is three
    /// seconds of a pass spent wondering whether it is working.
    #[test]
    fn everything_is_due_at_the_start() {
        let start = Instant::now();
        let schedule = Schedule::of(&thermostat(), start);

        let due = schedule.due(start);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].group, "cabin_temps");
        assert_eq!(schedule.until_next(start), Some(Duration::ZERO));
    }

    /// A group read is next due an interval after it was due.
    ///
    /// From the due time and not from the finish time: the second is how a
    /// tenth-of-a-second loop becomes a hundred-and-ten-millisecond loop and
    /// drifts away from the clock it was meant to keep.
    #[test]
    fn the_next_time_does_not_drift() {
        let start = Instant::now();
        let mut schedule = Schedule::of(&thermostat(), start);

        // Read ten milliseconds late, as a real pass would be.
        let late = start + Duration::from_millis(10);
        let then = schedule.read("cabin_temps", late).expect("a next time");
        assert_eq!(
            then,
            start + Duration::from_millis(100),
            "the next time was taken from the finish rather than from the due time"
        );

        // And it is not due before then.
        assert!(schedule.due(start + Duration::from_millis(50)).is_empty());
        assert_eq!(schedule.due(start + Duration::from_millis(100)).len(), 1);

        // A hundred passes, each late: still on the original clock.
        let mut schedule = Schedule::of(&thermostat(), start);
        for n in 0..100 {
            let due = start + Duration::from_millis(100 * n) + Duration::from_millis(7);
            schedule.read("cabin_temps", due);
        }
        assert_eq!(
            schedule.until_next(start + Duration::from_secs(300)),
            Some(Duration::ZERO),
            "a hundred late passes drifted"
        );
    }

    /// A group that has fallen far behind is not due for ever.
    ///
    /// Adding the interval to a time long past leaves it still in the past,
    /// so the group would be due on every pass and starve everything else.
    #[test]
    fn a_group_that_fell_behind_is_caught_up_rather_than_starving_the_rest() {
        let start = Instant::now();
        let mut schedule = Schedule::of(&thermostat(), start);

        // Ten minutes late: two hundred intervals missed.
        let very_late = start + Duration::from_secs(600);
        let then = schedule
            .read("cabin_temps", very_late)
            .expect("a next time");

        assert!(
            then > very_late,
            "the next time is in the past, so the group is due for ever"
        );
        assert_eq!(then, very_late + Duration::from_millis(100));
        assert!(schedule.due(very_late).is_empty());
    }

    /// A configuration with no groups has nothing to wait for.
    #[test]
    fn no_groups_means_nothing_to_wake_for() {
        let graph = Graph {
            description: None,
            // What tcspecial is configured with is nothing to do with the
            // routing, the ports or the schedule: these tests build a graph
            // by hand and the checking that requires a section is not on
            // their path.
            tcspecial: None,
            groups: Vec::new(),
            nodes: Vec::new(),
            links: Vec::new(),
            commands: Vec::new(),
            files: Vec::new(),
            homes: BTreeMap::new(),
            placed: BTreeMap::new(),
            set_aside: Default::default(),
        };
        let schedule = Schedule::of(&graph, Instant::now());

        assert!(schedule.groups().is_empty());
        assert_eq!(schedule.until_next(Instant::now()), None);
        assert!(schedule.due(Instant::now()).is_empty());
    }

    /// Two groups on different intervals come due independently.
    #[test]
    fn groups_keep_their_own_clocks() {
        let mut graph = thermostat();
        graph.groups.push(tcslibgs::graph::Group {
            name: "slow".to_string(),
            interval: "10 s".to_string(),
        });

        let start = Instant::now();
        let mut schedule = Schedule::of(&graph, start);
        schedule.read("cabin_temps", start);
        schedule.read("slow", start);

        // At four seconds the fast one is due and the slow one is not.
        let due = schedule.due(start + Duration::from_secs(4));
        assert_eq!(due.len(), 1, "{due:?}");
        assert_eq!(due[0].group, "cabin_temps");

        // And the wait is until the sooner of the two.
        assert_eq!(
            schedule.until_next(start),
            Some(Duration::from_millis(100)),
            "the wait should be until the soonest group, not the latest"
        );
    }

    /// The one that has waited longest is read first.
    #[test]
    fn the_longest_waiting_group_goes_first() {
        let mut graph = thermostat();
        graph.groups.push(tcslibgs::graph::Group {
            name: "other".to_string(),
            interval: "1 s".to_string(),
        });

        let start = Instant::now();
        let mut schedule = Schedule::of(&graph, start);
        // cabin_temps was read later, so `other` has waited longer.
        schedule.read("other", start);
        schedule.read("cabin_temps", start + Duration::from_secs(2));

        let due = schedule.due(start + Duration::from_secs(10));
        assert_eq!(due.len(), 2);
        assert_eq!(
            due[0].group, "other",
            "the group that waited longest should be first: {due:?}"
        );
    }
}
