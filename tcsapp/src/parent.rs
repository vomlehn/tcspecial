//! Starting an app, and holding the other end of each of its interfaces.
//!
//! What tcspecial does with an app node, and what a test of an app does: the
//! two want exactly the same thing, which is why it is here rather than in the
//! flight software. A test that started an app its own way would be a test of
//! an arrangement nothing uses.
//!
//! # The other end of each interface
//!
//! Each of the two data interfaces is a socket pair, not a pipe, because each
//! is read *and* written at both ends: the app reads payload data and writes
//! MOC data on the one descriptor, so this end writes payload data and reads
//! MOC data on its own. The names here are the app's, turned around -- what
//! [`Running::write_payload`] writes is what the app reads with
//! `read_payload` -- so that both ends of a sentence about an interface use
//! the same words.
//!
//! The error interface is a socket pair too, of which only this end is read.
//!
//! # What it is told on its command line
//!
//! A short, fixed list, built by `tcslibgs::node::app`: the name of the node
//! it is, and then the argument the configuration states for each stream its
//! kind handles, payload before MOC. See [`start`].
//!
//! An interface the app does not support is not opened for it at all: the
//! descriptor is closed in the child between the fork and the exec, so the
//! program starts with nothing there. Not `/dev/null`, which would be a write
//! that silently succeeds and a read that silently ends -- the two outcomes an
//! app node must never have.
//!
//! # Reading does not wait
//!
//! The reading here answers with whatever has arrived and does not wait for
//! more: [`Arrived::Nothing`] is the ordinary answer from a program that has
//! nothing to say. That is what the flight software needs -- a vehicle reads
//! what is ready and goes on with the pass, and one that waited on a program
//! would stop reading its sensors while it waited -- and it is why each end
//! keeps the packet that has half arrived until the rest of it comes.
//!
//! The waiting kind is here as well, for a tool and for a test, which have
//! nothing else to do until the program answers: [`Running::read_moc`] and its
//! neighbours wait up to [`Running::waits`], and the `_now` forms beside them
//! are what the fabric calls.

use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use crate::{packet, Interface};

/// How long a waiting read waits before it is a failure, unless something says
/// otherwise.
///
/// Long enough that a program doing real work is not cut off, short enough
/// that a program which has stopped answering is reported while somebody is
/// still watching. It bounds only the waiting reads: what the fabric calls
/// answers at once, with nothing, and waits for nobody.
///
/// A wait that is up part way through a packet is not the end of the
/// interface. What has arrived is kept, and the next read carries on from
/// there -- which is the whole point of each end holding its own part.
pub const WAITS: Duration = Duration::from_secs(5);

/// Which of the two data interfaces an app supports.
///
/// The error interface is not here because every app has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Supports {
    /// Reads payload data and writes MOC data, at descriptor 0.
    pub payload: bool,
    /// Reads MOC data and writes payload data, at descriptor 1.
    pub moc: bool,
}

impl Supports {
    /// What an `app_payload_veh` supports.
    pub const PAYLOAD: Supports = Supports {
        payload: true,
        moc: false,
    };
    /// What an `app_moc_veh` supports, and the payload-side pair with it.
    ///
    /// An `app_pl` and an `app_all_pl` are run with this interface too: a
    /// payload's readings are what it writes, and this is the interface payload
    /// data is written on. What tells the three apart is what each does with
    /// it, which is the app's end of the arrangement and not this one's -- see
    /// [`crate::app`].
    pub const MOC: Supports = Supports {
        payload: false,
        moc: true,
    };
    /// And an `app_all_veh`.
    pub const ALL: Supports = Supports {
        payload: true,
        moc: true,
    };
}

/// One end of an interface, and the packet that has half arrived on it.
///
/// Non-blocking underneath, always, because the flight software reads what is
/// ready and goes on with the pass: a vehicle that waited on a program with
/// nothing to say would stop reading its sensors while it waited. Which is
/// what this structure is for -- a socket read when nothing is there answers
/// with nothing, and a socket read in the middle of a packet answers with the
/// part that came, so the part has to be kept until the rest of it arrives.
///
/// A packet in two reads was going to happen anyway: a program that writes a
/// length and then its bytes has written twice, and a reader that threw the
/// first away would lose the datum and then blame the program for not sending
/// one. `tcspecial::node::vehicle::compute::process` keeps a partial line for
/// the same reason.
#[derive(Debug)]
struct End {
    socket: UnixStream,
    /// How long the packet being read is, once its length has arrived.
    want: Option<usize>,
    /// What has arrived of whichever of the two is being read: the length, or
    /// the bytes after it.
    ///
    /// Kept between packets, cleared rather than given away by
    /// [`End::read_now_into`], so that it grows to the longest packet the
    /// program writes and is not made again.
    got: Vec<u8>,
    /// What a packet is framed in before it is written: kept, as `got` is.
    framed: Vec<u8>,
}

/// What came off an interface into a caller's buffer: see
/// [`End::read_now_into`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadInto {
    /// A whole packet of this many bytes, now at the front of the buffer.
    Packet(usize),
    /// A whole packet of this many bytes, more than the buffer holds: taken
    /// off the interface, and not copied, part of a packet not being a
    /// smaller packet.
    TooLong(usize),
    /// Nothing yet.
    Nothing,
    /// The interface has ended.
    Ended,
}

/// How far a read has got: see [`End::fill`].
enum Filled {
    /// A whole packet is in `got`.
    Packet,
    Nothing,
    Ended,
}

/// What came off an interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arrived {
    /// A whole packet, which may be of no bytes at all.
    Packet(Vec<u8>),
    /// Nothing yet. The ordinary answer from a program that has nothing to
    /// say, and not a failure.
    Nothing,
    /// The interface has ended: the program has finished with it, or has
    /// stopped.
    Ended,
}

impl End {
    /// This end of an interface, read and written without waiting.
    fn of(socket: UnixStream) -> io::Result<End> {
        socket.set_nonblocking(true)?;
        Ok(End {
            socket,
            want: None,
            got: Vec::new(),
            framed: Vec::new(),
        })
    }

    /// Whatever has arrived: a whole packet, nothing yet, or the end.
    ///
    /// Reads at most one packet, so a program that has written three leaves
    /// two for the next two calls. Which is what the fabric wants: one datum
    /// per delivery, and the next one when it comes round again.
    fn read_now(&mut self) -> io::Result<Arrived> {
        Ok(match self.fill()? {
            // Given away, which is what a caller wanting the packet as a
            // `Vec` asks for: a tool, a test. The fabric reads with
            // `read_now_into` instead, which keeps the buffer.
            Filled::Packet => Arrived::Packet(std::mem::take(&mut self.got)),
            Filled::Nothing => Arrived::Nothing,
            Filled::Ended => Arrived::Ended,
        })
    }

    /// The same, the packet copied to the front of `out` rather than handed
    /// over, so that reading one allocates nothing once `got` has grown to
    /// the longest packet the program writes.
    fn read_now_into(&mut self, out: &mut [u8]) -> io::Result<ReadInto> {
        Ok(match self.fill()? {
            Filled::Packet => {
                let len = self.got.len();
                let into = match out.get_mut(..len) {
                    Some(room) => {
                        room.copy_from_slice(&self.got);
                        ReadInto::Packet(len)
                    }
                    None => ReadInto::TooLong(len),
                };
                self.got.clear();
                into
            }
            Filled::Nothing => ReadInto::Nothing,
            Filled::Ended => ReadInto::Ended,
        })
    }

    /// Read until a whole packet is in `got`, or there is nothing more to
    /// read now, or the interface has ended.
    fn fill(&mut self) -> io::Result<Filled> {
        loop {
            let wanted = match self.want {
                None => packet::LENGTH,
                Some(length) => length,
            };

            while self.got.len() < wanted {
                let mut byte = [0u8; 4096];
                let room = (wanted - self.got.len()).min(byte.len());
                match self.socket.read(&mut byte[..room]) {
                    Ok(0) => {
                        // The far end has gone. Part of a packet left behind
                        // is a program that died between writing a length and
                        // writing its bytes, which is not an end: half a datum
                        // is not a datum, and passing it on would pass on
                        // something nobody wrote.
                        return if self.got.is_empty() && self.want.is_none() {
                            Ok(Filled::Ended)
                        } else {
                            Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                format!(
                                    "the interface ended {} bytes into a \
                                     packet",
                                    self.got.len()
                                ),
                            ))
                        };
                    }
                    Ok(some) => self.got.extend_from_slice(&byte[..some]),
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        return Ok(Filled::Nothing)
                    }
                    // What a signal leaves behind: nothing has been lost, so
                    // the read is made again.
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                        continue
                    }
                    Err(e) => return Err(e),
                }
            }

            match self.want {
                // The length has arrived; the bytes after it may be there too,
                // so round again rather than answering with nothing.
                None => {
                    // Read where it lies, and the buffer cleared for the
                    // bytes after it: nothing made.
                    let said: [u8; packet::LENGTH] = self.got[..packet::LENGTH]
                        .try_into()
                        .expect("four bytes of length");
                    self.got.clear();
                    self.want = Some(u32::from_ne_bytes(said) as usize);
                }
                Some(_) => {
                    self.want = None;
                    return Ok(Filled::Packet);
                }
            }
        }
    }

    /// And the same, waiting up to `how_long` for one.
    ///
    /// For a tool and for a test, which have nothing else to do until the
    /// program answers. `None` waits as long as it takes.
    fn read_waiting(
        &mut self,
        how_long: Option<Duration>,
    ) -> io::Result<Arrived> {
        let deadline = how_long.map(|how_long| Instant::now() + how_long);
        loop {
            match self.read_now()? {
                Arrived::Nothing => {}
                answer => return Ok(answer),
            }

            let left = match deadline {
                None => None,
                Some(deadline) => {
                    let left =
                        deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "nothing arrived before the wait was up",
                        ));
                    }
                    Some(left)
                }
            };
            self.ready(libc::POLLIN, left)?;
        }
    }

    /// Write a whole packet, waiting for room where there is none.
    ///
    /// A packet written in part would be a length with somebody else's bytes
    /// after it, so this does not answer until all of it has gone. The waiting
    /// is for a program that is not reading: its socket fills, and the only
    /// alternatives to waiting are to lose the datum or to hold it in a queue
    /// of our own, which is a queue with no bound.
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        // Framed in the end's own buffer, taken out while it is written and
        // put back after: it grows to the longest packet written, once.
        let mut whole = std::mem::take(&mut self.framed);
        whole.clear();
        let written = self.write_framed(&mut whole, data);
        self.framed = whole;
        written
    }

    fn write_framed(
        &mut self,
        whole: &mut Vec<u8>,
        data: &[u8],
    ) -> io::Result<()> {
        packet::write(whole, data)?;

        let mut gone = 0;
        while gone < whole.len() {
            match self.socket.write(&whole[gone..]) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "the interface took none of a packet",
                    ))
                }
                Ok(some) => gone += some,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.ready(libc::POLLOUT, None)?;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Wait until the socket can be read or written, or `how_long` is up.
    ///
    /// `poll` on the one descriptor, because a non-blocking socket with
    /// nothing on it has to be waited for somehow and a sleeping loop would
    /// answer late by however long it slept.
    fn ready(
        &self,
        for_what: libc::c_short,
        how_long: Option<Duration>,
    ) -> io::Result<()> {
        let mut asking = libc::pollfd {
            fd: self.socket.as_raw_fd(),
            events: for_what,
            revents: 0,
        };
        let waiting = match how_long {
            // Milliseconds, rounded up, because `poll` counts in those: a
            // wait rounded down to nothing would be a loop that spins.
            Some(how_long) => {
                how_long.as_millis().min(i32::MAX as u128).max(1) as i32
            }
            None => -1,
        };
        loop {
            // SAFETY: poll needs an array of as many pollfds as it is told,
            // writable for the call: `asking` is one local pollfd, and the
            // count is 1. Its descriptor is `self.socket`'s, open for as long
            // as `self` is borrowed.
            let said = unsafe { libc::poll(&mut asking, 1, waiting) };
            if said >= 0 {
                return Ok(());
            }
            let e = io::Error::last_os_error();
            if e.kind() != io::ErrorKind::Interrupted {
                return Err(e);
            }
        }
    }

    /// Say that nothing more will be written.
    fn no_more(&self) -> io::Result<()> {
        self.socket.shutdown(Shutdown::Write)
    }
}

/// An app, running, and this end of its interfaces.
#[derive(Debug)]
pub struct Running {
    child: Child,
    /// This end of the payload interface, where it has one: payload data is
    /// written here and MOC data read from it.
    payload: Option<End>,
    /// And of the MOC interface: MOC data written, payload data read.
    moc: Option<End>,
    /// This end of the error interface, which is only read.
    errors: End,
    /// How long a waiting read waits. See [`WAITS`].
    waits: Option<Duration>,
}

/// Start `executable` as an app supporting `supports`, run with `arguments`.
///
/// The environment it is given is the one this process has. The arguments are
/// a short, fixed list and are the one thing an app is told other than over
/// its interfaces: the name of the node it is, and then what the
/// configuration says about each stream its kind handles -- payload before
/// MOC. `tcslibgs::node::app::AppParams::arguments` is what builds the list,
/// and the order is fixed by the kind rather than by what a file happened to
/// state, so a program reads argument two without counting.
///
/// The name is first because a program may be several nodes at once: three
/// `app_pl` nodes running one program are three processes which otherwise
/// could not say which of them each is, in a complaint or in anything they
/// write.
///
/// The data itself never comes this way. What arrives on an interface is what
/// the node was given, and a command line that carried readings would be a
/// node whose work was outside both the configuration and the program.
pub fn start(
    executable: &Path,
    arguments: &[String],
    supports: Supports,
) -> Result<Running, String> {
    let named = |what: &str, e: io::Error| {
        format!("{}: {what}: {e}", executable.display())
    };

    let payload = supports
        .payload
        .then(|| {
            UnixStream::pair().map_err(|e| named("the payload interface", e))
        })
        .transpose()?;
    let moc = supports
        .moc
        .then(|| UnixStream::pair().map_err(|e| named("the MOC interface", e)))
        .transpose()?;
    let errors =
        UnixStream::pair().map_err(|e| named("the error interface", e))?;

    let mut command = Command::new(executable);
    command.args(arguments);
    // Each of the three descriptors is set from this end's own pair, or to
    // nothing where the app does not support it -- and the nothing is closed
    // below, `Stdio::null` being a descriptor that is open on /dev/null.
    match &payload {
        Some((_, theirs)) => command.stdin(childs_half(theirs)),
        None => command.stdin(Stdio::null()),
    };
    match &moc {
        Some((_, theirs)) => command.stdout(childs_half(theirs)),
        None => command.stdout(Stdio::null()),
    };
    command.stderr(childs_half(&errors.1));

    // What the app does not support, closed in the child. After the stdio is
    // in place, which is the order the standard library does it in, and before
    // the program is anything but this one: `close` is as much as may be done
    // between a fork and an exec, and it is all that is wanted.
    let shut: Vec<i32> = [
        (!supports.payload).then_some(Interface::Payload.fd()),
        (!supports.moc).then_some(Interface::Moc.fd()),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !shut.is_empty() {
        // SAFETY: pre_exec needs a closure that is safe to run between fork
        // and exec, in a child that may have one thread: nothing that
        // allocates, locks or is not async-signal-safe. This walks a Vec it
        // owns, allocated before the fork, and calls close, which is
        // async-signal-safe; and the descriptors it closes are the child's
        // copies, which nothing in the child owns or uses again before exec.
        unsafe {
            command.pre_exec(move || {
                for fd in &shut {
                    libc::close(*fd);
                }
                Ok(())
            });
        }
    }

    let child = command.spawn().map_err(|e| {
        format!(
            "{} could not be started: {e}. It is the program an app node is",
            executable.display()
        )
    })?;

    let end = |socket: UnixStream, which: &str| {
        End::of(socket)
            .map_err(|e| named(&format!("{which} would not be made ready"), e))
    };
    Ok(Running {
        child,
        payload: payload
            .map(|(mine, _)| end(mine, "the payload interface"))
            .transpose()?,
        moc: moc
            .map(|(mine, _)| end(mine, "the MOC interface"))
            .transpose()?,
        errors: end(errors.0, "the error interface")?,
        waits: Some(WAITS),
    })
}

/// The child's half of a pair, as what a command wants for a descriptor.
///
/// Cloned rather than moved out of the pair, so that `start` can hold the
/// pair until it has every one of them: a half given away before the spawn
/// could not be given back if the next pair failed to be made.
fn childs_half(stream: &UnixStream) -> Stdio {
    let held = stream
        .try_clone()
        .map(OwnedFd::from)
        .expect("a socket pair's own half can be held");
    Stdio::from(held)
}

impl Running {
    /// How long a waiting read waits.
    ///
    /// `None` waits as long as it takes, which is what a caller with a timer
    /// of its own wants. Nothing here bounds what the fabric calls: those
    /// answer at once.
    pub fn waits(&mut self, how_long: Option<Duration>) {
        self.waits = how_long;
    }

    /// Write a packet of payload data, which the app reads as payload data.
    pub fn write_payload(&mut self, data: &[u8]) -> Result<(), String> {
        let end = Self::held(&mut self.payload, Interface::Payload)?;
        end.write_all(data)
            .map_err(|e| said(Interface::Payload, "written", e))
    }

    /// Whatever MOC data has arrived from the app, without waiting.
    ///
    /// What the flight software calls: [`Arrived::Nothing`] is a program with
    /// nothing to say, which is the ordinary answer and not a failure.
    pub fn read_moc_now(&mut self) -> Result<Arrived, String> {
        let end = Self::held(&mut self.payload, Interface::Payload)?;
        end.read_now()
            .map_err(|e| said(Interface::Payload, "read", e))
    }

    /// The same, into `out`, allocating nothing: what the fabric reads with.
    pub fn read_moc_now_into(
        &mut self,
        out: &mut [u8],
    ) -> Result<ReadInto, String> {
        let end = Self::held(&mut self.payload, Interface::Payload)?;
        end.read_now_into(out)
            .map_err(|e| said(Interface::Payload, "read", e))
    }

    /// And the same, waiting up to [`Running::waits`] for a packet.
    pub fn read_moc(&mut self) -> Result<Option<Vec<u8>>, String> {
        let waits = self.waits;
        let end = Self::held(&mut self.payload, Interface::Payload)?;
        waited(end.read_waiting(waits), Interface::Payload)
    }

    /// Write a packet of MOC data, which the app reads as MOC data.
    pub fn write_moc(&mut self, data: &[u8]) -> Result<(), String> {
        let end = Self::held(&mut self.moc, Interface::Moc)?;
        end.write_all(data)
            .map_err(|e| said(Interface::Moc, "written", e))
    }

    /// Whatever payload data has arrived from the app, without waiting.
    pub fn read_payload_now(&mut self) -> Result<Arrived, String> {
        let end = Self::held(&mut self.moc, Interface::Moc)?;
        end.read_now().map_err(|e| said(Interface::Moc, "read", e))
    }

    /// The same, into `out`, allocating nothing: what the fabric reads with.
    pub fn read_payload_now_into(
        &mut self,
        out: &mut [u8],
    ) -> Result<ReadInto, String> {
        let end = Self::held(&mut self.moc, Interface::Moc)?;
        end.read_now_into(out)
            .map_err(|e| said(Interface::Moc, "read", e))
    }

    /// And the same, waiting up to [`Running::waits`] for a packet.
    pub fn read_payload(&mut self) -> Result<Option<Vec<u8>>, String> {
        let waits = self.waits;
        let end = Self::held(&mut self.moc, Interface::Moc)?;
        waited(end.read_waiting(waits), Interface::Moc)
    }

    /// Whatever the app has said went wrong, without waiting.
    pub fn read_complaint_now(&mut self) -> Result<Arrived, String> {
        self.errors
            .read_now()
            .map_err(|e| said(Interface::Errors, "read", e))
    }

    /// And the same, waiting up to [`Running::waits`].
    ///
    /// One message per packet, as text: whatever an app writes there is meant
    /// to be read by a person, and a message that is not text is a message
    /// nobody reads -- so bytes that are not UTF-8 are reported as such rather
    /// than passed on.
    pub fn read_complaint(&mut self) -> Result<Option<String>, String> {
        let waits = self.waits;
        let said = waited(self.errors.read_waiting(waits), Interface::Errors)?;
        said.map(words).transpose()
    }

    /// Say that nothing more will be written to the app.
    ///
    /// Which is how an app is told to finish: its interfaces end, its reads
    /// answer with nothing left, and it stops of its own accord. A parent that
    /// killed it instead would be a parent that lost whatever it had not yet
    /// written.
    pub fn no_more(&mut self) -> Result<(), String> {
        for end in [self.payload.as_ref(), self.moc.as_ref()]
            .into_iter()
            .flatten()
        {
            end.no_more().map_err(|e| {
                format!("an app's interface would not be shut down: {e}")
            })?;
        }
        Ok(())
    }

    /// Wait for the app to stop, and say how it stopped.
    pub fn waited_for(&mut self) -> Result<ExitStatus, String> {
        self.child
            .wait()
            .map_err(|e| format!("an app could not be waited for: {e}"))
    }

    /// Whether it has stopped, without waiting for it to.
    pub fn stopped(&mut self) -> Result<Option<ExitStatus>, String> {
        self.child.try_wait().map_err(|e| {
            format!("an app could not be asked whether it has stopped: {e}")
        })
    }

    /// Stop the app, whatever it is doing.
    ///
    /// For a parent that is giving up on it. [`Running::no_more`] is the way
    /// to end one that is working.
    pub fn stop(&mut self) -> Result<ExitStatus, String> {
        let _ = self.child.kill();
        self.waited_for()
    }

    /// The interface, or a sentence about an app that has not got one.
    ///
    /// Unreachable through the five node types, each of which offers only
    /// what its kind supports -- see `tcspecial::node` -- and here all the
    /// same, because this end of an interface is held in a structure and a
    /// structure cannot refuse at compile time what a type can.
    fn held(
        end: &mut Option<End>,
        interface: Interface,
    ) -> Result<&mut End, String> {
        end.as_mut().ok_or_else(|| {
            format!(
                "this app does not support {}, on which {}: it was started \
                 without it, and descriptor {} is not open in it",
                interface.name(),
                interface.carries(),
                interface.fd()
            )
        })
    }
}

/// A packet that was waited for, as the callers before `Arrived` wanted it.
///
/// `None` is the interface having ended. A wait that was up is an error, and
/// says so: a caller that asked to be told within a time and was not has
/// learned something about the program.
fn waited(
    answer: io::Result<Arrived>,
    interface: Interface,
) -> Result<Option<Vec<u8>>, String> {
    match answer {
        Ok(Arrived::Packet(data)) => Ok(Some(data)),
        Ok(Arrived::Ended) => Ok(None),
        Ok(Arrived::Nothing) => Err(format!(
            "{} said nothing before the wait was up",
            interface.name()
        )),
        Err(e) => Err(said(interface, "read", e)),
    }
}

/// A complaint as the text it is meant to be.
fn words(said: Vec<u8>) -> Result<String, String> {
    String::from_utf8(said).map_err(|e| {
        format!(
            "an app wrote a complaint that is not text: {} bytes of it are not",
            e.as_bytes().len()
        )
    })
}

/// What went wrong with an interface, as a sentence.
fn said(interface: Interface, doing: &str, e: io::Error) -> String {
    format!("{} could not be {doing}: {e}", interface.name())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This end of an interface, and the other end to write on.
    fn pair() -> (End, UnixStream) {
        let (mine, theirs) = UnixStream::pair().expect("a socket pair");
        (End::of(mine).expect("an end"), theirs)
    }

    /// Counting the allocations made on one thread, for
    /// [`a_running_interface_allocates_nothing`]: the process's allocator,
    /// counting while the thread asks it to.
    mod counting {
        use std::alloc::{GlobalAlloc, Layout, System};
        use std::cell::Cell;

        thread_local! {
            static COUNTING: Cell<bool> = const { Cell::new(false) };
            static COUNT: Cell<usize> = const { Cell::new(0) };
        }

        pub struct Counting;

        fn note() {
            if COUNTING.try_with(Cell::get).unwrap_or(false) {
                let _ = COUNT.try_with(|count| count.set(count.get() + 1));
            }
        }

        // SAFETY: GlobalAlloc asks that an allocator not unwind and that it
        // give and take back memory as its layouts say. Every method here
        // hands its arguments unchanged to System, which keeps that contract,
        // and before it only counts: `note` touches two const-initialized
        // thread locals through `try_with`, which neither allocates nor
        // panics, even while the thread is being torn down.
        unsafe impl GlobalAlloc for Counting {
            unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
                note();
                System.alloc(layout)
            }
            unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
                note();
                System.alloc_zeroed(layout)
            }
            unsafe fn realloc(
                &self,
                ptr: *mut u8,
                layout: Layout,
                size: usize,
            ) -> *mut u8 {
                note();
                System.realloc(ptr, layout, size)
            }
            unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
                System.dealloc(ptr, layout)
            }
        }

        #[global_allocator]
        static ALLOCATOR: Counting = Counting;

        pub fn allocations_in(work: impl FnOnce()) -> usize {
            COUNT.with(|count| count.set(0));
            COUNTING.with(|on| on.set(true));
            work();
            COUNTING.with(|on| on.set(false));
            COUNT.with(Cell::get)
        }
    }

    /// Once its buffers have grown to the packets it carries, an interface
    /// reads a packet into a caller's buffer and writes one without
    /// allocating: what the fabric does with an app node on every pass.
    #[test]
    fn a_running_interface_allocates_nothing() {
        let (mut end, mut theirs) = pair();
        let mut out = [0u8; 64];
        let mut read_back = [0u8; 64];

        let mut round = |end: &mut End, theirs: &mut UnixStream| {
            packet::write(theirs, b"293.15").expect("the program writes");
            assert_eq!(
                end.read_now_into(&mut out).expect("a read"),
                ReadInto::Packet(6)
            );
            end.write_all(b"1").expect("a packet written");
            let mut whole = [0u8; packet::LENGTH + 1];
            std::io::Read::read_exact(theirs, &mut whole)
                .expect("the program reads");
            read_back[..1].copy_from_slice(&whole[packet::LENGTH..]);
        };

        // Settling: the end's buffers grow to these packets, once.
        for _ in 0..3 {
            round(&mut end, &mut theirs);
        }
        let made = counting::allocations_in(|| {
            for _ in 0..20 {
                round(&mut end, &mut theirs);
            }
        });
        assert_eq!(made, 0, "twenty packets each way allocated {made} times");
        assert_eq!(&read_back[..1], b"1");
    }

    /// Nothing on an interface is nothing, and not a failure.
    ///
    /// The ordinary answer from a program with nothing to say, which the
    /// fabric asks of every app on every pass.
    #[test]
    fn an_interface_with_nothing_on_it_says_nothing() {
        let (mut end, _theirs) = pair();
        assert_eq!(end.read_now().expect("nothing yet"), Arrived::Nothing);
        assert_eq!(end.read_now().expect("still nothing"), Arrived::Nothing);
    }

    /// A packet that arrives in pieces is one packet.
    ///
    /// The whole reason an end keeps what has arrived: a program writes a
    /// length and then its bytes, and a reader that threw away what came
    /// first would lose the datum and then blame the program for not sending
    /// one. Nothing is said until all of it is there.
    #[test]
    fn a_packet_that_arrives_in_pieces_is_one_packet() {
        let (mut end, mut theirs) = pair();

        let mut whole = Vec::new();
        packet::write(&mut whole, b"293.15").expect("a packet");
        assert_eq!(whole.len(), packet::LENGTH + 6);

        // Half a length, then the rest of it with one byte of the data, then
        // what is left: nothing until the last of it.
        for piece in [&whole[..2], &whole[2..packet::LENGTH + 1]] {
            theirs.write_all(piece).expect("a piece of it");
            assert_eq!(
                end.read_now().expect("a piece is not a packet"),
                Arrived::Nothing
            );
        }
        theirs
            .write_all(&whole[packet::LENGTH + 1..])
            .expect("the rest");
        assert_eq!(
            end.read_now().expect("the whole of it"),
            Arrived::Packet(b"293.15".to_vec())
        );
        // And one packet at a time: nothing is left over.
        assert_eq!(end.read_now().expect("nothing more"), Arrived::Nothing);
    }

    /// Several packets at once are read one at a time.
    ///
    /// Which is what the fabric wants: one datum per delivery, and the next
    /// one when it comes round again.
    #[test]
    fn packets_are_read_one_at_a_time() {
        let (mut end, mut theirs) = pair();

        let mut whole = Vec::new();
        for data in [b"one".to_vec(), Vec::new(), b"three".to_vec()] {
            packet::write(&mut whole, &data).expect("a packet");
        }
        theirs.write_all(&whole).expect("all three");

        assert_eq!(
            end.read_now().expect("the first"),
            Arrived::Packet(b"one".to_vec())
        );
        // A packet of no bytes is a packet, and does not read as the end.
        assert_eq!(
            end.read_now().expect("the second"),
            Arrived::Packet(Vec::new())
        );
        assert_eq!(
            end.read_now().expect("the third"),
            Arrived::Packet(b"three".to_vec())
        );
        assert_eq!(end.read_now().expect("no more"), Arrived::Nothing);
    }

    /// An interface whose far end has gone has ended.
    #[test]
    fn an_interface_whose_far_end_has_gone_has_ended() {
        let (mut end, theirs) = pair();
        drop(theirs);
        assert_eq!(end.read_now().expect("the end"), Arrived::Ended);
    }

    /// But one that goes away mid-packet is a failure, not an end.
    ///
    /// Half a datum is not a datum: a program killed between writing a length
    /// and writing its bytes has left something nobody wrote, and passing it
    /// on as a short packet would pass that on.
    #[test]
    fn a_far_end_that_goes_mid_packet_is_a_failure() {
        let (mut end, mut theirs) = pair();
        theirs.write_all(&6u32.to_ne_bytes()).expect("a length");
        theirs.write_all(b"293").expect("half the bytes");
        assert_eq!(end.read_now().expect("not yet"), Arrived::Nothing);

        drop(theirs);
        let said = end.read_now().expect_err("a packet cut short");
        assert_eq!(said.kind(), io::ErrorKind::UnexpectedEof);
        assert!(said.to_string().contains("3 bytes into"), "{said}");
    }

    /// A waiting read gives up, and what arrived is still there afterwards.
    #[test]
    fn a_wait_that_is_up_leaves_what_arrived_where_it_was() {
        let (mut end, mut theirs) = pair();
        theirs.write_all(&6u32.to_ne_bytes()).expect("a length");

        let said = end
            .read_waiting(Some(Duration::from_millis(50)))
            .expect_err("a packet that never finishes arriving");
        assert_eq!(said.kind(), io::ErrorKind::TimedOut);

        // The rest of it, and the packet is whole: the wait being up is not
        // the interface being lost.
        theirs.write_all(b"293.15").expect("the rest");
        assert_eq!(
            end.read_waiting(Some(Duration::from_millis(50)))
                .expect("the whole of it"),
            Arrived::Packet(b"293.15".to_vec())
        );
    }
}
