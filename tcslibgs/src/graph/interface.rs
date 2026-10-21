//! Which kind of interface a payload has: one it sends on by itself, or one
//! that has nothing on it until the vehicle reads.
//!
//! The one table that says which, as the design asks: it is a property of the
//! node's type and not a thing for a file to state, since a payload that said
//! it originated nothing while sitting on a socket would be a file disagreeing
//! with the interface it names.
//!
//! What it decides is what a timer drives. For a part that originates
//! nothing -- a register on a bus, a device file, a peripheral on a clocked
//! link -- nothing arrives unless the vehicle reads, so a timer drives the
//! read, or, for one that is asked, the whole exchange: ask, wait, read. For a
//! part that sends -- a socket, a line, a channel, a set of discrete lines --
//! data arrives when it arrives and is taken as it does; a timer drives at
//! most the asking.

/// Whether a payload of this type puts data on its interface by itself.
///
/// `false` for the types whose data is there only because the vehicle read
/// it: `i2c` and `spi`, where the vehicle clocks the bus, and `device-file`,
/// where a read is what produces the bytes. `true` for everything else a
/// payload can be: a socket, a serial or HDLC line, a CAN bus, an rpmsg
/// channel, an IIO buffer, GPIO lines, and a program.
///
/// A type this table does not know is taken to send: data that arrives is
/// taken as it arrives, which is the safer of the two for a part nobody has
/// described -- it is never asked to do more than it was.
pub fn sends(node_type: &str) -> bool {
    !matches!(node_type, "i2c" | "spi" | "device-file")
}

/// Whether a datagram payload is the end that speaks first, and so the end
/// that reaches out to the address its node states rather than the end
/// that waits there.
///
/// A datagram connect sends nothing, so one end has to speak before the
/// other knows where it is, and the end that speaks first is the one that
/// is not found. A payload that is read and not asked produces data because
/// it is running, so it speaks first: the vehicle waits at the address and
/// learns where the payload is from what arrives. Any other payload is
/// spoken to first -- one that is asked hears its trigger, one that is only
/// driven hears what the vehicle decided -- so it waits at the address and
/// the vehicle sends there from a socket of its own, which is where any
/// answer comes back to.
///
/// The one statement of the rule, read aboard to open the socket, by the
/// simulator to stand in for the payload, and by the check that refuses an
/// init with nowhere to go.
pub fn speaks_first(read: bool, asked: bool) -> bool {
    read && !asked
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only a payload that is read and nothing asks speaks first.
    #[test]
    fn a_payload_speaks_first_only_where_nothing_speaks_to_it() {
        assert!(speaks_first(true, false), "a sensor that sends");
        assert!(!speaks_first(true, true), "a sensor that is asked");
        assert!(!speaks_first(false, false), "a heater");
    }

    /// The parts that originate nothing are the clocked buses and a file.
    #[test]
    fn only_a_bus_and_a_file_originate_nothing() {
        for quiet in ["i2c", "spi", "device-file"] {
            assert!(!sends(quiet), "{quiet}");
        }
        for talking in [
            "network",
            "serial",
            "hdlc",
            "can",
            "rpmsg",
            "iio",
            "gpio",
            "app_pl",
            "app_all_pl",
        ] {
            assert!(sends(talking), "{talking}");
        }
    }
}
