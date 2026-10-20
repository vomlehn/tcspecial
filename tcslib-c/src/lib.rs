//! tcslib for C and C++.
//!
//! What a ground station is built on -- the vehicle's beacons, its data from
//! a `moc` node, and commands to it -- behind one header, `include/tcs.h`,
//! which says the whole of the interface and is includable from C and C++
//! alike. Nothing here is a second implementation of anything: each function
//! is the C spelling of a call into `tcslib`, so a C or C++ station reads the
//! configuration, counts ports and ages beacons exactly as tcsmoc does.
//!
//! Every function keeps the header's conventions. A handle is a `Box` handed
//! out as a pointer and taken back by its `_close`; a function that cannot do
//! what it was asked returns NULL or -1 and leaves the reason for
//! [`tcs_last_error`]; and no panic crosses into C, where unwinding is
//! undefined -- each body runs under [`guard`], and a panic is reported as an
//! error like any other.
//!
//! # Memory
//!
//! What this layer hands to C it holds in buffers of fixed size, so that it
//! allocates nothing as a station runs: the text of an error, what a beacon
//! said, a pass's remarks and warnings -- see [`Text`] and [`Notes`], and
//! `TCS_ERROR_BYTES`, `TCS_TEXT_BYTES` and `TCS_MAX_REMARKS` in the header,
//! which say how large. Text too long for its buffer is cut short at a
//! character, ending "...", so that a cut is seen rather than taken for the
//! whole. Each place that does allocate says so where it does it, and why:
//! the handles, once, at startup, and what tcslib itself builds as it reads a
//! beacon, a datum or an answer.
//!
//! The header is written by hand, for people, and `tests/c_and_cpp.rs` is
//! what keeps it honest: it compiles a ground station against it as C with
//! gcc and as C++ with g++, links each with this library, and runs both
//! against a vehicle the test plays.

// The types are named as the header names them, which is how a C reader will
// look for them.
#![allow(non_camel_case_types)]

use std::cell::RefCell;
use std::ffi::{c_char, c_int, CStr, CString};
use std::fmt::{self, Display, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tcslib::beacon::{Beacons, Contact, Schedule, Stage, STANDARD_INTERVAL};
use tcslib::moc::{Moc, DATUM_BYTES};
use tcslib::ping::{route_of, Pinger, Uplink, NO_MESSAGE};
use tcslib::{command_links_of, CommandStatus, TcsClient};
use tcslibgs::config_digest::{digest_of_graph, ConfigDigest, ConfigVersion};
use tcslibgs::graph::parameters::{beacon_interval_of, beacon_of};
use tcslibgs::graph::{telemetry, Graph};
use tcslibgs::{ArmKey, BeaconInterval, DHSample, NodeId, DH_SAMPLE_BYTES};

/// The size of the buffer `tcs_last_error` returns, its NUL included:
/// `TCS_ERROR_BYTES` in the header.
pub const ERROR_BYTES: usize = 512;

/// The size of each other buffer of text handed to C -- what a beacon said,
/// each remark and warning -- its NUL included: `TCS_TEXT_BYTES`.
pub const TEXT_BYTES: usize = 256;

/// How many remarks, and how many warnings, one pass keeps:
/// `TCS_MAX_REMARKS`.
pub const MAX_REMARKS: usize = 8;

// ---- Fixed buffers ----------------------------------------------------------

/// Text held for C in `N` bytes, NUL-terminated, allocating nothing.
///
/// Written with `write!`, a piece at a time, so that a message is put
/// together in place rather than built as a `String` and copied. Text that
/// does not fit is cut at a character, ending "...", and what comes after is
/// dropped. A NUL in what is written, which C would read as the end, is a
/// space.
struct Text<const N: usize> {
    bytes: [u8; N],
    len: usize,
    /// Whether it has been cut short, after which writing does nothing.
    full: bool,
}

impl<const N: usize> Text<N> {
    /// Nothing: an empty string.
    const EMPTY: Self = Self {
        bytes: [0; N],
        len: 0,
        full: false,
    };

    /// Say `text`, in place of what was said before.
    fn set(&mut self, text: impl Display) {
        self.len = 0;
        self.full = false;
        self.bytes[0] = 0;
        // A cut is the only error, and it has been marked in the text.
        let _ = write!(self, "{text}");
    }

    fn as_ptr(&self) -> *const c_char {
        self.bytes.as_ptr().cast()
    }

    #[cfg(test)]
    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).expect("it holds UTF-8")
    }

    /// End what is held with "...", at a character, where it would not fit.
    fn cut(&mut self) {
        const MARK: &[u8] = b"...";
        let mut end = self.len.min(N - 1 - MARK.len());
        // The held bytes are UTF-8, so a character starts where a byte is
        // not a continuation byte.
        while end > 0 && self.bytes[end] & 0xC0 == 0x80 {
            end -= 1;
        }
        self.bytes[end..end + MARK.len()].copy_from_slice(MARK);
        self.len = end + MARK.len();
        self.bytes[self.len] = 0;
        self.full = true;
    }
}

impl<const N: usize> Write for Text<N> {
    fn write_str(&mut self, said: &str) -> fmt::Result {
        if self.full {
            return Err(fmt::Error);
        }
        for character in said.chars() {
            let character = if character == '\0' { ' ' } else { character };
            let mut encoded = [0u8; 4];
            let encoded = character.encode_utf8(&mut encoded).as_bytes();
            // One byte is kept for the NUL.
            if self.len + encoded.len() > N - 1 {
                self.cut();
                return Err(fmt::Error);
            }
            self.bytes[self.len..self.len + encoded.len()].copy_from_slice(encoded);
            self.len += encoded.len();
        }
        self.bytes[self.len] = 0;
        Ok(())
    }
}

/// Up to [`MAX_REMARKS`] lines of text, for C, allocating nothing.
///
/// A pass rarely says more than one or two things. Where one says more than
/// there is room for, the last line kept says how many were left out, so
/// that a station is not left believing it heard everything.
struct Notes {
    lines: [Text<TEXT_BYTES>; MAX_REMARKS],
    count: usize,
}

impl Notes {
    const EMPTY: Self = Self {
        lines: [Text::EMPTY; MAX_REMARKS],
        count: 0,
    };

    /// Hold `said`, in place of what was held before.
    fn set(&mut self, said: &[String]) {
        if said.len() <= MAX_REMARKS {
            for (line, text) in self.lines.iter_mut().zip(said) {
                line.set(text);
            }
            self.count = said.len();
        } else {
            let kept = MAX_REMARKS - 1;
            for (line, text) in self.lines.iter_mut().zip(&said[..kept]) {
                line.set(text);
            }
            self.lines[kept].set(format_args!("... and {} more", said.len() - kept));
            self.count = MAX_REMARKS;
        }
    }

    fn get(&self, index: usize) -> Option<&Text<TEXT_BYTES>> {
        self.lines[..self.count].get(index)
    }
}

// ---- Errors -------------------------------------------------------------------

thread_local! {
    /// What went wrong with this thread's last call that failed.
    static LAST_ERROR: RefCell<Text<ERROR_BYTES>> = const { RefCell::new(Text::EMPTY) };
}

/// That a call failed, its reason already left for [`tcs_last_error`].
struct Failed;

/// Leave `why` for [`tcs_last_error`], written straight into its buffer.
fn fail(why: impl Display) -> Failed {
    LAST_ERROR.with(|last| last.borrow_mut().set(why));
    Failed
}

/// Run `body`, turning a panic into a failure and a failure into `otherwise`.
fn guard<T>(otherwise: T, body: impl FnOnce() -> Result<T, Failed>) -> T {
    match catch_unwind(AssertUnwindSafe(body)) {
        Ok(Ok(value)) => value,
        Ok(Err(Failed)) => otherwise,
        Err(_) => {
            fail("an internal error in tcslib: this is a bug, and worth reporting");
            otherwise
        }
    }
}

/// A string from C, or why not.
///
/// # Safety
///
/// `text` is NULL or points to a NUL-terminated string.
unsafe fn text_of<'a>(text: *const c_char, what: &str) -> Result<&'a str, Failed> {
    if text.is_null() {
        return Err(fail(format_args!("{what} is NULL")));
    }
    CStr::from_ptr(text)
        .to_str()
        .map_err(|_| fail(format_args!("{what} is not UTF-8")))
}

/// The handle behind a pointer from C, or why not.
///
/// # Safety
///
/// `handle` is NULL or was made by this library and not yet closed.
unsafe fn handle_of<'a, T>(handle: *const T, what: &str) -> Result<&'a T, Failed> {
    handle
        .as_ref()
        .ok_or_else(|| fail(format_args!("{what} is NULL")))
}

/// The same, to change.
///
/// # Safety
///
/// As [`handle_of`], and used from one thread at a time.
unsafe fn handle_of_mut<'a, T>(handle: *mut T, what: &str) -> Result<&'a mut T, Failed> {
    handle
        .as_mut()
        .ok_or_else(|| fail(format_args!("{what} is NULL")))
}

/// Somewhere to put an answer, or why not.
///
/// # Safety
///
/// `out` is NULL or points to a `T` that may be written.
unsafe fn out_of<'a, T>(out: *mut T, what: &str) -> Result<&'a mut T, Failed> {
    out.as_mut()
        .ok_or_else(|| fail(format_args!("{what} is NULL")))
}

// ---- The library ----------------------------------------------------------

/// This library's version: `"0.1.0"`.
#[no_mangle]
pub extern "C" fn tcs_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

/// What went wrong with this thread's last call that failed, or `""`.
#[no_mangle]
pub extern "C" fn tcs_last_error() -> *const c_char {
    // The buffer is the thread's for as long as the thread is, so the pointer
    // outlives the borrow.
    LAST_ERROR.with(|last| last.borrow().as_ptr())
}

// ---- A configuration --------------------------------------------------------

/// A configuration, read and checked as the vehicle reads it.
pub struct tcs_configuration {
    path: PathBuf,
    graph: Graph,
    digest: ConfigDigest,
    /// Thirty-two hex digits and a NUL.
    digest_text: Text<33>,
}

/// Read and check the configuration at `path`.
///
/// # Safety
///
/// `path` is NULL or a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn tcs_configuration_open(path: *const c_char) -> *mut tcs_configuration {
    guard(std::ptr::null_mut(), || {
        // Allocates, once, at startup: the path, the graph a whole
        // configuration is read into, and the handle, which outlives this
        // call and whose size C does not see. Nothing of this is done again
        // as a station runs.
        let path = PathBuf::from(text_of(path, "the path")?);
        let graph =
            Graph::load(&path).map_err(|e| fail(format_args!("{}: {e}", path.display())))?;
        // The checks the vehicle runs before it opens anything: a station
        // should not run on a configuration the vehicle would refuse.
        let problems = graph.check();
        if let Some(problem) = problems.first() {
            return Err(fail(format_args!(
                "{}: {}",
                path.display(),
                problem.describe()
            )));
        }
        let digest =
            digest_of_graph(&graph).map_err(|e| fail(format_args!("{}: {e}", path.display())))?;
        let mut digest_text = Text::EMPTY;
        digest_text.set(digest);
        Ok(Box::into_raw(Box::new(tcs_configuration {
            path,
            graph,
            digest,
            digest_text,
        })))
    })
}

/// Free a configuration.
///
/// # Safety
///
/// `configuration` is NULL or was made by `tcs_configuration_open` and not
/// yet closed.
#[no_mangle]
pub unsafe extern "C" fn tcs_configuration_close(configuration: *mut tcs_configuration) {
    if !configuration.is_null() {
        drop(Box::from_raw(configuration));
    }
}

/// The configuration's digest, as thirty-two hex digits.
///
/// # Safety
///
/// As [`tcs_configuration_close`], not yet closed.
#[no_mangle]
pub unsafe extern "C" fn tcs_configuration_digest(
    configuration: *const tcs_configuration,
) -> *const c_char {
    guard(std::ptr::null(), || {
        Ok(handle_of(configuration, "the configuration")?
            .digest_text
            .as_ptr())
    })
}

/// The id of the node called `name`, into `*id`.
///
/// # Safety
///
/// `configuration` as [`tcs_configuration_digest`]; `name` NULL or a
/// NUL-terminated string; `id` NULL or writable.
#[no_mangle]
pub unsafe extern "C" fn tcs_configuration_node_id(
    configuration: *const tcs_configuration,
    name: *const c_char,
    id: *mut u32,
) -> c_int {
    guard(-1, || {
        let configuration = handle_of(configuration, "the configuration")?;
        let name = text_of(name, "the node's name")?;
        let out = out_of(id, "the id")?;
        // Allocates: tcslib's map of every node's id. A station asks for an
        // id as it starts, to know which node to command, and not as it runs.
        let ids = telemetry::ids(&configuration.graph);
        let found = ids.get(name).ok_or_else(|| {
            fail(format_args!(
                "{}: there is no node called \"{name}\"",
                configuration.path.display()
            ))
        })?;
        *out = u32::try_from(*found)
            .map_err(|_| fail(format_args!("\"{name}\"'s id is too large")))?;
        Ok(0)
    })
}

// ---- Beacons ------------------------------------------------------------------

/// `tcs_stage`, as the header numbers it.
fn stage_number(stage: Stage) -> c_int {
    match stage {
        Stage::Unheard => 0,
        Stage::Heard => 1,
        Stage::Aging => 2,
        Stage::Waiting => 3,
    }
}

/// What the header's `tcs_pass` is: the same fields in the same order.
#[repr(C)]
pub struct tcs_pass {
    stage: c_int,
    lit: c_int,
    in_contact: c_int,
    heard: c_int,
    at_seconds: i64,
    at_nanos: u32,
}

/// What a stage is called where a person reads it.
#[no_mangle]
pub extern "C" fn tcs_stage_label(stage: c_int) -> *const c_char {
    let label: &'static CStr = match stage {
        0 => c"Unheard",
        1 => c"Heard",
        2 => c"Aging",
        3 => c"Waiting",
        _ => c"",
    };
    // The words are Stage::label's: see the test that holds them to it.
    label.as_ptr()
}

/// The ground's end of the beacon link, and what the last pass said.
pub struct tcs_beacons {
    beacons: Beacons,
    said: Text<TEXT_BYTES>,
    remarks: Notes,
    warnings: Notes,
}

/// Listen where the configuration says beacons arrive.
///
/// # Safety
///
/// `configuration` as [`tcs_configuration_digest`].
#[no_mangle]
pub unsafe extern "C" fn tcs_beacons_listen(
    configuration: *const tcs_configuration,
) -> *mut tcs_beacons {
    guard(std::ptr::null_mut(), || {
        let configuration = handle_of(configuration, "the configuration")?;
        let graph = &configuration.graph;
        let place = configuration.path.display();
        let link = beacon_of(graph)
            .map_err(|e| fail(format_args!("{place}: {e}")))?
            .ok_or_else(|| {
                fail(format_args!(
                    "{place} has no tcspecial section, so sends no beacons"
                ))
            })?;
        let interval = beacon_interval_of(graph)
            .map_err(|e| fail(format_args!("{place}: {e}")))?
            .unwrap_or(STANDARD_INTERVAL);
        let beacons = Beacons::listening(
            link,
            Schedule::for_interval(interval),
            Some(configuration.digest),
        )
        .map_err(|e| fail(format_args!("{place}: beacons cannot be received: {e}")))?;
        let mut said = Text::EMPTY;
        said.set(tcslib::beacon::NO_BEACON_MESSAGE);
        // Allocates, once, at startup: the handle, which outlives this call
        // and whose size C does not see -- the fixed buffers in it included.
        Ok(Box::into_raw(Box::new(tcs_beacons {
            beacons,
            said,
            remarks: Notes::EMPTY,
            warnings: Notes::EMPTY,
        })))
    })
}

/// One pass of the beacon watch, into `*pass`.
///
/// # Safety
///
/// `beacons` NULL or made by `tcs_beacons_listen` and not yet closed, used
/// from one thread at a time; `pass` NULL or writable.
#[no_mangle]
pub unsafe extern "C" fn tcs_beacons_pass(beacons: *mut tcs_beacons, pass: *mut tcs_pass) -> c_int {
    guard(-1, || {
        let watch = handle_of_mut(beacons, "the beacons")?;
        let out = out_of(pass, "the pass")?;
        // Allocates nothing but what tcslib does: a remark or warning, on
        // the pass where something changed. What the pass holds is copied
        // into the handle's fixed buffers.
        let found = watch.beacons.pass();
        let at = found
            .at
            .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
            .unwrap_or(Duration::ZERO);
        *out = tcs_pass {
            stage: stage_number(found.showing.stage),
            lit: c_int::from(found.showing.lit),
            in_contact: c_int::from(found.contact == Contact::Made),
            heard: c_int::from(found.at.is_some()),
            at_seconds: i64::try_from(at.as_secs()).unwrap_or(i64::MAX),
            at_nanos: at.subsec_nanos(),
        };
        watch.said.set(&found.said);
        watch.remarks.set(&found.remarks);
        watch.warnings.set(&found.warnings);
        Ok(0)
    })
}

/// What the last beacon said.
///
/// # Safety
///
/// `beacons` as [`tcs_beacons_pass`].
#[no_mangle]
pub unsafe extern "C" fn tcs_beacons_said(beacons: *const tcs_beacons) -> *const c_char {
    guard(std::ptr::null(), || {
        Ok(handle_of(beacons, "the beacons")?.said.as_ptr())
    })
}

/// How many remarks the last pass made.
///
/// # Safety
///
/// `beacons` as [`tcs_beacons_pass`].
#[no_mangle]
pub unsafe extern "C" fn tcs_beacons_remark_count(beacons: *const tcs_beacons) -> usize {
    guard(0, || Ok(handle_of(beacons, "the beacons")?.remarks.count))
}

/// The last pass's remark at `index`, or NULL past the end.
///
/// # Safety
///
/// `beacons` as [`tcs_beacons_pass`].
#[no_mangle]
pub unsafe extern "C" fn tcs_beacons_remark(
    beacons: *const tcs_beacons,
    index: usize,
) -> *const c_char {
    guard(std::ptr::null(), || {
        let watch = handle_of(beacons, "the beacons")?;
        let said = watch.remarks.get(index).ok_or_else(|| {
            fail(format_args!(
                "there are {} remarks, and no remark {index}",
                watch.remarks.count
            ))
        })?;
        Ok(said.as_ptr())
    })
}

/// How many warnings the last pass made.
///
/// # Safety
///
/// `beacons` as [`tcs_beacons_pass`].
#[no_mangle]
pub unsafe extern "C" fn tcs_beacons_warning_count(beacons: *const tcs_beacons) -> usize {
    guard(0, || Ok(handle_of(beacons, "the beacons")?.warnings.count))
}

/// The last pass's warning at `index`, or NULL past the end.
///
/// # Safety
///
/// `beacons` as [`tcs_beacons_pass`].
#[no_mangle]
pub unsafe extern "C" fn tcs_beacons_warning(
    beacons: *const tcs_beacons,
    index: usize,
) -> *const c_char {
    guard(std::ptr::null(), || {
        let watch = handle_of(beacons, "the beacons")?;
        let said = watch.warnings.get(index).ok_or_else(|| {
            fail(format_args!(
                "there are {} warnings, and no warning {index}",
                watch.warnings.count
            ))
        })?;
        Ok(said.as_ptr())
    })
}

/// Stop listening.
///
/// # Safety
///
/// `beacons` NULL or made by `tcs_beacons_listen` and not yet closed.
#[no_mangle]
pub unsafe extern "C" fn tcs_beacons_close(beacons: *mut tcs_beacons) {
    if !beacons.is_null() {
        drop(Box::from_raw(beacons));
    }
}

// ---- The vehicle's data -------------------------------------------------------

/// A `moc` node, open, and the last datum it received.
pub struct tcs_moc {
    moc: Moc,
    /// Each source's name as C reads it, in the order tcslib numbers them,
    /// made once when the node is opened: a datum's source is one of these,
    /// so a receive points at one rather than copying a name.
    names: Vec<CString>,
    /// Where each datum is received, made once when the node is opened and
    /// room for any: it holds the last until the next receive.
    data: Vec<u8>,
}

/// What the header's `tcs_datum` is.
#[repr(C)]
pub struct tcs_datum {
    source: *const c_char,
    data: *const u8,
    length: usize,
}

/// Open the `moc` node called `moc`.
///
/// # Safety
///
/// `configuration` as [`tcs_configuration_digest`]; `moc` NULL or a
/// NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn tcs_moc_open(
    configuration: *const tcs_configuration,
    moc: *const c_char,
) -> *mut tcs_moc {
    guard(std::ptr::null_mut(), || {
        let configuration = handle_of(configuration, "the configuration")?;
        let name = text_of(moc, "the moc node's name")?;
        let moc = Moc::open(&configuration.graph, name)
            .map_err(|e| fail(format_args!("{}: {e}", configuration.path.display())))?;
        // Allocates, once, at startup: the handle, which outlives this call;
        // the names of the node's sources, which are known now and read by
        // every receive after; and the buffer every datum is received in.
        let names = moc
            .sources()
            .iter()
            .map(|source| CString::new(source.replace('\0', " ")).unwrap_or_default())
            .collect();
        Ok(Box::into_raw(Box::new(tcs_moc {
            moc,
            names,
            data: vec![0; DATUM_BYTES],
        })))
    })
}

/// The next datum, waiting up to `wait_ms`, or for ever where it is negative.
///
/// # Safety
///
/// `moc` NULL or made by `tcs_moc_open` and not yet closed, used from one
/// thread at a time; `datum` NULL or writable.
#[no_mangle]
pub unsafe extern "C" fn tcs_moc_receive(
    moc: *mut tcs_moc,
    wait_ms: i64,
    datum: *mut tcs_datum,
) -> c_int {
    guard(-1, || {
        let node = handle_of_mut(moc, "the moc node")?;
        let out = out_of(datum, "the datum")?;
        let wait = u64::try_from(wait_ms).ok().map(Duration::from_millis);
        let Some(arrival) = node.moc.receive_into(wait, &mut node.data).map_err(fail)? else {
            return Ok(0);
        };
        let Some(source) = node.names.get(arrival.source) else {
            return Err(fail(format_args!(
                "a datum came from source {}, which this moc node does not draw",
                arrival.source
            )));
        };
        *out = tcs_datum {
            source: source.as_ptr(),
            data: node.data.as_ptr(),
            length: arrival.length,
        };
        Ok(1)
    })
}

/// Close a `moc` node.
///
/// # Safety
///
/// `moc` NULL or made by `tcs_moc_open` and not yet closed.
#[no_mangle]
pub unsafe extern "C" fn tcs_moc_close(moc: *mut tcs_moc) {
    if !moc.is_null() {
        drop(Box::from_raw(moc));
    }
}

// ---- Commands -----------------------------------------------------------------

/// `tcs_status`, as the header numbers it.
fn status_number(status: CommandStatus) -> c_int {
    match status {
        CommandStatus::Success => 0,
        CommandStatus::Failure => 1,
        CommandStatus::InvalidCommand => 2,
        CommandStatus::InvalidParameter => 3,
        CommandStatus::NotArmed => 4,
        CommandStatus::NotFound => 5,
        CommandStatus::AlreadyExists => 6,
        CommandStatus::Timeout => 7,
    }
}

/// A status's name.
#[no_mangle]
pub extern "C" fn tcs_status_name(status: c_int) -> *const c_char {
    let name: &'static CStr = match status {
        0 => c"Success",
        1 => c"Failure",
        2 => c"InvalidCommand",
        3 => c"InvalidParameter",
        4 => c"NotArmed",
        5 => c"NotFound",
        6 => c"AlreadyExists",
        7 => c"Timeout",
        _ => c"",
    };
    name.as_ptr()
}

/// The ground's end of the two command links, and the digest a CONNECT
/// is answered against.
pub struct tcs_client {
    client: TcsClient,
    digest: ConfigDigest,
}

/// Open the command links the configuration states.
///
/// # Safety
///
/// `configuration` as [`tcs_configuration_digest`].
#[no_mangle]
pub unsafe extern "C" fn tcs_client_open(
    configuration: *const tcs_configuration,
) -> *mut tcs_client {
    guard(std::ptr::null_mut(), || {
        let configuration = handle_of(configuration, "the configuration")?;
        let place = configuration.path.display();
        let links = command_links_of(&configuration.graph)
            .map_err(|e| fail(format_args!("{place}: {e}")))?
            .ok_or_else(|| {
                fail(format_args!(
                    "{place} has no tcspecial section, so names no command link"
                ))
            })?;
        let client = TcsClient::open(&links).map_err(|e| fail(format_args!("{place}: {e}")))?;
        // Allocates, once, at startup: the handle, which outlives this call,
        // and the client's two links.
        Ok(Box::into_raw(Box::new(tcs_client {
            client,
            digest: configuration.digest,
        })))
    })
}

// Every command below allocates, in tcslib: the command is written as JSON
// and the answer read from it, as the link carries them. Not this layer's to
// change.

/// Send CONNECT, and say in `*agrees` whether the vehicle is flying this
/// configuration.
///
/// # Safety
///
/// `client` NULL or made by `tcs_client_open` and not yet closed, used from
/// one thread at a time; `agrees` NULL or writable.
#[no_mangle]
pub unsafe extern "C" fn tcs_client_connect(client: *mut tcs_client, agrees: *mut c_int) -> c_int {
    guard(-1, || {
        let link = handle_of_mut(client, "the client")?;
        let out = out_of(agrees, "agrees")?;
        let (_, theirs) = link
            .client
            .connect(ConfigVersion::of_this_build(), link.digest)
            .map_err(|e| fail(format_args!("CONNECT: {e}")))?;
        *out = c_int::from(theirs == link.digest);
        Ok(0)
    })
}

/// Send PING.
///
/// # Safety
///
/// `client` as [`tcs_client_connect`].
#[no_mangle]
pub unsafe extern "C" fn tcs_client_ping(client: *mut tcs_client) -> c_int {
    guard(-1, || {
        let link = handle_of_mut(client, "the client")?;
        link.client
            .ping()
            .map_err(|e| fail(format_args!("PING: {e}")))?;
        Ok(0)
    })
}

/// Start (`start`) or stop a node, the answer's status into `*status`.
///
/// # Safety
///
/// As [`tcs_client_connect`]; `status` NULL or writable.
unsafe fn command_a_node(
    client: *mut tcs_client,
    node: u32,
    status: *mut c_int,
    start: bool,
) -> c_int {
    guard(-1, || {
        let link = handle_of_mut(client, "the client")?;
        let out = out_of(status, "the status")?;
        let verb = if start { "START_NODE" } else { "STOP_NODE" };
        let asked = NodeId(node);
        let answer = if start {
            link.client.start_node(asked)
        } else {
            link.client.stop_node(asked)
        };
        let (answered, about) = answer.map_err(|e| fail(format_args!("{verb} {node}: {e}")))?;
        if about != asked {
            return Err(fail(format_args!(
                "{verb} {node} was answered about node {}: the two ends are not reading \
                 the same configuration",
                about.0
            )));
        }
        *out = status_number(answered);
        Ok(0)
    })
}

/// Start a node.
///
/// # Safety
///
/// As [`command_a_node`].
#[no_mangle]
pub unsafe extern "C" fn tcs_client_start_node(
    client: *mut tcs_client,
    node: u32,
    status: *mut c_int,
) -> c_int {
    command_a_node(client, node, status, true)
}

/// Stop a node.
///
/// # Safety
///
/// As [`command_a_node`].
#[no_mangle]
pub unsafe extern "C" fn tcs_client_stop_node(
    client: *mut tcs_client,
    node: u32,
    status: *mut c_int,
) -> c_int {
    command_a_node(client, node, status, false)
}

/// Ask what a node is doing: the status into `*status`, and into `*enabled`
/// whether it is running.
///
/// # Safety
///
/// As [`tcs_client_connect`]; `status` and `enabled` NULL or writable.
#[no_mangle]
pub unsafe extern "C" fn tcs_client_query_node(
    client: *mut tcs_client,
    node: u32,
    status: *mut c_int,
    enabled: *mut c_int,
) -> c_int {
    guard(-1, || {
        let link = handle_of_mut(client, "the client")?;
        let status_out = out_of(status, "the status")?;
        let enabled_out = out_of(enabled, "enabled")?;
        let (answered, running, _) = link
            .client
            .query_node(NodeId(node))
            .map_err(|e| fail(format_args!("QUERY_NODE {node}: {e}")))?;
        *status_out = status_number(answered);
        *enabled_out = c_int::from(running);
        Ok(0)
    })
}

/// Close the command links.
///
/// # Safety
///
/// `client` NULL or made by `tcs_client_open` and not yet closed.
#[no_mangle]
pub unsafe extern "C" fn tcs_client_close(client: *mut tcs_client) {
    if !client.is_null() {
        let mut link = Box::from_raw(client);
        let _ = link.client.close();
    }
}

/// Arm a restart.
///
/// # Safety
///
/// As [`tcs_client_connect`]; `status` NULL or writable.
#[no_mangle]
pub unsafe extern "C" fn tcs_client_restart_arm(
    client: *mut tcs_client,
    key: u64,
    status: *mut c_int,
) -> c_int {
    guard(-1, || {
        let link = handle_of_mut(client, "the client")?;
        let out = out_of(status, "the status")?;
        let answered = link
            .client
            .restart_arm(ArmKey(key))
            .map_err(|e| fail(format_args!("RESTART_ARM: {e}")))?;
        *out = status_number(answered);
        Ok(0)
    })
}

/// Restart the vehicle.
///
/// # Safety
///
/// As [`tcs_client_restart_arm`].
#[no_mangle]
pub unsafe extern "C" fn tcs_client_restart(
    client: *mut tcs_client,
    key: u64,
    status: *mut c_int,
) -> c_int {
    guard(-1, || {
        let link = handle_of_mut(client, "the client")?;
        let out = out_of(status, "the status")?;
        let answered = link
            .client
            .restart(ArmKey(key))
            .map_err(|e| fail(format_args!("RESTART: {e}")))?;
        *out = status_number(answered);
        Ok(0)
    })
}

/// Change how often the vehicle beacons, in microseconds.
///
/// # Safety
///
/// As [`tcs_client_restart_arm`].
#[no_mangle]
pub unsafe extern "C" fn tcs_client_configure(
    client: *mut tcs_client,
    beacon_interval_us: u64,
    status: *mut c_int,
) -> c_int {
    guard(-1, || {
        let link = handle_of_mut(client, "the client")?;
        let out = out_of(status, "the status")?;
        let answered = link
            .client
            .configure(BeaconInterval(beacon_interval_us))
            .map_err(|e| fail(format_args!("CONFIG: {e}")))?;
        *out = status_number(answered);
        Ok(0)
    })
}

/// Set an attribute of a node.
///
/// # Safety
///
/// As [`tcs_client_connect`]; `attribute` NULL or a NUL-terminated string;
/// `value` NULL with `length` 0, or `length` readable bytes; `status` NULL
/// or writable.
#[no_mangle]
pub unsafe extern "C" fn tcs_client_set_node(
    client: *mut tcs_client,
    node: u32,
    attribute: *const c_char,
    value: *const u8,
    length: usize,
    status: *mut c_int,
) -> c_int {
    guard(-1, || {
        let link = handle_of_mut(client, "the client")?;
        let attribute = text_of(attribute, "the attribute")?;
        let out = out_of(status, "the status")?;
        let value: &[u8] = if length == 0 {
            &[]
        } else if value.is_null() {
            return Err(fail("the value is NULL"));
        } else {
            std::slice::from_raw_parts(value, length)
        };
        let asked = NodeId(node);
        let (answered, about) = link
            .client
            .set_node(asked, attribute, value)
            .map_err(|e| fail(format_args!("SET_NODE {node} {attribute}: {e}")))?;
        if about != asked {
            return Err(fail(format_args!(
                "SET_NODE {node} was answered about node {}: the two ends are not reading \
                 the same configuration",
                about.0
            )));
        }
        *out = status_number(answered);
        Ok(0)
    })
}

/// What the header's `tcs_sample` is.
#[repr(C)]
pub struct tcs_sample {
    moved: c_int,
    at_micros: i64,
    total: u32,
    length: u8,
    bytes: [u8; DH_SAMPLE_BYTES],
}

impl tcs_sample {
    fn of(sample: &DHSample) -> tcs_sample {
        tcs_sample {
            moved: c_int::from(sample.time.is_some()),
            at_micros: sample
                .time
                .map_or(0, |at| i64::try_from(at.micros()).unwrap_or(i64::MAX)),
            total: sample.total,
            length: sample.len,
            bytes: sample.bytes,
        }
    }
}

/// The last datum a node took, and the last it gave.
///
/// # Safety
///
/// As [`tcs_client_connect`]; `status`, `taken` and `given` NULL or
/// writable.
#[no_mangle]
pub unsafe extern "C" fn tcs_client_query_node_sample(
    client: *mut tcs_client,
    node: u32,
    status: *mut c_int,
    taken: *mut tcs_sample,
    given: *mut tcs_sample,
) -> c_int {
    guard(-1, || {
        let link = handle_of_mut(client, "the client")?;
        let status_out = out_of(status, "the status")?;
        let taken_out = out_of(taken, "taken")?;
        let given_out = out_of(given, "given")?;
        let (answered, took, gave) = link
            .client
            .query_node_sample(NodeId(node))
            .map_err(|e| fail(format_args!("QUERY_NODE_SAMPLE {node}: {e}")))?;
        *status_out = status_number(answered);
        *taken_out = tcs_sample::of(&took);
        *given_out = tcs_sample::of(&gave);
        Ok(0)
    })
}

// ---- Pinging a ping node --------------------------------------------------------

/// What the header's `tcs_exchange` is.
#[repr(C)]
pub struct tcs_exchange {
    sent: c_int,
    sent_at: i64,
    answered: c_int,
    received_at: i64,
    answered_at: i64,
    round_trip: i64,
    up: i64,
    down: i64,
}

/// A ping node's way up and down, and what came of the last ping.
pub struct tcs_pinger {
    uplink: Uplink,
    pinger: Pinger,
    node: CString,
    moc: CString,
    sent: Text<TEXT_BYTES>,
    received: Text<TEXT_BYTES>,
}

/// Nanoseconds since the epoch, as the header gives a time.
fn nanos(at: Option<Duration>) -> i64 {
    at.map_or(0, |at| i64::try_from(at.as_nanos()).unwrap_or(i64::MAX))
}

/// A length of time in nanoseconds, as the header gives one.
fn length(nanoseconds: Option<i128>) -> i64 {
    nanoseconds.map_or(0, |n| {
        i64::try_from(n).unwrap_or(if n < 0 { i64::MIN } else { i64::MAX })
    })
}

/// Now, by this machine's clock, as a ping is timed.
fn now() -> Result<Duration, Failed> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| fail("this machine's clock reads before 1970"))
}

/// Ping the configuration's first ping node the ground can reach both ways.
///
/// # Safety
///
/// `configuration` as [`tcs_configuration_digest`].
#[no_mangle]
pub unsafe extern "C" fn tcs_pinger_open(
    configuration: *const tcs_configuration,
) -> *mut tcs_pinger {
    guard(std::ptr::null_mut(), || {
        let configuration = handle_of(configuration, "the configuration")?;
        let place = configuration.path.display();
        let route = route_of(&configuration.graph).ok_or_else(|| {
            fail(format_args!(
                "{place} has no ping node the ground can reach both ways, through a moc \
                 node that binds an address aboard"
            ))
        })?;
        let uplink = Uplink::open(&configuration.graph, &route.moc)
            .map_err(|e| fail(format_args!("{place}: {e}")))?;
        let mut sent = Text::EMPTY;
        sent.set(NO_MESSAGE);
        let mut received = Text::EMPTY;
        received.set(NO_MESSAGE);
        // Allocates, once, at startup: the handle, which outlives this call,
        // and the two nodes' names, read by every ping after.
        Ok(Box::into_raw(Box::new(tcs_pinger {
            uplink,
            pinger: Pinger::new(),
            node: CString::new(route.node).unwrap_or_default(),
            moc: CString::new(route.moc).unwrap_or_default(),
            sent,
            received,
        })))
    })
}

/// The ping node's name.
///
/// # Safety
///
/// `pinger` NULL or made by `tcs_pinger_open` and not yet closed.
#[no_mangle]
pub unsafe extern "C" fn tcs_pinger_node(pinger: *const tcs_pinger) -> *const c_char {
    guard(std::ptr::null(), || {
        Ok(handle_of(pinger, "the pinger")?.node.as_ptr())
    })
}

/// The moc node's name.
///
/// # Safety
///
/// As [`tcs_pinger_node`].
#[no_mangle]
pub unsafe extern "C" fn tcs_pinger_moc(pinger: *const tcs_pinger) -> *const c_char {
    guard(std::ptr::null(), || {
        Ok(handle_of(pinger, "the pinger")?.moc.as_ptr())
    })
}

/// Send the next ping.
///
/// # Safety
///
/// As [`tcs_pinger_node`], used from one thread at a time.
#[no_mangle]
pub unsafe extern "C" fn tcs_pinger_ping(pinger: *mut tcs_pinger) -> c_int {
    guard(-1, || {
        let ping = handle_of_mut(pinger, "the pinger")?;
        let message = ping.pinger.ping(now()?).map_err(fail)?;
        ping.uplink.send(message.as_bytes()).map_err(fail)?;
        ping.sent.set(message);
        Ok(0)
    })
}

/// Fold in what the ping node sent down.
///
/// # Safety
///
/// As [`tcs_pinger_ping`]; `data` NULL with `length` 0, or `length` readable
/// bytes.
#[no_mangle]
pub unsafe extern "C" fn tcs_pinger_answered(
    pinger: *mut tcs_pinger,
    data: *const u8,
    length: usize,
) -> c_int {
    guard(-1, || {
        let ping = handle_of_mut(pinger, "the pinger")?;
        let data: &[u8] = if length == 0 {
            &[]
        } else if data.is_null() {
            return Err(fail("the data is NULL"));
        } else {
            std::slice::from_raw_parts(data, length)
        };
        // Allocates, in tcslib, only where what arrived is not an answer, to
        // say why. Copied here into a fixed buffer.
        ping.pinger.answered(data, now()?);
        ping.received.set(
            ping.pinger
                .exchange()
                .received
                .as_deref()
                .unwrap_or(NO_MESSAGE),
        );
        Ok(0)
    })
}

/// What there is to know of the pings now.
///
/// # Safety
///
/// As [`tcs_pinger_node`]; `exchange` NULL or writable.
#[no_mangle]
pub unsafe extern "C" fn tcs_pinger_exchange(
    pinger: *const tcs_pinger,
    exchange: *mut tcs_exchange,
) -> c_int {
    guard(-1, || {
        let ping = handle_of(pinger, "the pinger")?;
        let out = out_of(exchange, "the exchange")?;
        let known = ping.pinger.exchange();
        *out = tcs_exchange {
            sent: c_int::from(known.sent_at.is_some()),
            sent_at: nanos(known.sent_at),
            answered: c_int::from(known.received_at.is_some()),
            received_at: nanos(known.received_at),
            answered_at: nanos(known.answered_at),
            round_trip: length(known.round_trip()),
            up: length(known.up()),
            down: length(known.down()),
        };
        Ok(0)
    })
}

/// The last ping sent, as text.
///
/// # Safety
///
/// As [`tcs_pinger_node`].
#[no_mangle]
pub unsafe extern "C" fn tcs_pinger_sent(pinger: *const tcs_pinger) -> *const c_char {
    guard(std::ptr::null(), || {
        Ok(handle_of(pinger, "the pinger")?.sent.as_ptr())
    })
}

/// The last thing received, as text.
///
/// # Safety
///
/// As [`tcs_pinger_node`].
#[no_mangle]
pub unsafe extern "C" fn tcs_pinger_received(pinger: *const tcs_pinger) -> *const c_char {
    guard(std::ptr::null(), || {
        Ok(handle_of(pinger, "the pinger")?.received.as_ptr())
    })
}

/// Stop pinging.
///
/// # Safety
///
/// `pinger` NULL or made by `tcs_pinger_open` and not yet closed.
#[no_mangle]
pub unsafe extern "C" fn tcs_pinger_close(pinger: *mut tcs_pinger) {
    if !pinger.is_null() {
        drop(Box::from_raw(pinger));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The words the header promises are the library's own: Stage::label
    /// and CommandStatus's names, every one, in the header's numbering.
    #[test]
    fn the_names_are_the_librarys() {
        for stage in [Stage::Unheard, Stage::Heard, Stage::Aging, Stage::Waiting] {
            let said = unsafe { CStr::from_ptr(tcs_stage_label(stage_number(stage))) };
            assert_eq!(said.to_str(), Ok(stage.label()));
        }
        for status in [
            CommandStatus::Success,
            CommandStatus::Failure,
            CommandStatus::InvalidCommand,
            CommandStatus::InvalidParameter,
            CommandStatus::NotArmed,
            CommandStatus::NotFound,
            CommandStatus::AlreadyExists,
            CommandStatus::Timeout,
        ] {
            let said = unsafe { CStr::from_ptr(tcs_status_name(status_number(status))) };
            assert_eq!(said.to_str(), Ok(format!("{status:?}").as_str()));
        }
    }

    /// A NULL or a file that is not there is an error with a reason, and not
    /// a crash.
    #[test]
    fn what_cannot_be_done_is_said() {
        let none = unsafe { tcs_configuration_open(std::ptr::null()) };
        assert!(none.is_null());
        let why = unsafe { CStr::from_ptr(tcs_last_error()) };
        assert!(why.to_str().unwrap().contains("NULL"), "{why:?}");

        let missing = CString::new("/nonexistent/tcs.yaml").unwrap();
        let none = unsafe { tcs_configuration_open(missing.as_ptr()) };
        assert!(none.is_null());
        let why = unsafe { CStr::from_ptr(tcs_last_error()) };
        assert!(
            why.to_str().unwrap().contains("/nonexistent/tcs.yaml"),
            "{why:?}"
        );

        // Closing nothing is nothing.
        unsafe {
            tcs_configuration_close(std::ptr::null_mut());
            tcs_beacons_close(std::ptr::null_mut());
            tcs_moc_close(std::ptr::null_mut());
            tcs_client_close(std::ptr::null_mut());
            tcs_pinger_close(std::ptr::null_mut());
        }
    }

    /// Text that fits is held whole; text that does not is cut at a
    /// character and ends "...", NUL-terminated either way; and a NUL in it
    /// is a space.
    #[test]
    fn a_buffer_holds_what_fits_and_says_where_it_was_cut() {
        let mut text: Text<8> = Text::EMPTY;
        text.set("hello");
        assert_eq!(text.as_str(), "hello");
        assert_eq!(text.bytes[5], 0);

        // Seven bytes of room: four of the text and the mark.
        text.set("hello world");
        assert_eq!(text.as_str(), "hell...");
        assert_eq!(text.bytes[7], 0);

        // Never in the middle of a character: "é" is two bytes.
        text.set("aaéééé");
        assert_eq!(text.as_str(), "aaé...");

        text.set("a\0b");
        assert_eq!(text.as_str(), "a b");

        // Written in pieces, as a message is put together, and cut once.
        text.set(format_args!("{}{}{}", "abc", "def", "ghi"));
        assert_eq!(text.as_str(), "abcd...");
    }

    /// A pass with more to say than there is room for keeps what fits and
    /// says how many were left out.
    #[test]
    fn too_many_remarks_are_counted_rather_than_lost() {
        let mut notes = Notes::EMPTY;
        notes.set(&["one".to_string(), "two".to_string()]);
        assert_eq!(notes.count, 2);
        assert_eq!(notes.get(1).map(Text::as_str), Some("two"));
        assert!(notes.get(2).is_none(), "nothing past what was said");

        let many: Vec<String> = (1..=MAX_REMARKS + 3)
            .map(|n| format!("remark {n}"))
            .collect();
        notes.set(&many);
        assert_eq!(notes.count, MAX_REMARKS);
        assert_eq!(notes.get(0).map(Text::as_str), Some("remark 1"));
        assert_eq!(
            notes.get(MAX_REMARKS - 1).map(Text::as_str),
            Some("... and 4 more")
        );
    }

    /// The sizes the header states are the sizes this holds to.
    #[test]
    fn the_header_states_these_sizes() {
        let header = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("include/tcs.h"),
        )
        .expect("the header reads");
        for (name, size) in [
            ("TCS_ERROR_BYTES", ERROR_BYTES),
            ("TCS_TEXT_BYTES", TEXT_BYTES),
            ("TCS_MAX_REMARKS", MAX_REMARKS),
            ("TCS_SAMPLE_BYTES", DH_SAMPLE_BYTES),
        ] {
            let line = format!("#define {name} {size}");
            assert!(header.contains(&line), "the header has no \"{line}\"");
        }
    }
}
