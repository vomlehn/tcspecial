//! The file interfaces an app node's program is run with.
//!
//! An app node is a node whose work is a program, run by tcspecial, that takes
//! part in the graph: data reaches it, it answers, and what it answers goes on
//! to wherever the configuration says. What a built-in node cannot be -- a
//! part of the arrangement written in something else, or changed without
//! rebuilding the flight software. Most are the vehicle's own; two kinds are
//! payloads, a payload that is a program rather than a device on a bus.
//!
//! This crate is the one statement of how such a program is talked to. Both
//! ends of it are here: [`app`], which is what the program itself holds, and
//! [`parent`], which is what starts the program and holds the other end of
//! each interface. One crate for both because a protocol written down twice is
//! a protocol with two meanings, and the end that is wrong is whichever one
//! was not changed.
//!
//! # The three interfaces
//!
//! Three file interfaces: things that are read and written the way a file is,
//! which is what a program in any language already has a library for. On a
//! UNIX-like system they are descriptors 0, 1 and 2, which the program
//! inherits, so a program that can read its standard input can take part.
//!
//! | Interface | Descriptor | The program reads | The program writes |
//! |---|---|---|---|
//! | [`Interface::Payload`] | 0 | payload data | MOC data |
//! | [`Interface::Moc`] | 1 | MOC data | payload data |
//! | [`Interface::Errors`] | 2 | -- | what went wrong |
//!
//! Each of the first two is read *and* written, which is why each is one
//! interface and not two: an app that is handed payload data answers with MOC
//! data on the same interface, and the pairing is the point. A descriptor like
//! that is one end of a socket pair rather than a pipe -- see [`parent`].
//!
//! Payload data is what travels groundward, from what the vehicle carries
//! towards the ground; MOC data is what travels vehicleward, from the ground
//! towards what it carries. The names are the graph's own: see
//! `tcslibgs::graph::terminals`.
//!
//! # What an app supports, and what it does not
//!
//! Five kinds of app, and the kind is the node type a configuration names.
//! Three of them are nodes of the vehicle's own:
//!
//! * [`app::AppPayloadVeh`] reads payload data and writes MOC data. Descriptor
//!   1 is not open at all.
//! * [`app::AppMocVeh`] reads MOC data and writes payload data. Descriptor 0
//!   is not open at all.
//! * [`app::AppAllVeh`] does both.
//!
//! And two are payloads, which send readings and are sent what asks for them.
//! Payload data is what a payload *writes*, and the interface payload data is
//! written on is the MOC one -- so both of these are run with descriptor 1 and
//! neither has descriptor 0 open:
//!
//! * [`app::AppPl`] writes payload data and reads nothing: a payload that is
//!   never asked, and reports of its own accord. Its handle has no read on it.
//! * [`app::AppAllPl`] reads MOC data and writes payload data, which is the
//!   shape every other payload has: asked for a reading, and answering with
//!   one.
//!
//! Every one of the five has the error interface.
//!
//! An interface an app does not support is not opened for it: [`parent`]
//! closes the descriptor in the child before the program starts, so for any
//! program that leaves it closed, reaching for it is `EBADF`.
//!
//! A Rust program is not such a program, and it is worth knowing why. The
//! standard library opens `/dev/null` over any of descriptors 0, 1 and 2 that
//! it finds closed, before `main` runs -- which it does for a good reason, that
//! reason being this very hazard: a closed standard descriptor is a number the
//! next `open` will hand out, and then a write meant for the terminal goes
//! into somebody's file. So what a Rust app finds at an interface it does not
//! support is a descriptor that swallows what is written and reads as having
//! ended: no error, but no data either, which is the second of the two
//! outcomes worth having.
//!
//! Which is why the real refusal is the one that does not need a descriptor at
//! all: an app holds a type with nothing on it for the interface it has not
//! got, so reaching for it does not compile. The descriptor is the second line
//! of defence and [`is_handed_over`] is the third -- an app says at once if
//! what it was given is not what its kind is run with, rather than reading a
//! terminal and waiting.
//!
//! # What an app is told, other than data
//!
//! A short, fixed list of arguments: the name of the node the program is, and
//! then the argument the configuration states for each stream the kind handles
//! -- `app-payload-arg` before `app-moc-arg`. So an `app_all_veh` named
//! `through` is run as `program through <payload> <moc>`, an `app_moc_veh` as
//! `program back <moc>`, and an `app_pl` as `program bay <payload>`.
//!
//! The order is fixed by the kind and not by what a file happened to state,
//! which is why both attributes are required of the kinds that take them:
//! a list a program has to count before it can read is a list it would count
//! wrongly on the day somebody changed a node's type. Each kind's `inherited`
//! takes its own arguments and refuses a wrong number.
//!
//! Nothing else comes that way. The data is what arrives on the interfaces,
//! and a command line that carried readings would be a node whose work was
//! neither in the configuration nor in the program.
//!
//! # Packets
//!
//! Every interface carries packets rather than a stream of bytes, and
//! [`packet`] is the whole of what one is: four bytes of length, then that
//! many bytes. A zero-length packet is a packet.

pub mod app;
pub mod packet;
pub mod parent;

use std::os::fd::RawFd;

/// One of the three file interfaces an app is run with.
///
/// Named by what it carries rather than by its number, because the number is
/// a property of this kind of system and what it carries is a property of the
/// node: a reader of an app's code should not have to remember which of three
/// descriptors is the errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interface {
    /// Payload data in, MOC data out.
    Payload,
    /// MOC data in, payload data out.
    Moc,
    /// What went wrong, out.
    Errors,
}

impl Interface {
    /// The descriptor this interface is, on a UNIX-like system.
    pub fn fd(self) -> RawFd {
        match self {
            Interface::Payload => 0,
            Interface::Moc => 1,
            Interface::Errors => 2,
        }
    }

    /// What it is called, for a sentence about it.
    pub fn name(self) -> &'static str {
        match self {
            Interface::Payload => "the payload interface",
            Interface::Moc => "the MOC interface",
            Interface::Errors => "the error interface",
        }
    }

    /// What an app does with it, for a sentence about one that cannot.
    pub fn carries(self) -> &'static str {
        match self {
            Interface::Payload => "payload data is read and MOC data written",
            Interface::Moc => "MOC data is read and payload data written",
            Interface::Errors => "error messages are written",
        }
    }
}

/// Whether a descriptor is open at all.
pub fn is_open(fd: RawFd) -> bool {
    // F_GETFD asks for the descriptor's flags, which is the cheapest question
    // that has to be answered by the descriptor existing.
    //
    // SAFETY: F_GETFD only reads the descriptor table, so any number may be
    // asked about: one that is not open is answered with -1 and EBADF, and
    // nothing is changed or read through a pointer.
    unsafe { libc::fcntl(fd, libc::F_GETFD) != -1 }
}

/// Whether what is at this descriptor is an interface that was handed over.
///
/// A socket, which is what [`parent`] makes each of the two data interfaces
/// out of and what each of them has to be: an interface that is read *and*
/// written cannot be a pipe. So this is the question an app asks of the
/// descriptors it was started with, and "is it open" is not -- a Rust
/// program's own standard library will have opened `/dev/null` over anything
/// it found closed, and `/dev/null` answers "open" while carrying nothing.
///
/// What it catches: an app run as a kind of app it is not, which finds a
/// socket where its kind has none; and an app run by hand or by something that
/// attached pipes, which finds no socket where it needs one. Both are said at
/// once rather than discovered as a read that never answers.
pub fn is_handed_over(fd: RawFd) -> bool {
    let mut about = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fstat needs a stat it can write, which `about` is, alive for
    // the call; any descriptor may be asked about, and one that is not open
    // fails with EBADF. assume_init needs the stat written, which it is once
    // fstat has not failed -- and a failure returns before it is reached.
    unsafe {
        if libc::fstat(fd, about.as_mut_ptr()) == -1 {
            return false;
        }
        (about.assume_init().st_mode & libc::S_IFMT) == libc::S_IFSOCK
    }
}
