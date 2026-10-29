//! The beacon, as it goes on the link.
//!
//! What tcspecial sends every `beacon.default_interval`, whether or not anything has
//! connected to it: the build it is, the digest of the configuration it read,
//! the beacon's own number, how many times tcspecial has started, whether the
//! ground commanded this start, and which interface it speaks. A ground station that
//! is only listening can then see which software is flying, which payload set
//! it is serving and whether it has just come back, rather than having to ask
//! before it can know.
//!
//! The layout is stated once, in XTCE, in `tcslibgs/xtce/beacon.xml`, and
//! packed and unpacked by the code tcs-xtce generates from it: the Rust in
//! `beacon_packet.rs` here, and the Python in `python/tcslibgs/`. Neither is
//! written by hand, so the two ends, and the two languages, cannot come to
//! disagree about where a byte goes.
//!
//! ```text
//! "tcspecial " major minor patch " " digest[16] sequence[4] generation[4]
//! commanded_restart:1 spare:7 interface[16]
//! ```
//!
//! The numbers are most significant byte first. A beacon carries no CRC of
//! its own: the transport under it checks it, as UDP's checksum does here.

use std::fmt;

use tcs_xtce::packet::{Codec, Packet, PacketError};

use crate::beacon_packet;
use crate::config_digest::{ConfigDigest, ConfigVersion};
use crate::interface::InterfaceDigest;

/// How long a beacon is: every one is exactly this long.
pub const BEACON_BYTES: usize = beacon_packet::Beacon::MAX_BYTES;

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
    /// How many times tcspecial has started on this vehicle, kept in its
    /// state file across every restart, crash and power cycle.
    ///
    /// The ground judges a restart by this changing, not by its growing: a
    /// state file that was damaged starts the count again from one, and a
    /// count that went down is a restart as surely as one that went up.
    pub generation: u32,
    /// Whether this start of tcspecial is one the ground commanded: an armed
    /// RESTART -- or, once there are load sets, a SWITCH -- was taken by the
    /// start before, which is how a new set of definitions is taken up.
    ///
    /// Set in every beacon of such a start, and clear in every beacon of one
    /// nobody commanded -- a crash the supervisor restarted, a power cycle --
    /// which runs with the definitions it had, so that the interface does not
    /// change under a ground station that did not ask for it to.
    pub commanded_restart: bool,
    /// The interface the sender speaks: the digest of the XTCE documents it
    /// was built from. See [`crate::interface`].
    pub interface: InterfaceDigest,
}

impl Beacon {
    /// A beacon from a start nobody commanded, speaking the interface this
    /// build was generated from.
    pub fn new(
        sequence: u32,
        version: ConfigVersion,
        digest: ConfigDigest,
        generation: u32,
    ) -> Self {
        Self {
            sequence,
            version,
            digest,
            generation,
            commanded_restart: false,
            interface: InterfaceDigest::of_this_build(),
        }
    }

    /// This beacon, saying whether the ground commanded this start.
    pub fn commanded_restart(self, commanded_restart: bool) -> Self {
        Self {
            commanded_restart,
            ..self
        }
    }

    /// The beacon a datagram holds, or why it holds none.
    ///
    /// A datagram must be a beacon and nothing else: one of any other length
    /// is refused rather than read as far as a beacon goes, so that something
    /// else sending to the beacon port is seen for what it is.
    pub fn read(datagram: &[u8]) -> Result<Beacon, BeaconError> {
        if datagram.len() != BEACON_BYTES {
            return Err(BeaconError::Length(datagram.len()));
        }
        // Allocates nothing: the codec is generated without dynamic
        // allocation, so the digests are fixed buffers.
        let packet = beacon_packet::Beacon::decode(datagram)
            .map_err(BeaconError::Packet)?;
        let mut digest = [0u8; 16];
        digest.copy_from_slice(&packet.digest);
        let mut interface = [0u8; 16];
        interface.copy_from_slice(&packet.interface);
        Ok(Beacon {
            sequence: packet.sequence,
            version: ConfigVersion {
                major: packet.major,
                minor: packet.minor,
                patch: packet.patch,
            },
            digest: ConfigDigest(digest),
            generation: packet.generation,
            commanded_restart: packet.commanded_restart,
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

/// Packs beacons into a buffer it keeps, for a loop that sends them.
///
/// Everything a beacon is packed with is made here, once: the buffer, sized
/// for a beacon, and the packet tcs-xtce packs from.
#[derive(Debug, Clone)]
pub struct BeaconPacker {
    codec: Codec,
    packet: beacon_packet::Beacon,
}

impl BeaconPacker {
    pub fn new() -> Self {
        Self {
            codec: Codec::for_packet::<beacon_packet::Beacon>(),
            packet: beacon_packet::Beacon::default(),
        }
    }

    /// `beacon`, packed: [`BEACON_BYTES`] bytes in the packer's own buffer.
    pub fn pack(&mut self, beacon: &Beacon) -> &[u8] {
        self.packet.sequence = beacon.sequence;
        self.packet.major = beacon.version.major;
        self.packet.minor = beacon.version.minor;
        self.packet.patch = beacon.version.patch;
        self.packet.generation = beacon.generation;
        self.packet.commanded_restart = beacon.commanded_restart;
        // Every field is a fixed size and every value fits it, so there is
        // nothing a beacon can hold that does not pack: the digests are
        // sixteen bytes into buffers of sixteen.
        self.packet.digest.clear();
        let _ = self.packet.digest.extend_from_slice(&beacon.digest.0);
        self.packet.interface.clear();
        let _ = self.packet.interface.extend_from_slice(&beacon.interface.0);
        self.codec
            .encode(&self.packet)
            .expect("every beacon fits the beacon layout")
    }
}

impl Default for BeaconPacker {
    fn default() -> Self {
        Self::new()
    }
}

/// Why a datagram is not a beacon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BeaconError {
    /// It is this long, which no beacon is.
    Length(usize),
    /// It is the right length, and does not say what a beacon says.
    Packet(PacketError),
}

impl fmt::Display for BeaconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BeaconError::Length(length) => write!(
                f,
                "{length} bytes, where a beacon is {BEACON_BYTES}: something \
                 other than tcspecial, or a tcspecial of another layout, is \
                 sending to the beacon port"
            ),
            BeaconError::Packet(why) => write!(
                f,
                "not a beacon ({why}): something other than tcspecial is \
                 sending to the beacon port"
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

    fn digest() -> ConfigDigest {
        ConfigDigest([
            0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09,
            0x98, 0xec, 0xf8, 0x42, 0x7e,
        ])
    }

    #[test]
    fn a_beacon_is_its_fields_in_order() {
        let beacon = Beacon::new(0x01020304, version(), digest(), 0x0a0b0c0d)
            .commanded_restart(true);
        let packed = beacon.to_bytes();
        let mut want = b"tcspecial ".to_vec();
        want.extend([1, 2, 3]);
        want.push(b' ');
        want.extend(digest().0);
        want.extend([1, 2, 3, 4]);
        want.extend([0x0a, 0x0b, 0x0c, 0x0d]);
        want.push(0x80);
        want.extend(InterfaceDigest::of_this_build().0);
        assert_eq!(packed, want);
        assert_eq!(packed.len(), BEACON_BYTES);
        assert_eq!(BEACON_BYTES, 55);
    }

    #[test]
    fn a_beacon_reads_back_as_itself() {
        let beacon = Beacon::new(7, version(), digest(), 3);
        assert_eq!(Beacon::read(&beacon.to_bytes()), Ok(beacon));
        let commanded = beacon.commanded_restart(true);
        assert_eq!(Beacon::read(&commanded.to_bytes()), Ok(commanded));
    }

    #[test]
    fn one_packer_packs_beacon_after_beacon_in_the_one_buffer() {
        let mut packer = BeaconPacker::new();
        let first = packer
            .pack(&Beacon::new(1, version(), digest(), 1))
            .as_ptr();
        let other = Beacon::new(
            2,
            ConfigVersion::of_this_build(),
            ConfigDigest([9; 16]),
            2,
        );
        let second = packer.pack(&other);
        assert_eq!(second.as_ptr(), first);
        assert_eq!(Beacon::read(second), Ok(other));
    }

    /// The spare bits are nought, and a beacon with any of them set is not
    /// one this ground reads: a layout that used them would be another.
    #[test]
    fn a_beacon_with_a_spare_bit_set_is_refused() {
        let mut packed = Beacon::new(1, version(), digest(), 1).to_bytes();
        packed[38] |= 0x01;
        assert!(matches!(
            Beacon::read(&packed),
            Err(BeaconError::Packet(PacketError::Mismatch { .. }))
        ));
    }

    #[test]
    fn a_datagram_of_another_length_is_not_a_beacon() {
        let mut packed = Beacon::new(1, version(), digest(), 1).to_bytes();
        packed.push(0);
        assert_eq!(
            Beacon::read(&packed),
            Err(BeaconError::Length(BEACON_BYTES + 1))
        );
        assert_eq!(Beacon::read(b""), Err(BeaconError::Length(0)));
    }

    #[test]
    fn a_datagram_without_the_name_or_the_space_is_not_a_beacon() {
        let good = Beacon::new(1, version(), digest(), 1).to_bytes();
        let mut renamed = good.clone();
        renamed[0] = b'T';
        assert!(matches!(
            Beacon::read(&renamed),
            Err(BeaconError::Packet(PacketError::Mismatch { .. }))
        ));
        let mut unspaced = good;
        unspaced[13] = b'-';
        assert!(matches!(
            Beacon::read(&unspaced),
            Err(BeaconError::Packet(PacketError::Mismatch { .. }))
        ));
    }
}
