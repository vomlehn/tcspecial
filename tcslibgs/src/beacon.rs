//! The beacon, as it goes on the link.
//!
//! What tcspecial sends every `beacon.default_interval`, whether or not
//! anything has connected to it: the build it is, the digest of the
//! configuration it read, the beacon's own number, whether this start has
//! yet been matched by the ground, and which interface it speaks. A ground
//! station that is only listening can then see which software is flying,
//! which payload set it is serving and whether it has just come back, rather
//! than having to ask before it can know.
//!
//! A beacon is text, and nothing but text: its fields in order, a colon
//! between each, so that a person reading a packet capture or a log reads it
//! as it is, with no decoder. Numbers are decimal and digests hexadecimal:
//!
//! ```text
//! tcspecial:1.2.3:d41d8cd98f00b204e9800998ecf8427e:42:normal:0123...cdef
//! ```
//!
//! which is the name, the version, the configuration's digest, the beacon's
//! number, `restart` or `normal` for the restart flag, and the
//! interface's digest. It is read strictly: anything else at the beacon
//! port, or a field out of its form, is not a beacon. A beacon carries no
//! check of its own: the transport under it checks it, as UDP's checksum
//! does here.
//!
//! Stated here and in `python/tcslibgs/beacon.py`, which are held to each
//! other by the same examples in their tests.

use std::fmt;

use crate::config_digest::{ConfigDigest, ConfigVersion};
use crate::interface::InterfaceDigest;

/// The name every beacon starts with.
pub const BEACON_NAME: &str = "tcspecial";

/// What the restart flag's field says when the flag is set: this start has
/// not yet been matched, or has taken a RESTART.
pub const RESTART_FLAG: &str = "restart";
/// What it says when none is.
pub const NORMAL_FLAG: &str = "normal";

/// The longest a beacon is: the name, a version of three parts of up to
/// three digits, two digests of thirty-two hex digits, a number of up to ten
/// digits and the longer flag, with the five colons between them.
pub const BEACON_BYTES: usize = 9 + 11 + 32 + 10 + 7 + 32 + 5;

/// What a beacon says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Beacon {
    /// Which beacon this is, counted by the tcspecial that sent it from one.
    ///
    /// A beacon answers no command, so the number is the beacon's own: a
    /// ground station that sees 7 after 5 knows it missed one rather than
    /// wondering whether the spacecraft went quiet. Counted by the sender, so
    /// it starts again at one when tcspecial does; a number going backwards
    /// is therefore a restart, which is worth knowing and is not the same
    /// thing as a gap.
    pub sequence: u32,
    /// The version of the build that sent it, from its `Cargo.toml`.
    pub version: ConfigVersion,
    /// The digest of the configuration file the sender read, which is what
    /// says which payload set it is serving: a configuration states no version
    /// of its own.
    pub digest: ConfigDigest,
    /// The restart flag: set at every start of tcspecial -- the first, a
    /// commanded restart, a crash the supervisor restarted, a power cycle --
    /// and from the moment a RESTART is taken, and clear only once the start
    /// has taken a CONNECT whose configuration checksum is the one it runs.
    /// While it is set the vehicle sends the ground nothing but beacons and
    /// answers, and acts on no command but CONNECT, RESTART_ARM and RESTART.
    /// See "Configurations named by their checksum" in `docs/design.rst`.
    pub restart_flag: bool,
    /// The interface the sender speaks: the digest of the XTCE documents it
    /// was built from. See [`crate::interface`].
    pub interface: InterfaceDigest,
}

impl Beacon {
    /// Makes a beacon with its restart flag clear, speaking the
    /// interface this build was generated from.
    ///
    /// # Arguments
    ///
    /// * `sequence` - Which beacon this is, counted from one.
    /// * `version`  - The version of the build that sends it.
    /// * `digest`   - The digest of the configuration the sender read.
    ///
    /// # Returns
    ///
    /// The beacon, with `restart_flag` clear.
    pub fn new(
        sequence: u32,
        version: ConfigVersion,
        digest: ConfigDigest,
    ) -> Self {
        Self {
            sequence,
            version,
            digest,
            restart_flag: false,
            interface: InterfaceDigest::of_this_build(),
        }
    }

    /// Says whether the restart flag is set.
    ///
    /// # Arguments
    ///
    /// * `restart_flag` - Whether this start has not yet taken a matching
    ///                    CONNECT, or has taken a RESTART.
    ///
    /// # Returns
    ///
    /// This beacon, with `restart_flag` set to the value given.
    pub fn restart_flag(self, restart_flag: bool) -> Self {
        Self {
            restart_flag,
            ..self
        }
    }

    /// Reads the beacon a datagram holds.
    ///
    /// A datagram must be a beacon and nothing else, field for field: one
    /// that is longer, is not text, has another number of fields or a
    /// field out of its form is refused rather than read as far as it goes,
    /// so that something else sending to the beacon port is seen for what
    /// it is. Allocates nothing.
    ///
    /// # Arguments
    ///
    /// * `datagram` - The datagram received on the beacon port.
    ///
    /// # Returns
    ///
    /// The beacon it holds.
    ///
    /// # Errors
    ///
    /// [`BeaconError::Length`] if the datagram is longer than
    /// [`BEACON_BYTES`], and [`BeaconError::Field`] if it is not text or a
    /// field is not what a beacon says.
    pub fn read(datagram: &[u8]) -> Result<Beacon, BeaconError> {
        if datagram.len() > BEACON_BYTES {
            return Err(BeaconError::Length(datagram.len()));
        }
        let text = std::str::from_utf8(datagram)
            .map_err(|_| BeaconError::Field("it is not text"))?;
        let mut fields = text.split(':');
        let mut next = |what| fields.next().ok_or(BeaconError::Field(what));

        if next("it is empty")? != BEACON_NAME {
            return Err(BeaconError::Field("it does not start \"tcspecial:\""));
        }
        let version = version_of(next("it has no version")?)
            .ok_or(BeaconError::Field("the version is not three numbers"))?;
        let digest = hex_of(next("it has no configuration digest")?).ok_or(
            BeaconError::Field("the configuration digest is not 32 hex digits"),
        )?;
        let sequence = decimal_of(next("it has no number")?)
            .ok_or(BeaconError::Field("the number is not a number"))?;
        let restart_flag =
            match next("it has no restart flag")? {
                RESTART_FLAG => true,
                NORMAL_FLAG => false,
                _ => return Err(BeaconError::Field(
                    "the restart flag is neither \"restart\" nor \"normal\"",
                )),
            };
        let interface = hex_of(next("it has no interface digest")?).ok_or(
            BeaconError::Field("the interface digest is not 32 hex digits"),
        )?;
        if fields.next().is_some() {
            return Err(BeaconError::Field("it has more than six fields"));
        }
        Ok(Beacon {
            sequence,
            version,
            digest: ConfigDigest(digest),
            restart_flag,
            interface: InterfaceDigest(interface),
        })
    }

    /// This beacon as it goes on the link, in a vector of its own.
    ///
    /// For a beacon sent once, as a test sends one. Something that sends a
    /// beacon every interval packs it with a [`BeaconPacker`], which allocates
    /// nothing after it is made.
    pub fn to_bytes(&self) -> Vec<u8> {
        BeaconPacker::new().pack(self).to_vec()
    }
}

/// Reads a version written `major.minor.patch`, each part decimal digits
/// that fit a byte, and nothing else.
///
/// # Arguments
///
/// * `text` - The version.
///
/// # Returns
///
/// The version, or `None` where the text is not one.
fn version_of(text: &str) -> Option<ConfigVersion> {
    let mut parts = text.split('.');
    let mut part = || -> Option<u8> {
        let digits = parts.next()?;
        u8::try_from(decimal_of(digits)?).ok()
    };
    let version = ConfigVersion {
        major: part()?,
        minor: part()?,
        patch: part()?,
    };
    parts.next().is_none().then_some(version)
}

/// Reads a number written in decimal digits, and nothing else -- no sign, no
/// space -- that fits 32 bits.
///
/// # Arguments
///
/// * `text` - The digits.
///
/// # Returns
///
/// The number, or `None` where the text is not that.
fn decimal_of(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Reads sixteen bytes written as thirty-two lower-case hex digits, as the
/// digests' own `Display` writes them.
///
/// # Arguments
///
/// * `text` - The digits.
///
/// # Returns
///
/// The bytes, or `None` where the text is not that.
fn hex_of(text: &str) -> Option<[u8; 16]> {
    let text = text.as_bytes();
    if text.len() != 32 {
        return None;
    }
    let digit = |b: u8| match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    };
    let mut out = [0u8; 16];
    for (i, pair) in text.chunks(2).enumerate() {
        out[i] = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    Some(out)
}

/// Packs beacons into a buffer it keeps, for a loop that sends them.
///
/// The buffer is made here, once, as long as the longest beacon, and every
/// beacon is written into it in place.
#[derive(Debug, Clone)]
pub struct BeaconPacker {
    buffer: [u8; BEACON_BYTES],
}

impl BeaconPacker {
    /// Makes a packer, with its buffer and packet, ready to pack beacons.
    ///
    /// # Returns
    ///
    /// The packer.
    pub fn new() -> Self {
        Self {
            buffer: [0u8; BEACON_BYTES],
        }
    }

    /// Writes `beacon`: its text, in the packer's own buffer.
    ///
    /// # Arguments
    ///
    /// * `beacon` - The beacon to write.
    ///
    /// # Returns
    ///
    /// The beacon's text, in the packer's buffer, good until the next call.
    pub fn pack(&mut self, beacon: &Beacon) -> &[u8] {
        let mut out = Cursor {
            buffer: &mut self.buffer,
            at: 0,
        };
        // Every field fits: the buffer is as long as the longest beacon, so
        // the write cannot fail, and `fmt` into a fixed buffer allocates
        // nothing.
        let _ = fmt::write(
            &mut out,
            format_args!(
                "{BEACON_NAME}:{}:{}:{}:{}:{}",
                beacon.version,
                beacon.digest,
                beacon.sequence,
                if beacon.restart_flag {
                    RESTART_FLAG
                } else {
                    NORMAL_FLAG
                },
                beacon.interface
            ),
        );
        let at = out.at;
        &self.buffer[..at]
    }
}

impl Default for BeaconPacker {
    fn default() -> Self {
        Self::new()
    }
}

/// A place in a fixed buffer that `fmt` writes into.
struct Cursor<'a> {
    buffer: &'a mut [u8],
    at: usize,
}

impl fmt::Write for Cursor<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.at + s.len();
        let room = self.buffer.get_mut(self.at..end).ok_or(fmt::Error)?;
        room.copy_from_slice(s.as_bytes());
        self.at = end;
        Ok(())
    }
}

/// Why a datagram is not a beacon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BeaconError {
    /// It is this long, which is longer than any beacon.
    Length(usize),
    /// It does not say what a beacon says, and this is the first thing
    /// wrong with it.
    Field(&'static str),
}

impl fmt::Display for BeaconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BeaconError::Length(length) => write!(
                f,
                "{length} bytes, where a beacon is at most {BEACON_BYTES}: \
                 something other than tcspecial, or a tcspecial of another \
                 layout, is sending to the beacon port"
            ),
            BeaconError::Field(why) => write!(
                f,
                "not a beacon ({why}): something other than tcspecial, or a \
                 tcspecial of another layout, is sending to the beacon port"
            ),
        }
    }
}

impl std::error::Error for BeaconError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn version() -> ConfigVersion {
        ConfigVersion::of_text("1.2.3").expect("1.2.3 is a version")
    }

    /// Gives the digest the tests' beacons carry.
    ///
    /// # Returns
    ///
    /// A fixed, recognizable digest.
    fn digest() -> ConfigDigest {
        ConfigDigest([
            0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09,
            0x98, 0xec, 0xf8, 0x42, 0x7e,
        ])
    }

    /// The fields in order, as text, a colon between each.
    #[test]
    fn a_beacon_is_its_fields_in_order_as_text() {
        let beacon =
            Beacon::new(0x01020304, version(), digest()).restart_flag(true);
        let want = format!(
            "tcspecial:1.2.3:d41d8cd98f00b204e9800998ecf8427e:16909060:\
             restart:{}",
            InterfaceDigest::of_this_build()
        );
        assert_eq!(beacon.to_bytes(), want.as_bytes());
        let normal = Beacon::new(7, version(), digest()).to_bytes();
        assert!(
            String::from_utf8(normal).unwrap().contains(":7:normal:"),
            "the flag in words"
        );
    }

    /// The longest beacon there can be fits the packer, exactly.
    #[test]
    fn the_longest_beacon_is_beacon_bytes_long() {
        let longest = Beacon::new(
            u32::MAX,
            ConfigVersion {
                major: 255,
                minor: 255,
                patch: 255,
            },
            digest(),
        )
        .restart_flag(true);
        let packed = longest.to_bytes();
        assert_eq!(packed.len(), BEACON_BYTES);
        assert_eq!(Beacon::read(&packed), Ok(longest));
    }

    #[test]
    fn a_beacon_reads_back_as_itself() {
        let beacon = Beacon::new(7, version(), digest());
        assert_eq!(Beacon::read(&beacon.to_bytes()), Ok(beacon));
        let flagged = beacon.restart_flag(true);
        assert_eq!(Beacon::read(&flagged.to_bytes()), Ok(flagged));
    }

    #[test]
    fn one_packer_packs_beacon_after_beacon_in_the_one_buffer() {
        let mut packer = BeaconPacker::new();
        let first = packer.pack(&Beacon::new(1, version(), digest())).as_ptr();
        let other = Beacon::new(
            2,
            ConfigVersion::of_this_build(),
            ConfigDigest([9; 16]),
        );
        let second = packer.pack(&other);
        assert_eq!(second.as_ptr(), first);
        assert_eq!(Beacon::read(second), Ok(other));
    }

    /// A datagram is a beacon only where every field is in its form: what
    /// is read is what was written, and nothing is guessed at.
    #[test]
    fn a_field_out_of_its_form_is_not_a_beacon() {
        let good =
            String::from_utf8(Beacon::new(1, version(), digest()).to_bytes())
                .unwrap();
        assert!(Beacon::read(good.as_bytes()).is_ok());
        let interface = InterfaceDigest::of_this_build().to_string();
        let md5 = digest().to_string();
        let upper = md5.to_uppercase();
        for bad in [
            format!("Tcspecial:1.2.3:{md5}:1:normal:{interface}"),
            format!("tcspecial:1.2:{md5}:1:normal:{interface}"),
            format!("tcspecial:1.2.256:{md5}:1:normal:{interface}"),
            format!("tcspecial:1.2.3:{}:1:normal:{interface}", &md5[1..]),
            format!("tcspecial:1.2.3:{upper}:1:normal:{interface}"),
            format!("tcspecial:1.2.3:{md5}:+1:normal:{interface}"),
            format!("tcspecial:1.2.3:{md5}: 1:normal:{interface}"),
            format!("tcspecial:1.2.3:{md5}:4294967296:normal:{interface}"),
            format!("tcspecial:1.2.3:{md5}:1:yes:{interface}"),
            format!("tcspecial:1.2.3:{md5}:1:normal"),
            format!("tcspecial:1.2.3:{md5}:1:normal:{interface}:"),
            format!("{good}\n"),
        ] {
            assert!(
                matches!(
                    Beacon::read(bad.as_bytes()),
                    Err(BeaconError::Field(_))
                ),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_datagram_longer_than_a_beacon_or_not_text_is_not_one() {
        let long = vec![b'a'; BEACON_BYTES + 1];
        assert_eq!(
            Beacon::read(&long),
            Err(BeaconError::Length(BEACON_BYTES + 1))
        );
        assert!(matches!(
            Beacon::read(b"tcspecial:\xff"),
            Err(BeaconError::Field(_))
        ));
        assert!(matches!(Beacon::read(b""), Err(BeaconError::Field(_))));
    }
}
