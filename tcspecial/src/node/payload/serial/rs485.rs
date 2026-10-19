//! Putting a serial line into RS-485 mode.
//!
//! **Experimental, and not yet supported.** This has been run against a
//! pseudo-terminal, which refuses the mode, and against no real RS-485
//! hardware at all.
//!
//! A two-wire bus has one pair of wires and several transmitters on it, so a
//! transmitter must stop driving the pair the moment it has finished. The
//! turnaround is too quick to do from user space -- it is bit times, and a
//! write returns before the last bit has left the shift register -- so Linux
//! does it in the UART driver, told through `TIOCSRS485`. That is the whole of
//! this module: fill the kernel's structure from what the configuration said,
//! ask the port for it, and ask the port back what it took.
//!
//! Read back, because a driver that does not support the mode fails quietly
//! in both directions: some return success and clear the flag, and a line
//! left in point-to-point mode on a two-wire bus transmits into other
//! transmitters. A handler whose line will not take what its file asked for
//! is refused rather than started; see [`apply`].
//!
//! The structure and the two ioctls are declared here rather than taken from
//! `libc`, which has the ioctl numbers and not the structure.

use std::os::unix::io::RawFd;

use tcslibgs::{Rs485Params, TcsError, TcsResult};

/// The kernel's `struct serial_rs485`, from `linux/serial.h`.
///
/// Laid out exactly as the kernel reads it: two words of flags and delays,
/// then a union of five words that the flags decide the meaning of. Nothing
/// here sets any flag that uses them, so they are the padding the kernel
/// asks to be zero.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SerialRs485 {
    flags: u32,
    delay_rts_before_send: u32,
    delay_rts_after_send: u32,
    padding: [u32; 5],
}

/// RS-485 is on.
const SER_RS485_ENABLED: u32 = 1 << 0;
/// RTS is high while sending.
const SER_RS485_RTS_ON_SEND: u32 = 1 << 1;
/// RTS is high once sending has finished. The two are a pair: a driver reads
/// them as the level during and the level after, so the one that is not
/// `RTS_ON_SEND` is the other.
const SER_RS485_RTS_AFTER_SEND: u32 = 1 << 2;

/// What this asks a port for, as the kernel's flags.
fn flags_of(terms: &Rs485Params) -> u32 {
    let during = if terms.rts_on_send {
        SER_RS485_RTS_ON_SEND
    } else {
        SER_RS485_RTS_AFTER_SEND
    };

    SER_RS485_ENABLED | during
}

/// What the port should hold once it has been told, from what was asked.
fn asked_for(terms: &Rs485Params) -> SerialRs485 {
    SerialRs485 {
        flags: flags_of(terms),
        delay_rts_before_send: terms.delay_before_send.as_millis() as u32,
        delay_rts_after_send: terms.delay_after_send.as_millis() as u32,
        padding: [0; 5],
    }
}

/// Whether a port that was told `asked` has taken it.
///
/// The flags this module sets and the two delays, and nothing else: a driver
/// may add flags of its own -- bus termination, full duplex -- and a port
/// that has done so has not failed to do what it was told.
fn took_it(asked: &SerialRs485, got: &SerialRs485) -> bool {
    let wanted = asked.flags;

    got.flags & wanted == wanted
        && got.delay_rts_before_send == asked.delay_rts_before_send
        && got.delay_rts_after_send == asked.delay_rts_after_send
}

/// Put the line open on `fd` into RS-485 mode, as `terms` asks.
///
/// `what` is the handler's name, so a failure says which device's line it
/// was; see the endpoint factories, which pass it down for exactly this.
///
/// Refused rather than reported and run. A line that will not take the mode
/// is a line that drives the pair for as long as it is open, which on a bus
/// is a device that stops every other transmitter on it -- including the one
/// this handler is trying to read. The error says the mode was asked for and
/// not taken, which is the fact that tells a wrong port from a wrong
/// configuration.
pub(crate) fn apply(fd: RawFd, what: &str, terms: &Rs485Params) -> TcsResult<()> {
    let asked = asked_for(terms);

    // SAFETY: the fd is an open serial port, and the structure is laid out as
    // the kernel's own and lives for the call.
    let set = unsafe { libc::ioctl(fd, libc::TIOCSRS485, &asked as *const SerialRs485) };
    if set < 0 {
        return Err(TcsError::Config(format!(
            "{what}: this line will not go into RS-485 mode: {}",
            std::io::Error::last_os_error()
        )));
    }

    let mut got = SerialRs485::default();
    // SAFETY: as above, and the kernel writes through the pointer.
    let read_back = unsafe { libc::ioctl(fd, libc::TIOCGRS485, &mut got as *mut SerialRs485) };
    if read_back < 0 {
        return Err(TcsError::Config(format!(
            "{what}: this line took RS-485 mode and will not say what it is in: {}",
            std::io::Error::last_os_error()
        )));
    }

    if !took_it(&asked, &got) {
        return Err(TcsError::Config(format!(
            "{what}: this line did not take RS-485 mode: it was asked for flags \
             {:#x} with {} ms before and {} ms after, and holds flags {:#x} with \
             {} ms and {} ms",
            asked.flags,
            asked.delay_rts_before_send,
            asked.delay_rts_after_send,
            got.flags,
            got.delay_rts_before_send,
            got.delay_rts_after_send,
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// The structure is the size the kernel's own is.
    ///
    /// Eight words: flags, two delays, and the five-word union. A structure
    /// of the wrong size is an ioctl that reads or writes past what was
    /// handed to it, and nothing else in a test would notice.
    #[test]
    fn the_structure_is_what_the_kernel_reads() {
        assert_eq!(std::mem::size_of::<SerialRs485>(), 8 * 4);
        assert_eq!(std::mem::align_of::<SerialRs485>(), 4);
    }

    /// Which way RTS goes is one flag or the other, never both and never
    /// neither.
    ///
    /// A driver reads the pair as the level while sending and the level
    /// after, so asking for both would be asking for RTS to be high
    /// throughout -- a transmitter that never lets go -- and asking for
    /// neither leaves the driver to pick.
    #[test]
    fn the_level_while_sending_is_one_flag_or_the_other() {
        let high = flags_of(&Rs485Params {
            rts_on_send: true,
            delay_before_send: Duration::from_millis(0),
            delay_after_send: Duration::from_millis(0),
        });
        assert_eq!(
            high & SER_RS485_ENABLED,
            SER_RS485_ENABLED,
            "the mode is on"
        );
        assert_eq!(high & SER_RS485_RTS_ON_SEND, SER_RS485_RTS_ON_SEND);
        assert_eq!(high & SER_RS485_RTS_AFTER_SEND, 0);

        let low = flags_of(&Rs485Params {
            rts_on_send: false,
            delay_before_send: Duration::from_millis(0),
            delay_after_send: Duration::from_millis(0),
        });
        assert_eq!(low & SER_RS485_ENABLED, SER_RS485_ENABLED);
        assert_eq!(low & SER_RS485_RTS_ON_SEND, 0);
        assert_eq!(low & SER_RS485_RTS_AFTER_SEND, SER_RS485_RTS_AFTER_SEND);
    }

    /// The delays are carried in milliseconds, which is the unit the kernel's
    /// fields are in.
    #[test]
    fn the_turnaround_is_carried_as_the_kernel_takes_it() {
        let asked = asked_for(&Rs485Params {
            rts_on_send: true,
            delay_before_send: Duration::from_millis(2),
            delay_after_send: Duration::from_millis(3),
        });
        assert_eq!(asked.delay_rts_before_send, 2);
        assert_eq!(asked.delay_rts_after_send, 3);
        assert_eq!(
            asked.padding, [0; 5],
            "the kernel asks for these to be zero"
        );
    }

    /// A port has taken the mode when it holds what it was asked for, and a
    /// port that cleared a flag or changed a delay has not.
    #[test]
    fn a_port_that_did_not_take_it_is_not_mistaken_for_one_that_did() {
        let terms = Rs485Params {
            rts_on_send: true,
            delay_before_send: Duration::from_millis(1),
            delay_after_send: Duration::from_millis(1),
        };
        let asked = asked_for(&terms);

        assert!(
            took_it(&asked, &asked),
            "a port that holds what it was told"
        );

        // What a driver that does not support the mode does: returns success
        // and leaves the flag clear.
        let cleared = SerialRs485 { flags: 0, ..asked };
        assert!(!took_it(&asked, &cleared), "the mode is not on");

        // And one that took the mode and rounded a delay away.
        let rounded = SerialRs485 {
            delay_rts_after_send: 0,
            ..asked
        };
        assert!(
            !took_it(&asked, &rounded),
            "the turnaround is not what was asked"
        );

        // A port that added a flag of its own has still done what it was
        // told: bus termination and full duplex are the driver's business.
        let more = SerialRs485 {
            flags: asked.flags | (1 << 5),
            ..asked
        };
        assert!(
            took_it(&asked, &more),
            "a flag of the driver's own is not a refusal"
        );
    }

    /// A pseudo-terminal will not take the mode, and says so rather than
    /// pretending.
    ///
    /// Which is the case every run against tcssim is: a pty has no
    /// transceiver and no RTS to drive. The point of the test is the error --
    /// a handler whose line cannot do what its file asked for must be refused
    /// rather than started on a bus it would hold down.
    #[test]
    fn a_pty_refuses_the_mode_and_is_reported() {
        let mut master = 0;
        let mut slave = 0;
        // SAFETY: both ends are written by the call and checked below.
        let made = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        assert_eq!(made, 0, "a pty: {}", std::io::Error::last_os_error());

        let said = apply(
            slave,
            "line",
            &Rs485Params {
                rts_on_send: true,
                delay_before_send: Duration::from_millis(0),
                delay_after_send: Duration::from_millis(0),
            },
        )
        .expect_err("a pty cannot be an RS-485 bus");
        let said = format!("{said}");
        assert!(
            said.contains("line") && said.contains("RS-485"),
            "the error should name the device and the mode: {said}"
        );

        unsafe {
            libc::close(master);
            libc::close(slave);
        }
    }
}
