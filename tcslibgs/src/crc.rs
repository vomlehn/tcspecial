//! The CRC for what has no transport to check it.
//!
//! Packets on the link carry none: the transport under them checks them.
//! What is kept on storage has nothing under it that does -- the state file
//! tcspecial keeps what must survive a restart in is one -- so it carries the
//! CCSDS CRC-16, computed through this by tcs-xtce's routine, so that there
//! is one CRC in the system and one implementation of it.

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
}
