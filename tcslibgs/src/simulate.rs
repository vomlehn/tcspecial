//! The simulation mode: how many of a run's payloads may be simulated or
//! dummy devices.
//!
//! Given to tcspecial, tcssim and tcsmoc as `--simulate=<mode>` and never
//! written in a configuration, so that the file flown is the file tested,
//! byte for byte. Read here, once, so that the three programs take the flag
//! the same way and refuse the same mistakes in the same words. See
//! "Simulation Modes" in `docs/design.rst`.
//!
//! What the mode is checked against is here too: whether each payload is a
//! real device, a dummy device or a simulated device, [`judge`], found from
//! what is at the end of its endpoint and not from what any file says, and
//! whether a run's payloads are what its mode allows, [`refusals`].
//!
//! A simulated device is one tcssim makes: a pseudo-terminal, a socket. A
//! dummy device is one the kernel makes under a real device's name: the
//! i2c-stub's adapter, a gpio-sim chip, a vcan interface, an iio_dummy
//! sensor, which the scripts in `bin/` make. How each type's are told apart
//! is that type's own rule, beside its other rules in
//! `crate::node::payload`; this says which of them a node is asked, and what
//! the answers come to.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::gateway::is_gateway;
use crate::graph::reach::reach_of;
use crate::graph::Node;
use crate::node::payload;
use crate::types::{DHType, EndpointConfig};

/// The flag, as it is written before its `=`.
pub const FLAG: &str = "--simulate";

/// How many of a run's payloads may be simulated or dummy devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Every payload must be a real device: the flight setting, and the
    /// setting a run gets when the flag is left out.
    None,
    /// At least one payload is a simulated or dummy device and at least one
    /// is not.
    Some,
    /// Every payload is a simulated or dummy device, so that no hardware is
    /// exercised at all.
    All,
}

impl Mode {
    /// Every mode, in the words the flag takes.
    pub const NAMES: [&'static str; 3] = ["none", "some", "all"];

    /// Names this mode as the flag takes it.
    ///
    /// # Returns
    ///
    /// `none`, `some` or `all`.
    pub fn name(self) -> &'static str {
        match self {
            Mode::None => "none",
            Mode::Some => "some",
            Mode::All => "all",
        }
    }

    /// Reads the mode a flag's word names.
    ///
    /// # Arguments
    ///
    /// * `name` - The word after `--simulate=`.
    ///
    /// # Returns
    ///
    /// The mode, or `None` for a word that names none.
    fn from_name(name: &str) -> Option<Mode> {
        match name {
            "none" => Some(Mode::None),
            "some" => Some(Mode::Some),
            "all" => Some(Mode::All),
            _ => None,
        }
    }

    /// Whether this is the setting a vehicle flies with.
    pub fn is_flight(self) -> bool {
        self == Mode::None
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Takes `--simulate=<mode>` out of a program's arguments, giving the mode and
/// what is left.
///
/// The mode is `none` when the flag is not there, so that a flight launch
/// that leaves it out gets the flight setting. A mode that is not one of the
/// three, a `--simulate` with no mode, and the flag given twice are each
/// refused, naming the three modes: a second `--simulate` is more likely a
/// mistake about which run this is than something meant to override the
/// first.
///
/// # Arguments
///
/// * `args` - The program's arguments.
///
/// # Returns
///
/// The mode, and the arguments without the flag, in their order.
///
/// # Errors
///
/// A message naming the three modes, for a mode that is not one of them, a
/// `--simulate` with no mode, or the flag given twice.
pub fn take_mode(args: Vec<String>) -> Result<(Mode, Vec<String>), String> {
    let modes = Mode::NAMES.join(", ");
    let mut mode = None;
    let mut rest = Vec::with_capacity(args.len());
    for arg in args {
        let Some(after) = arg.strip_prefix(FLAG) else {
            rest.push(arg);
            continue;
        };
        let Some(name) = after.strip_prefix('=') else {
            if after.is_empty() {
                return Err(format!(
                    "{FLAG} needs a mode, written {FLAG}=<mode>, \
                     where the mode is one of {modes}"
                ));
            }
            // Some other flag that begins the same way: not this one's to
            // judge.
            rest.push(arg);
            continue;
        };
        let Some(this) = Mode::from_name(name) else {
            return Err(format!(
                "{FLAG}={name} is not a simulation mode: give one of {modes}"
            ));
        };
        if mode.is_some() {
            return Err(format!(
                "{FLAG} is given twice: give it once, with one of {modes}"
            ));
        }
        mode = Some(this);
    }
    Ok((mode.unwrap_or(Mode::None), rest))
}

/// What a payload was found to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// A simulated device: one tcssim makes, a pseudo-terminal or a socket.
    Simulated,
    /// A dummy device: one the kernel makes under a real device's name, such
    /// as the i2c-stub's adapter, which the scripts in `bin/` make.
    Dummy,
    /// Hardware, the real device: what was found is neither of those.
    Hardware,
    /// Nothing could be told: what is there could not be looked at, or says
    /// nothing either way. Counted as hardware, because failing that way
    /// round is the point of the mode -- a simulated or dummy device mistaken
    /// for hardware costs a refused run, and hardware mistaken for either
    /// costs the hardware.
    CannotTell,
    /// Nothing is there at all: no file at the path, no interface of the
    /// name. Counted as hardware for the mode, as what cannot be told is; and
    /// told apart from it for tcssim, for which nothing at a path is where a
    /// simulated device it makes is about to go.
    Absent,
}

/// What one payload was found to be, and how to make it simulated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judged {
    pub verdict: Verdict,
    /// What was found, as a clause about what the node reaches:
    /// `/dev/i2c-1 is a real adapter (i915 gmbus)`.
    pub found: String,
    /// The attribute that says what the node reaches: `path`, `interface` or
    /// `address`. What a refusal says to change.
    pub attribute: &'static str,
    /// What would make the payload simulated, in the configuration's terms:
    /// `point path at the i2c-stub bus, which bin/mki2c makes`.
    pub to_simulate: &'static str,
}

impl Judged {
    /// Judges a payload a simulated device.
    ///
    /// # Arguments
    ///
    /// * `found`       - What was found, as a clause about what the node
    ///                   reaches.
    /// * `attribute`   - The attribute that says what the node reaches.
    /// * `to_simulate` - What would make the payload simulated.
    ///
    /// # Returns
    ///
    /// The judgement.
    pub fn simulated(
        found: String,
        attribute: &'static str,
        to_simulate: &'static str,
    ) -> Judged {
        Judged {
            verdict: Verdict::Simulated,
            found,
            attribute,
            to_simulate,
        }
    }

    /// Judges a payload a dummy device.
    ///
    /// # Arguments
    ///
    /// * `found`       - What was found, as a clause about what the node
    ///                   reaches.
    /// * `attribute`   - The attribute that says what the node reaches.
    /// * `to_simulate` - What would make the payload simulated.
    ///
    /// # Returns
    ///
    /// The judgement.
    pub fn dummy(
        found: String,
        attribute: &'static str,
        to_simulate: &'static str,
    ) -> Judged {
        Judged {
            verdict: Verdict::Dummy,
            found,
            attribute,
            to_simulate,
        }
    }

    /// Judges a payload hardware.
    ///
    /// # Arguments
    ///
    /// * `found`       - What was found, as a clause about what the node
    ///                   reaches.
    /// * `attribute`   - The attribute that says what the node reaches.
    /// * `to_simulate` - What would make the payload simulated.
    ///
    /// # Returns
    ///
    /// The judgement.
    pub fn hardware(
        found: String,
        attribute: &'static str,
        to_simulate: &'static str,
    ) -> Judged {
        Judged {
            verdict: Verdict::Hardware,
            found,
            attribute,
            to_simulate,
        }
    }

    /// Judges a payload one about which nothing could be told.
    ///
    /// # Arguments
    ///
    /// * `found`       - What was found, as a clause about what the node
    ///                   reaches.
    /// * `attribute`   - The attribute that says what the node reaches.
    /// * `to_simulate` - What would make the payload simulated.
    ///
    /// # Returns
    ///
    /// The judgement.
    pub fn cannot_tell(
        found: String,
        attribute: &'static str,
        to_simulate: &'static str,
    ) -> Judged {
        Judged {
            verdict: Verdict::CannotTell,
            found,
            attribute,
            to_simulate,
        }
    }

    /// Judges a payload one with nothing where it should be.
    ///
    /// # Arguments
    ///
    /// * `found`       - What was found, as a clause about what the node
    ///                   reaches.
    /// * `attribute`   - The attribute that says what the node reaches.
    /// * `to_simulate` - What would make the payload simulated.
    ///
    /// # Returns
    ///
    /// The judgement.
    pub fn absent(
        found: String,
        attribute: &'static str,
        to_simulate: &'static str,
    ) -> Judged {
        Judged {
            verdict: Verdict::Absent,
            found,
            attribute,
            to_simulate,
        }
    }

    /// Whether the payload was found to be a simulated device.
    pub fn is_simulated(&self) -> bool {
        self.verdict == Verdict::Simulated
    }

    /// Whether the payload was found to be a dummy device.
    pub fn is_dummy(&self) -> bool {
        self.verdict == Verdict::Dummy
    }

    /// Whether the payload was found to be a simulated or a dummy device:
    /// anything but the real device, which is what the mode counts.
    pub fn is_simulated_or_dummy(&self) -> bool {
        self.is_simulated() || self.is_dummy()
    }

    /// Names what was found, with its article: for a message.
    ///
    /// # Returns
    ///
    /// "a simulated device", "a dummy device" or "hardware".
    fn kind(&self) -> &'static str {
        match self.verdict {
            Verdict::Simulated => "a simulated device",
            Verdict::Dummy => "a dummy device",
            _ => "hardware",
        }
    }

    /// Says what was found, in a word and a clause: for the startup summary.
    ///
    /// # Returns
    ///
    /// The verdict in a word, then what was found.
    pub fn said(&self) -> String {
        let word = match self.verdict {
            Verdict::CannotTell | Verdict::Absent => {
                "taken for hardware, as what could not be told"
            }
            _ => self.kind(),
        };
        format!("{word}: {}", self.found)
    }
}

/// Where a host's devices are described: `/sys`, or a tree made to look like
/// it in a test.
///
/// Every judgement that looks past a path reads the kernel's account of what
/// is there, which is the one place a bus number or a chip says what it is.
#[derive(Debug, Clone)]
pub struct Host {
    pub sys: PathBuf,
}

impl Host {
    /// This host.
    pub fn this() -> Host {
        Host {
            sys: PathBuf::from("/sys"),
        }
    }

    /// Finds the kernel's directory for a character device.
    ///
    /// Through `/sys/dev/char/<major>:<minor>`, which every character device
    /// the kernel made has: a device node's name says nothing, since anyone
    /// can make one called anything, and its numbers say which device it is.
    ///
    /// # Arguments
    ///
    /// * `path` - The device node's path, followed through any links.
    ///
    /// # Returns
    ///
    /// The device's directory under this host's `/sys`.
    ///
    /// # Errors
    ///
    /// [`Unseen`], saying as a clause what was found at the path instead:
    /// nothing, something that is not a character device, or one the kernel
    /// does not describe.
    pub fn device_of(&self, path: &str) -> Result<PathBuf, Unseen> {
        let (major, minor) = character_device(path)?;
        let entry = self.sys.join(format!("dev/char/{major}:{minor}"));
        std::fs::canonicalize(&entry).map_err(|_| Unseen {
            absent: false,
            said: format!(
                "{path} is a character device ({major}:{minor}) the kernel \
                 does not describe in {}",
                entry.display()
            ),
        })
    }
}

/// What was at a path instead of the device looked for, as a clause, and
/// whether it was nothing at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unseen {
    pub absent: bool,
    pub said: String,
}

impl Unseen {
    /// Makes what was seen the judgement of a payload.
    ///
    /// # Arguments
    ///
    /// * `attribute`   - The attribute that says what the node reaches.
    /// * `to_simulate` - What would make the payload simulated.
    ///
    /// # Returns
    ///
    /// Absent when nothing was at the path, and cannot-tell otherwise.
    pub fn judged(
        self,
        attribute: &'static str,
        to_simulate: &'static str,
    ) -> Judged {
        if self.absent {
            Judged::absent(self.said, attribute, to_simulate)
        } else {
            Judged::cannot_tell(self.said, attribute, to_simulate)
        }
    }
}

/// Reads the device numbers of a character device, following links.
///
/// # Arguments
///
/// * `path` - The device node's path.
///
/// # Returns
///
/// Its major and minor numbers.
///
/// # Errors
///
/// [`Unseen`], saying as a clause what is there instead: nothing, which is
/// absent, or something that cannot be looked at or is not a character
/// device.
pub(crate) fn character_device(path: &str) -> Result<(u64, u64), Unseen> {
    let unseen = |said: String| Unseen {
        absent: false,
        said,
    };
    let found = match std::fs::metadata(path) {
        Ok(found) => found,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Unseen {
                absent: true,
                said: format!("nothing is at {path}"),
            })
        }
        Err(e) => {
            return Err(unseen(format!("{path} cannot be looked at ({e})")))
        }
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        if !found.file_type().is_char_device() {
            return Err(unseen(format!("{path} is {}", kind_of_file(&found))));
        }
        Ok(split_device(found.rdev()))
    }
    #[cfg(not(unix))]
    {
        let _ = found;
        Err(unseen(format!(
            "{path} cannot be looked at on this platform"
        )))
    }
}

/// Says what a file that is not a character device is, for a clause.
///
/// # Arguments
///
/// * `found` - The file's metadata.
///
/// # Returns
///
/// What it is, such as "a directory".
#[cfg(unix)]
fn kind_of_file(found: &std::fs::Metadata) -> &'static str {
    use std::os::unix::fs::FileTypeExt;
    let kind = found.file_type();
    if kind.is_dir() {
        "a directory"
    } else if kind.is_file() {
        "an ordinary file"
    } else if kind.is_fifo() {
        "a pipe"
    } else if kind.is_socket() {
        "a socket"
    } else if kind.is_block_device() {
        "a block device"
    } else {
        "not a character device"
    }
}

/// A device number's major and minor halves, as Linux packs them.
fn split_device(device: u64) -> (u64, u64) {
    let major = ((device >> 8) & 0xfff) | ((device >> 32) & !0xfff);
    let minor = (device & 0xff) | ((device >> 12) & !0xff);
    (major, minor)
}

/// The majors Linux gives a pseudo-terminal's slave: 136 to 143.
const PTY_SLAVE_MAJORS: std::ops::RangeInclusive<u64> = 136..=143;

/// Judges whether a path resolves to a pseudo-terminal's slave, which is what
/// tcssim makes for a payload reached through a device node: the one
/// judgement of serial, spi, rpmsg and device-file payloads, and the second
/// of an iio one.
///
/// By the device's numbers, after following every link: tcssim links the
/// payload's path to the slave it made, and a name under `/dev/pts` is only a
/// name.
///
/// # Arguments
///
/// * `path`        - The device node's path.
/// * `attribute`   - The attribute that names it.
/// * `to_simulate` - What would make the payload simulated.
///
/// # Returns
///
/// A simulated device for a pseudo-terminal's slave, hardware for any other
/// character device, and what [`Unseen::judged`] says for anything else.
pub(crate) fn pseudo_terminal(
    path: &str,
    attribute: &'static str,
    to_simulate: &'static str,
) -> Judged {
    match character_device(path) {
        Err(found) => found.judged(attribute, to_simulate),
        Ok((major, minor)) if PTY_SLAVE_MAJORS.contains(&major) => {
            let resolved = std::fs::canonicalize(path)
                .map(|to| to.display().to_string())
                .unwrap_or_else(|_| format!("{major}:{minor}"));
            Judged::simulated(
                format!("{path} resolves to a pseudo-terminal, {resolved}"),
                attribute,
                to_simulate,
            )
        }
        Ok((major, minor)) => Judged::hardware(
            format!(
                "{path} is a character device ({major}:{minor}) and not a \
                 pseudo-terminal"
            ),
            attribute,
            to_simulate,
        ),
    }
}

/// Whether `node` is counted by the mode: a node of an interface type other
/// than the gateway, `network` and `device-file`.
///
/// Computations, logs, tees and app nodes have no hardware behind them to
/// simulate -- an app node's program is the thing itself -- and the
/// gateway is the link the run is commanded over, set up by whoever started
/// the ground station. A `network` or `device-file` node is a real device,
/// a socket or a file, whatever answers at its far end: tcssim may be what
/// does, where the simulator file says so, but the node is the same node
/// either way, so there is nothing for the mode to tell apart.
///
/// # Arguments
///
/// * `node` - The node.
///
/// # Returns
///
/// Whether the mode counts it.
pub fn counted(node: &Node) -> bool {
    !is_gateway(&node.node_type)
        && DHType::from_spelling(&node.node_type).is_some_and(|kind| {
            !matches!(kind, DHType::Network | DHType::Device)
        })
}

/// Judges what a node was found to be under a mode, where the mode counts it.
///
/// From what is at the end of the endpoint tcspecial will open under `mode`,
/// by its type's own rule: its own endpoint, or the UDP socket its
/// `sim_address` names where the run simulates -- see
/// `crate::graph::reach::Reach::opened`. Asked
/// before anything is opened: opening is not always harmless, since a serial
/// port raises its modem lines as it opens and a GPIO line requested as an
/// output is driven the moment the request is granted.
///
/// # Arguments
///
/// * `host` - The host, whose `/sys` is read.
/// * `node` - The node judged.
/// * `mode` - The run's mode, which says which endpoint is opened.
///
/// # Returns
///
/// The judgement, naming `sim_address` as what to change where that is what is
/// opened, or `None` for a node the mode does not count.
pub fn judge(host: &Host, node: &Node, mode: Mode) -> Option<Judged> {
    if !counted(node) {
        return None;
    }
    let reach = match reach_of(node) {
        Ok(reach) => reach,
        Err(why) => {
            return Some(Judged::cannot_tell(
                why,
                "path",
                "correct the node, which is refused as it is",
            ))
        }
    };
    let judged = match &reach.opened(mode) {
        // A node reached at its sim_address, the only network endpoint a
        // counted node opens.
        EndpointConfig::Network(at) => payload::network::judged(at),
        // Never opened by a counted node: see `counted`.
        EndpointConfig::Device(at) => Judged::hardware(
            format!("{} is a device file, which is a real device", at.path),
            "path",
            "nothing: a device-file node is a real device",
        ),
        EndpointConfig::Serial(at) => payload::serial::judged(&at.path),
        EndpointConfig::I2c(at) => payload::i2c::judged(host, &at.bus),
        EndpointConfig::Spi(at) => payload::spi::judged(&at.path),
        EndpointConfig::I2cTmass(at) => {
            payload::i2c_tmass::judged(host, &at.bus)
        }
        EndpointConfig::SpiTmass(at) => payload::spi_tmass::judged(&at.path),
        EndpointConfig::Can(at) => payload::can::judged(host, &at.interface),
        EndpointConfig::Iio(at) => payload::iio::judged(host, &at.path),
        EndpointConfig::Rpmsg(at) => payload::rpmsg::judged(&at.path),
        EndpointConfig::Gpio(at) => payload::gpio::judged(host, &at.path),
        EndpointConfig::Hdlc(at) => payload::hdlc::judged(&at.interface),
    };
    // Reached at its sim_address, which is what to change, where it is.
    if reach.sim_address.is_some() && !mode.is_flight() {
        return Some(Judged {
            attribute: "sim_address",
            to_simulate: "give sim_address as a loopback address, such as \
                 127.0.0.1:7950",
            ..judged
        });
    }
    Some(judged)
}

/// Lists everything about a run's payloads that a mode forbids.
///
/// Every refusal rather than the first, as every other check here gives
/// them: a run with three payloads on the wrong bus is one trip to a desk.
///
/// # Arguments
///
/// * `mode`     - The run's mode.
/// * `payloads` - Each payload's name and judgement.
///
/// # Returns
///
/// The refusals, each naming the node, what it reaches, what was found
/// there and the mode, and saying what would satisfy the mode. Empty where
/// the run may go ahead.
pub fn refusals(mode: Mode, payloads: &[(&str, &Judged)]) -> Vec<String> {
    let flag = format!("{FLAG}={mode}");
    match mode {
        Mode::None => payloads
            .iter()
            .filter(|(_, judged)| judged.is_simulated_or_dummy())
            .map(|(name, judged)| {
                format!(
                    "{name}: {}, {}, and {flag}, the flight setting, allows \
                     only real devices: point {} at the hardware, or give \
                     {FLAG}=some or {FLAG}=all for a run that is not a flight",
                    judged.found,
                    judged.kind(),
                    judged.attribute
                )
            })
            .collect(),
        Mode::All => payloads
            .iter()
            .filter(|(_, judged)| !judged.is_simulated_or_dummy())
            .map(|(name, judged)| {
                let found = match judged.verdict {
                    Verdict::CannotTell | Verdict::Absent => format!(
                        "{}, so whether it is a real device cannot be told",
                        judged.found
                    ),
                    _ => judged.found.clone(),
                };
                format!(
                    "{name}: {found}, and {flag} allows only simulated and \
                     dummy devices: {}",
                    judged.to_simulate
                )
            })
            .collect(),
        Mode::Some => {
            let unreal = payloads
                .iter()
                .filter(|(_, j)| j.is_simulated_or_dummy())
                .count();
            if payloads.is_empty() {
                vec![format!(
                    "{flag} wants at least one payload that is a real device \
                     and one that is not, and this configuration has no \
                     payloads the mode counts (network and device-file \
                     payloads are real devices, and not counted): give \
                     {FLAG}=all or {FLAG}=none"
                )]
            } else if unreal == 0 {
                vec![format!(
                    "{flag} wants at least one payload that is a simulated or \
                     dummy device, and none of the {} is one ({}): give \
                     {FLAG}=none for a run on hardware alone, or simulate one \
                     of them",
                    payloads.len(),
                    names(payloads)
                )]
            } else if unreal == payloads.len() {
                vec![format!(
                    "{flag} wants at least one payload that is a real device, \
                     and none is ({}): give {FLAG}=all for a run on simulated \
                     and dummy devices alone",
                    names(payloads)
                )]
            } else {
                Vec::new()
            }
        }
    }
}

/// Whether tcssim makes a simulated device of this type for itself -- a
/// pseudo-terminal, a socket -- rather than finding a dummy device the kernel
/// made, as it does for an I2C bus, a GPIO chip and a CAN interface, which the
/// scripts in `bin/` make.
///
/// Nothing at the path of one tcssim makes for itself is where it is about to
/// go; nothing where the kernel's should be is a script that was not run.
///
/// # Arguments
///
/// * `node_type` - The payload's type.
///
/// # Returns
///
/// Whether tcssim makes what it simulates the payload at.
pub fn tcssim_makes(node_type: &str) -> bool {
    !matches!(
        node_type,
        "i2c" | "gpio_in" | "gpio_out" | "gpio_inout" | "can" | "hdlc"
    )
}

/// Says what is wrong, if anything, with a payload's `simulate` against what
/// is at its endpoint: for tcssim, which compares the two before it
/// simulates anything. See "Saying Which Payloads Are Simulated" in
/// docs/design.rst.
///
/// `simulate: true` where the endpoint is real hardware is a configuration
/// pointing at hardware in a file that says it is simulated; `simulate:
/// false` where it is a simulated or dummy device is a payload meant to be
/// real that the run would in fact simulate. Nothing at the path is a
/// disagreement only for a dummy device, which the kernel has to have made
/// before tcssim starts.
///
/// A payload the simulator file does not name is the real device; one whose
/// endpoint is a simulated or dummy device all the same is refused, so that
/// every payload simulated is one tcssim controls and shows.
///
/// # Arguments
///
/// * `name`      - The payload's name.
/// * `node_type` - Its node type, which says whether tcssim makes a
///                 simulated device for it or finds a dummy device.
/// * `simulate`  - Its `simulate`, or `None` where the simulator file does
///                 not name it.
/// * `judged`    - What was found at its endpoint.
///
/// # Returns
///
/// What is wrong, or `None` when the two agree.
pub fn disagreement(
    name: &str,
    node_type: &str,
    simulate: Option<bool>,
    judged: &Judged,
) -> Option<String> {
    let found = &judged.found;
    let Some(simulate) = simulate else {
        return judged.is_simulated_or_dummy().then(|| {
            format!(
                "{name}: {found}, {}, and the simulator file does not name \
                 it: name it with simulate: true, for tcssim to control it \
                 and show it as every other simulated payload is, or point {} \
                 at the real device",
                judged.kind(),
                judged.attribute
            )
        });
    };
    match (simulate, judged.verdict) {
        (true, Verdict::Simulated | Verdict::Dummy)
        | (false, Verdict::Hardware | Verdict::CannotTell) => None,
        (false, Verdict::Absent) => None,
        (true, Verdict::Absent) if tcssim_makes(node_type) => None,
        (true, _) => Some(format!(
            "{name}: {found}, and the simulator file says simulate: true: {}, \
             or say simulate: false for the real device",
            judged.to_simulate
        )),
        (false, Verdict::Simulated | Verdict::Dummy) => Some(format!(
            "{name}: {found}, {}, and the simulator file says simulate: \
             false: point {} at the real device, or say simulate: true",
            judged.kind(),
            judged.attribute
        )),
    }
}

/// Lists payloads' names for a message.
///
/// # Arguments
///
/// * `payloads` - Each payload's name and judgement.
///
/// # Returns
///
/// The names, joined with commas.
fn names(payloads: &[(&str, &Judged)]) -> String {
    payloads
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Whether `path` is a directory, for a judgement that asks whether the
/// kernel lists something.
pub(crate) fn is_dir(path: &Path) -> bool {
    path.is_dir()
}

/// The first line of a file the kernel keeps, trimmed, where it can be read.
pub(crate) fn read_line(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(of: &[&str]) -> Vec<String> {
        of.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn left_out_it_is_the_flight_setting() {
        let (mode, rest) = take_mode(args(&["tcspecial", "x.ytcs"])).unwrap();
        assert_eq!(mode, Mode::None);
        assert!(mode.is_flight());
        assert_eq!(rest, args(&["tcspecial", "x.ytcs"]));
    }

    #[test]
    fn each_mode_is_taken_out_wherever_it_is() {
        for name in Mode::NAMES {
            let flag = format!("--simulate={name}");
            let (mode, rest) =
                take_mode(args(&["tcspecial", &flag, "x.ytcs"])).unwrap();
            assert_eq!(mode.name(), name);
            assert_eq!(rest, args(&["tcspecial", "x.ytcs"]));
            let (after, _) =
                take_mode(args(&["tcspecial", "x.ytcs", &flag])).unwrap();
            assert_eq!(after, mode);
        }
    }

    #[test]
    fn a_mistaken_mode_is_refused_naming_the_three() {
        for wrong in [
            &["p", "--simulate=most"][..],
            &["p", "--simulate"],
            &["p", "--simulate="],
            &["p", "--simulate=all", "--simulate=all"],
            &["p", "--simulate=all", "--simulate=none"],
        ] {
            let said = take_mode(args(wrong)).unwrap_err();
            assert!(said.contains("none, some, all"), "{said}");
        }
    }

    /// Makes the judgement of an I2C payload on the stub's adapter.
    ///
    /// # Returns
    ///
    /// A dummy device.
    fn dummy() -> Judged {
        Judged::dummy(
            "/dev/i2c-7 is the i2c-stub's adapter".into(),
            "path",
            "fix it",
        )
    }

    /// Makes the judgement of a serial payload on tcssim's pseudo-terminal.
    ///
    /// # Returns
    ///
    /// A simulated device.
    fn simulated() -> Judged {
        Judged::simulated(
            "/tmp/ttyS0 resolves to a pseudo-terminal, /dev/pts/3".into(),
            "path",
            "fix it",
        )
    }

    /// Makes the judgement of an I2C payload on a real adapter.
    ///
    /// # Returns
    ///
    /// Hardware.
    fn hardware() -> Judged {
        Judged::hardware(
            "/dev/i2c-1 is a real adapter (gmbus)".into(),
            "path",
            "fix it",
        )
    }

    fn unknown() -> Judged {
        Judged::cannot_tell("nothing is at /dev/i2c-9".into(), "path", "fix it")
    }

    #[test]
    fn none_refuses_each_unreal_device_saying_which_flag_a_test_wants() {
        let (s, h, m) = (dummy(), hardware(), simulated());
        let said = refusals(Mode::None, &[("a", &s), ("b", &h), ("c", &m)]);
        assert_eq!(said.len(), 2, "{said:?}");
        assert!(said[0].starts_with(
            "a: /dev/i2c-7 is the i2c-stub's adapter, a dummy device"
        ));
        assert!(
            said[1].starts_with(
                "c: /tmp/ttyS0 resolves to a pseudo-terminal, /dev/pts/3, a \
                 simulated device"
            ),
            "{}",
            said[1]
        );
        assert!(
            said[0].contains("--simulate=some or --simulate=all"),
            "{}",
            said[0]
        );
        assert!(refusals(Mode::None, &[("b", &h)]).is_empty());
        assert!(refusals(Mode::None, &[]).is_empty());
    }

    #[test]
    fn all_refuses_real_devices_and_says_what_could_not_be_told() {
        let (s, h, u) = (dummy(), hardware(), unknown());
        let said = refusals(Mode::All, &[("a", &s), ("b", &h), ("c", &u)]);
        assert_eq!(
            said,
            [
                "b: /dev/i2c-1 is a real adapter (gmbus), and --simulate=all \
                 allows only simulated and dummy devices: fix it",
                "c: nothing is at /dev/i2c-9, so whether it is a real device \
                 cannot be told, and --simulate=all allows only simulated and \
                 dummy devices: fix it",
            ]
        );
        assert!(refusals(Mode::All, &[("a", &s)]).is_empty());
        assert!(refusals(Mode::All, &[("a", &simulated())]).is_empty());
        // Nothing to simulate is nothing exercised.
        assert!(refusals(Mode::All, &[]).is_empty());
    }

    #[test]
    fn some_wants_one_of_each_and_counts_what_could_not_be_told_as_hardware() {
        let (s, h, u) = (dummy(), hardware(), unknown());
        assert!(refusals(Mode::Some, &[("a", &s), ("b", &h)]).is_empty());
        assert!(refusals(Mode::Some, &[("a", &s), ("c", &u)]).is_empty());
        let said = refusals(Mode::Some, &[("a", &s), ("b", &s)]);
        assert!(said[0].contains("give --simulate=all"), "{said:?}");
        let said = refusals(Mode::Some, &[("b", &h)]);
        assert!(said[0].contains("give --simulate=none"), "{said:?}");
        assert_eq!(refusals(Mode::Some, &[]).len(), 1);
    }

    #[test]
    fn simulate_disagrees_with_hardware_and_with_a_dummy_meant_to_be_real() {
        let (s, h, u) = (dummy(), hardware(), unknown());
        let absent =
            Judged::absent("nothing is at /tmp/ttyS9".into(), "path", "fix it");

        assert_eq!(disagreement("a", "i2c", Some(true), &s), None);
        assert_eq!(disagreement("a", "i2c", Some(false), &h), None);
        assert_eq!(disagreement("a", "i2c", Some(false), &u), None);
        let said = disagreement("a", "i2c", Some(true), &h).expect("refused");
        assert!(
            said.starts_with("a: /dev/i2c-1 is a real adapter (gmbus)"),
            "{said}"
        );
        assert!(said.contains("simulate: true: fix it"), "{said}");
        let said = disagreement("a", "i2c", Some(false), &s).expect("refused");
        assert!(said.contains(
            "a dummy device, and the simulator file says simulate: false"
        ));

        // Nothing yet where tcssim makes the simulated device itself is where
        // it is about to go; nothing where the kernel's dummy should be is
        // not.
        assert_eq!(disagreement("a", "serial", Some(true), &absent), None);
        assert!(disagreement("a", "i2c", Some(true), &absent).is_some());

        // A dummy device the file leaves out is one tcssim would not
        // control.
        let said = disagreement("a", "gpio_in", None, &s).expect("refused");
        assert!(said.contains("does not name it"), "{said}");
        assert_eq!(disagreement("a", "gpio_in", None, &h), None);
    }

    /// What the mode counts: the payloads reached over an interface, and not
    /// a network or device-file node, the gateway, a computation, a log or an
    /// app.
    #[test]
    fn the_mode_counts_interfaces_but_not_real_devices_or_the_gateway() {
        let node = |node_type: &str| Node {
            name: "n".into(),
            node_type: node_type.into(),
            group: None,
            attributes: Default::default(),
        };
        for counted_type in
            ["i2c", "serial", "hdlc", "gpio_in", "gpio_out", "gpio_inout"]
        {
            assert!(counted(&node(counted_type)), "{counted_type}");
        }
        for not in [
            "network",
            "device-file",
            "gateway_radio",
            "compute",
            "log",
            "app_pl",
            "tee-gateway",
            "ping",
        ] {
            assert!(!counted(&node(not)), "{not}");
        }
    }
}
