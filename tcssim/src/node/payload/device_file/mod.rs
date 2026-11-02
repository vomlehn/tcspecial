//! A simulated device standing in for a device file: a file of text.
//!
//! A device file is opened and read by tcspecial itself, so what there is to
//! stand in for is something at the path that produces data. The simulator
//! makes a regular file there and lengthens it with a packet of lorem ipsum
//! every packet interval, segment by segment, as the pacing says, until the
//! text runs out. tcspecial reads what has been added each time it looks, and
//! once the text is all there it reads nothing more: the data is sent for a
//! while, and then stops. The faults apply to the text as to any packet: a
//! dropped packet takes no text, and a truncated or corrupted one goes as it
//! was cut or changed.
//!
//! The file is the simulator's own. It is made new at the start, and removed
//! when the payload stops. A path where there is already something else -- a
//! real device, or a file the simulator did not write -- is left alone and
//! reported: a simulator that replaced `/dev/urandom` to stand in for it
//! would be worse than one that could not. A file left by a run that ended
//! without removing it is recognised by holding the start of the text, and
//! is replaced.
//!
//! See [`crate::payload`] for that pacing, which is common to every kind.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::payload::{
    a_packet_from, could_not_stand_in, lateness, send_in_segments,
    wait_out_packet, wait_while_running, Pacing, PayloadConfig, PayloadStats,
    Produce,
};

/// The text the file is made of, a packet at a time.
pub(crate) const LOREM: &str = include_str!("lorem.txt");

/// The file a simulated device file writes its text into, removed when it
/// is dropped.
struct TextFile {
    file: File,
    path: PathBuf,
}

impl TextFile {
    /// Makes the file at `path`, empty, for the simulator to lengthen.
    ///
    /// # Arguments
    ///
    /// * `path` - The path the device-file payload names.
    ///
    /// # Returns
    ///
    /// The file, open for appending.
    ///
    /// # Errors
    ///
    /// Why it was not made: something that is not a file this simulator
    /// wrote is at the path, or the file cannot be made or replaced there.
    fn make(path: &Path) -> Result<TextFile, String> {
        if let Ok(found) = std::fs::symlink_metadata(path) {
            let ours = found.is_file()
                && std::fs::read(path)
                    .is_ok_and(|held| LOREM.as_bytes().starts_with(&held));
            if !ours {
                return Err(format!(
                    "{} is there already and is not a file of text this \
                     simulator made: it is left alone",
                    path.display()
                ));
            }
            std::fs::remove_file(path).map_err(|e| {
                format!(
                    "{}, left by an earlier run, cannot be removed: {e}",
                    path.display()
                )
            })?;
        }
        let file = OpenOptions::new()
            .append(true)
            .create_new(true)
            .open(path)
            .map_err(|e| format!("{} cannot be made: {e}", path.display()))?;
        Ok(TextFile {
            file,
            path: path.to_path_buf(),
        })
    }
}

impl Drop for TextFile {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.path) {
            eprintln!("{} cannot be removed: {e}", self.path.display());
        }
    }
}

/// Runs a simulated device file: a file of lorem ipsum, lengthened a packet
/// at a time, paced and faulted as the simulator file says, until the text
/// runs out.
///
/// # Arguments
///
/// * `config`  - The payload's configuration.
/// * `running` - Cleared to stop it; the function returns soon after.
/// * `stats`   - Where it counts what it sends and receives.
pub fn run_device_file_node(
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
) {
    let mut text = match TextFile::make(Path::new(&config.address)) {
        Ok(text) => text,
        Err(why) => {
            could_not_stand_in(
                &config,
                format!("failed to stand in for a device file: {why}"),
            );
            return;
        }
    };
    config.standing.made();

    let mut rng = rand::thread_rng();
    // Packets that have gone, which the faults that are counted rather than
    // random ask about.
    let mut sent = 0u64;
    // How much of the text has been taken.
    let mut taken = 0usize;

    while running.load(Ordering::SeqCst) {
        let started = Instant::now();
        let pacing = Pacing::read(&config);

        if taken < LOREM.len() && pacing.a_packet_is_due(config.triggered, 0) {
            let late = lateness(&config, &mut rng);
            let produced =
                a_packet_from(&config, &pacing, sent, &mut rng, |_, size| {
                    let end = (taken + size).min(LOREM.len());
                    let packet = LOREM.as_bytes()[taken..end].to_vec();
                    taken = end;
                    packet
                });
            let packet = match produced {
                Produce::Send(packet) => packet,
                // Dropped, or gone quiet: nothing goes, and nothing
                // is counted, because nothing moved.
                Produce::Nothing => Vec::new(),
            };
            // However late this one is being made, before it goes.
            wait_while_running(late, &running);
            let (bytes, whole) = send_in_segments(
                &packet,
                pacing.segment_size,
                pacing.segment_interval,
                &running,
                &mut |segment| {
                    text.file.write_all(segment)?;
                    Ok(segment.len())
                },
            );

            if bytes > 0 {
                let mut guard = stats.lock().unwrap();
                guard.bytes_sent += bytes;
                if whole {
                    guard.a_packet_has_gone(&packet);
                    sent += 1;
                }
            }
        }

        wait_out_packet(&pacing, started, &running);
    }

    // The file goes with the payload; see TextFile::drop. Named here so that
    // it is plain the file is not merely dropped at the end of a scope.
    drop(text);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::{PayloadProtocol, SimulatedPayload};
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;

    /// Makes a simulated device file that adds a packet every 10 ms, whole.
    ///
    /// # Arguments
    ///
    /// * `path`   - The path the payload names.
    /// * `packet` - Its packet size in bytes.
    ///
    /// # Returns
    ///
    /// The device, not yet started.
    fn device_at(path: &Path, packet: u32) -> SimulatedPayload {
        SimulatedPayload::new(PayloadConfig {
            attachment: None,
            thermostats: Vec::new(),
            name: "DH0".to_string(),
            protocol: PayloadProtocol::Device,
            address: path.display().to_string(),
            port: 0,
            bus_address: 0,
            can_id: 0,
            can_fd: false,
            gpio_lines: Vec::new(),
            faults: Default::default(),
            own_address: None,
            own_port: None,
            packet_size: Arc::new(AtomicU32::new(packet)),
            segment_size: Arc::new(AtomicU32::new(packet)),
            packet_interval_ms: Arc::new(AtomicU32::new(10)),
            segment_interval_ms: Arc::new(AtomicU32::new(0)),
            sending: Arc::new(AtomicBool::new(true)),
            standing: Default::default(),
            driven: false,
            triggered: false,
        })
    }

    /// The file is made where nothing was, and removed when it is dropped.
    #[test]
    fn the_file_is_made_and_removed() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("payload");
        let text = TextFile::make(&path).expect("made");
        assert!(path.is_file());
        assert_eq!(std::fs::read(&path).unwrap(), b"");
        drop(text);
        assert!(!path.exists(), "removed");
    }

    /// A file an earlier run left, holding the start of the text, is
    /// replaced; anything else at the path is left alone.
    #[test]
    fn only_a_file_of_the_text_is_replaced() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("payload");
        std::fs::write(&path, &LOREM.as_bytes()[..100]).unwrap();
        let text = TextFile::make(&path).expect("replaced");
        assert_eq!(std::fs::read(&path).unwrap(), b"", "made new");
        drop(text);

        std::fs::write(&path, b"somebody's notes").unwrap();
        let why = TextFile::make(&path).err().expect("left alone");
        assert!(why.contains("left alone"), "{why}");
        assert_eq!(std::fs::read(&path).unwrap(), b"somebody's notes");

        let why = TextFile::make(Path::new("/dev/null"))
            .err()
            .expect("a device is left alone");
        assert!(why.contains("left alone"), "{why}");
    }

    /// The text is what a reader of the file finds, whole and in order, a
    /// packet at a time, and then nothing more; and the file goes when the
    /// payload stops.
    #[test]
    fn the_text_goes_into_the_file_and_then_stops() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("payload");
        let mut device = device_at(&path, 512);
        device.start().expect("started");

        let deadline = Instant::now() + Duration::from_secs(10);
        let mut read = Vec::new();
        while read.len() < LOREM.len() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
            read = std::fs::read(&path).unwrap_or_default();
        }
        assert_eq!(read, LOREM.as_bytes(), "the whole text, in order");
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            std::fs::read(&path).unwrap().len(),
            LOREM.len(),
            "and nothing after it"
        );
        let packets = LOREM.len().div_ceil(512) as u64;
        assert_eq!(device.stats().packets_sent, packets);

        device.stop();
        assert!(!path.exists(), "removed when it stops");
    }
}
