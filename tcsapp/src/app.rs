//! What the program itself holds: the interfaces it was run with and what its
//! command line told it, and nothing else.
//!
//! One type per kind of app, each holding a handle for every interface that
//! kind supports and no handle at all for the ones it does not. So an
//! `app_payload_veh` that reaches for the gateway interface does not compile,
//! which is the first and best place to refuse it: the descriptor not being
//! open is the second, and it only answers once the program is running.
//!
//! Five kinds: three that are nodes of the vehicle's own and two that are
//! payloads. The payload-side pair are run with the gateway interface, that
//! being the interface payload data is written on, and a payload's readings are
//! what it writes -- so [`AppPl`] and [`AppAllPl`] hold descriptor 1 where a
//! reader might have expected descriptor 0. Which of them a program is
//! depends on whether anything asks it: an `app_pl` is asked nothing and
//! holds a handle it can only write with.
//!
//! Each handle's methods are named for what they carry and not for the
//! direction -- `read_payload` and `write_gateway` rather than `read` and
//! `write` -- because both happen on the one interface and a reader of an app
//! should not have to remember which way round descriptor 0 is.
//!
//! # What the command line told it
//!
//! A short, fixed list, the same for every node of a kind: the name of the
//! node this program is, and then the argument the configuration states for
//! each stream the kind handles -- payload before gateway. Each kind's
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
//!
//! # And the standard streams
//!
//! Descriptors 0, 1 and 2 are the interfaces, which is what lets a program in
//! any language take part; they are also a Rust program's stdin, stdout and
//! stderr. A `println!` would write unframed text into a data interface, and a
//! panic message into the error interface, each read at the far end as a
//! packet's length followed by nonsense. So once `inherited` has its handles
//! it moves each interface to a descriptor of its own, above 2 and closed on
//! exec, and opens `/dev/null` over 0, 1 and 2: a stray `println!` goes
//! nowhere, a read of stdin finds it ended, and a program the app starts
//! inherits none of the interfaces. A panic is said on the error interface as
//! one packet, by a panic hook `inherited` installs. Anything else an app has
//! to say goes on [`Errors::say`].

use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use crate::{is_handed_over, is_open, packet, Interface};

/// Payload data in, gateway data out: the interface at descriptor 0.
#[derive(Debug)]
pub struct PayloadInterface(UnixStream);

impl PayloadInterface {
    /// Read the next packet of payload data, or `None` once the interface has
    /// ended.
    pub fn read_payload(&mut self) -> io::Result<Option<Vec<u8>>> {
        packet::read(&mut self.0)
    }

    /// Write a packet of gateway data.
    pub fn write_gateway(&mut self, data: &[u8]) -> io::Result<()> {
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

/// Gateway data in, payload data out: the interface at descriptor 1.
#[derive(Debug)]
pub struct GatewayInterface(UnixStream);

impl GatewayInterface {
    /// Read the next packet of gateway data, or `None` once the interface has
    /// ended.
    pub fn read_gateway(&mut self) -> io::Result<Option<Vec<u8>>> {
        packet::read(&mut self.0)
    }

    /// Write a packet of payload data.
    pub fn write_payload(&mut self, data: &[u8]) -> io::Result<()> {
        packet::write(&mut self.0, data)
    }

    /// Another handle on the same interface. See
    /// [`PayloadInterface::try_clone`].
    pub fn try_clone(&self) -> io::Result<GatewayInterface> {
        Ok(GatewayInterface(self.0.try_clone()?))
    }
}

/// Payload data out, and nothing in: the interface at descriptor 1, held one
/// way about.
///
/// What an `app_pl` has. The interface is the gateway one, that being the
/// interface payload data is written on, but a payload that is never asked has
/// nothing to read there: nothing is linked into such a node, so nothing is
/// ever written to it, and a program waiting on the read would wait for ever.
///
/// Which is why it is a type of its own rather than a [`GatewayInterface`] used
/// by halves. The rule this module exists for is that an app cannot reach for
/// what it has not got, and "has not got" is as true of a direction as it is
/// of an interface: `read_gateway` is not on this, so an `app_pl` that waits to
/// be asked does not compile.
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

/// An app that reads payload data and writes gateway data.
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
    /// Takes the interfaces and the arguments this program was run with.
    ///
    /// Called once, at the start of the program: the descriptors are owned
    /// from here on.
    ///
    /// # Returns
    ///
    /// The app, holding its interfaces and arguments.
    ///
    /// # Errors
    ///
    /// A sentence saying what is wrong unless the program was run with
    /// the payload interface and the errors handed over, and the node's name
    /// and its app-payload-arg, and nothing else.
    pub fn inherited() -> Result<AppPayloadVeh, String> {
        taking(&[Interface::Payload, Interface::Errors])?;
        let [name, payload_arg] = told(["the node's name", "app-payload-arg"])?;
        let payload = PayloadInterface(stream(Interface::Payload)?);
        let errors = Errors(file(Interface::Errors)?);
        quiet(&name, &errors)?;
        Ok(AppPayloadVeh {
            name,
            payload_arg,
            payload,
            errors,
        })
    }
}

/// An app that reads gateway data and writes payload data.
///
/// The node type `app_gateway_veh`. Descriptor 0 is not open.
#[derive(Debug)]
pub struct AppGatewayVeh {
    /// What the node this program is runs under in the configuration.
    pub name: String,
    /// What the configuration says about the gateway stream, this kind handling
    /// gateway data.
    pub gateway_arg: String,
    pub gateway: GatewayInterface,
    pub errors: Errors,
}

impl AppGatewayVeh {
    /// Takes the interfaces and the arguments this program was run with.
    ///
    /// Called once, at the start of the program: the descriptors are owned
    /// from here on.
    ///
    /// # Returns
    ///
    /// The app, holding its interfaces and arguments.
    ///
    /// # Errors
    ///
    /// A sentence saying what is wrong unless the program was run with
    /// the gateway interface and the errors handed over, and the node's name
    /// and its app-gateway-arg, and nothing else.
    pub fn inherited() -> Result<AppGatewayVeh, String> {
        taking(&[Interface::Gateway, Interface::Errors])?;
        let [name, gateway_arg] = told(["the node's name", "app-gateway-arg"])?;
        let gateway = GatewayInterface(stream(Interface::Gateway)?);
        let errors = Errors(file(Interface::Errors)?);
        quiet(&name, &errors)?;
        Ok(AppGatewayVeh {
            name,
            gateway_arg,
            gateway,
            errors,
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
    /// And about the gateway stream. It comes second on the command line, this
    /// kind handling both.
    pub gateway_arg: String,
    pub payload: PayloadInterface,
    pub gateway: GatewayInterface,
    pub errors: Errors,
}

impl AppAllVeh {
    /// Takes the interfaces and the arguments this program was run with.
    ///
    /// Called once, at the start of the program: the descriptors are owned
    /// from here on.
    ///
    /// # Returns
    ///
    /// The app, holding its interfaces and arguments.
    ///
    /// # Errors
    ///
    /// A sentence saying what is wrong unless the program was run with
    /// both data interfaces and the errors handed over, and the node's
    /// name, its app-payload-arg and its app-gateway-arg, and nothing else.
    pub fn inherited() -> Result<AppAllVeh, String> {
        taking(&[Interface::Payload, Interface::Gateway, Interface::Errors])?;
        let [name, payload_arg, gateway_arg] =
            told(["the node's name", "app-payload-arg", "app-gateway-arg"])?;
        let payload = PayloadInterface(stream(Interface::Payload)?);
        let gateway = GatewayInterface(stream(Interface::Gateway)?);
        let errors = Errors(file(Interface::Errors)?);
        quiet(&name, &errors)?;
        Ok(AppAllVeh {
            name,
            payload_arg,
            gateway_arg,
            payload,
            gateway,
            errors,
        })
    }
}

/// An app that is a payload nothing asks: it writes payload data and reads
/// nothing.
///
/// The node type `app_pl`. Descriptor 0 is not open, as for an
/// `app_gateway_veh`, and descriptor 1 is held as a [`PayloadOut`]: a payload
/// that reports of its own accord has no question to wait for, and the type
/// says so by having nothing to read with.
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
    /// Takes the interfaces and the arguments this program was run with.
    ///
    /// Called once, at the start of the program: the descriptors are owned
    /// from here on.
    ///
    /// # Returns
    ///
    /// The app, holding its interfaces and arguments.
    ///
    /// # Errors
    ///
    /// A sentence saying what is wrong unless the program was run with
    /// descriptor 1 and the errors handed over, and the node's name and
    /// its app-payload-arg, and nothing else.
    pub fn inherited() -> Result<AppPl, String> {
        taking(&[Interface::Gateway, Interface::Errors])?;
        let [name, payload_arg] = told(["the node's name", "app-payload-arg"])?;
        let payload = PayloadOut(stream(Interface::Gateway)?);
        let errors = Errors(file(Interface::Errors)?);
        quiet(&name, &errors)?;
        Ok(AppPl {
            name,
            payload_arg,
            payload,
            errors,
        })
    }
}

/// An app that is a payload and is asked: gateway data in, payload data out.
///
/// The node type `app_all_pl`, and the same interface an `app_gateway_veh` is
/// run with -- what tells the two apart is which side of the graph the node
/// stands on, not what its program holds. A payload is asked for a reading and
/// answers with one, so this is the kind that stands in for a payload as every
/// other payload behaves: see `tcslibgs::node::app`.
#[derive(Debug)]
pub struct AppAllPl {
    /// What the node this program is runs under in the configuration.
    pub name: String,
    /// What the configuration says about the payload stream: the readings this
    /// payload answers with.
    pub payload_arg: String,
    /// And about the gateway stream: what asks it for them. Second on the
    /// command line, this kind handling both.
    pub gateway_arg: String,
    pub gateway: GatewayInterface,
    pub errors: Errors,
}

impl AppAllPl {
    /// Takes the interfaces and the arguments this program was run with.
    ///
    /// Called once, at the start of the program: the descriptors are owned
    /// from here on.
    ///
    /// # Returns
    ///
    /// The app, holding its interfaces and arguments.
    ///
    /// # Errors
    ///
    /// A sentence saying what is wrong unless the program was run with
    /// descriptor 1 and the errors handed over, and the node's name, its
    /// app-payload-arg and its app-gateway-arg, and nothing else.
    pub fn inherited() -> Result<AppAllPl, String> {
        taking(&[Interface::Gateway, Interface::Errors])?;
        let [name, payload_arg, gateway_arg] =
            told(["the node's name", "app-payload-arg", "app-gateway-arg"])?;
        let gateway = GatewayInterface(stream(Interface::Gateway)?);
        let errors = Errors(file(Interface::Errors)?);
        quiet(&name, &errors)?;
        Ok(AppAllPl {
            name,
            payload_arg,
            gateway_arg,
            gateway,
            errors,
        })
    }
}

/// Reads this program's command line, for a kind that expects these
/// arguments.
///
/// Refused and not made good: a program run with the wrong number of arguments
/// has been started by something that has misunderstood which kind of app it
/// is, which is the mistake `taking` catches for the interfaces. Reading on
/// would mean taking one stream's argument for the other's.
///
/// # Arguments
///
/// * `expects` - The configuration's own word for each argument, in order,
///               which is what a wrong count is refused with: a program
///               told "it is run with the node's name and app-payload-arg"
///               can be looked up in the file that started it.
///
/// # Returns
///
/// The arguments, the program's own name left out.
///
/// # Errors
///
/// A sentence saying so if an argument is not text or there are not exactly
/// `N` of them.
fn told<const N: usize>(expects: [&str; N]) -> Result<[String; N], String> {
    let mut said: Vec<String> = Vec::with_capacity(N);
    for (at, argument) in std::env::args_os().enumerate().skip(1) {
        said.push(argument.into_string().map_err(|_| {
            format!(
                "argument {at} is not text: every argument an app is given is \
                 a word from its configuration, and a configuration is text"
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

/// Says what went wrong when there is no app to say it with.
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
/// bytes that are not coming. Once `inherited` has moved the interface off
/// descriptor 2, it is said where the interface went.
///
/// # Arguments
///
/// * `message` - What went wrong, written as one packet.
pub fn complain(message: &str) {
    if let Some(errors) = MOVED.get() {
        let _ = packet::write(&mut &*errors, message.as_bytes());
        return;
    }
    if !is_open(Interface::Errors.fd()) {
        return;
    }
    // Held rather than owned: this does not close the interface, which an app
    // that goes on to use it would need and a `File` would close as it went
    // out of scope.
    //
    // SAFETY: from_raw_fd needs an open descriptor, which descriptor 2 was
    // just found to be. Nothing owns it, the interfaces not having been
    // taken (MOVED is set when they are), and the ManuallyDrop means this
    // File never closes it either.
    let mut errors = std::mem::ManuallyDrop::new(unsafe {
        File::from_raw_fd(Interface::Errors.fd())
    });
    let _ = packet::write(&mut *errors, message.as_bytes());
}

/// The error interface where `inherited` moved it, for [`complain`] and for
/// a panic.
static MOVED: OnceLock<File> = OnceLock::new();

/// Whether the interfaces have been taken: see [`taking`].
static TAKEN: AtomicBool = AtomicBool::new(false);

/// Whether this program was run the way this kind of app is run, and if it
/// was, the interfaces are this call's to take.
///
/// Once, for the life of the program: each interface's descriptor is owned by
/// the handle `inherited` makes of it, and a second `inherited` would make a
/// second owner of each, which would close it twice. So the second call is
/// refused, rather than relying on every program to call it once.
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
///
/// # Arguments
///
/// * `supported` - The data interfaces this kind supports.
///
/// # Errors
///
/// A sentence naming the interface and the descriptor if a supported data
/// interface was not handed over, an unsupported one was, or the error
/// interface is not open.
fn taking(supported: &[Interface]) -> Result<(), String> {
    for interface in [Interface::Payload, Interface::Gateway] {
        let wanted = supported.contains(&interface);
        match (wanted, is_handed_over(interface.fd())) {
            (true, false) => {
                return Err(format!(
                    "{} was not handed over, and this app is one on which {}: \
                     it is run by tcspecial, which opens descriptor {} for it \
                     as one end of a socket pair",
                    interface.name(),
                    interface.carries(),
                    interface.fd()
                ))
            }
            (false, true) => {
                return Err(format!(
                    "{} was handed over at descriptor {}, and this app does \
                     not support it: a program run as a kind of app it is not \
                     would write the vehicle's data wherever that descriptor \
                     goes",
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
    take_once(&TAKEN)
}

/// Note that the interfaces are taken, unless they have been already.
///
/// # Arguments
///
/// * `taken` - Whether they have been: [`TAKEN`], or a test's own.
///
/// # Errors
///
/// Why not, if they have been taken already.
fn take_once(taken: &AtomicBool) -> Result<(), String> {
    if taken.swap(true, Ordering::SeqCst) {
        return Err("the interfaces have been taken already: an app takes \
                    them once, with the one inherited it is"
            .to_string());
    }
    Ok(())
}

/// One of the two interfaces that is read and written, which is a socket.
///
/// Called only from an `inherited`, once [`taking`] has said the interfaces
/// are its to take, which it says once: the interface is moved to a
/// descriptor of its own (see [`moved`]), owned from here on and closed when
/// the handle is dropped.
///
/// # Arguments
///
/// * `interface` - The interface.
///
/// # Returns
///
/// The handle.
///
/// # Errors
///
/// Why not, if it could not be moved.
fn stream(interface: Interface) -> Result<UnixStream, String> {
    let fd = moved(interface)?;
    // SAFETY: from_raw_fd needs an open descriptor that nothing else owns,
    // and `moved` has just made this one: nothing else has its number.
    Ok(unsafe { UnixStream::from_raw_fd(fd) })
}

/// And the one that is only written, which need be no more than a file.
///
/// Called as [`stream`] is.
///
/// # Arguments
///
/// * `interface` - The interface.
///
/// # Returns
///
/// The handle.
///
/// # Errors
///
/// Why not, if it could not be moved.
fn file(interface: Interface) -> Result<File, String> {
    let fd = moved(interface)?;
    // SAFETY: as for `stream`.
    Ok(unsafe { File::from_raw_fd(fd) })
}

/// Gives an interface a descriptor of its own, the lowest above 2, closed on
/// exec, so that it is not std's stdin, stdout or stderr as well and a
/// program the app starts does not inherit it. Its standard descriptor is
/// left as it was, for [`quiet`] to cover.
///
/// # Arguments
///
/// * `interface` - The interface.
///
/// # Returns
///
/// The new descriptor.
///
/// # Errors
///
/// Why not, if the system would not make one.
fn moved(interface: Interface) -> Result<RawFd, String> {
    // SAFETY: F_DUPFD_CLOEXEC makes a new descriptor from an open one and
    // reads nothing through a pointer; `taking` has checked this one is
    // open, and one that is not would fail with EBADF.
    let fd = unsafe { libc::fcntl(interface.fd(), libc::F_DUPFD_CLOEXEC, 3) };
    if fd == -1 {
        return Err(format!(
            "{} could not be moved off descriptor {}: {}",
            interface.name(),
            interface.fd(),
            io::Error::last_os_error()
        ));
    }
    Ok(fd)
}

/// Takes the standard streams off the interfaces, now that each has a
/// descriptor of its own: `/dev/null` over descriptors 0, 1 and 2, and a
/// panic hook that says a panic on the error interface. See this module's
/// documentation.
///
/// # Arguments
///
/// * `name`   - The node's name, which a panic is said with.
/// * `errors` - The error interface.
///
/// # Errors
///
/// Why not, if the error interface could not be held again or `/dev/null`
/// could not be put in place.
fn quiet(name: &str, errors: &Errors) -> Result<(), String> {
    let held = errors.0.try_clone().map_err(|e| {
        format!("the error interface could not be held for panics: {e}")
    })?;
    // Set once, `taking` refusing a second taking.
    let _ = MOVED.set(held);
    let name = name.to_string();
    std::panic::set_hook(Box::new(move |info| {
        complain(&format!("{name}: {info}"));
    }));
    let null = File::options()
        .read(true)
        .write(true)
        .open("/dev/null")
        .map_err(|e| format!("/dev/null could not be opened: {e}"))?;
    for fd in 0..=2 {
        // SAFETY: dup2 puts a copy of an open descriptor at a number, closing
        // what was there, and reads nothing through a pointer. `null` is
        // open, and what is closed at 0, 1 and 2 nothing owns: the
        // interfaces' handles hold the descriptors `moved` made, and std's
        // standard streams name these numbers without owning them -- which
        // is the point, as they now reach /dev/null.
        if unsafe { libc::dup2(null.as_raw_fd(), fd) } == -1 {
            return Err(format!(
                "/dev/null could not be put at descriptor {fd}: {}",
                io::Error::last_os_error()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The interfaces are taken once, and a second taking is refused rather
    /// than making a second owner of each descriptor.
    #[test]
    fn the_interfaces_are_taken_once() {
        let taken = AtomicBool::new(false);
        assert_eq!(take_once(&taken), Ok(()));
        let why = take_once(&taken).expect_err("a second taking");
        assert!(why.contains("taken already"), "{why}");
    }
}
