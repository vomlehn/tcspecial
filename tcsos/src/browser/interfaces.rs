//! The interfaces a program is started with, in a page: each a TCP
//! connection of the page's own -- see [`crate::browser::net`] -- carrying
//! what it carries on Linux, packets of four bytes of length and that many
//! bytes, in place of a Unix-domain socket pair at descriptors 0, 1 and 2.
//!
//! Handed over in the program's slots, kept with the program -- see
//! [`crate::browser::context`] -- and taken out of them by the program as
//! on Linux.

use std::io::{self, Write};

use crate::browser::context;
use crate::browser::net::TcpStream;
use crate::browser::process::Command;

/// What an interface is carried on: one end of a connection, read and
/// written.
pub type Stream = TcpStream;

/// What the error interface is carried on once taken: the same.
pub type Written = TcpStream;

/// Makes the two ends of an interface.
///
/// # Returns
///
/// The ends.
///
/// # Errors
///
/// None, in a page.
pub fn pair() -> io::Result<(Stream, Stream)> {
    Ok(TcpStream::pair())
}

/// Does nothing: there are no signals to unblock.
///
/// # Arguments
///
/// * `_command` - The command, not yet spawned.
pub fn unblocking(_command: &mut Command) {}

/// Has a command start its program with its interfaces in its slots -- the
/// payload at 0, the gateway at 1, the errors at 2 -- and nothing in a slot
/// it does not have.
///
/// # Arguments
///
/// * `command` - The command, not yet spawned.
/// * `payload` - The child's end of the payload interface, where it has
///               one.
/// * `gateway` - The child's end of the gateway interface, where it has
///               one.
/// * `errors`  - The child's end of the error interface.
///
/// # Errors
///
/// Where an end cannot be held again.
pub fn hand_over(
    command: &mut Command,
    payload: Option<&Stream>,
    gateway: Option<&Stream>,
    errors: &Stream,
) -> io::Result<()> {
    if let Some(theirs) = payload {
        command.hand(0, theirs.try_clone()?);
    }
    if let Some(theirs) = gateway {
        command.hand(1, theirs.try_clone()?);
    }
    command.hand(2, errors.try_clone()?);
    Ok(())
}

/// Whether a slot has anything in it.
///
/// # Arguments
///
/// * `slot` - 0, 1 or 2.
///
/// # Returns
///
/// `true` where it has.
pub fn is_open(slot: i32) -> bool {
    is_handed_over(slot)
}

/// Whether a slot holds an interface that was handed over.
///
/// # Arguments
///
/// * `slot` - 0, 1 or 2.
///
/// # Returns
///
/// `true` where it does.
pub fn is_handed_over(slot: i32) -> bool {
    let program = context::current();
    let slots = program.slots.lock().expect("the slots");
    usize::try_from(slot)
        .ok()
        .and_then(|slot| slots.get(slot))
        .is_some_and(|held| held.is_some())
}

/// Takes what is in a slot out of it.
///
/// # Arguments
///
/// * `slot` - 0, 1 or 2.
///
/// # Returns
///
/// The interface.
///
/// # Errors
///
/// `NotConnected` where the slot is empty.
fn take(slot: i32) -> io::Result<TcpStream> {
    let program = context::current();
    let mut slots = program.slots.lock().expect("the slots");
    usize::try_from(slot)
        .ok()
        .and_then(|slot| slots.get_mut(slot))
        .and_then(|held| held.take())
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotConnected))
}

/// Takes a data interface out of its slot.
///
/// # Arguments
///
/// * `slot` - 0 or 1.
///
/// # Returns
///
/// The interface.
///
/// # Errors
///
/// `NotConnected` where the slot is empty.
pub fn take_stream(slot: i32) -> io::Result<Stream> {
    take(slot)
}

/// Takes the error interface out of its slot.
///
/// # Arguments
///
/// * `slot` - 2.
///
/// # Returns
///
/// The interface.
///
/// # Errors
///
/// `NotConnected` where the slot is empty.
pub fn take_file(slot: i32) -> io::Result<Written> {
    take(slot)
}

/// Writes to what is in a slot without taking it.
///
/// # Arguments
///
/// * `slot`  - 2.
/// * `write` - What to do with it.
pub fn with_slot(slot: i32, write: impl FnOnce(&mut dyn Write)) {
    let program = context::current();
    let slots = program.slots.lock().expect("the slots");
    if let Some(Some(held)) =
        usize::try_from(slot).ok().and_then(|slot| slots.get(slot))
    {
        let mut held: &TcpStream = held;
        write(&mut held);
    }
}

/// Does nothing: a program in a page has no standard slots of the page's
/// to cover.
///
/// # Errors
///
/// None.
pub fn quiet_standard_slots() -> io::Result<()> {
    Ok(())
}
