//! Configuration loading for TCSpecial

/// The two ends of the beacon link: `tcslibgs::BeaconLink`, under the name
/// this module has always called it.
///
/// The type is in the shared library because both ends need it and neither
/// owns it -- tcspecial sends and a ground station listens -- and the name
/// stays here because this is where a reader of the flight software looks for
/// it.
pub use tcslibgs::BeaconLink as Beacon;

/// Configuration constants
pub mod constants {
    use std::time::Duration;

    pub const BEACON_DEFAULT_MS: Duration = Duration::new(20, 0);

    /// Initial delay for endpoint retry
    pub const ENDPOINT_DELAY_INIT: Duration = Duration::from_millis(100);

    /// Maximum delay for endpoint retry
    pub const ENDPOINT_DELAY_MAX: Duration = Duration::from_secs(10);

    /// Maximum number of endpoint retries
    pub const ENDPOINT_MAX_RETRIES: u32 = 10;

    /// How long a data handler keeps trying to reach a payload that refuses
    /// the connection.
    ///
    /// Bounded, and well under the ground's command timeout, because StartDH
    /// is answered on the command interpreter's own thread: retrying for
    /// ENDPOINT_MAX_RETRIES with ENDPOINT_DELAY_MAX between attempts would
    /// take some forty seconds, holding up every other command and leaving
    /// the ground to time out rather than be told anything.
    ///
    /// Two seconds is enough for the case this exists for -- a simulated
    /// payload started a moment after its handler -- and short enough that a
    /// payload which is simply not there is reported promptly.
    pub const ENDPOINT_CONNECT_BUDGET: Duration = Duration::from_secs(2);

    /// Stream endpoint delay for collecting bytes
    pub const STREAM_EP_DELAY: Duration = Duration::from_millis(50);

    /// Default buffer size for endpoints
    pub const ENDPOINT_BUFFER_SIZE: usize = 4096;

    /// First part of every telemetry log segment file name
    pub const TELEMETRY_LOG_PREFIX: &str = "telem-";

    /// Last part of every telemetry log segment file name
    pub const TELEMETRY_LOG_SUFFIX: &str = ".tcslog";
}
