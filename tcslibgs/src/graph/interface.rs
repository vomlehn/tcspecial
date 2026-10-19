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

#[cfg(test)]
mod tests {
    use super::*;

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
