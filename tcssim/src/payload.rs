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

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Payload configuration
#[derive(Clone)]
pub struct PayloadConfig {
    pub _id: u32,
    pub protocol: PayloadProtocol,
    pub address: String,
    pub port: u16,
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
}

/// Statistics for a payload
#[derive(Default)]
pub struct PayloadStats {
    pub packets_sent: u64,
    pub packets_recv: u64,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
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

    /// What a packet is divided into, and what the pieces add up to.
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
            packet_size: Arc::new(AtomicU32::new(12)),
            segment_size: Arc::new(AtomicU32::new(12)),
            packet_interval_ms: Arc::new(AtomicU32::new(1000)),
            segment_interval_ms: Arc::new(AtomicU32::new(1000)),
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
