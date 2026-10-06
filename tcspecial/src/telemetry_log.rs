//! The telemetry log, shared by everything that sends telemetry.
//!
//! The log is opened once and handed to each sender, so that beacons written
//! from the beacon thread land in the same log as the command responses
//! written from the main loop. A log that only one sender could reach would
//! record part of what went to the ground, which is the one thing a
//! telemetry log exists to make reconstructable.

use std::sync::{Arc, Mutex};

use log::error;

use tcslibgs::{CIConfig, TcsError, TcsResult};
use tcslog::{Format, LogWrite, SEGMENT_FILE_HEADER_LEN};

use crate::config::constants::{TELEMETRY_LOG_PREFIX, TELEMETRY_LOG_SUFFIX};

/// A handle on the telemetry log.
///
/// Cloning yields another handle on the same log, which is how a sender on
/// its own thread records into it. A handle on nothing is what a
/// configuration naming no log directory produces: recording through it does
/// nothing, so no caller has to ask whether logging is on.
#[derive(Clone)]
pub struct TelemetryLog(Option<Arc<Mutex<LogWrite>>>);

impl TelemetryLog {
    /// Open the log the configuration describes.
    ///
    /// Call this before the main loop. `LogWrite::new` enumerates the log
    /// directory and is the one part of tcslog that allocates; recording a
    /// message afterwards does not, which is what lets the main loop stay
    /// free of allocation.
    pub fn open(config: &CIConfig) -> TcsResult<Self> {
        let Some(dir) = config.log_dir.as_deref() else {
            return Ok(TelemetryLog(None));
        };

        let log = LogWrite::new(
            dir,
            TELEMETRY_LOG_PREFIX,
            TELEMETRY_LOG_SUFFIX,
            SEGMENT_FILE_HEADER_LEN.saturating_add(config.log_segment_bytes),
            // TelemetryHeader carries its own Timestamp, so the 20-byte
            // VariableTsRc header would store a second copy of it. A length
            // is all this log needs to add.
            Format::VariableSimple,
            // No callbacks: filled segment files stay in the log directory
            // until something clears them. There is no downlink-of-files
            // path to hand them to yet.
            (),
        )
        .map_err(|e| TcsError::Log(format!("opening telemetry log in {dir}: {e}")))?;

        Ok(TelemetryLog(Some(Arc::new(Mutex::new(log)))))
    }

    /// A handle that records nothing.
    pub fn disabled() -> Self {
        TelemetryLog(None)
    }

    /// Whether this handle records anything.
    pub fn is_enabled(&self) -> bool {
        self.0.is_some()
    }

    /// Record one serialized telemetry message.
    ///
    /// Never fails the caller: losing a record is better than dropping the
    /// telemetry the ground is waiting on, so a failed write is reported and
    /// otherwise ignored.
    pub fn record(&self, data: &[u8]) {
        self.with(|log| {
            if let Err(e) = log.write(data) {
                error!("telemetry log write failed: {e}");
            }
        });
    }

    /// Flush what has been recorded.
    pub fn flush(&self) {
        self.with(|log| {
            if let Err(e) = log.flush() {
                error!("telemetry log flush failed: {e}");
            }
        });
    }

    /// Run `f` against the log, if there is one and it is still usable.
    fn with<F: FnOnce(&mut LogWrite)>(&self, f: F) {
        let Some(log) = self.0.as_ref() else {
            return;
        };

        match log.lock() {
            Ok(mut guard) => f(&mut guard),
            // A writer panicked holding the lock, so the log may be part way
            // through a record. Left alone rather than written past.
            Err(_) => error!("telemetry log unusable: a writer panicked"),
        }
    }
}
