//! Application state and logic for tcsmoc

use std::time::{SystemTime, UNIX_EPOCH};
use tcslibgs::Statistics;

/// Format a timestamp for display
pub fn format_timestamp(seconds: u64, _nanos: u32) -> String {
    let total_secs = seconds % 86400; // Seconds in a day
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, secs)
}

/// Format current time for display
pub fn current_time_str() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format_timestamp(duration.as_secs(), duration.subsec_nanos())
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

/// Data handler display state
#[derive(Clone, Default)]
pub struct DHDisplayState {
    pub dh_id: u32,
    pub status: String,
    pub last_sent_time: String,
    pub last_sent_data: String,
    pub last_recv_time: String,
    pub last_recv_data: String,
    pub stats: Statistics,
}

impl DHDisplayState {
    pub fn new(dh_id: u32) -> Self {
        Self {
            dh_id,
            status: "Stopped".to_string(),
            last_sent_time: "--:--:--".to_string(),
            last_sent_data: String::new(),
            last_recv_time: "--:--:--".to_string(),
            last_recv_data: String::new(),
            stats: Statistics::default(),
        }
    }

    pub fn update_stats(&mut self, stats: Statistics) {
        self.stats = stats;
    }

    pub fn update_sent(&mut self, data: &[u8]) {
        self.last_sent_time = current_time_str();
        self.last_sent_data = bytes_to_hex(data, 8);
    }

    pub fn update_recv(&mut self, data: &[u8]) {
        self.last_recv_time = current_time_str();
        self.last_recv_data = bytes_to_hex(data, 8);
    }
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
