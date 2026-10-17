//! What the program itself holds: the interfaces it was run with and what its
//! command line told it, and nothing else.
//!
//! One type per kind of app, each holding a handle for every interface that
//! kind supports and no handle at all for the ones it does not. So an
//! `app_payload_veh` that reaches for the MOC interface does not compile,
//! which is the first and best place to refuse it: the descriptor not being
//! open is the second, and it only answers once the program is running.
//!
//! Five kinds: three that are nodes of the vehicle's own and two that are
//! payloads. The payload-side pair are run with the MOC interface, that being
//! the interface payload data is written on, and a payload's readings are what
//! it writes -- so [`AppPl`] and [`AppAllPl`] hold descriptor 1 where a
//! reader might have expected descriptor 0. Which of them a program is
//! depends on whether anything asks it: an `app_pl` is asked nothing and
//! holds a handle it can only write with.
//!
//! Each handle's methods are named for what they carry and not for the
//! direction -- `read_payload` and `write_moc` rather than `read` and `write`
//! -- because both happen on the one interface and a reader of an app should
//! not have to remember which way round descriptor 0 is.
//!
//! # What the command line told it
//!
//! A short, fixed list, the same for every node of a kind: the name of the
//! node this program is, and then the argument the configuration states for
//! each stream the kind handles -- payload before MOC. Each kind's
//! `inherited` takes its own, so the type says how many there are and a
//! program run with the wrong number is refused with a sentence rather than
//! reading somebody else's argument as its own.
//!
//! The name is there because a program may be several nodes at once: three
//! nodes running one program are three processes, and a complaint that does
//! not say which of them it came from is a complaint an operator cannot act
//! on.
//!
//! # Taking the interfaces
//!
//! [`AppPayloadVeh::inherited`] and its four neighbours take ownership of the
//! descriptors, so one of them is called once, at the start of a program. They
//! check what they take: every interface the kind supports has to be open, and
//! every interface it does not support has to be closed. A program run by
//! something that has misunderstood which kind of app it is would otherwise
//! read a terminal and wait, or write telemetry into somebody's shell -- and
//! the sentence it is refused with says which interface and what that
//! interface is for.

use std::fs::File;
use std::io;
use std::os::fd::{FromRawFd, RawFd};
use std::os::unix::net::UnixStream;

use crate::{is_handed_over, is_open, packet, Interface};

/// Payload data in, MOC data out: the interface at descriptor 0.
#[derive(Debug)]
pub struct PayloadInterface(UnixStream);

impl PayloadInterface {
    /// Read the next packet of payload data, or `None` once the interface has
    /// ended.
    pub fn read_payload(&mut self) -> io::Result<Option<Vec<u8>>> {
        packet::read(&mut self.0)
    }

    /// Write a packet of MOC data.
    pub fn write_moc(&mut self, data: &[u8]) -> io::Result<()> {
        packet::write(&mut self.0, data)
    }

    /// Another handle on the same interface.
    ///
    /// For an app that reads on one thread and writes on another, which is
    /// what an app supporting both interfaces is: each direction holds its own
    /// handle, and the two share the interface underneath.
    pub fn try_clone(&self) -> io::Result<PayloadInterface> {
        Ok(PayloadInterface(self.0.try_clone()?))
    }
}

/// MOC data in, payload data out: the interface at descriptor 1.
#[derive(Debug)]
pub struct MocInterface(UnixStream);

impl MocInterface {
    /// Read the next packet of MOC data, or `None` once the interface has
    /// ended.
    pub fn read_moc(&mut self) -> io::Result<Option<Vec<u8>>> {
        packet::read(&mut self.0)
    }

    /// Write a packet of payload data.
    pub fn write_payload(&mut self, data: &[u8]) -> io::Result<()> {
        packet::write(&mut self.0, data)
    }

    /// Another handle on the same interface. See
    /// [`PayloadInterface::try_clone`].
    pub fn try_clone(&self) -> io::Result<MocInterface> {
        Ok(MocInterface(self.0.try_clone()?))
    }
}

/// Payload data out, and nothing in: the interface at descriptor 1, held one
/// way about.
///
/// What an `app_pl` has. The interface is the MOC one, that being the
/// interface payload data is written on, but a payload that is never asked has
/// nothing to read there: nothing is linked into such a node, so nothing is
/// ever written to it, and a program waiting on the read would wait for ever.
///
/// Which is why it is a type of its own rather than a [`MocInterface`] used by
/// halves. The rule this module exists for is that an app cannot reach for
/// what it has not got, and "has not got" is as true of a direction as it is
/// of an interface: `read_moc` is not on this, so an `app_pl` that waits to be
/// asked does not compile.
#[derive(Debug)]
pub struct PayloadOut(UnixStream);

impl PayloadOut {
    /// Write a packet of payload data: a reading, on its way to the ground.
    pub fn write_payload(&mut self, data: &[u8]) -> io::Result<()> {
        packet::write(&mut self.0, data)
    }

    /// Another handle on the same interface. See
    /// [`PayloadInterface::try_clone`].
    pub fn try_clone(&self) -> io::Result<PayloadOut> {
        Ok(PayloadOut(self.0.try_clone()?))
    }
}

/// What went wrong, out: the interface at descriptor 2.
///
/// Packets like the other two, so that a message of several lines arrives as
/// one message: an app complaining while another thread complains would
/// otherwise have the two interleaved a line at a time, and the reader of the
/// result would be reading two half sentences.
#[derive(Debug)]
pub struct Errors(File);

impl Errors {
    /// Say what went wrong.
    pub fn say(&mut self, message: &str) -> io::Result<()> {
        packet::write(&mut self.0, message.as_bytes())
    }

    /// Another handle on the error interface, for an app that complains from
    /// more than one thread.
    pub fn try_clone(&self) -> io::Result<Errors> {
        Ok(Errors(self.0.try_clone()?))
    }
}

/// An app that reads payload data and writes MOC data.
///
/// The node type `app_payload_veh`. Descriptor 1 is not open: there is nothing
/// here to reach it with, and nothing else may reach it either.
#[derive(Debug)]
pub struct AppPayloadVeh {
    /// What the node this program is runs under in the configuration.
    pub name: String,
    /// What the configuration says about the payload stream, this kind
    /// handling payload data.
    pub payload_arg: String,
    pub payload: PayloadInterface,
    pub errors: Errors,
}

impl AppPayloadVeh {
    /// Take the interfaces and the arguments this program was run with.
    pub fn inherited() -> Result<AppPayloadVeh, String> {
        taking(&[Interface::Payload, Interface::Errors])?;
        let [name, payload_arg] = told(["the node's name", "app-payload-arg"])?;
        Ok(AppPayloadVeh {
            name,
            payload_arg,
            payload: PayloadInterface(stream(Interface::Payload)),
            errors: Errors(file(Interface::Errors)),
        })
    }
}

/// An app that reads MOC data and writes payload data.
///
/// The node type `app_moc_veh`. Descriptor 0 is not open.
#[derive(Debug)]
pub struct AppMocVeh {
    /// What the node this program is runs under in the configuration.
    pub name: String,
    /// What the configuration says about the MOC stream, this kind handling
    /// MOC data.
    pub moc_arg: String,
    pub moc: MocInterface,
    pub errors: Errors,
}

impl AppMocVeh {
    /// Take the interfaces and the arguments this program was run with.
    pub fn inherited() -> Result<AppMocVeh, String> {
        taking(&[Interface::Moc, Interface::Errors])?;
        let [name, moc_arg] = told(["the node's name", "app-moc-arg"])?;
        Ok(AppMocVeh {
            name,
            moc_arg,
            moc: MocInterface(stream(Interface::Moc)),
            errors: Errors(file(Interface::Errors)),
        })
    }
}

/// An app that does both.
///
/// The node type `app_all_veh`. All three interfaces are open, which makes it
/// the one kind that can carry a stream through: payload data read on the one
/// interface can be written on as payload data on the other.
#[derive(Debug)]
pub struct AppAllVeh {
    /// What the node this program is runs under in the configuration.
    pub name: String,
    /// What the configuration says about the payload stream.
    pub payload_arg: String,
    /// And about the MOC stream. It comes second on the command line, this
    /// kind handling both.
    pub moc_arg: String,
    pub payload: PayloadInterface,
    pub moc: MocInterface,
    pub errors: Errors,
}

impl AppAllVeh {
    /// Take the interfaces and the arguments this program was run with.
    pub fn inherited() -> Result<AppAllVeh, String> {
        taking(&[Interface::Payload, Interface::Moc, Interface::Errors])?;
        let [name, payload_arg, moc_arg] =
            told(["the node's name", "app-payload-arg", "app-moc-arg"])?;
        Ok(AppAllVeh {
            name,
            payload_arg,
            moc_arg,
            payload: PayloadInterface(stream(Interface::Payload)),
            moc: MocInterface(stream(Interface::Moc)),
            errors: Errors(file(Interface::Errors)),
        })
    }
}

/// An app that is a payload nothing asks: it writes payload data and reads
/// nothing.
///
/// The node type `app_pl`. Descriptor 0 is not open, as for an `app_moc_veh`,
/// and descriptor 1 is held as a [`PayloadOut`]: a payload that reports of its
/// own accord has no question to wait for, and the type says so by having
/// nothing to read with.
///
/// When it writes is its own business, which is the whole of what this kind is
/// for: a payload that samples on a clock of its own, or reports when
/// something happens, rather than one that answers a trigger.
#[derive(Debug)]
pub struct AppPl {
    /// What the node this program is runs under in the configuration.
    pub name: String,
    /// What the configuration says about the payload stream, which is the one
    /// stream this kind handles.
    pub payload_arg: String,
    pub payload: PayloadOut,
    pub errors: Errors,
}

impl AppPl {
    /// Take the interfaces and the arguments this program was run with.
    pub fn inherited() -> Result<AppPl, String> {
        taking(&[Interface::Moc, Interface::Errors])?;
        let [name, payload_arg] = told(["the node's name", "app-payload-arg"])?;
        Ok(AppPl {
            name,
            payload_arg,
            payload: PayloadOut(stream(Interface::Moc)),
            errors: Errors(file(Interface::Errors)),
        })
    }
}

/// An app that is a payload and is asked: MOC data in, payload data out.
///
/// The node type `app_all_pl`, and the same interface an `app_moc_veh` is run
/// with -- what tells the two apart is which side of the graph the node stands
/// on, not what its program holds. A payload is asked for a reading and
/// answers with one, so this is the kind that stands in for a payload as every
/// other payload behaves: see `tcslibgs::node::app`.
#[derive(Debug)]
pub struct AppAllPl {
    /// What the node this program is runs under in the configuration.
    pub name: String,
    /// What the configuration says about the payload stream: the readings this
    /// payload answers with.
    pub payload_arg: String,
    /// And about the MOC stream: what asks it for them. Second on the command
    /// line, this kind handling both.
    pub moc_arg: String,
    pub moc: MocInterface,
    pub errors: Errors,
}

impl AppAllPl {
    /// Take the interfaces and the arguments this program was run with.
    pub fn inherited() -> Result<AppAllPl, String> {
        taking(&[Interface::Moc, Interface::Errors])?;
        let [name, payload_arg, moc_arg] =
            told(["the node's name", "app-payload-arg", "app-moc-arg"])?;
        Ok(AppAllPl {
            name,
            payload_arg,
            moc_arg,
            moc: MocInterface(stream(Interface::Moc)),
            errors: Errors(file(Interface::Errors)),
        })
    }
}

/// What this program's command line said, for a kind that expects these
/// arguments.
///
/// `expects` is the configuration's own word for each of them, in order, which
/// is what a wrong count is refused with: a program told "it is run with the
/// node's name and app-payload-arg" can be looked up in the file that started
/// it.
///
/// Refused and not made good: a program run with the wrong number of arguments
/// has been started by something that has misunderstood which kind of app it
/// is, which is the mistake `taking` catches for the interfaces. Reading on
/// would mean taking one stream's argument for the other's.
fn told<const N: usize>(expects: [&str; N]) -> Result<[String; N], String> {
    let mut said: Vec<String> = Vec::with_capacity(N);
    for (at, argument) in std::env::args_os().enumerate().skip(1) {
        said.push(argument.into_string().map_err(|_| {
            format!(
                "argument {at} is not text: every argument an app is given is a \
                 word from its configuration, and a configuration is text"
            )
        })?);
    }
    if said.len() != N {
        return Err(format!(
            "this app is run with {N} argument{}, {}, and it was given {}: the \
             list is fixed by the kind of app it is",
            if N == 1 { "" } else { "s" },
            expects.join(" then "),
            said.len()
        ));
    }
    Ok(said.try_into().expect("a list of the length just counted"))
}

/// Say what went wrong when there is no app to say it with.
///
/// For the one moment an app has nothing to complain on: before `inherited`
/// has answered, and because it has refused. A program that cannot take its
/// interfaces still has to say why somewhere a parent reads, and its exit
/// status alone says only that it would not start.
///
/// A packet on the error interface where that is open, and nothing at all
/// where it is not -- there being nowhere to say it then, which is itself what
/// went wrong. Unframed text would be worse than silence: the first four bytes
/// of it would be read as a packet's length, and the parent would wait for
/// bytes that are not coming.
pub fn complain(message: &str) {
    if !is_open(Interface::Errors.fd()) {
        return;
    }
    // Held rather than owned: this does not close the interface, which an app
    // that goes on to use it would need and a `File` would close as it went
    // out of scope.
    let mut errors = std::mem::ManuallyDrop::new(file(Interface::Errors));
    let _ = packet::write(&mut *errors, message.as_bytes());
}

/// Whether this program was run the way this kind of app is run.
///
/// Every data interface it supports handed over, and every data interface it
/// does not support not. Both halves, because each catches a different
/// mistake: the first is a program that would wait forever on something nobody
/// attached, and the second is a program being run as a kind of app it is not
/// -- by a parent that read the configuration wrongly -- which would write the
/// vehicle's data wherever that descriptor happens to go.
///
/// Asked of what the descriptor *is* and not of whether it is open: see
/// [`is_handed_over`], and the note in this crate's own documentation about
/// what a Rust program finds at a descriptor its parent closed.
///
/// The error interface is asked only to be open, which it always is. Anything
/// that can be written will do for a complaint -- a socket from tcspecial, a
/// pipe, the terminal of somebody running the program to look at it -- and
/// refusing to start for want of somewhere to complain would be the one
/// refusal that could not be reported.
fn taking(supported: &[Interface]) -> Result<(), String> {
    for interface in [Interface::Payload, Interface::Moc] {
        let wanted = supported.contains(&interface);
        match (wanted, is_handed_over(interface.fd())) {
            (true, false) => {
                return Err(format!(
                    "{} was not handed over, and this app is one on which {}: it \
                     is run by tcspecial, which opens descriptor {} for it as \
                     one end of a socket pair",
                    interface.name(),
                    interface.carries(),
                    interface.fd()
                ))
            }
            (false, true) => {
                return Err(format!(
                    "{} was handed over at descriptor {}, and this app does not \
                     support it: a program run as a kind of app it is not would \
                     write the vehicle's data wherever that descriptor goes",
                    interface.name(),
                    interface.fd()
                ))
            }
            _ => {}
        }
    }
    if !is_open(Interface::Errors.fd()) {
        return Err(format!(
            "{} is not open at descriptor {}, and there is nowhere to say what \
             goes wrong",
            Interface::Errors.name(),
            Interface::Errors.fd()
        ));
    }
    Ok(())
}

/// One of the two interfaces that is read and written, which is a socket.
///
/// # Safety
///
/// Called once per interface, from the `inherited` of the one kind of app that
/// supports it, which is itself called once at the start of a program: the
/// descriptor is owned from here on and closed when the handle is dropped.
fn stream(interface: Interface) -> UnixStream {
    unsafe { UnixStream::from_raw_fd(interface.fd() as RawFd) }
}

/// And the one that is only written, which need be no more than a file.
///
/// # Safety
///
/// As [`stream`].
fn file(interface: Interface) -> File {
    unsafe { File::from_raw_fd(interface.fd() as RawFd) }
}
