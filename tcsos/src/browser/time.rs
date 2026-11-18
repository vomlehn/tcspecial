//! Time, in a page: the page's clock, since the standard library's
//! `Instant` panics in a browser and its `SystemTime` cannot be read.

pub use std::time::{Duration, TryFromFloatSecsError};
pub use web_time::{Instant, SystemTime, SystemTimeError, UNIX_EPOCH};
