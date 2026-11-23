//! What the tests of TCSpecial's crates share: ports for the programs they
//! start to bind.
//!
//! A test that starts tcspecial, or a ground station, writes a
//! configuration naming the ports it is to bind, and finds ports nothing
//! holds by binding port 0 and taking the one the kernel chose. The socket
//! is then let go, the port being for the program, and so it is free again:
//! the next port asked for can be the same one. A test that asks for a dozen
//! named two sockets one port about once in five hundred runs, and the
//! configuration was refused, or the second program could not bind. So
//! [`free`] remembers every port it has given in this process and never
//! gives one twice, and [`bound`] never gives a socket on one of them, so
//! that a socket a test holds does not take a port another test has been
//! given for its program.
//!
//! Within a process, that is, which is where the tests of a crate run, all
//! at once. Another program on the machine can still bind a port between
//! [`free`] giving it and the program it was for binding it: only that
//! program binding port 0 itself would rule that out.
//!
//! A dev-dependency only: it is a verification tool, and nothing that flies
//! is built from it.

use std::collections::BTreeSet;
use std::sync::{Mutex, MutexGuard};

use tcsos::net::UdpSocket;

/// Every port [`free`] has given in this process.
static GIVEN: Mutex<BTreeSet<u16>> = Mutex::new(BTreeSet::new());

/// The ports given so far: a test that panicked while holding them left them
/// as they were, which is still right.
///
/// # Returns
///
/// The set, locked.
fn given() -> MutexGuard<'static, BTreeSet<u16>> {
    GIVEN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The port a socket is bound to.
///
/// # Arguments
///
/// * `socket` - The socket.
///
/// # Returns
///
/// Its port.
///
/// # Panics
///
/// If the socket cannot say its address.
pub fn port(socket: &UdpSocket) -> u16 {
    socket.local_addr().expect("its address").port()
}

/// A UDP socket of a test's own on the loopback address, on a port nothing
/// holds and [`free`] has not given.
///
/// A port the kernel offers that [`free`] has given is held until one it
/// has not is found, so that it is not offered again meanwhile, and then let
/// go.
///
/// # Returns
///
/// The socket.
///
/// # Panics
///
/// If no port can be bound.
pub fn bound() -> UdpSocket {
    let mut passed_over = Vec::new();
    loop {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("a port of its own");
        if !given().contains(&port(&socket)) {
            return socket;
        }
        passed_over.push(socket);
    }
}

/// A port nothing holds and no test in this process has been given, for a
/// program a test starts to bind. Given once, never again in this process.
///
/// # Returns
///
/// The port.
///
/// # Panics
///
/// If no port can be bound.
pub fn free() -> u16 {
    let socket = bound();
    let port = port(&socket);
    given().insert(port);
    port
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No port is given twice, however many are asked for: far more than a
    /// test asks for, and enough that letting each go would have repeated
    /// one many times over.
    #[test]
    fn no_port_is_given_twice() {
        let ports: Vec<u16> = (0..3000).map(|_| free()).collect();
        let distinct: BTreeSet<u16> = ports.iter().copied().collect();
        assert_eq!(distinct.len(), ports.len());
    }

    /// A socket of a test's own is never on a port given for a program.
    #[test]
    fn a_socket_is_not_on_a_port_given() {
        let ports: BTreeSet<u16> = (0..500).map(|_| free()).collect();
        let held: Vec<UdpSocket> = (0..2000).map(|_| bound()).collect();
        assert!(held.iter().all(|socket| !ports.contains(&port(socket))));
    }
}
