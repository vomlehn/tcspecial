//! A serial line through `termios`: its rate, byte length, stop bits and
//! parity, and RS-485 mode through the UART driver. Linux alone: in a
//! browser, a serial line is only ever simulated.
//!
//! What a node asks of its line, and what it makes of a line that keeps
//! less than it was given, is the node's. This is the system's part.

use std::fs::File;
use std::os::fd::AsRawFd;

/// A parity a line can have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    /// None.
    None,
    /// Even.
    Even,
    /// Odd.
    Odd,
    /// Always one.
    Mark,
    /// Always nought.
    Space,
}

/// How many stop bits a line sends: `termios` has one bit for it, so one or
/// two, and never one and a half.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopBits {
    /// One.
    One,
    /// Two.
    Two,
}

/// The terms a line is set to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Terms {
    /// Bits per second.
    pub rate: u32,
    /// Bits in a byte, five to eight.
    pub byte_length: u8,
    /// Stop bits.
    pub stop_bits: StopBits,
    /// Parity.
    pub parity: Parity,
}

/// Why a line could not be set.
#[derive(Debug)]
pub enum Refused {
    /// The rate is not one a port can be set to.
    Rate(u32),
    /// The byte length is not one a port offers.
    ByteLength(u8),
    /// The file cannot be read as a serial port: it is not one.
    NotALine,
    /// The line cannot be set to the rate.
    Speed,
    /// The line would not take the terms.
    Terms,
}

/// The flags that ask a port for a parity.
///
/// Mark and space are the two that need CMSPAR: they send a constant bit
/// rather than one computed from the data, and the flag that asks for it is
/// Linux's own. The two differ only in which constant, which is PARODD.
///
/// # Arguments
///
/// * `parity` - The parity.
///
/// # Returns
///
/// The `c_cflag` bits, nought for none.
fn parity_flags(parity: Parity) -> libc::tcflag_t {
    match parity {
        Parity::None => 0,
        Parity::Even => libc::PARENB,
        Parity::Odd => libc::PARENB | libc::PARODD,
        Parity::Mark => libc::PARENB | libc::CMSPAR | libc::PARODD,
        Parity::Space => libc::PARENB | libc::CMSPAR,
    }
}

/// The parity a port's flags say it has: the inverse of [`parity_flags`].
///
/// # Arguments
///
/// * `flags` - The port's `c_cflag`.
///
/// # Returns
///
/// The parity.
fn parity_of(flags: libc::tcflag_t) -> Parity {
    if flags & libc::PARENB == 0 {
        return Parity::None;
    }
    match (flags & libc::CMSPAR != 0, flags & libc::PARODD != 0) {
        (true, true) => Parity::Mark,
        (true, false) => Parity::Space,
        (false, true) => Parity::Odd,
        (false, false) => Parity::Even,
    }
}

/// The `termios` constant for a rate.
///
/// # Arguments
///
/// * `rate` - Bits per second.
///
/// # Returns
///
/// The `B*` constant; `None` where `termios` has none for it.
fn speed_of(rate: u32) -> Option<libc::speed_t> {
    Some(match rate {
        50 => libc::B50,
        75 => libc::B75,
        110 => libc::B110,
        134 => libc::B134,
        150 => libc::B150,
        200 => libc::B200,
        300 => libc::B300,
        600 => libc::B600,
        1200 => libc::B1200,
        1800 => libc::B1800,
        2400 => libc::B2400,
        4800 => libc::B4800,
        9600 => libc::B9600,
        19200 => libc::B19200,
        38400 => libc::B38400,
        57600 => libc::B57600,
        115_200 => libc::B115200,
        230_400 => libc::B230400,
        460_800 => libc::B460800,
        500_000 => libc::B500000,
        576_000 => libc::B576000,
        921_600 => libc::B921600,
        1_000_000 => libc::B1000000,
        1_152_000 => libc::B1152000,
        1_500_000 => libc::B1500000,
        2_000_000 => libc::B2000000,
        2_500_000 => libc::B2500000,
        3_000_000 => libc::B3000000,
        3_500_000 => libc::B3500000,
        4_000_000 => libc::B4000000,
        _ => return None,
    })
}

/// The `termios` flag for a byte length.
///
/// # Arguments
///
/// * `bits` - Bits in a byte.
///
/// # Returns
///
/// The `CS*` flag; `None` where the length is not five to eight.
fn byte_length_of(bits: u8) -> Option<libc::tcflag_t> {
    match bits {
        5 => Some(libc::CS5),
        6 => Some(libc::CS6),
        7 => Some(libc::CS7),
        8 => Some(libc::CS8),
        _ => None,
    }
}

/// What a line is set to now.
///
/// # Arguments
///
/// * `line` - The open line.
///
/// # Returns
///
/// Its terms, or `None` where it cannot be read as a serial port.
fn terms_of(line: &File) -> Option<libc::termios> {
    // SAFETY: termios is a C struct of integers and arrays of integers, for
    // which all zeroes is a valid value; tcgetattr overwrites it anyway.
    let mut terms: libc::termios = unsafe { std::mem::zeroed() };
    // SAFETY: tcgetattr needs an open descriptor, which `line` holds for the
    // call, and a termios it can write, which `terms` is.
    (unsafe { libc::tcgetattr(line.as_raw_fd(), &mut terms) } == 0)
        .then_some(terms)
}

/// Sets a line, raw, to terms: what they do not name is left as it was.
///
/// Raw rather than canonical: a device link carries bytes, and the line
/// discipline's editing, echo and newline translation would corrupt them.
/// Read and write both, and local: a port that is not CLOCAL waits on
/// carrier a device link does not have.
///
/// # Arguments
///
/// * `line`  - The open line.
/// * `terms` - What to set it to.
///
/// # Errors
///
/// What was refused.
pub fn set_terms(line: &File, terms: &Terms) -> Result<(), Refused> {
    let speed = speed_of(terms.rate).ok_or(Refused::Rate(terms.rate))?;
    let size = byte_length_of(terms.byte_length)
        .ok_or(Refused::ByteLength(terms.byte_length))?;
    let mut now = terms_of(line).ok_or(Refused::NotALine)?;

    // SAFETY: cfmakeraw needs a termios it can write, which `now` is.
    unsafe { libc::cfmakeraw(&mut now) };
    now.c_cflag &= !(libc::CSIZE
        | libc::CSTOPB
        | libc::PARENB
        | libc::PARODD
        | libc::CMSPAR);
    now.c_cflag |= size;
    if terms.stop_bits == StopBits::Two {
        now.c_cflag |= libc::CSTOPB;
    }
    now.c_cflag |= parity_flags(terms.parity);
    now.c_cflag |= libc::CREAD | libc::CLOCAL;

    // SAFETY: cfsetispeed and cfsetospeed need a termios they can write,
    // which `now` is; a speed they do not know is refused by their return.
    if unsafe { libc::cfsetispeed(&mut now, speed) } != 0
        // SAFETY: as for cfsetispeed.
        || unsafe { libc::cfsetospeed(&mut now, speed) } != 0
    {
        return Err(Refused::Speed);
    }
    // SAFETY: tcsetattr needs an open descriptor, which `line` holds, and a
    // termios it can read.
    if unsafe { libc::tcsetattr(line.as_raw_fd(), libc::TCSANOW, &now) } != 0 {
        return Err(Refused::Terms);
    }
    Ok(())
}

/// The parity a line has now: a port may take a call and keep less than
/// it was given, and a pty quietly drops any parity but none.
///
/// # Arguments
///
/// * `line` - The open line.
///
/// # Returns
///
/// Its parity, or `None` where it cannot be read.
pub fn parity_now(line: &File) -> Option<Parity> {
    terms_of(line).map(|terms| parity_of(terms.c_cflag))
}

/// Whether a rate is one a port can be set to.
///
/// # Arguments
///
/// * `rate` - Bits per second.
///
/// # Returns
///
/// `true` where `termios` has a constant for it.
pub fn is_a_rate(rate: u32) -> bool {
    speed_of(rate).is_some()
}

/// Whether a byte length is one a port offers.
///
/// # Arguments
///
/// * `bits` - Bits in a byte.
///
/// # Returns
///
/// `true` for five to eight.
pub fn is_a_byte_length(bits: u8) -> bool {
    byte_length_of(bits).is_some()
}

/// RS-485 mode, through the UART driver: a two-wire bus's turnaround is
/// too quick to do from user space, so the driver does it, told through
/// `TIOCSRS485`.
pub mod rs485 {
    use std::fs::File;
    use std::os::fd::AsRawFd;

    /// RS-485 is on.
    pub const ENABLED: u32 = 1 << 0;
    /// RTS is high while sending.
    pub const RTS_ON_SEND: u32 = 1 << 1;
    /// RTS is high once sending has finished.
    pub const RTS_AFTER_SEND: u32 = 1 << 2;

    /// The kernel's `struct serial_rs485`, from `linux/serial.h`: two
    /// words of flags and delays, then a union of five words that the flags
    /// decide the meaning of. Nothing here sets a flag that uses them, so
    /// they are the padding the kernel asks to be zero.
    #[repr(C)]
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct Mode {
        /// [`ENABLED`] and the RTS flags.
        pub flags: u32,
        /// How long RTS is raised before sending, in milliseconds.
        pub delay_rts_before_send: u32,
        /// How long it is held after, in milliseconds.
        pub delay_rts_after_send: u32,
        /// Nought, as the kernel asks.
        pub padding: [u32; 5],
    }

    /// Puts a line into a mode.
    ///
    /// # Arguments
    ///
    /// * `line` - The open serial port.
    /// * `mode` - The mode.
    ///
    /// # Errors
    ///
    /// What the port said where it refused.
    pub fn set(line: &File, mode: &Mode) -> std::io::Result<()> {
        // SAFETY: the descriptor is an open serial port, and the structure
        // is laid out as the kernel's own and lives for the call.
        let set = unsafe {
            libc::ioctl(line.as_raw_fd(), libc::TIOCSRS485, mode as *const Mode)
        };
        if set < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    /// What mode a line is in.
    ///
    /// # Arguments
    ///
    /// * `line` - The open serial port.
    ///
    /// # Returns
    ///
    /// Its mode.
    ///
    /// # Errors
    ///
    /// What the port said where it will not say.
    pub fn get(line: &File) -> std::io::Result<Mode> {
        let mut got = Mode::default();
        // SAFETY: as for `set`, and the kernel writes through the pointer.
        let read = unsafe {
            libc::ioctl(
                line.as_raw_fd(),
                libc::TIOCGRS485,
                &mut got as *mut Mode,
            )
        };
        if read < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(got)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The structure is the size the kernel's own is: eight words.
        #[test]
        fn the_structure_is_what_the_kernel_reads() {
            assert_eq!(std::mem::size_of::<Mode>(), 8 * 4);
            assert_eq!(std::mem::align_of::<Mode>(), 4);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Asking for a parity and reading one back are inverses, for every
    /// parity.
    #[test]
    fn asking_for_a_parity_and_reading_one_back_are_inverses() {
        for parity in [
            Parity::None,
            Parity::Even,
            Parity::Odd,
            Parity::Mark,
            Parity::Space,
        ] {
            assert_eq!(parity_of(parity_flags(parity)), parity);
        }

        // Anchored, so that the two being inverses is not two matching
        // mistakes: these are the combinations termios defines.
        assert_eq!(parity_flags(Parity::None), 0);
        assert_eq!(parity_flags(Parity::Even), libc::PARENB);
        assert_eq!(parity_flags(Parity::Odd) & libc::PARODD, libc::PARODD);
        assert_eq!(parity_flags(Parity::Odd) & libc::CMSPAR, 0);
        assert_eq!(parity_flags(Parity::Mark) & libc::CMSPAR, libc::CMSPAR);
        assert_eq!(
            parity_flags(Parity::Mark) & libc::PARODD,
            libc::PARODD,
            "mark sends a one, which is PARODD with CMSPAR"
        );
        assert_eq!(parity_flags(Parity::Space) & libc::CMSPAR, libc::CMSPAR);
        assert_eq!(
            parity_flags(Parity::Space) & libc::PARODD,
            0,
            "space sends a zero, which is CMSPAR without PARODD"
        );
    }

    /// The rates and lengths a port offers are known, and others are not.
    #[test]
    fn the_rates_and_lengths_a_port_offers_are_known() {
        assert!(is_a_rate(9600) && is_a_rate(115_200));
        assert!(!is_a_rate(9601));
        assert!((5..=8).all(is_a_byte_length));
        assert!(!is_a_byte_length(9));
    }
}
