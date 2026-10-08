//! Simulated payloads: what one is, and the pace it keeps.
//!
//! A simulated payload stands in for the hardware at the far end of a data
//! handler's payload endpoint. What it does is the same whatever that
//! endpoint is -- produce a packet every packet interval, in segments of its
//! own, from Start until Stop -- and this module holds that: the
//! configuration, the statistics, the thread, and the pacing.
//!
//! How the bytes actually leave depends on the endpoint, and each kind has a
//! file of its own:
//!
//! * [`crate::payload_tcp`] -- a stream the handler connects to
//! * [`crate::payload_udp`] -- datagrams sent to where the handler waits
//! * [`crate::payload_device`] -- a device, which produces rather than sends
//! * [`crate::payload_serial`] -- a line, which is a pty the configured path
//!   leads to
//! * [`crate::payload_i2c`] -- a device on a bus, which is a chip of the
//!   kernel's `i2c-stub`
//! * [`crate::payload_spi`] -- a peripheral, which is a pty again: the bytes
//!   of one, and none of its clocking
//! * [`crate::payload_unix`] -- a Unix-domain socket of either flavour, which
//!   the simulator simply is

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::os::unix::fs::{symlink, FileTypeExt};
use std::path::Path;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rand::Rng;
use tcslibgs::DHSample;

use tcslibgs::Faults;

/// Payload configuration
#[derive(Clone)]
pub struct PayloadConfig {
    pub _id: u32,
    pub protocol: PayloadProtocol,
    /// Where the payload is, in the terms its kind is located by: a host for
    /// a network payload, a path for every other kind.
    pub address: String,
    /// The port, for a network payload; zero for the kinds that have none.
    pub port: u16,
    /// The address of the device on its bus, for an I2C payload; zero for the
    /// kinds that are not on one. Not the `port`, which a bus has not, and
    /// not the `address`, which for a bus is the bus itself.
    pub bus_address: u16,
    /// Whether this payload answers requests rather than sending on its own.
    ///
    /// From the payload configuration, not the simulator's: which kind of
    /// payload a payload is, is a property of the payload. A triggered one
    /// sends one packet for each trigger it is sent, and nothing otherwise.
    pub triggered: bool,
    /// What this payload is asked to do wrong; see [`Faults`].
    pub faults: Faults,
    /// The socket this payload binds for itself, where the simulator file
    /// states one: a host and a port for a UDP payload, a path for a Unix
    /// datagram one.
    ///
    /// The other end of the link from `address` and `port`, which are the
    /// handler's. Only the datagram kinds have one to bind: a stream payload
    /// is the end that waits, so it binds the handler's address. `None` is
    /// any interface and a port the system chooses, which is what every
    /// simulator file asked for before this could be stated.
    pub own_address: Option<String>,
    pub own_port: Option<u16>,
    pub packet_size: Arc<AtomicU32>,
    pub segment_size: Arc<AtomicU32>,
    pub packet_interval_ms: Arc<AtomicU32>,
    pub segment_interval_ms: Arc<AtomicU32>,
}

/// Payload protocol type
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PayloadProtocol {
    Tcp,
    Udp,
    Device,
    /// A serial line, which the simulator stands in for with a pty: see
    /// [`crate::payload_serial`].
    Serial,
    /// A device on an I2C bus, which the simulator stands in for with the
    /// kernel's `i2c-stub`: see [`crate::payload_i2c`].
    I2c,
    /// A SPI peripheral, which the simulator stands in for with a pty -- the
    /// bytes but not the clocking, there being nothing that emulates a
    /// peripheral: see [`crate::payload_spi`].
    Spi,
    /// A Unix-domain stream, which the simulator listens at: see
    /// [`crate::payload_unix`].
    UnixStream,
    /// A Unix-domain datagram socket, which the simulator sends to from a path
    /// of its own: see [`crate::payload_unix`].
    UnixDgram,
}

/// Statistics for a payload
#[derive(Default)]
pub struct PayloadStats {
    pub packets_sent: u64,
    pub packets_recv: u64,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
    /// The last whole packet each way: when it moved and the head of it.
    ///
    /// The same sample tcspecial keeps for a handler and answers
    /// QUERY_DH_SAMPLE with, so the panel at this end of a link and the panel
    /// at the other end show the one transfer the same way. Empty until
    /// something has moved: a payload that has sent nothing has no time to
    /// show, which is a different thing from having sent something at
    /// midnight.
    pub last_sent: DHSample,
    pub last_recv: DHSample,
}

impl PayloadStats {
    /// A whole packet has gone, now, and this is what it was.
    ///
    /// Counted, timed and sampled in one call so the three cannot come apart:
    /// a count that moved without the time moving would have a panel showing
    /// traffic at a time it had stopped, which is worse than showing neither.
    pub fn a_packet_has_gone(&mut self, packet: &[u8]) {
        self.packets_sent += 1;
        self.last_sent.record(packet);
    }

    /// And one has arrived.
    pub fn a_packet_has_come(&mut self, packet: &[u8]) {
        self.packets_recv += 1;
        self.last_recv.record(packet);
    }
}

/// Simulated payload
pub struct SimulatedPayload {
    config: PayloadConfig,
    running: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
    stats: Arc<std::sync::Mutex<PayloadStats>>,
}

impl SimulatedPayload {
    /// Create a new simulated payload
    pub fn new(config: PayloadConfig) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: None,
            stats: Arc::new(std::sync::Mutex::new(PayloadStats::default())),
        }
    }

    /// Start the payload simulation
    pub fn start(&mut self) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Err("Already running".to_string());
        }

        // What this payload was told to do wrong, said once. A run whose
        // payload was dropping a tenth of its packets should not have to be
        // guessed at from the counters afterwards.
        if self.config.faults.any() {
            eprintln!("injecting faults: {:?}", self.config.faults);
        }

        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();
        let config = self.config.clone();
        let stats = self.stats.clone();

        let handle = thread::spawn(move || {
            match config.protocol {
                PayloadProtocol::Tcp => crate::payload_tcp::run_tcp_payload(config, running, stats),
                PayloadProtocol::Udp => crate::payload_udp::run_udp_payload(config, running, stats),
                PayloadProtocol::Device => {
                    crate::payload_device::run_device_payload(config, running, stats)
                }
                PayloadProtocol::Serial => {
                    crate::payload_serial::run_serial_payload(config, running, stats)
                }
                PayloadProtocol::I2c => {
                    crate::payload_i2c::run_i2c_payload(config, running, stats)
                }
                PayloadProtocol::Spi => {
                    crate::payload_spi::run_spi_payload(config, running, stats)
                }
                PayloadProtocol::UnixStream => {
                    crate::payload_unix::run_unix_stream_payload(config, running, stats)
                }
                PayloadProtocol::UnixDgram => {
                    crate::payload_unix::run_unix_dgram_payload(config, running, stats)
                }
            }
        });

        self.thread_handle = Some(handle);
        Ok(())
    }

    /// Stop the payload simulation
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    /// Check if the payload is running
    pub fn _is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get the current statistics
    pub fn stats(&self) -> PayloadStats {
        let guard = self.stats.lock().unwrap();
        PayloadStats {
            packets_sent: guard.packets_sent,
            packets_recv: guard.packets_recv,
            bytes_sent: guard.bytes_sent,
            bytes_recv: guard.bytes_recv,
            last_sent: guard.last_sent,
            last_recv: guard.last_recv,
        }
    }

    /// Update packet size
    pub fn set_packet_size(&self, size: u32) {
        self.config.packet_size.store(size, Ordering::SeqCst);
    }

    /// Update segment size
    pub fn set_segment_size(&self, size: u32) {
        self.config.segment_size.store(size, Ordering::SeqCst);
    }

    /// Update packet interval
    pub fn set_packet_interval(&self, interval_ms: u32) {
        self.config.packet_interval_ms.store(interval_ms, Ordering::SeqCst);
    }

    /// Update segment interval
    pub fn set_segment_interval(&self, interval_ms: u32) {
        self.config.segment_interval_ms.store(interval_ms, Ordering::SeqCst);
    }
}

impl Drop for SimulatedPayload {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Make `path` a link to `leads_to`, refusing to displace anything else.
///
/// A link the simulator itself left behind -- from a run that was killed
/// rather than stopped -- is replaced, since it points at a pty that no longer
/// exists. Anything else at that path is somebody else's: a real port, a file,
/// a directory. The refusal says which, because "permission denied" on
/// `/dev/ttyS0` and "there is a real device there" call for different answers.
pub(crate) fn make_the_path_lead_to(path: &Path, leads_to: &str) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(found) if found.file_type().is_symlink() => {
            std::fs::remove_file(path).map_err(|e| {
                format!(
                    "{} is a link left by an earlier run and cannot be replaced: {e}",
                    path.display()
                )
            })?;
        }
        Ok(found) if found.file_type().is_char_device() => {
            return Err(format!(
                "{} is a real serial device, so the simulator will not stand in for it: \
                 point the payload file at a path it may create, such as /tmp/{}",
                path.display(),
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "ttyS0".to_string())
            ))
        }
        Ok(_) => {
            return Err(format!(
                "{} already exists and is not a link the simulator made, so it is left \
                 alone",
                path.display()
            ))
        }
        Err(_) => {}
    }

    symlink(leads_to, path).map_err(|e| {
        format!(
            "{} cannot be made to lead to {leads_to}: {e}",
            path.display()
        )
    })
}

/// How long a wait is taken in one go.
///
/// The waits here are as long as the simulator file says, up to ten seconds,
/// and Stop has to be answered sooner than that: a thread asleep for a packet
/// interval cannot see that it has been told to stop. So every wait is taken
/// in slices and the flag is read between them.
const WAIT_SLICE: Duration = Duration::from_millis(50);

/// Wait `how_long`, giving up early if the payload has been stopped.
///
/// Returns whether it is still running, so a caller can abandon the rest of
/// what it was doing.
pub(crate) fn wait_while_running(how_long: Duration, running: &AtomicBool) -> bool {
    let until = Instant::now() + how_long;

    loop {
        if !running.load(Ordering::SeqCst) {
            return false;
        }

        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return true;
        }

        thread::sleep(left.min(WAIT_SLICE));
    }
}

/// Send one packet, a segment at a time.
///
/// What a simulator file means by a segment: a packet of `packet` bytes
/// reaches the handler in pieces of `segment_size`, `segment_interval` apart,
/// the last piece short where the size does not divide the packet. The pieces
/// are cut from one packet rather than generated one by one, which is the
/// difference between a packet in four pieces and four small packets.
///
/// A segment size of zero, or one no smaller than the packet, sends the
/// packet whole. That is what a file saying nothing about segments asks for,
/// since the size then comes from the packet size itself -- so the behaviour
/// of every file written before this existed is unchanged.
///
/// The wait falls between segments, never before the first or after the last:
/// what follows the last segment is the rest of the packet interval, which
/// the caller waits out. Returns the bytes that went and whether the whole
/// packet did -- a packet cut short by a stop or a failed write is not
/// counted as sent, though the bytes that did go are.
pub(crate) fn send_in_segments(
    packet: &[u8],
    segment_size: usize,
    segment_interval: Duration,
    running: &AtomicBool,
    send: &mut impl FnMut(&[u8]) -> std::io::Result<usize>,
) -> (u64, bool) {
    if packet.is_empty() {
        return (0, false);
    }

    let size = if segment_size == 0 {
        packet.len()
    } else {
        segment_size
    };

    let mut sent = 0;
    for (n, segment) in packet.chunks(size).enumerate() {
        if n > 0 && !wait_while_running(segment_interval, running) {
            return (sent, false);
        }

        match send(segment) {
            Ok(went) => sent += went as u64,
            Err(_) => return (sent, false),
        }
    }

    (sent, true)
}

/// What becomes of one packet.
///
/// A payload that works always sends. One with faults may send something else,
/// or nothing: dropping a packet and having gone silent are both nothing to
/// the handler, which is the point of them -- a handler cannot tell a payload
/// that skipped a packet from one that never had it.
pub(crate) enum Produce {
    /// Send these bytes, which may not be the bytes that were generated.
    Send(Vec<u8>),
    /// Send nothing at all.
    Nothing,
}

/// Make the packet this pass should send, and do to it whatever the faults
/// ask.
///
/// `sent` is how many packets have gone so far, which the faults that are
/// counted rather than random need: a payload that goes quiet after a hundred
/// packets has to know it has sent a hundred.
///
/// The order is the order a real fault would happen in. A dropped packet is
/// never generated, there being nothing yet to corrupt. A truncated packet is
/// cut before it is corrupted, so that the corruption lands in what is
/// actually sent rather than in the part that was cut off.
pub(crate) fn a_packet(
    config: &PayloadConfig,
    pacing: &Pacing,
    sent: u64,
    rng: &mut impl Rng,
) -> Produce {
    let faults = &config.faults;

    // Gone quiet and still connected: what a wedged instrument looks like
    // from the handler's end, and harder to notice than one that closed.
    if faults.silent_after > 0 && sent >= faults.silent_after {
        return Produce::Nothing;
    }

    if happens(faults.drop_percent, rng) {
        return Produce::Nothing;
    }

    let mut packet: Vec<u8> = (0..pacing.packet_size).map(|_| rng.gen()).collect();

    // A packet of one byte is left alone: there is no length that is both
    // shorter than one and not empty, and an empty packet is the dropping
    // fault rather than this one.
    if packet.len() > 1 && happens(faults.truncate_percent, rng) {
        // Shorter than it was, and never empty. Never its own length either:
        // a packet with every byte still in it was not truncated, so keeping
        // it in range would have a file asking for a tenth of its packets
        // truncated quietly get fewer -- the same reason corrupt() never
        // leaves a byte as it found it.
        let keep = rng.gen_range(1..packet.len());
        packet.truncate(keep);
    }

    if !packet.is_empty() && happens(faults.corrupt_percent, rng) {
        let which = rng.gen_range(0..packet.len());
        packet[which] = corrupt(packet[which], rng);
    }

    Produce::Send(packet)
}

/// One byte, altered.
///
/// Never the byte it was: something is added to it, and never nothing. A
/// corruption that left the value alone would be a fault that did not happen,
/// and a run that was told to corrupt a tenth of its packets would quietly
/// corrupt fewer. Zero is not used as the replacement either -- a zero is a
/// plausible value and would pass for data, where a byte that has changed is
/// wrong in the way a checksum or a sequence number is meant to catch.
fn corrupt(byte: u8, rng: &mut impl Rng) -> u8 {
    byte.wrapping_add(rng.gen_range(1..=255u8))
}

/// Whether something that happens `percent` times in a hundred happens now.
fn happens(percent: u32, rng: &mut impl Rng) -> bool {
    match percent {
        0 => false,
        100 => true,
        percent => rng.gen_range(0..100) < percent,
    }
}

/// How much later than it should this packet is.
///
/// Waited before the packet goes rather than added to the interval, which is
/// what makes it jitter and not a slower rate: the pass still ends when the
/// interval ends, so a payload told to send every second still sends every
/// second, with the packet itself a little late inside that. Late and never
/// early, because early would be the simulator sending before the interval
/// its own configuration asked for.
pub(crate) fn lateness(config: &PayloadConfig, rng: &mut impl Rng) -> Duration {
    match config.faults.jitter_ms {
        0 => Duration::ZERO,
        jitter => Duration::from_millis(rng.gen_range(0..=jitter) as u64),
    }
}

/// Whether this request goes unanswered.
///
/// What exercises a handler's own waiting: a trigger sent and nothing returned
/// is the case a response timeout exists for.
pub(crate) fn ignore_this_request(config: &PayloadConfig, rng: &mut impl Rng) -> bool {
    happens(config.faults.ignore_trigger_percent, rng)
}

/// Whether the payload hangs up now, for the kinds with a link to hang up.
pub(crate) fn hangs_up(config: &PayloadConfig, sent: u64) -> bool {
    config.faults.close_after > 0 && sent >= config.faults.close_after
}

/// What the sender threads read out of the shared configuration each time
/// round, since the window can change any of it while a payload runs.
pub(crate) struct Pacing {
    pub(crate) packet_size: usize,
    pub(crate) segment_size: usize,
    pub(crate) packet_interval: Duration,
    pub(crate) segment_interval: Duration,
}

impl Pacing {
    pub(crate) fn read(config: &PayloadConfig) -> Self {
        Self {
            packet_size: config.packet_size.load(Ordering::SeqCst) as usize,
            segment_size: config.segment_size.load(Ordering::SeqCst) as usize,
            packet_interval: Duration::from_millis(
                config.packet_interval_ms.load(Ordering::SeqCst) as u64,
            ),
            segment_interval: Duration::from_millis(
                config.segment_interval_ms.load(Ordering::SeqCst) as u64,
            ),
        }
    }

    /// Whether this payload produces anything at all. A packet interval of
    /// zero is a payload that is connected and silent.
    pub(crate) fn produces(&self) -> bool {
        !self.packet_interval.is_zero()
    }

    /// Whether a packet is due now.
    ///
    /// For a payload that sends on its own, every pass of its loop: the
    /// interval is waited out at the end of the pass rather than tested here.
    /// For one that answers requests, exactly when a request has arrived --
    /// `asked` being the bytes just read -- so that a trigger is answered once
    /// and silence is answered not at all.
    pub(crate) fn a_packet_is_due(&self, triggered: bool, asked: usize) -> bool {
        if triggered {
            asked > 0
        } else {
            self.produces()
        }
    }
}

/// Wait out what is left of the packet interval after a packet has gone.
///
/// The segments of a packet are sent inside the interval rather than on top
/// of it, so the packet rate is what the file says for as long as the
/// segments fit in it. A segmentation that takes longer than the interval
/// makes packets late instead of overlapping the next one, which is the
/// lesser of two wrong answers: the alternative is a payload that quietly
/// sends more than it was asked to.
pub(crate) fn wait_out_packet(pacing: &Pacing, since: Instant, running: &AtomicBool) {
    let left = pacing
        .packet_interval
        .saturating_sub(since.elapsed())
        // A payload that produces nothing still has to notice Stop, and a
        // loop with no wait in it at all would spin.
        .max(Duration::from_millis(if pacing.produces() { 0 } else { 10 }));

    wait_while_running(left, running);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    /// What a packet is divided into, and what the pieces add up to.
    /// A payload with faults, for the tests below. Everything else is what a
    /// payload that works has.
    fn with(faults: Faults) -> PayloadConfig {
        PayloadConfig {
            _id: 0,
            protocol: PayloadProtocol::Udp,
            address: "127.0.0.1".to_string(),
            port: 5000,
            bus_address: 0,
            triggered: false,
            faults,
            own_address: None,
            own_port: None,
            packet_size: Arc::new(AtomicU32::new(12)),
            segment_size: Arc::new(AtomicU32::new(12)),
            packet_interval_ms: Arc::new(AtomicU32::new(100)),
            segment_interval_ms: Arc::new(AtomicU32::new(100)),
        }
    }

    fn pacing_of(config: &PayloadConfig) -> Pacing {
        Pacing::read(config)
    }

    /// A payload with no faults sends every packet, whole, as it always did.
    #[test]
    fn a_payload_with_no_faults_sends_every_packet_whole() {
        let config = with(Faults::default());
        let pacing = pacing_of(&config);
        let mut rng = rand::thread_rng();

        for sent in 0..20 {
            match a_packet(&config, &pacing, sent, &mut rng) {
                Produce::Send(packet) => assert_eq!(packet.len(), 12),
                Produce::Nothing => panic!("a payload that works dropped a packet"),
            }
        }
    }

    /// The percentages are tested at their ends, where they are certain.
    ///
    /// A tenth of anything cannot be asserted from one run without either a
    /// seed or a tolerance, and both would be testing the distribution rather
    /// than the plumbing. Nought and a hundred say whether the knob is
    /// connected, which is the part that can be got wrong.
    #[test]
    fn every_packet_or_none_of_them_is_what_the_ends_of_a_percentage_mean() {
        let mut rng = rand::thread_rng();

        let all_dropped = with(Faults {
            drop_percent: 100,
            ..Faults::default()
        });
        for sent in 0..10 {
            assert!(
                matches!(
                    a_packet(&all_dropped, &pacing_of(&all_dropped), sent, &mut rng),
                    Produce::Nothing
                ),
                "a payload dropping everything sent something"
            );
        }

        let all_truncated = with(Faults {
            truncate_percent: 100,
            ..Faults::default()
        });
        for sent in 0..20 {
            match a_packet(&all_truncated, &pacing_of(&all_truncated), sent, &mut rng) {
                Produce::Send(packet) => {
                    // Shorter than the twelve asked for, and not empty.
                    // Twelve would be a packet that was not truncated, which
                    // is what a file asking for every packet to be truncated
                    // did not ask for; empty would be the dropping fault.
                    assert!(
                        (1..12).contains(&packet.len()),
                        "a truncated packet of twelve bytes came back {} bytes long",
                        packet.len()
                    );
                }
                Produce::Nothing => panic!("truncating is not dropping"),
            }
        }

        // A packet of one byte is the case truncation cannot serve: there is
        // no length both shorter than one and not empty. It goes whole rather
        // than not at all, because not at all is the other fault.
        all_truncated.packet_size.store(1, Ordering::SeqCst);
        for sent in 0..20 {
            match a_packet(&all_truncated, &pacing_of(&all_truncated), sent, &mut rng) {
                Produce::Send(packet) => assert_eq!(
                    packet.len(),
                    1,
                    "a one-byte packet cannot be shortened and must still be sent"
                ),
                Produce::Nothing => panic!("a packet too short to truncate was dropped"),
            }
        }

        // Corruption is of a byte within a packet, so what can be required of
        // it from outside is that the packet is still a packet: the fault
        // that can be seen from here is a packet whose length changed, which
        // would be truncation wearing the wrong name.
        let all_corrupted = with(Faults {
            corrupt_percent: 100,
            ..Faults::default()
        });
        for sent in 0..20 {
            match a_packet(&all_corrupted, &pacing_of(&all_corrupted), sent, &mut rng) {
                Produce::Send(packet) => assert_eq!(packet.len(), 12),
                Produce::Nothing => panic!("corrupting is not dropping"),
            }
        }
    }

    /// A corrupted byte is never the byte it was.
    ///
    /// Every value, many times over, because the fault is only a fault if it
    /// changes something: a corruption that left a value alone would be a run
    /// that corrupted fewer packets than it was told to, and nothing would
    /// say so.
    #[test]
    fn a_corrupted_byte_is_never_the_byte_it_was() {
        let mut rng = rand::thread_rng();
        for byte in 0..=255u8 {
            for _ in 0..20 {
                assert_ne!(corrupt(byte, &mut rng), byte, "{byte} came back itself");
            }
        }
    }

    /// A payload told to go quiet after so many packets goes quiet, and stays
    /// that way: that is what distinguishes it from one that drops a few.
    #[test]
    fn a_payload_that_goes_quiet_stays_quiet() {
        let config = with(Faults {
            silent_after: 3,
            ..Faults::default()
        });
        let pacing = pacing_of(&config);
        let mut rng = rand::thread_rng();

        for sent in 0..3 {
            assert!(
                matches!(a_packet(&config, &pacing, sent, &mut rng), Produce::Send(_)),
                "it went quiet after {sent} packets rather than 3"
            );
        }
        for sent in 3..30 {
            assert!(
                matches!(a_packet(&config, &pacing, sent, &mut rng), Produce::Nothing),
                "it spoke again after going quiet, at {sent}"
            );
        }
    }

    /// Jitter is a wait before a packet goes, bounded by what was asked for,
    /// and never negative: a packet is late or on time, never early.
    #[test]
    fn jitter_is_bounded_and_never_early() {
        let none = with(Faults::default());
        let mut rng = rand::thread_rng();
        assert_eq!(lateness(&none, &mut rng), Duration::ZERO);

        let jittery = with(Faults {
            jitter_ms: 20,
            ..Faults::default()
        });
        for _ in 0..200 {
            let late = lateness(&jittery, &mut rng);
            assert!(
                late <= Duration::from_millis(20),
                "{late:?} is later than the 20ms asked for"
            );
        }
    }

    /// A request is ignored or answered, and at the ends of the percentage it
    /// is certain which.
    #[test]
    fn an_ignored_request_is_one_the_payload_does_not_answer() {
        let mut rng = rand::thread_rng();

        let answers = with(Faults::default());
        assert!(!ignore_this_request(&answers, &mut rng));

        let deaf = with(Faults {
            ignore_trigger_percent: 100,
            ..Faults::default()
        });
        for _ in 0..20 {
            assert!(ignore_this_request(&deaf, &mut rng));
        }
    }

    /// Hanging up happens on the packet it was asked for and not before.
    #[test]
    fn a_payload_hangs_up_on_the_packet_it_was_told_to() {
        let never = with(Faults::default());
        assert!(!hangs_up(&never, 1000), "a payload with no fault hung up");

        let hangs = with(Faults {
            close_after: 5,
            ..Faults::default()
        });
        for sent in 0..5 {
            assert!(!hangs_up(&hangs, sent), "it hung up after {sent}");
        }
        assert!(hangs_up(&hangs, 5));
    }

    #[test]
    fn a_packet_is_cut_into_segments_of_the_size_asked_for() {
        let packet: Vec<u8> = (0..12).collect();
        let running = AtomicBool::new(true);

        let mut went: Vec<Vec<u8>> = Vec::new();
        let (bytes, whole) = send_in_segments(
            &packet,
            5,
            Duration::ZERO,
            &running,
            &mut |segment| {
                went.push(segment.to_vec());
                Ok(segment.len())
            },
        );

        assert!(whole, "the whole packet went");
        assert_eq!(bytes, 12);
        assert_eq!(
            went.iter().map(|s| s.len()).collect::<Vec<_>>(),
            vec![5, 5, 2],
            "twelve bytes in fives is two whole segments and a short one"
        );
        assert_eq!(
            went.concat(),
            packet,
            "the segments are the one packet, in order, rather than packets of their own"
        );
    }

    /// A size that does not divide the packet sends it whole: zero, which is
    /// what no segment size at all comes to, and any size the packet already
    /// fits in.
    #[test]
    fn a_packet_goes_whole_when_nothing_divides_it() {
        let packet: Vec<u8> = (0..12).collect();
        let running = AtomicBool::new(true);

        for segment_size in [0, 12, 100] {
            let mut went: Vec<Vec<u8>> = Vec::new();
            let (bytes, whole) = send_in_segments(
                &packet,
                segment_size,
                Duration::ZERO,
                &running,
                &mut |segment| {
                    went.push(segment.to_vec());
                    Ok(segment.len())
                },
            );

            assert!(whole);
            assert_eq!(bytes, 12);
            assert_eq!(went.len(), 1, "a segment size of {segment_size} divided the packet");
            assert_eq!(went[0], packet);
        }
    }

    /// The wait falls between segments: three of them 40ms apart take 80ms,
    /// not 120 -- nothing is waited before the first segment or after the
    /// last, because what follows the last is the rest of the packet
    /// interval.
    #[test]
    fn the_wait_falls_between_segments_and_nowhere_else() {
        let packet = vec![0u8; 3];
        let running = AtomicBool::new(true);

        let started = Instant::now();
        let (_, whole) =
            send_in_segments(&packet, 1, Duration::from_millis(40), &running, &mut |s| {
                Ok(s.len())
            });
        let took = started.elapsed();

        assert!(whole);
        assert!(
            took >= Duration::from_millis(80),
            "three segments 40ms apart took {took:?}, so they were not waited between"
        );
        assert!(
            took < Duration::from_millis(110),
            "three segments 40ms apart took {took:?}, which is a wait too many"
        );
    }

    /// Stopped part-way through a packet, the rest of it is abandoned: the
    /// bytes that went are counted and the packet is not.
    #[test]
    fn a_stop_abandons_the_rest_of_the_packet() {
        let packet: Vec<u8> = (0..12).collect();
        let running = AtomicBool::new(true);

        let mut segments = 0;
        let (bytes, whole) = send_in_segments(
            &packet,
            4,
            Duration::from_millis(10),
            &running,
            &mut |segment| {
                segments += 1;
                if segments == 2 {
                    running.store(false, Ordering::SeqCst);
                }
                Ok(segment.len())
            },
        );

        assert_eq!(segments, 2, "the third segment was sent after the stop");
        assert_eq!(bytes, 8, "the bytes that went are counted");
        assert!(!whole, "a packet cut short is not a packet sent");
    }

    /// The segments are sent inside the packet interval rather than on top of
    /// it: what is waited afterwards is the rest of the interval, and nothing
    /// at all where the segments took longer than the whole of it.
    #[test]
    fn the_segments_are_sent_inside_the_packet_interval() {
        let running = AtomicBool::new(true);
        let pacing = Pacing {
            packet_size: 3,
            segment_size: 1,
            packet_interval: Duration::from_millis(100),
            segment_interval: Duration::from_millis(40),
        };

        // 80ms of a 100ms interval already spent on segments.
        let at = Instant::now();
        wait_out_packet(&pacing, Instant::now() - Duration::from_millis(80), &running);
        let waited = at.elapsed();
        assert!(
            waited >= Duration::from_millis(20) && waited < Duration::from_millis(60),
            "waited {waited:?} of the 20ms left of the interval"
        );

        // More spent than the interval allows: the packet is late, and no
        // more waiting is added to it.
        let at = Instant::now();
        wait_out_packet(&pacing, Instant::now() - Duration::from_millis(200), &running);
        let waited = at.elapsed();
        assert!(
            waited < Duration::from_millis(20),
            "waited {waited:?} after an interval that was already spent"
        );
    }

    #[test]
    fn test_payload_config() {
        let config = PayloadConfig {
            _id: 0,
            protocol: PayloadProtocol::Udp,
            address: "127.0.0.1".to_string(),
            port: 5000,
            bus_address: 0,
            faults: Default::default(),
            own_address: None,
            own_port: None,
            packet_size: Arc::new(AtomicU32::new(12)),
            segment_size: Arc::new(AtomicU32::new(12)),
            packet_interval_ms: Arc::new(AtomicU32::new(1000)),
            segment_interval_ms: Arc::new(AtomicU32::new(1000)),
            triggered: false,
        };

        assert_eq!(config.address, "127.0.0.1");
        assert_eq!(config.port, 5000);
        assert!(config.protocol == PayloadProtocol::Udp);
        // The shared counters are what the sender threads actually read, so
        // check they survive construction.
        assert_eq!(config.packet_size.load(Ordering::Relaxed), 12);
        assert_eq!(config.segment_size.load(Ordering::Relaxed), 12);
        assert_eq!(config.packet_interval_ms.load(Ordering::Relaxed), 1000);
        assert_eq!(config.segment_interval_ms.load(Ordering::Relaxed), 1000);
    }
}
