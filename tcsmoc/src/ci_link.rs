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

use tcslib::{TcsClient, UdpConnection};
use tcslibgs::{TcsError, TcsResult, Timestamp};

/// The local address the MOC sends from where nothing says otherwise: any
/// interface, any port.
///
/// What the payload link uses, and what a link that follows another one uses
/// for the spacecraft's own: the command interpreter answers whoever asked, so
/// a port the system chose works for a socket nobody has to find.
///
/// The window's own spacecraft link does not use this. It binds
/// `tcs_cmd.send`, which the configuration states and tcspecial checks every
/// command against -- see [`CiLink::down_from`].
const LOCAL_BIND_ANY: &str = "0.0.0.0:0";

/// The payload command link beside a spacecraft command link.
///
/// The same host, a different port: the two links are two sockets of one
/// process, so an operator who moves the link moves both by typing one
/// address. The port is the configuration's, not the operator's, for the
/// same reason the beacon's is -- it is what the other end binds.
fn payload_link_at(address: &str, payload_port: u16) -> TcsResult<String> {
    let host = address.rsplit_once(':').map(|(host, _)| host).ok_or_else(|| {
        TcsError::Config(format!(
            "\"{address}\" is not a host and a port, so there is no host to reach \
             the payload link at"
        ))
    })?;

    Ok(format!("{host}:{payload_port}"))
}

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
    /// What the spacecraft link binds locally: `tcs_cmd.send` for the
    /// window's link, and any port at all for one that follows it.
    ///
    /// Stated because the other end checks it. tcspecial takes commands only
    /// from where its configuration says the MOC sends them, so the socket
    /// this sends from is a socket the file names -- which is also what lets
    /// a firewall rule or a capture filter describe the link beforehand.
    from: String,
    /// The port payload commands go to, which the spacecraft takes them on.
    ///
    /// Held rather than passed to [`CiLink::connect`], because a link that
    /// follows another one is given an address and nothing else: see
    /// [`CiLink::follow`]. One number for the life of the link, as the
    /// configuration states it.
    payload_port: u16,
    client: Option<TcsClient>,
}

impl CiLink {
    /// A link that is down and will send spacecraft commands from `from`.
    ///
    /// `tcs_cmd.send`, which tcspecial refuses a command from anywhere else
    /// than -- both ends reading it out of the same file. One socket, so one
    /// link: the MOC has one sender of spacecraft commands now that the panel
    /// poller is gone, and `LOCAL_BIND_ANY` is what a test that does not care
    /// passes.
    pub fn down_from(from: &str, payload_port: u16) -> Self {
        Self {
            address: String::new(),
            from: from.to_string(),
            payload_port,
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

    /// Bring the link up at `address`, replacing whatever it had.
    ///
    /// Any existing connection is closed first, so pressing Connect with a
    /// new address in the box moves the link rather than leaving two. A
    /// failure to open leaves the link down and the new address remembered:
    /// the address is what was asked for, and showing the old one would
    /// invite a second press that looks like it should work.
    /// Both links are opened, or neither: a client with one of them could
    /// send only half of what there is to send, and the half it could not
    /// send is the half the panels use.
    pub fn connect(&mut self, address: &str) -> TcsResult<()> {
        self.disconnect();
        self.address = address.to_string();

        let payload_address = payload_link_at(address, self.payload_port)?;
        let connection = UdpConnection::new(&self.from, address)?;
        let payload = UdpConnection::new(LOCAL_BIND_ANY, &payload_address)?;
        self.client = Some(TcsClient::new(Box::new(connection), Box::new(payload)));
        Ok(())
    }

    /// Where this link sends payload commands, up or down.
    pub fn payload_address(&self) -> TcsResult<String> {
        payload_link_at(&self.address, self.payload_port)
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

    /// When the last command went out and when its answer came back, if the
    /// link is up and has carried one.
    ///
    /// The client's own account of it: every command goes through one place
    /// there, so nothing here has to remember at each press. A link that is
    /// down has nothing to say -- the times belonged to a client that is
    /// closed, and the next Connect starts a fresh one.
    pub fn command_times(&self) -> Option<(Option<Timestamp>, Option<Timestamp>)> {
        self.client
            .as_ref()
            .map(|client| (client.last_sent(), client.last_received()))
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    /// A link says when it last sent a command and when the answer came.
    ///
    /// Which is what the window shows under Last Response. The times are the
    /// client's, so this is the chain from the socket to the thing the window
    /// reads: a link that is up and has carried a command has both, and one
    /// that is down has nothing to say.
    #[test]
    fn a_link_says_when_it_last_sent_and_received() {
        use std::net::UdpSocket;
        use tcslibgs::{Command, PingTelemetry, Telemetry};

        // A stub spacecraft that answers one PING on the command link. The
        // payload link is never sent anything by a ping.
        let spacecraft = UdpSocket::bind("127.0.0.1:0").expect("a stub socket");
        let at = spacecraft.local_addr().expect("where it is").to_string();
        let answering = std::thread::spawn(move || {
            let mut buffer = [0u8; 65535];
            let (size, from) = spacecraft.recv_from(&mut buffer).expect("a command");
            let command: Command =
                serde_json::from_slice(&buffer[..size]).expect("a valid command");
            let answer = Telemetry::Ping(PingTelemetry::new(
                command.sequence(),
                tcslibgs::CommandStatus::Success,
            ));
            let data = serde_json::to_vec(&answer).expect("it serializes");
            spacecraft.send_to(&data, from).expect("the answer is sent");
        });

        let mut link = CiLink::down_from(LOCAL_BIND_ANY, 4001);
        assert_eq!(link.command_times(), None, "a link that is down has no times");

        link.connect(&at).expect("the link comes up");
        assert_eq!(
            link.command_times(),
            Some((None, None)),
            "a fresh link has sent nothing"
        );

        link.client()
            .expect("the link is up")
            .ping()
            .expect("the stub answers");
        answering.join().expect("the stub finishes");

        let (sent, received) = link.command_times().expect("the link is up");
        let sent = sent.expect("the command was timed");
        let received = received.expect("so was the answer");
        assert!(received >= sent, "the answer came before the command went");

        // And a disconnect takes them with the client: the times belonged to
        // it, and the next Connect starts a fresh one.
        link.disconnect();
        assert_eq!(link.command_times(), None);
    }

    /// The payload link is the operator's host and the configuration's port.
    ///
    /// The operator types one address, and both links move with it: they are
    /// two sockets of one process. The port is not the operator's to type --
    /// it is what the other end binds -- so it comes from the configuration
    /// both ends read.
    #[test]
    fn the_payload_link_is_the_same_host_on_its_own_port() {
        assert_eq!(
            payload_link_at("127.0.0.1:4000", 4001).expect("a host and a port"),
            "127.0.0.1:4001"
        );

        // A host of any shape: a name, or an address of either family.
        assert_eq!(
            payload_link_at("spacecraft.example:4000", 4001).expect("a name"),
            "spacecraft.example:4001"
        );
        assert_eq!(
            payload_link_at("[::1]:4000", 4001).expect("the other family"),
            "[::1]:4001"
        );

        // And an address with no port at all is refused, saying why: there
        // is no host to put a port beside.
        let said = payload_link_at("127.0.0.1", 4001)
            .expect_err("a port is what tells the two links apart")
            .to_string();
        assert!(said.contains("127.0.0.1"), "{said}");
    }

    /// The port a test's payload commands would go to.
    ///
    /// Nothing answers on it in these tests: what they are about is the link
    /// the operator controls, and the payload link is opened beside it by the
    /// same call either way.
    const A_PAYLOAD_PORT: u16 = 4001;

    /// A link that is down has no client to hand out, and one that is up has.
    ///
    /// No command interpreter is needed for any of this: these are UDP
    /// sockets, which bind locally and are told where to send without anyone
    /// having to be there.
    #[test]
    fn a_link_hands_out_a_client_only_while_it_is_up() {
        let mut link = CiLink::down_from(LOCAL_BIND_ANY, A_PAYLOAD_PORT);
        assert!(!link.is_connected());
        assert!(link.client().is_none());

        link.connect("127.0.0.1:4000").unwrap();
        assert!(link.is_connected());
        assert!(link.client().is_some());
        assert_eq!(link.address(), "127.0.0.1:4000");

        link.disconnect();
        assert!(!link.is_connected());
        assert!(link.client().is_none());
    }

    /// Disconnect and connect again, which is the sequence the two buttons
    /// make, leaves a link that works.
    #[test]
    fn a_link_can_be_brought_back_up() {
        let mut link = CiLink::down_from(LOCAL_BIND_ANY, A_PAYLOAD_PORT);
        link.connect("127.0.0.1:4000").unwrap();
        link.disconnect();
        link.connect("127.0.0.1:4000").unwrap();
        assert!(link.is_connected());
    }

    /// Connecting somewhere else moves the link rather than adding one.
    #[test]
    fn connecting_elsewhere_moves_the_link() {
        let mut link = CiLink::down_from(LOCAL_BIND_ANY, A_PAYLOAD_PORT);
        link.connect("127.0.0.1:4000").unwrap();

        link.connect("127.0.0.1:4001").unwrap();
        assert!(link.is_connected());
        assert_eq!(link.address(), "127.0.0.1:4001");
    }

    /// An address that cannot be used leaves the link down, and remembering
    /// the address that was asked for.
    #[test]
    fn an_address_that_cannot_be_used_leaves_the_link_down() {
        let mut link = CiLink::down_from(LOCAL_BIND_ANY, A_PAYLOAD_PORT);
        link.connect("127.0.0.1:4000").unwrap();

        assert!(link.connect("this is not an address").is_err());
        assert!(!link.is_connected());
        assert_eq!(link.address(), "this is not an address");
    }

    /// Disconnecting a link that is already down is not an error, since the
    /// button can be pressed twice.
    #[test]
    fn disconnecting_twice_is_harmless() {
        let mut link = CiLink::down_from(LOCAL_BIND_ANY, A_PAYLOAD_PORT);
        link.connect("127.0.0.1:4000").unwrap();
        link.disconnect();
        link.disconnect();
        assert!(!link.is_connected());
    }
}
