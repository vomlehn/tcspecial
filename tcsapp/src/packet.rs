//! What an interface carries: four bytes of length, then that many bytes.
//!
//! Packets and not a stream of bytes, because what travels on a link in this
//! system is a datum with edges -- a reading, a command, a record -- and a
//! stream would leave both ends inventing a rule for where one ends. An app
//! that answered in two writes would otherwise be an app whose answer arrived
//! as one datum or three, depending on timing.
//!
//! The length is four bytes in this machine's own byte order. Native, and so
//! not a stated encoding, because both ends of one of these interfaces are
//! processes on one vehicle started by one program: there is no byte order to
//! agree on, and a stated one would be a conversion written twice to no end.
//! An interface that crossed a machine would need one and would be a
//! different interface.
//!
//! # A zero-length packet is a packet
//!
//! An app with nothing to say and an app that has said something empty are
//! different things -- a pass-through that was handed an empty record, a
//! reading that is absent rather than wrong -- and an interface that could not
//! carry the second would make the two indistinguishable. So [`read`] answers
//! with an `Option`: `Some` of no bytes is an empty packet, and `None` is the
//! interface having ended.

use std::io::{self, Read, Write};

/// How many bytes a packet's length is written in.
pub const LENGTH: usize = std::mem::size_of::<u32>();

/// Write one packet.
///
/// Flushed before it returns, because a packet held in a buffer is a packet
/// the other end is waiting for: an app that answered and then waited for the
/// next question would deadlock against a parent waiting for the answer.
pub fn write<W: Write>(to: &mut W, data: &[u8]) -> io::Result<()> {
    let length = u32::try_from(data.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{} bytes is more than a packet's length can say, which is {} \
                 bytes at most",
                data.len(),
                u32::MAX
            ),
        )
    })?;

    to.write_all(&length.to_ne_bytes())?;
    to.write_all(data)?;
    to.flush()
}

/// Read one packet, or `None` where the interface has ended.
///
/// An interface that ends part way through a packet is an error and not an
/// end: a program killed between writing a length and writing its bytes has
/// left half a datum behind, and passing that on as a short packet would pass
/// on something nobody wrote.
pub fn read<R: Read>(from: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut said = [0u8; LENGTH];
    match fill(from, &mut said)? {
        Filled::Nothing => return Ok(None),
        Filled::Part(got) => {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "the interface ended {got} bytes into a packet's length, \
                     which is {LENGTH} bytes"
                ),
            ))
        }
        Filled::All => {}
    }
    let length = u32::from_ne_bytes(said) as usize;

    // Read into what arrives rather than reserving the length first. The
    // length is four bytes from the far end of a socket, and a program that
    // had just started writing them wrongly would otherwise have this one ask
    // for four gigabytes before finding out.
    let mut data = Vec::new();
    let got = from.take(length as u64).read_to_end(&mut data)?;
    if got != length {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            format!("a packet said {length} bytes and the interface ended after {got}"),
        ));
    }
    Ok(Some(data))
}

/// How much of a buffer was filled before the interface ended.
enum Filled {
    /// Nothing at all, which is the interface having ended cleanly.
    Nothing,
    /// Some of it, which is an interface that ended mid-packet.
    Part(usize),
    All,
}

/// Fill `into` from `from`, saying how far it got.
///
/// `Read::read_exact` would do this but cannot tell "ended cleanly" from
/// "ended half way through", and the difference is the difference between an
/// app that has finished and an app that has died.
fn fill<R: Read>(from: &mut R, into: &mut [u8]) -> io::Result<Filled> {
    let mut got = 0;
    while got < into.len() {
        match from.read(&mut into[got..]) {
            Ok(0) => break,
            Ok(some) => got += some,
            // What a signal leaves behind. The read has to be made again:
            // nothing has been lost, and treating it as a failure would make
            // an app die of a timer going off somewhere else.
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(match got {
        0 => Filled::Nothing,
        got if got == into.len() => Filled::All,
        got => Filled::Part(got),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A packet written is the packet read.
    #[test]
    fn a_packet_is_what_was_written() {
        let mut said = Vec::new();
        write(&mut said, b"293.15").expect("it writes");

        // Four bytes of length and six of data, and the length is this
        // machine's own: read back the way it was written.
        assert_eq!(said.len(), LENGTH + 6);
        assert_eq!(said[..LENGTH], 6u32.to_ne_bytes());

        let mut reading = said.as_slice();
        assert_eq!(
            read(&mut reading).expect("it reads"),
            Some(b"293.15".to_vec())
        );
        // And then the interface has ended, which is not an error.
        assert_eq!(read(&mut reading).expect("the end"), None);
    }

    /// A packet of no bytes is a packet, and not the end.
    #[test]
    fn a_zero_length_packet_is_a_packet() {
        let mut said = Vec::new();
        write(&mut said, b"").expect("it writes");
        write(&mut said, b"after").expect("it writes");
        assert_eq!(said.len(), LENGTH + LENGTH + 5);

        let mut reading = said.as_slice();
        assert_eq!(read(&mut reading).expect("it reads"), Some(Vec::new()));
        assert_eq!(
            read(&mut reading).expect("it reads"),
            Some(b"after".to_vec())
        );
        assert_eq!(read(&mut reading).expect("the end"), None);
    }

    /// Several packets keep their edges, however they were written.
    #[test]
    fn packets_keep_their_edges() {
        let mut said = Vec::new();
        for data in [b"one".to_vec(), Vec::new(), b"three".to_vec()] {
            write(&mut said, &data).expect("it writes");
        }

        let mut reading = said.as_slice();
        let mut read_back = Vec::new();
        while let Some(data) = read(&mut reading).expect("it reads") {
            read_back.push(data);
        }
        assert_eq!(
            read_back,
            vec![b"one".to_vec(), Vec::new(), b"three".to_vec()]
        );
    }

    /// An interface that ends inside a packet's length is an error.
    #[test]
    fn half_a_length_is_not_an_end() {
        let mut reading: &[u8] = &[0, 0];
        let said = read(&mut reading).expect_err("half a length");
        assert_eq!(said.kind(), io::ErrorKind::UnexpectedEof);
        assert!(said.to_string().contains("2 bytes into"), "{said}");
    }

    /// And one that ends inside a packet's data is an error too.
    ///
    /// Not a short packet: a datum that was cut off is not a datum, and
    /// passing on what arrived would pass on something nobody wrote.
    #[test]
    fn a_packet_cut_short_is_not_a_packet() {
        let mut said = 6u32.to_ne_bytes().to_vec();
        said.extend_from_slice(b"293");

        let mut reading = said.as_slice();
        let said = read(&mut reading).expect_err("a packet cut short");
        assert!(said.to_string().contains("said 6 bytes"), "{said}");
    }
}
