//! Whether a payload's data is current, as its panel says it.
//!
//! The ground does not ask a payload how it is. An enabled payload sends on
//! its own -- that is what enabling it does -- so the one thing the ground
//! needs to say about it is whether what it is sending is still arriving, and
//! the ground already knows that: every datagram arrives on that payload's own
//! telemetry socket and is timed there. See [`crate::oc_link`].
//!
//! This replaced a poller that asked every payload what it was doing once a
//! second. The asking cost a command and an answer per payload per second --
//! forty payloads were eighty datagrams a second of nothing but housekeeping
//! -- and told the panels little that the arrivals did not: a payload that is
//! enabled and sending is a payload whose data keeps arriving. What the poller
//! did say that this cannot is the spacecraft's own counters, which Query All
//! still asks for when somebody wants them.
//!
//! **Needs examination.** The rule below is a first statement of it. Three
//! things in particular are unsettled: what interval suits a payload that
//! answers requests rather than sending on its own, since its data arrives
//! only when tcspecial asks; whether `enabled` should mean what the ground
//! last commanded or what the spacecraft last said; and what a panel ought to
//! show for a payload that states no interval at all, which is grey here and
//! is grey for a payload nobody has enabled.

use std::time::Duration;

/// What a payload's data indicator says.
///
/// No red state, deliberately. A payload whose data has stopped may be a
/// payload with nothing to say, a payload somebody stopped, or a link that has
/// failed -- and nothing here can tell those apart. Red is for a condition
/// that is dangerous; this is a condition that is worth looking at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// Nothing to judge by: as far as the ground knows the payload is not
    /// enabled, or the payload states no interval. Grey.
    Unjudged,
    /// Data has arrived within the payload's interval. Green.
    Current,
    /// The payload is enabled and nothing has arrived from it for longer than
    /// its interval. Yellow.
    Stale,
}

/// What a payload's indicator should say, in milliseconds since the epoch.
///
/// `enabled` is whether the ground believes the payload is running. `since` is
/// the later of when its last datagram arrived and when it was enabled, which
/// is what makes a payload just switched on current rather than late: it has
/// its interval to produce something. `interval` is the payload's own
/// `stale_interval_ms`, and `now` is the clock.
///
/// A `since` in the future is a clock that moved, and is not a reason to claim
/// the data is current: it reads as stale, the conservative of the two, as the
/// beacon indicator goes back to grey rather than claiming contact.
pub fn freshness_of(
    enabled: bool,
    since: Option<u64>,
    interval: Option<Duration>,
    now: u64,
) -> Freshness {
    if !enabled {
        return Freshness::Unjudged;
    }
    let Some(interval) = interval else {
        return Freshness::Unjudged;
    };
    let Some(since) = since else {
        return Freshness::Stale;
    };

    match now.checked_sub(since) {
        Some(elapsed) if elapsed <= interval.as_millis() as u64 => Freshness::Current,
        _ => Freshness::Stale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTERVAL: Option<Duration> = Some(Duration::from_millis(3000));

    /// A payload nobody has enabled says nothing about its data.
    ///
    /// Grey rather than stale: data that is not expected cannot be late, and
    /// a panel that said it was would have every stopped payload asking to be
    /// looked at.
    #[test]
    fn a_payload_that_is_not_enabled_is_not_judged() {
        assert_eq!(
            freshness_of(false, Some(1_000), INTERVAL, 100_000),
            Freshness::Unjudged
        );
    }

    /// Nor does one that states no interval, however long it has been quiet.
    ///
    /// There is nothing to judge by, and a figure nobody chose would make a
    /// payload that speaks once an hour look late every minute of it.
    #[test]
    fn a_payload_with_no_interval_is_not_judged() {
        assert_eq!(
            freshness_of(true, Some(1_000), None, 100_000),
            Freshness::Unjudged
        );
    }

    /// Inside its interval is current, past it is stale, and the boundary is
    /// inside.
    #[test]
    fn data_within_the_interval_is_current() {
        assert_eq!(
            freshness_of(true, Some(10_000), INTERVAL, 11_000),
            Freshness::Current
        );
        // Exactly the interval: still current. A payload that sends every
        // 3000 ms against an interval of 3000 ms would otherwise be stale at
        // the moment its next datagram is due, which is every time.
        assert_eq!(
            freshness_of(true, Some(10_000), INTERVAL, 13_000),
            Freshness::Current
        );
        assert_eq!(
            freshness_of(true, Some(10_000), INTERVAL, 13_001),
            Freshness::Stale
        );
    }

    /// An enabled payload that has sent nothing at all is stale.
    ///
    /// `since` carries when it was enabled, so this is the case where even
    /// that is unknown -- and a payload said to be running with nothing to
    /// show for it is exactly what this indicator is for.
    #[test]
    fn an_enabled_payload_with_nothing_at_all_is_stale() {
        assert_eq!(freshness_of(true, None, INTERVAL, 100_000), Freshness::Stale);
    }

    /// A time in the future is a clock that moved, and reads as stale.
    #[test]
    fn a_reading_from_the_future_is_not_called_current() {
        assert_eq!(
            freshness_of(true, Some(200_000), INTERVAL, 100_000),
            Freshness::Stale
        );
    }
}
