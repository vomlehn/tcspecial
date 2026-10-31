//! A ground station that only listens, and the CONNECT it sends all the
//! same.
//!
//! A vehicle sends nothing down but its beacons and the answers to its
//! commands until it takes a CONNECT carrying the checksum of the
//! configuration it runs, so a station that only reads the vehicle's data
//! -- a gateway node's, through [`crate::gateway`] -- receives nothing unless
//! somebody has sent one. A station that cannot count on a commanding
//! station being there sends its own: [`Listening`] sends CONNECT with its
//! configuration's checksum on the spacecraft command link, and sends it
//! again each time no answer comes in time, until the vehicle answers with
//! that checksum. See "Configurations named by their checksum" in
//! `docs/design.rst`.
//!
//! It commands nothing else. A CONNECT changes nothing aboard but the
//! beacons, the downlink and which commands are taken, so a repeat is safe,
//! and one carrying the checksum of the configuration the vehicle runs is
//! as good from this station as from any other.

use std::time::Duration;

use tcslibgs::config_digest::{digest_of_graph, ConfigDigest, ConfigVersion};
use tcslibgs::graph::Graph;

use crate::client::{command_links_of, TcsClient};
use crate::matched::Matched;

/// What one CONNECT heard back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heard {
    /// The vehicle runs this station's configuration, and is matched: it
    /// sends its data down from now. Its build is said, for the station to
    /// show.
    Matched(ConfigVersion),
    /// The vehicle runs another configuration, whose checksum this is: it
    /// sends nothing this station could read, and nothing changed aboard.
    Other(ConfigDigest),
    /// No answer came in time, or the link failed, as said.
    Nothing(String),
}

impl Heard {
    /// Says what was heard, for a station to print.
    ///
    /// # Arguments
    ///
    /// * `ours` - The station's configuration's checksum.
    ///
    /// # Returns
    ///
    /// A sentence.
    pub fn said(&self, ours: ConfigDigest) -> String {
        match self {
            Heard::Matched(version) => format!(
                "matched: tcspecial v{version} answered CONNECT with this \
                 configuration's checksum, {ours}, and sends its data from now"
            ),
            Heard::Other(theirs) => format!(
                "tcspecial runs configuration {theirs}, not this one, {ours}: \
                 it sends nothing this station could read; CONNECT goes again"
            ),
            Heard::Nothing(why) => {
                format!("no answer to CONNECT ({why}): it goes again")
            }
        }
    }
}

/// The spacecraft command link of a station that only listens, from which
/// it sends CONNECT.
pub struct Listening {
    client: TcsClient,
    ours: ConfigDigest,
}

impl Listening {
    /// Opens the spacecraft command link a configuration states, from the
    /// ground's end, `tcs_cmd.send`.
    ///
    /// The one address the vehicle takes commands from, so a station that
    /// commands, tcsmoc among them, cannot be started beside this on the
    /// same configuration: one of the two would not bind it.
    ///
    /// # Arguments
    ///
    /// * `graph` - The station's configuration.
    ///
    /// # Returns
    ///
    /// The link, or `None` for a configuration with no `tcspecial` section,
    /// which states no command link: there is nowhere to send a CONNECT.
    ///
    /// # Errors
    ///
    /// What is wrong with the section, why the configuration has no
    /// checksum, or why the link cannot be bound.
    pub fn open(graph: &Graph) -> Result<Option<Listening>, String> {
        let Some(links) = command_links_of(graph)? else {
            return Ok(None);
        };
        let ours = digest_of_graph(graph).map_err(|e| e.to_string())?;
        let connection =
            crate::UdpConnection::new(&links.from, &links.interpreter)
                .map_err(|e| {
                    format!(
                        "the spacecraft command link cannot be opened at {} \
                         ({e}): tcs_cmd.send must be an address this station \
                         can bind, and nothing else may hold it",
                        links.from
                    )
                })?;
        let client = TcsClient::spacecraft_only(
            Box::new(connection),
            Matched::for_configuration(ours),
        );
        Ok(Some(Listening { client, ours }))
    }

    /// The checksum of the configuration this station read, which its
    /// CONNECT carries.
    pub fn ours(&self) -> ConfigDigest {
        self.ours
    }

    /// Sends one CONNECT, and waits up to `wait` for its answer.
    ///
    /// # Arguments
    ///
    /// * `wait` - How long to wait for the answer.
    ///
    /// # Returns
    ///
    /// What was heard.
    pub fn connect(&mut self, wait: Duration) -> Heard {
        self.client.set_timeout(wait);
        match self.client.connect(self.ours) {
            Ok((version, theirs)) if theirs == self.ours => {
                Heard::Matched(version)
            }
            Ok((_, theirs)) => Heard::Other(theirs),
            Err(e) => Heard::Nothing(e.to_string()),
        }
    }

    /// Sends CONNECT, and again each time what comes back is not a match,
    /// until the vehicle answers with this station's checksum.
    ///
    /// A vehicle that is not there yet, or that runs another configuration
    /// and may be restarted onto this one, is asked again rather than given
    /// up on: a listening station has nothing else to do until it is
    /// matched.
    ///
    /// # Arguments
    ///
    /// * `wait` - How long to wait for each answer before sending again.
    /// * `say`  - Told of every answer that is not a match, and of the
    ///            match, for the station to say.
    ///
    /// # Returns
    ///
    /// The vehicle's build, once it has answered with this checksum.
    pub fn until_matched(
        &mut self,
        wait: Duration,
        say: &mut dyn FnMut(&Heard),
    ) -> ConfigVersion {
        loop {
            let heard = self.connect(wait);
            say(&heard);
            if let Heard::Matched(version) = heard {
                return version;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::UdpSocket;
    use tcslibgs::{Command, ConnectTelemetry, Telemetry};

    /// A configuration whose spacecraft command link is on loopback ports
    /// nothing holds, the vehicle's end at `vehicle`.
    ///
    /// # Arguments
    ///
    /// * `vehicle` - The port the vehicle takes commands on.
    ///
    /// # Returns
    ///
    /// The configuration.
    fn graph(vehicle: u16) -> Graph {
        let free = || {
            UdpSocket::bind("127.0.0.1:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .port()
        };
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("listening.yaml");
        std::fs::write(
            &path,
            format!(
                "tcspecial:
  gateway: radio
  interface: \"{}\"
  tcs_cmd: {{ recv: \"127.0.0.1:{vehicle}\", send: \"127.0.0.1:{}\" }}
  payload_command: {{ source_base: \"127.0.0.1:{}\", dest_base: \"127.0.0.1:{}\" }}
  payload_telemetry: {{ source_base: \"127.0.0.1:{}\", dest_base: \"127.0.0.1:{}\" }}
  beacon: {{ default_interval: \"1 s\", reset_interval: \"1 s\", send: \"127.0.0.1:{}\", recv: \"127.0.0.1:{}\" }}
  state_file: \"/tmp/listening.state\"
",
                tcslibgs::InterfaceDigest::of_this_build(),
                free(),
                free(),
                free(),
                free(),
                free(),
                free(),
                free(),
            ),
        )
        .expect("written");
        Graph::load(&path).expect("it reads")
    }

    /// A CONNECT unanswered goes again; one answered with another checksum
    /// goes again; one answered with this station's is the match, and the
    /// station is told of each.
    #[test]
    fn connect_goes_again_until_the_vehicle_answers_with_ours() {
        let vehicle = UdpSocket::bind("127.0.0.1:0").expect("the vehicle");
        let at = vehicle.local_addr().unwrap().port();
        let graph = graph(at);
        let ours = digest_of_graph(&graph).unwrap();
        let mut listening = Listening::open(&graph)
            .expect("it opens")
            .expect("it has a section");
        assert_eq!(listening.ours(), ours);

        let answering = std::thread::spawn(move || {
            let mut buffer = [0u8; 256];
            let mut asked = 0;
            for answer in [None, Some(ConfigDigest([1; 16])), Some(ours)] {
                let (size, from) = vehicle.recv_from(&mut buffer).unwrap();
                let Ok(Command::Connect(cmd)) = Command::read(&buffer[..size])
                else {
                    panic!("a CONNECT");
                };
                assert_eq!(cmd.digest, ours, "it carries the station's");
                asked += 1;
                if let Some(running) = answer {
                    let reply = Telemetry::Connect(ConnectTelemetry::new(
                        cmd.header.sequence,
                        ConfigVersion::of_this_build(),
                        running,
                        cmd.header.sequence,
                    ));
                    vehicle.send_to(&reply.to_bytes(), from).unwrap();
                }
            }
            asked
        });

        let mut heard = Vec::new();
        let version = listening
            .until_matched(Duration::from_millis(200), &mut |h| {
                heard.push(h.clone())
            });
        assert_eq!(version, ConfigVersion::of_this_build());
        assert_eq!(answering.join().unwrap(), 3, "sent until matched");
        assert!(matches!(heard[0], Heard::Nothing(_)), "{heard:?}");
        assert_eq!(heard[1], Heard::Other(ConfigDigest([1; 16])));
        assert!(matches!(heard[2], Heard::Matched(_)), "{heard:?}");
        assert!(heard[2].said(ours).contains("matched"));
        assert!(heard[1].said(ours).contains("goes again"));
    }

    /// A configuration with no tcspecial section has no command link, and
    /// so nowhere to send a CONNECT.
    #[test]
    fn a_configuration_with_no_section_has_nothing_to_connect_to() {
        let dir = tempfile::tempdir().expect("a directory");
        let path = dir.path().join("none.yaml");
        std::fs::write(&path, "vehicle:\n  - { name: p, type: ping }\n")
            .unwrap();
        let graph = Graph::load(&path).unwrap();
        assert!(Listening::open(&graph).expect("it reads").is_none());
    }
}
