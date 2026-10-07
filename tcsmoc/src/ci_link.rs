//! The MOC's link to a command interpreter, which may be up or down.
//!
//! Every control in the window that sends a command works through one of
//! these. It exists because the link is no longer something the MOC opens
//! once and keeps: Disconnect takes it down, Connect brings it back up, and
//! possibly to a different address than before. A [`TcsClient`] cannot
//! express that by itself -- it owns its connection from the moment it is
//! built -- so what the handlers share is this, and they ask it for a client
//! each time rather than holding one.
//!
//! That is the whole reason for the indirection. The poll thread, the menu,
//! Query All, and every panel's Start and Stop button each kept a clone of
//! one `Arc<Mutex<TcsClient>>`, so a disconnect had nowhere to put the fact
//! that there was no longer a connection: taking the client away from one
//! holder would have left the others talking to a socket that was closed.
//!
//! The panel poller is the one holder that does not share the window's link.
//! It has a link of its own and keeps it wherever that one is, by
//! [`CiLink::follow`]: its commands wait on the spacecraft for as long as the
//! timeout allows, and a window whose buttons waited behind that is a window
//! that does nothing at the moment the operator is trying to disconnect.
//! Following keeps one account of what the operator asked for -- the link the
//! buttons control -- without the poller having to hold it.

use std::time::Duration;

use tcslib::{TcsClient, UdpConnection};
use tcslibgs::TcsResult;

/// The local address the MOC sends from: any interface, any port.
///
/// The MOC does not care which port it sends from, only which one it sends
/// to, and the command interpreter answers whoever asked.
const LOCAL_BIND_ANY: &str = "0.0.0.0:0";

/// What a control says when it is pressed with the link down.
///
/// One wording for all of them, because the reason is the same whichever
/// control it was and the remedy is always the same button.
pub const NOT_CONNECTED: &str = "Not connected - press Connect first";

/// A link to a command interpreter at a particular address.
///
/// The address is remembered whether the link is up or down, so what the
/// window shows after a disconnect is still where it would reconnect to.
pub struct CiLink {
    address: String,
    client: Option<TcsClient>,
}

impl CiLink {
    /// A link that is down and has not been anywhere yet.
    pub fn down() -> Self {
        Self {
            address: String::new(),
            client: None,
        }
    }

    /// Where this link goes, up or down.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Whether there is a connection to send on.
    pub fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    /// The client to send with, or `None` if the link is down.
    ///
    /// Asked for one command at a time rather than kept, so that a caller
    /// cannot go on using a client the link no longer has.
    pub fn client(&mut self) -> Option<&mut TcsClient> {
        self.client.as_mut()
    }

    /// Where this link is up, or `None` while it is down.
    ///
    /// This is what a link that follows another one is given: a follower has
    /// to know both whether to talk at all and where to talk, and only the
    /// two together say anything useful.
    pub fn connected_to(&self) -> Option<&str> {
        self.client.as_ref().map(|_| self.address.as_str())
    }

    /// Bring this link into step with `wanted`, another link's
    /// [`connected_to`](Self::connected_to).
    ///
    /// Usually nothing happens, this link already being up at that address.
    /// It is how the panel poller keeps a connection of its own without
    /// keeping a second account of what the operator asked for: it reads the
    /// link the buttons control and follows it.
    ///
    /// A failure to open is not reported. The address came from a link that
    /// did open it, so a failure here is a passing one, and the follower is
    /// back on its next pass; until then it is down, which is what it would
    /// have been anyway.
    pub fn follow(&mut self, wanted: Option<&str>) {
        match wanted {
            None => self.disconnect(),
            Some(address) => {
                if self.connected_to() != Some(address) {
                    let _ = self.connect(address);
                }
            }
        }
    }

    /// Bring the link up at `address`, replacing whatever it had.
    ///
    /// Any existing connection is closed first, so pressing Connect with a
    /// new address in the box moves the link rather than leaving two. A
    /// failure to open leaves the link down and the new address remembered:
    /// the address is what was asked for, and showing the old one would
    /// invite a second press that looks like it should work.
    pub fn connect(&mut self, address: &str) -> TcsResult<()> {
        self.disconnect();
        self.address = address.to_string();

        let connection = UdpConnection::new(LOCAL_BIND_ANY, address)?;
        self.client = Some(TcsClient::new(Box::new(connection)));
        Ok(())
    }

    /// Take the link down.
    ///
    /// The close error is not returned, and deliberately: this is what
    /// Disconnect does, and a link whose close complained is down either way.
    /// There is nothing the MOC or the operator would do differently about
    /// it, and reporting it would put a failure in front of someone whose
    /// request succeeded.
    pub fn disconnect(&mut self) {
        if let Some(mut client) = self.client.take() {
            let _ = client.close();
        }
    }

    /// How long a command on this link waits for its answer.
    ///
    /// Does nothing while the link is down, and a link brought up later
    /// starts from the client's own default, so a caller that wants a
    /// particular timeout says so each time rather than once.
    pub fn set_timeout(&mut self, timeout: Duration) {
        if let Some(client) = self.client.as_mut() {
            client.set_timeout(timeout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A link that is down has no client to hand out, and one that is up has.
    ///
    /// No command interpreter is needed for any of this: these are UDP
    /// sockets, which bind locally and are told where to send without anyone
    /// having to be there.
    #[test]
    fn a_link_hands_out_a_client_only_while_it_is_up() {
        let mut link = CiLink::down();
        assert!(!link.is_connected());
        assert!(link.client().is_none());
        assert_eq!(link.connected_to(), None);

        link.connect("127.0.0.1:4000").unwrap();
        assert!(link.is_connected());
        assert!(link.client().is_some());
        assert_eq!(link.connected_to(), Some("127.0.0.1:4000"));

        link.disconnect();
        assert!(!link.is_connected());
        assert!(link.client().is_none());
    }

    /// Disconnect and connect again, which is the sequence the two buttons
    /// make, leaves a link that works.
    #[test]
    fn a_link_can_be_brought_back_up() {
        let mut link = CiLink::down();
        link.connect("127.0.0.1:4000").unwrap();
        link.disconnect();
        link.connect("127.0.0.1:4000").unwrap();
        assert!(link.is_connected());
    }

    /// Connecting somewhere else moves the link rather than adding one.
    #[test]
    fn connecting_elsewhere_moves_the_link() {
        let mut link = CiLink::down();
        link.connect("127.0.0.1:4000").unwrap();

        link.connect("127.0.0.1:4001").unwrap();
        assert!(link.is_connected());
        assert_eq!(link.address(), "127.0.0.1:4001");
    }

    /// An address that cannot be used leaves the link down, and remembering
    /// the address that was asked for.
    #[test]
    fn an_address_that_cannot_be_used_leaves_the_link_down() {
        let mut link = CiLink::down();
        link.connect("127.0.0.1:4000").unwrap();

        assert!(link.connect("this is not an address").is_err());
        assert!(!link.is_connected());
        assert_eq!(link.address(), "this is not an address");
    }

    /// A follower ends up where the link it follows is, whichever way that
    /// link moved: up, elsewhere, or down.
    #[test]
    fn a_follower_goes_where_the_link_it_follows_is() {
        let mut follower = CiLink::down();

        follower.follow(Some("127.0.0.1:4000"));
        assert_eq!(follower.connected_to(), Some("127.0.0.1:4000"));

        follower.follow(Some("127.0.0.1:4001"));
        assert_eq!(follower.connected_to(), Some("127.0.0.1:4001"));

        // Following the same address again leaves it where it is.
        follower.follow(Some("127.0.0.1:4001"));
        assert_eq!(follower.connected_to(), Some("127.0.0.1:4001"));

        follower.follow(None);
        assert_eq!(follower.connected_to(), None);
    }

    /// A follower told to go somewhere it cannot is down rather than left
    /// where it was, so nothing is polled from an address nobody asked for.
    #[test]
    fn a_follower_that_cannot_go_there_is_down() {
        let mut follower = CiLink::down();
        follower.follow(Some("127.0.0.1:4000"));

        follower.follow(Some("this is not an address"));
        assert_eq!(follower.connected_to(), None);
    }

    /// Disconnecting a link that is already down is not an error, since the
    /// button can be pressed twice.
    #[test]
    fn disconnecting_twice_is_harmless() {
        let mut link = CiLink::down();
        link.connect("127.0.0.1:4000").unwrap();
        link.disconnect();
        link.disconnect();
        assert!(!link.is_connected());
    }
}
