//! The CRC every packet on the link ends with, for what is not a packet.
//!
//! The packets' CRCs are computed by the code tcs-xtce generates, from the
//! `ErrorDetectCorrect` their XTCE states. What else needs a check of its own
//! -- the state file tcspecial keeps its generation number in -- uses the same
//! CRC through this, computed by the same tcs-xtce routine, so that there is
//! one CRC in the system and one implementation of it.

use tcs_xtce::packet::{checksum, Checksum};

/// The CCSDS CRC-16: polynomial `0x1021`, initial remainder `0xFFFF`, no
/// reflection and no final XOR. The CRC catalogue calls it CRC-16/IBM-3740.
pub const CCSDS_CRC16: Checksum = Checksum::Crc {
    width: 16,
    poly: 0x1021,
    init: 0xffff,
    final_xor: 0,
    reflect_data: false,
    reflect_remainder: false,
};

/// The CCSDS CRC-16 of `data`.
pub fn crc16(data: &[u8]) -> u16 {
    // Over whole bytes, which is every case: the routine refuses nothing it
    // is given in range, and the range is the data's own.
    checksum(&CCSDS_CRC16, data, 0, data.len() * 8, "crc16")
        .map(|crc| crc as u16)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The catalogue's check value: the CRC of the nine digits.
    #[test]
    fn the_crc_is_the_catalogues() {
        assert_eq!(crc16(b"123456789"), 0x29b1);
    }

    /// And it is the CRC the beacon ends with, which tcs-xtce computed from
    /// the beacon's XTCE.
    #[test]
    fn it_is_the_crc_the_beacon_carries() {
        let beacon = crate::Beacon::new(
            1,
            crate::ConfigVersion::of_this_build(),
            crate::ConfigDigest([7; 16]),
            1,
        )
        .to_bytes();
        let end = beacon.len() - 2;
        assert_eq!(
            crc16(&beacon[..end]).to_be_bytes(),
            beacon[end..],
            "the beacon's CRC"
        );
    }
}
