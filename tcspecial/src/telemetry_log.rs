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
use crate::segsend::{Handoff, SegSend};

/// A handle on the telemetry log.
///
/// Cloning yields another handle on the same log, which is how a sender on
/// its own thread records into it. A handle on nothing is what a
/// configuration naming no log directory produces: recording through it does
/// nothing, so no caller has to ask whether logging is on.
#[derive(Clone)]
pub struct TelemetryLog(Option<Arc<Mutex<LogWrite<Handoff>>>>);

/// What is wrong with `dir` as the place the log is kept, said in the
/// configuration's terms, or `None` where there is a directory there.
///
/// Asked before the log is opened because what `tcslog` says of a directory
/// that is not there -- "invalid path name" -- names neither the directory nor
/// the setting. tcspecial makes no directory of its own: one named wrongly
/// would be a log kept somewhere nobody looks, and a vehicle refused at the
/// start is the one told so while somebody is watching.
fn unusable(dir: &str) -> Option<String> {
    let fix =
        "create it, or change log.directory in the configuration's tcspecial \
               section to a directory that is there";
    match std::fs::metadata(dir) {
        Ok(found) if found.is_dir() => None,
        Ok(_) => Some(format!(
            "the telemetry log directory, {dir}, is not a directory: {fix}"
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(format!(
            "the telemetry log directory, {dir}, does not exist: {fix}"
        )),
        Err(e) => Some(format!(
            "the telemetry log directory, {dir}, cannot be looked at ({e}): \
             {fix}"
        )),
    }
}

impl TelemetryLog {
    /// Open the log the configuration describes.
    ///
    /// Call this before the main loop. `LogWrite::new` enumerates the log
    /// directory and is the one part of tcslog that allocates; recording a
    /// message afterwards does not, which is what lets the main loop stay
    /// free of allocation.
    ///
    /// Each segment it fills is handed to `segsend`, and so are the ones a
    /// previous run left in the directory.
    pub fn open(config: &CIConfig, segsend: &SegSend) -> TcsResult<Self> {
        let Some(dir) = config.log_dir.as_deref() else {
            return Ok(TelemetryLog(None));
        };
        if let Some(why) = unusable(dir) {
            return Err(TcsError::Log(why));
        }
        let handoff = segsend
            .handoff(std::path::Path::new(dir))
            .map_err(TcsError::Log)?;

        let log = LogWrite::new(
            dir,
            TELEMETRY_LOG_PREFIX,
            TELEMETRY_LOG_SUFFIX,
            SEGMENT_FILE_HEADER_LEN.saturating_add(config.log_segment_bytes),
            // TelemetryHeader carries its own Timestamp, so the 20-byte
            // VariableTsRc header would store a second copy of it. A length
            // is all this log needs to add.
            Format::VariableSimple,
            // Each filled segment goes to segsend, which is what keeps the
            // directory from filling over a long flight.
            handoff,
        )
        .map_err(|e| {
            TcsError::Log(format!("opening telemetry log in {dir}: {e}"))
        })?;

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
    fn with<F: FnOnce(&mut LogWrite<Handoff>)>(&self, f: F) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory is a place for the log; nothing there, or a file there,
    /// is refused, naming it and the setting to change.
    #[test]
    fn a_log_directory_that_is_not_one_is_refused_by_name() {
        let dir = tempfile::tempdir().expect("a directory");
        let here = dir.path().to_str().expect("a path");
        assert_eq!(unusable(here), None);

        let missing = dir.path().join("telemetry");
        let missing = missing.to_str().expect("a path");
        let said = unusable(missing).expect("nothing is there");
        assert!(said.contains(missing), "{said}");
        assert!(said.contains("does not exist"), "{said}");
        assert!(said.contains("log.directory"), "{said}");

        let file = dir.path().join("a file");
        std::fs::write(&file, b"").expect("written");
        let file = file.to_str().expect("a path");
        let said = unusable(file).expect("a file is not a directory");
        assert!(said.contains("is not a directory"), "{said}");
    }
}
