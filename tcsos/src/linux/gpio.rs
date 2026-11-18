//! GPIO lines through the kernel's character device, version 2 of its
//! interface: a request for lines of a chip, what they are, and driving
//! them. Linux alone: in a browser, GPIO lines are only ever simulated.
//!
//! What a node does with its lines -- which are watched and which driven,
//! when a report is due, how a level is said on the wire -- is the node's.
//! This is the kernel's part alone: its structures, laid out as
//! `linux/gpio.h` has them, and the three ioctls.

use std::fs::{File, OpenOptions};
use std::os::fd::{AsRawFd, FromRawFd};
use std::time::Duration;

use crate::linux::wait::{Source, Waitable};

/// The most lines one request may hold, from `linux/gpio.h`.
pub const LINES_MAX: usize = 64;
/// The longest name a request may be held under, its nul included.
const MAX_NAME_SIZE: usize = 32;
/// The most attributes one request's configuration may have.
const LINE_NUM_ATTRS_MAX: usize = 10;

/// `GPIO_V2_GET_LINE_IOCTL`, which is `_IOWR(0xB4, 0x07, struct
/// gpio_v2_line_request)`.
const GET_LINE_IOCTL: libc::c_ulong = 0xc250_b407;
/// `GPIO_V2_LINE_GET_VALUES_IOCTL`, which is `_IOWR(0xB4, 0x0E, struct
/// gpio_v2_line_values)`: asked of the request a node already holds.
const LINE_GET_VALUES_IOCTL: libc::c_ulong = 0xc010_b40e;
/// `GPIO_V2_LINE_SET_VALUES_IOCTL`, which is `_IOWR(0xB4, 0x0F, struct
/// gpio_v2_line_values)`.
const LINE_SET_VALUES_IOCTL: libc::c_ulong = 0xc010_b40f;

/// Line flags, from `enum gpio_v2_line_flag`.
const LINE_FLAG_INPUT: u64 = 1 << 2;
const LINE_FLAG_OUTPUT: u64 = 1 << 3;
const LINE_FLAG_BIAS_PULL_UP: u64 = 1 << 8;
const LINE_FLAG_BIAS_PULL_DOWN: u64 = 1 << 9;

/// `GPIO_V2_LINE_ATTR_ID_DEBOUNCE`.
const LINE_ATTR_ID_DEBOUNCE: u32 = 3;

/// What lines are driven to, and which lines of the request it is about.
///
/// `gpio_v2_line_values`: the bits of `bits` are the values, the bits of
/// `mask` say which lines they apply to. Both are indexed by the line's
/// place in the request rather than by its offset on the chip: lines 3, 4
/// and 5 of a chip are bits 0, 1 and 2 here.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct LineValues {
    /// The values, a bit for each line by its place.
    bits: u64,
    /// Which lines they are of, the same way.
    mask: u64,
}

/// `struct gpio_v2_line_attribute`, whose union this uses as a debounce
/// period: a `u32` in a field eight bytes wide.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct LineAttribute {
    /// Which attribute: only the debounce, here.
    id: u32,
    /// Nought, as the kernel wants.
    padding: u32,
    /// The debounce period, in microseconds.
    value: u64,
}

/// `struct gpio_v2_line_config_attribute`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct ConfigAttribute {
    /// The attribute.
    attr: LineAttribute,
    /// The lines it applies to, a bit for each by its place.
    mask: u64,
}

/// `struct gpio_v2_line_config`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct LineConfig {
    /// The flags every line of the request has.
    flags: u64,
    /// How many of `attrs` are used.
    num_attrs: u32,
    /// Nought, as the kernel wants.
    padding: [u32; 5],
    /// The attributes.
    attrs: [ConfigAttribute; LINE_NUM_ATTRS_MAX],
}

impl Default for LineConfig {
    fn default() -> Self {
        Self {
            flags: 0,
            num_attrs: 0,
            padding: [0; 5],
            attrs: [ConfigAttribute::default(); LINE_NUM_ATTRS_MAX],
        }
    }
}

/// `struct gpio_v2_line_request`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct LineRequest {
    /// The lines' offsets on the chip.
    offsets: [u32; LINES_MAX],
    /// Whose the lines are, which `gpioinfo` shows beside them.
    consumer: [libc::c_char; MAX_NAME_SIZE],
    /// How the lines are to be set.
    config: LineConfig,
    /// How many of `offsets` are used.
    num_lines: u32,
    /// Nought: no events are asked for.
    event_buffer_size: u32,
    /// Nought, as the kernel wants.
    padding: [u32; 5],
    /// The descriptor the kernel gives back.
    fd: i32,
}

impl Default for LineRequest {
    fn default() -> Self {
        Self {
            offsets: [0; LINES_MAX],
            consumer: [0; MAX_NAME_SIZE],
            config: LineConfig::default(),
            num_lines: 0,
            event_buffer_size: 0,
            padding: [0; 5],
            fd: -1,
        }
    }
}

/// How watched lines are pulled, where nothing drives them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bias {
    /// Not at all.
    None,
    /// Up.
    PullUp,
    /// Down.
    PullDown,
}

/// How the lines of a request are set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settings {
    /// Driven: outputs, with nothing to hold or debounce.
    Output,
    /// Watched: plain inputs, with no edge detection.
    Input {
        /// How they are pulled.
        bias: Bias,
        /// The debounce the kernel does, and the lines it applies to, a bit
        /// for each by its place.
        debounce: Option<(Duration, u64)>,
    },
}

/// Why lines could not be had.
#[derive(Debug)]
pub enum Refused {
    /// None, or more than [`LINES_MAX`], were asked for.
    HowMany(usize),
    /// The chip could not be opened.
    Open(std::io::Error),
    /// The chip would not give up the lines: commonly, another program
    /// holds one.
    Request(std::io::Error),
    /// The chip took the request and gave no descriptor.
    NoDescriptor,
    /// The request's descriptor could not be made non-blocking.
    NonBlocking(std::io::Error),
}

/// Builds the request for lines of a chip.
///
/// # Arguments
///
/// * `offsets`  - The lines' offsets on the chip, in the request's order.
/// * `consumer` - Whose they are, as `gpioinfo` will show them.
/// * `settings` - How they are set.
///
/// # Returns
///
/// The request, ready for the kernel.
///
/// # Errors
///
/// [`Refused::HowMany`] if there are no lines or more than [`LINES_MAX`].
fn request_for(
    offsets: &[u32],
    consumer: &str,
    settings: &Settings,
) -> Result<LineRequest, Refused> {
    if offsets.is_empty() || offsets.len() > LINES_MAX {
        return Err(Refused::HowMany(offsets.len()));
    }
    let mut request = LineRequest::default();
    request.offsets[..offsets.len()].copy_from_slice(offsets);
    request.num_lines = offsets.len() as u32;
    for (n, byte) in consumer.bytes().take(MAX_NAME_SIZE - 1).enumerate() {
        request.consumer[n] = byte as libc::c_char;
    }
    match *settings {
        Settings::Output => request.config.flags = LINE_FLAG_OUTPUT,
        Settings::Input { bias, debounce } => {
            let bias = match bias {
                Bias::None => 0,
                Bias::PullUp => LINE_FLAG_BIAS_PULL_UP,
                Bias::PullDown => LINE_FLAG_BIAS_PULL_DOWN,
            };
            request.config.flags = LINE_FLAG_INPUT | bias;
            if let Some((steady, mask)) = debounce {
                request.config.attrs[0] = ConfigAttribute {
                    attr: LineAttribute {
                        id: LINE_ATTR_ID_DEBOUNCE,
                        padding: 0,
                        // The kernel's attribute counts microseconds.
                        value: steady.as_micros() as u64,
                    },
                    mask,
                };
                request.config.num_attrs = 1;
            }
        }
    }
    Ok(request)
}

/// Lines of a chip, held by one request.
#[derive(Debug)]
pub struct Lines {
    /// The chip's character device, kept open: the request belongs to it
    /// for as long as it lives.
    _chip: File,
    /// The request's own descriptor, which the levels are read and the
    /// lines driven through.
    request: File,
}

impl Lines {
    /// Asks a chip for lines.
    ///
    /// # Arguments
    ///
    /// * `path`     - The chip's character device.
    /// * `offsets`  - The lines' offsets on it, in the request's order.
    /// * `consumer` - Whose they are, as `gpioinfo` will show them.
    /// * `settings` - How they are set.
    ///
    /// # Returns
    ///
    /// The lines, their descriptor non-blocking.
    ///
    /// # Errors
    ///
    /// What was refused, and the system's reason.
    pub fn request(
        path: &str,
        offsets: &[u32],
        consumer: &str,
        settings: &Settings,
    ) -> Result<Lines, Refused> {
        let mut request = request_for(offsets, consumer, settings)?;
        let chip = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(Refused::Open)?;
        // SAFETY: the structure is laid out as the kernel's own -- the tests
        // below hold it to the kernel's sizes -- and lives across the call,
        // which writes the descriptor into it.
        let asked = unsafe {
            libc::ioctl(chip.as_raw_fd(), GET_LINE_IOCTL, &mut request)
        };
        if asked < 0 {
            return Err(Refused::Request(std::io::Error::last_os_error()));
        }
        if request.fd < 0 {
            return Err(Refused::NoDescriptor);
        }
        // Owned at once, so that a failure below closes it rather than
        // leaving the lines held by a descriptor nobody has.
        //
        // SAFETY: the kernel has just given this descriptor, and nothing
        // else owns it.
        let lines = unsafe { File::from_raw_fd(request.fd) };
        set_nonblocking(&lines).map_err(Refused::NonBlocking)?;
        Ok(Lines {
            _chip: chip,
            request: lines,
        })
    }

    /// What the lines are.
    ///
    /// # Arguments
    ///
    /// * `mask` - Which lines of the request to ask about, by place.
    ///
    /// # Returns
    ///
    /// A bit per line, bit 0 the first line, set where it is asserted.
    ///
    /// # Errors
    ///
    /// What the kernel said where it will not say.
    pub fn levels(&self, mask: u64) -> std::io::Result<u64> {
        let mut values = LineValues { bits: 0, mask };
        // SAFETY: a structure laid out as the kernel's own living across a
        // call on a descriptor this owns.
        let result = unsafe {
            libc::ioctl(
                self.request.as_raw_fd(),
                LINE_GET_VALUES_IOCTL,
                &mut values as *mut LineValues,
            )
        };
        if result < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(values.bits)
    }

    /// Drives lines.
    ///
    /// # Arguments
    ///
    /// * `bits` - What to drive them to, a bit for each by its place.
    /// * `mask` - Which lines, the same way.
    ///
    /// # Errors
    ///
    /// What the kernel said where it will not drive them.
    pub fn drive(&self, bits: u64, mask: u64) -> std::io::Result<()> {
        let values = LineValues { bits, mask };
        // SAFETY: a structure laid out as the kernel's own living across a
        // call on a descriptor this owns.
        let result = unsafe {
            libc::ioctl(
                self.request.as_raw_fd(),
                LINE_SET_VALUES_IOCTL,
                &values,
            )
        };
        if result < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    /// The same lines, on duplicates of these descriptors: the kernel gives
    /// a line to one request at a time, so a second request for the same
    /// lines would be refused.
    ///
    /// # Returns
    ///
    /// The lines again.
    ///
    /// # Errors
    ///
    /// What the system said where a descriptor cannot be duplicated.
    pub fn try_clone(&self) -> std::io::Result<Lines> {
        Ok(Lines {
            _chip: self._chip.try_clone()?,
            request: self.request.try_clone()?,
        })
    }

    /// Lines on a pipe in place of a chip and a request, for a test of what
    /// can be tested without a chip: the kernel refuses a values ioctl on a
    /// pipe, which is as far as such a test sees.
    ///
    /// # Returns
    ///
    /// The lines, and the write end of the pipe their request reads.
    ///
    /// # Errors
    ///
    /// What the system said where the pipe cannot be made.
    #[doc(hidden)]
    pub fn on_a_pipe() -> std::io::Result<(Lines, File)> {
        let (reader, writer) = std::io::pipe()?;
        let request = File::from(std::os::fd::OwnedFd::from(reader));
        set_nonblocking(&request)?;
        let writer = File::from(std::os::fd::OwnedFd::from(writer));
        Ok((
            Lines {
                _chip: writer.try_clone()?,
                request,
            },
            writer,
        ))
    }
}

impl Waitable for Lines {
    fn source(&self) -> Source {
        self.request.source()
    }
}

/// Makes reads of a file not wait: a read gathers everything waiting and
/// must not then wait for more.
///
/// # Arguments
///
/// * `file` - The file.
///
/// # Errors
///
/// What the system said.
fn set_nonblocking(file: &File) -> std::io::Result<()> {
    // SAFETY: both calls take the descriptor by value and return a status,
    // and `file` keeps it open across them.
    let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
    if flags < 0
        // SAFETY: as for F_GETFL.
        || unsafe {
            libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK)
        } < 0
    {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    /// Every structure is the size the kernel's own is.
    ///
    /// Measured from `linux/gpio.h`: 16, 24, 272, 592 and 16 bytes. A
    /// structure of the wrong size is an ioctl that reads or writes past
    /// what it was handed, and nothing else in a test would notice.
    #[test]
    fn the_structures_are_what_the_kernel_reads() {
        assert_eq!(mem::size_of::<LineAttribute>(), 16);
        assert_eq!(mem::size_of::<ConfigAttribute>(), 24);
        assert_eq!(mem::size_of::<LineConfig>(), 272);
        assert_eq!(mem::size_of::<LineRequest>(), 592);
        assert_eq!(mem::size_of::<LineValues>(), 16);

        // And the alignment the kernel's 64-bit fields need.
        assert_eq!(mem::align_of::<LineRequest>(), 8);
        assert_eq!(mem::align_of::<LineValues>(), 8);
    }

    /// A request asks for the lines named, as inputs, with the bias asked
    /// for, the debounce on the lines it names, and whose they are.
    #[test]
    fn a_request_asks_for_what_it_was_given() {
        let settings = Settings::Input {
            bias: Bias::PullUp,
            debounce: Some((Duration::from_micros(5_000), 0b111)),
        };
        let request =
            request_for(&[3, 7, 11], "discretes", &settings).expect("one");

        assert_eq!(request.num_lines, 3);
        assert_eq!(&request.offsets[..3], &[3, 7, 11]);
        assert_eq!(request.offsets[3], 0, "the rest left as the kernel wants");
        assert_eq!(
            request.config.flags,
            LINE_FLAG_INPUT | LINE_FLAG_BIAS_PULL_UP
        );
        assert_eq!(request.config.num_attrs, 1);
        assert_eq!(request.config.attrs[0].attr.id, LINE_ATTR_ID_DEBOUNCE);
        assert_eq!(request.config.attrs[0].attr.value, 5_000);
        assert_eq!(request.config.attrs[0].mask, 0b111);
        let consumer: Vec<u8> = request
            .consumer
            .iter()
            .take_while(|byte| **byte != 0)
            .map(|byte| *byte as u8)
            .collect();
        assert_eq!(String::from_utf8_lossy(&consumer), "discretes");
    }

    /// Driven lines ask for the output flag and nothing else.
    #[test]
    fn driven_lines_ask_only_to_be_driven() {
        let request = request_for(&[17, 18, 19], "relays", &Settings::Output)
            .expect("one");
        assert_eq!(request.config.flags, LINE_FLAG_OUTPUT);
        assert_eq!(request.config.num_attrs, 0);
    }

    /// No lines, and more than a request holds, are refused.
    #[test]
    fn a_request_must_hold_some_lines_and_not_too_many() {
        assert!(matches!(
            request_for(&[], "none", &Settings::Output),
            Err(Refused::HowMany(0))
        ));
        let many: Vec<u32> = (0..65).collect();
        assert!(matches!(
            request_for(&many, "many", &Settings::Output),
            Err(Refused::HowMany(65))
        ));
    }
}
