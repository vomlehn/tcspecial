//! Formatting for the display
//!
//! What a panel's last-sent and last-received lines are made of: a time and a
//! few bytes of the transfer. Both come from QUERY_DH_SAMPLE telemetry, which
//! carries the time the data moved, so neither function asks the clock -- a
//! panel showing the time it noticed a transfer rather than the time of the
//! transfer would read as activity whenever it was looked at.

/// Format a timestamp for display
pub fn format_timestamp(seconds: u64, _nanos: u32) -> String {
    let total_secs = seconds % 86400; // Seconds in a day
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, secs)
}

/// Format bytes as hex string
pub fn bytes_to_hex(bytes: &[u8], max_len: usize) -> String {
    let display_bytes = if bytes.len() > max_len {
        &bytes[..max_len]
    } else {
        bytes
    };

    let hex: Vec<String> = display_bytes.iter().map(|b| format!("{:02X}", b)).collect();
    let mut result = hex.join(" ");

    if bytes.len() > max_len {
        result.push_str("...");
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(3661, 0), "01:01:01");
        assert_eq!(format_timestamp(0, 0), "00:00:00");
    }

    #[test]
    fn test_bytes_to_hex() {
        assert_eq!(bytes_to_hex(&[0x01, 0x02, 0x03], 10), "01 02 03");
        assert_eq!(bytes_to_hex(&[0x01, 0x02, 0x03, 0x04, 0x05], 3), "01 02 03...");
    }
}
