//! Configuration loading for payloads

use std::fs;
use std::path::Path;

use serde::de::IgnoredAny;
use serde::Deserialize;

use crate::{
    endpoint_config, load_config_file, ConfigFormat, DHConfig, PayloadConfig, TcsError, TcsResult,
};

/// Which environment variable names each program's payload configuration, and
/// what every one of them reads when its variable is unset.
///
/// Each program has its own variable because tcsmoc starts the other two as
/// subprocesses, which inherit its environment: one shared name could not
/// point tcsmoc at one file and its children at another. Tcsmoc itself takes
/// its file as a command line argument, for the same reason, and then sets
/// each child's variable on that child's own command.
///
/// They live here, beside [`load_dh_configs`], rather than in the
/// programs: the name tcsmoc sets for a child is then the same constant the
/// child reads, checked by the compiler rather than by two string literals
/// being kept equal by hand.
pub const PAYLOAD_CONFIG_PATH_VAR: &str = "PAYLOAD_CONFIG_PATH";

/// Which environment variable names tcssim's payload configuration.
///
/// See [`PAYLOAD_CONFIG_PATH_VAR`]. Tcssim's simulator configuration is a
/// separate file, named by a variable of its own that only tcssim knows:
/// tcsmoc never reads it, so there is nothing here to keep in step.
pub const SIM_PAYLOAD_CONFIG_PATH_VAR: &str = "SIM_PAYLOAD_CONFIG_PATH";

/// The payload configuration read when nothing names one.
pub const DEFAULT_PAYLOAD_CONFIG_PATH: &str = "tcspecial1.yaml";

/// Load payload configuration from a JSON, YAML, or XML file.
///
/// The format is chosen from the file extension; see
/// [`crate::format::ConfigFormat`]. All formats deserialize into the same
/// [`PayloadConfig`], whose data handler groups are resolved here so a caller
/// gets handlers with every attribute settled.
pub fn load_payload_config<P: AsRef<Path>>(path: P) -> TcsResult<Vec<DHConfig>> {
    let payload_config: PayloadConfig = load_config_file(path)?;

    payload_config.to_dh_configs().map_err(TcsError::Config)
}

/// The payload configuration file a program was told to read.
///
/// Every program that serves, controls or simulates a payload set needs to be
/// pointed at one, and they are all pointed the same way: a command line
/// argument first, then the program's own environment variable, then
/// [`DEFAULT_PAYLOAD_CONFIG_PATH`]. The argument comes first because it is
/// the unambiguous one -- tcsmoc starts the other two as subprocesses and
/// they inherit its environment, so a variable can reach further than it was
/// meant to, where an argument cannot.
///
/// `var` is the program's variable, or `None` for a program that has none:
/// tcsmoc deliberately has no variable of its own, for the reason above.
///
/// One argument is expected. A second is refused rather than ignored, because
/// a second path is more likely a mistake about which file is being read than
/// something meant to have no effect.
pub fn payload_path_from_args<I: Iterator<Item = String>>(
    mut args: I,
    var: Option<&str>,
) -> Result<String, String> {
    let program = args.next().unwrap_or_else(|| "program".to_string());

    let from_args = args.next();
    if let Some(extra) = args.next() {
        return Err(format!(
            "unexpected argument \"{}\"\nusage: {} [payload configuration file]",
            extra, program
        ));
    }

    if let Some(path) = from_args {
        return Ok(path);
    }

    Ok(var
        .and_then(|var| std::env::var(var).ok())
        .unwrap_or_else(|| DEFAULT_PAYLOAD_CONFIG_PATH.to_string()))
}

/// The payload configuration file and the address to serve commands on.
///
/// For a program that answers the ground rather than only reading a payload
/// set: a command line of `[payload configuration file [command address]]`.
/// The payload path is settled exactly as [`payload_path_from_args`] settles
/// it, and the address is given back as it was written, for the caller to
/// take apart with [`command_address_parts`] -- nothing here knows what a
/// program's configuration file would otherwise have said.
///
/// The address is an argument rather than only a configuration file entry
/// because two programs have to agree on it: tcsmoc decides where it will
/// send commands and starts tcspecial, so it can hand over the address it
/// chose instead of both ends reading their own file and hoping. An argument
/// is also the one way a child can be told something tcsmoc's own
/// environment will not pass on to whatever that child starts in turn.
///
/// Positional, and in that order, so the file a program reads is named the
/// same way in every program here. An address cannot be given on its own; a
/// program told only an address would be reading a payload file nobody named.
pub fn payload_path_and_command_address<I: Iterator<Item = String>>(
    mut args: I,
    var: Option<&str>,
) -> Result<(String, Option<String>), String> {
    let program = args.next().unwrap_or_else(|| "program".to_string());
    let usage = format!(
        "usage: {} [payload configuration file [command address]]",
        program
    );

    let from_args = args.next();
    let address = args.next();
    if let Some(extra) = args.next() {
        return Err(format!("unexpected argument \"{}\"\n{}", extra, usage));
    }

    let path = match from_args {
        Some(path) => path,
        None => var
            .and_then(|var| std::env::var(var).ok())
            .unwrap_or_else(|| DEFAULT_PAYLOAD_CONFIG_PATH.to_string()),
    };

    Ok((path, address))
}

/// An `address:port` split into the two a socket is bound from.
///
/// Split at the last colon rather than parsed as a socket address, so that a
/// name is as good as a number: `localhost:4000` is what someone types, and
/// resolving it is the bind's business. The last colon is what makes an IPv6
/// address in brackets work as well.
///
/// A port is required. An address without one is not a thing to default,
/// because the whole reason the address is given is that two programs have to
/// agree on it -- and they agree on the port or not at all.
pub fn command_address_parts(address: &str) -> Result<(String, u16), String> {
    let (host, port) = address.rsplit_once(':').ok_or_else(|| {
        format!(
            "command address \"{}\" has no port: give it as address:port",
            address
        )
    })?;

    if host.is_empty() {
        return Err(format!(
            "command address \"{}\" has no address: give it as address:port",
            address
        ));
    }

    let port: u16 = port.parse().map_err(|_| {
        format!(
            "command address \"{}\" has \"{}\" where its port should be",
            address, port
        )
    })?;

    Ok((host.to_string(), port))
}

/// Which kind of configuration a file holds.
///
/// A file's extension says how it is spelled -- YAML, JSON or XML -- and not
/// what it describes. Both kinds of configuration can be written in any of the
/// three, so which one a file holds is read from the sections it has rather
/// than from its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerSource {
    /// A payload configuration, which names data handlers directly.
    Payload,
    /// An endpoint configuration, whose endpoints become data handlers.
    Endpoints,
}

/// Just enough of a file to tell what kind it is.
///
/// Every field is ignored once seen: this asks which sections exist and
/// nothing about their contents, so a file with an error inside a section is
/// still recognised and then reported by the real parser, which can say what
/// is wrong with it.
///
/// Each is a list rather than one ignored value, for XML's sake: a sequence
/// there is repeated sibling elements, so a file with two payloads presents
/// `payloads` twice and a single field is a duplicate-field error. That read
/// every XML payload file of more than one payload as unrecognisable -- and
/// said `duplicate field`, which is a complaint about this struct rather than
/// about the file.
#[derive(Deserialize)]
struct Sections {
    #[serde(default)]
    payloads: Vec<IgnoredAny>,
    /// What the payload section used to be called.
    ///
    /// Asked about only so that a file written to the old spelling is told
    /// the new one. Without this it would name no section this knows and be
    /// reported as describing no payloads at all, which is true and useless.
    #[serde(default)]
    data_handlers: Vec<IgnoredAny>,
    /// What a group of them used to be called, for the same reason.
    #[serde(default)]
    data_handler_groups: Vec<IgnoredAny>,
    #[serde(default, alias = "endpoint-groups")]
    endpoint_groups: Vec<IgnoredAny>,
    #[serde(default)]
    endpoints: Vec<IgnoredAny>,
}

/// Which kind of configuration `text` holds.
fn handler_source_of(text: &str, format: ConfigFormat) -> TcsResult<HandlerSource> {
    let sections: Sections = format.parse(text)?;

    // A payload file is recognised by its own section, so a file carrying both
    // -- which neither format describes -- is read as a payload file rather
    // than rejected. There is nothing a caller could do about it either way.
    if !sections.payloads.is_empty() {
        Ok(HandlerSource::Payload)
    } else if !sections.endpoints.is_empty() || !sections.endpoint_groups.is_empty() {
        Ok(HandlerSource::Endpoints)
    } else if !sections.data_handlers.is_empty() || !sections.data_handler_groups.is_empty() {
        // The old spelling, named rather than ignored. A file keeping it would
        // otherwise be read as naming no section at all, and the honest
        // report of that -- it describes no payloads -- would say nothing
        // about the one word that has to change.
        Err(TcsError::Config(
            "names data_handlers, which is what the payload section was called \
             before: payloads is the section now, and payload_groups a group of \
             them"
                .to_string(),
        ))
    } else {
        Err(TcsError::Config(
            "names neither payloads nor endpoints, so it describes no payloads"
                .to_string(),
        ))
    }
}

/// Which kind of configuration the file at `path` holds.
pub fn handler_source<P: AsRef<Path>>(path: P) -> TcsResult<HandlerSource> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)?;
    handler_source_of(&text, ConfigFormat::from_path(path))
}

/// Load data handlers from a payload or an endpoint configuration file.
///
/// Every program that needs data handlers reads them through this, so that any
/// of them can be pointed at either kind of file and all of them agree about
/// what a given file describes. That matters more than it sounds: tcsmoc's
/// panels, tcssim's payloads and tcspecial's handlers have to be the same
/// handlers, and one program reading a file a different way is the drift the
/// payload set mechanism exists to prevent.
pub fn load_dh_configs<P: AsRef<Path>>(path: P) -> TcsResult<Vec<DHConfig>> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)?;
    let format = ConfigFormat::from_path(path);

    match handler_source_of(&text, format)? {
        HandlerSource::Payload => {
            let config: PayloadConfig = format.parse(&text)?;
            config.to_dh_configs().map_err(TcsError::Config)
        }
        HandlerSource::Endpoints => {
            let doc = endpoint_config::from_str(&text, format)?;
            doc.to_dh_configs().map_err(TcsError::from)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD: &str = "
version: \"1.0\"
description: a payload file
payloads:
  - dh_id: 0
    name: DH0
    oc_address: 127.0.0.1
    oc_port: 6000
    type: network
    protocol: udp
    address: localhost
    port: 5000
    packet_size: 12
";

    const ENDPOINTS: &str = "
endpoint_groups:
  - name: payload_udp
    type: network
    protocol: udp
    packet_size: 12
endpoints:
  - name: DH0
    group: payload_udp
    dh_id: 0
    oc_address: 127.0.0.1
    oc_port: 6000
    address: localhost
    port: 5000
";

    /// Arguments as a program really receives them, its own name first.
    fn args(rest: &[&str]) -> std::vec::IntoIter<String> {
        let mut all = vec!["program".to_string()];
        all.extend(rest.iter().map(|s| s.to_string()));
        all.into_iter()
    }

    #[test]
    fn an_argument_names_the_payload_file() {
        assert_eq!(
            payload_path_from_args(args(&["tcspecial2.yaml"]), None).unwrap(),
            "tcspecial2.yaml"
        );
    }

    /// A second argument is the address commands are taken on.
    #[test]
    fn a_second_argument_is_the_command_address() {
        assert_eq!(
            payload_path_and_command_address(args(&["tcspecial2.yaml", "127.0.0.1:4000"]), None)
                .unwrap(),
            ("tcspecial2.yaml".to_string(), Some("127.0.0.1:4000".to_string()))
        );
    }

    /// With no second argument there is no address, and the caller is left to
    /// whatever its own configuration said.
    #[test]
    fn with_no_second_argument_there_is_no_command_address() {
        assert_eq!(
            payload_path_and_command_address(args(&["tcspecial2.yaml"]), None).unwrap(),
            ("tcspecial2.yaml".to_string(), None)
        );
        assert_eq!(
            payload_path_and_command_address(args(&[]), None).unwrap(),
            (DEFAULT_PAYLOAD_CONFIG_PATH.to_string(), None)
        );
    }

    /// A third argument is refused, and the refusal says what the two are.
    #[test]
    fn a_third_argument_is_refused() {
        let e = payload_path_and_command_address(
            args(&["tcspecial2.yaml", "127.0.0.1:4000", "extra"]),
            None,
        )
        .expect_err("three arguments");
        assert!(e.contains("extra"), "the refusal does not say which: {e}");
        assert!(
            e.contains("payload configuration file") && e.contains("command address"),
            "the usage does not say what is expected: {e}"
        );
    }

    /// An address is split where a socket needs it split, and a name is as
    /// good as a number.
    #[test]
    fn a_command_address_is_an_address_and_a_port() {
        assert_eq!(
            command_address_parts("0.0.0.0:4000").unwrap(),
            ("0.0.0.0".to_string(), 4000)
        );
        assert_eq!(
            command_address_parts("localhost:4000").unwrap(),
            ("localhost".to_string(), 4000)
        );
        // The last colon, which is what makes a bracketed IPv6 address work.
        assert_eq!(
            command_address_parts("[::1]:4000").unwrap(),
            ("[::1]".to_string(), 4000)
        );
    }

    /// An address that is not one is refused rather than defaulted.
    ///
    /// The address exists so that two programs agree on where commands go, and
    /// a default is exactly the disagreement it is there to prevent.
    #[test]
    fn an_address_that_is_not_one_is_refused() {
        for bad in ["127.0.0.1", "127.0.0.1:", ":4000", "127.0.0.1:no", "127.0.0.1:99999"] {
            let e = command_address_parts(bad).expect_err(bad);
            assert!(e.contains(bad), "{bad}: the complaint does not say it: {e}");
        }
    }

    #[test]
    fn with_no_argument_and_no_variable_the_default_is_read() {
        assert_eq!(
            payload_path_from_args(args(&[]), None).unwrap(),
            DEFAULT_PAYLOAD_CONFIG_PATH
        );
    }

    #[test]
    fn an_argument_beats_the_variable() {
        // The argument is the unambiguous one: a variable set for one program
        // reaches the subprocesses it starts, and an argument does not.
        let var = "TCS_TEST_PAYLOAD_PATH_PRECEDENCE";
        std::env::set_var(var, "from_the_environment.yaml");

        assert_eq!(
            payload_path_from_args(args(&["from_the_argument.yaml"]), Some(var)).unwrap(),
            "from_the_argument.yaml"
        );
        // And with no argument the variable is what is left.
        assert_eq!(
            payload_path_from_args(args(&[]), Some(var)).unwrap(),
            "from_the_environment.yaml"
        );

        std::env::remove_var(var);
    }

    #[test]
    fn a_second_payload_file_is_refused_rather_than_ignored() {
        let message = payload_path_from_args(args(&["one.yaml", "two.yaml"]), None)
            .expect_err("two paths must be refused");
        assert!(
            message.contains("two.yaml"),
            "the error should name the extra argument, but said: {message}"
        );
    }

    /// An XML file of several payloads is recognised as a payload file.
    ///
    /// A sequence in XML is repeated sibling elements, so a file of two
    /// payloads names `payloads` twice. Asking about the section with a single
    /// field made the second one a duplicate-field error, and every XML
    /// payload file of more than one payload was refused -- with a complaint
    /// about a duplicate field, which says nothing about the file and is not
    /// even true of it.
    #[test]
    fn an_xml_file_of_several_payloads_is_recognised() {
        let xml = "<payload>\
             <version>1.0</version>\
             <description>two of them</description>\
             <payloads><dh_id>0</dh_id><name>DH0</name><type>device</type>\
               <path>/dev/null</path><packet_size>1</packet_size></payloads>\
             <payloads><dh_id>1</dh_id><name>DH1</name><type>device</type>\
               <path>/dev/zero</path><packet_size>1</packet_size></payloads>\
             </payload>";

        assert_eq!(
            handler_source_of(xml, ConfigFormat::Xml).expect("an XML payload file"),
            HandlerSource::Payload
        );

        // And the whole file still loads, which is the thing the sniffer was
        // standing in the way of.
        let config: PayloadConfig = ConfigFormat::Xml.parse(xml).expect("it parses");
        assert_eq!(config.payloads.len(), 2);
    }

    /// The same of an endpoint file, whose sections are wrappers rather than
    /// repeats: both shapes have to reach the same answer.
    #[test]
    fn an_xml_endpoint_file_is_recognised() {
        let xml = "<endpoint-configuration>\
             <general version=\"1.0\"/>\
             <endpoint-groups><group name=\"g\" type=\"network\" protocol=\"udp\"/>\
               </endpoint-groups>\
             <endpoints><endpoint name=\"e\" group=\"g\" address=\"localhost\" \
               port=\"5000\"/></endpoints>\
             </endpoint-configuration>";

        assert_eq!(
            handler_source_of(xml, ConfigFormat::Xml).expect("an XML endpoint file"),
            HandlerSource::Endpoints
        );
    }

    #[test]
    fn each_kind_of_file_is_recognised_by_its_sections() {
        assert_eq!(
            handler_source_of(PAYLOAD, ConfigFormat::Yaml).unwrap(),
            HandlerSource::Payload
        );
        assert_eq!(
            handler_source_of(ENDPOINTS, ConfigFormat::Yaml).unwrap(),
            HandlerSource::Endpoints
        );
    }

    /// A file written to the old section name is told the new one.
    ///
    /// The sections were data_handlers and data_handler_groups, and a file
    /// that still names them describes payloads perfectly well -- it names
    /// them in a word this no longer reads. Saying it describes no payloads
    /// would be true and would not help.
    #[test]
    fn a_file_naming_the_old_payload_section_is_told_the_new_name() {
        for old in ["data_handlers:\n  - dh_id: 0\n", "data_handler_groups:\n  - name: g\n"] {
            let text = format!("version: \"1.0\"\n{old}");
            let message = handler_source_of(&text, ConfigFormat::Yaml)
                .expect_err("the old spelling must be rejected")
                .to_string();
            assert!(
                message.contains("data_handlers")
                    && message.contains("payloads")
                    && message.contains("payload_groups"),
                "the error should name the old section and both new ones, but \
                 said: {message}"
            );
        }
    }

    #[test]
    fn a_file_naming_neither_section_is_rejected() {
        // Not read as an endpoint configuration with no endpoints, which is
        // what an empty document would otherwise look like.
        let message = handler_source_of("version: \"1.0\"\n", ConfigFormat::Yaml)
            .expect_err("must be rejected")
            .to_string();
        assert!(
            message.contains("payloads") && message.contains("endpoints"),
            "the error should name both sections, but said: {message}"
        );
    }

    #[test]
    fn both_kinds_of_file_give_the_same_handler() {
        // The same data handler written each way, so this is what says the two
        // formats are interchangeable where they overlap.
        let from_payload = {
            let config: PayloadConfig = ConfigFormat::Yaml.parse(PAYLOAD).unwrap();
            config.to_dh_configs().unwrap()
        };
        let from_endpoints = endpoint_config::from_yaml_str(ENDPOINTS)
            .unwrap()
            .to_dh_configs()
            .unwrap();

        assert_eq!(from_payload.len(), 1);
        assert_eq!(from_endpoints.len(), 1);

        let a = &from_payload[0];
        let b = &from_endpoints[0];
        assert_eq!(a.dh_id, b.dh_id);
        assert_eq!(a.name, b.name);
        assert_eq!(a.endpoint, b.endpoint);
        assert_eq!(a.packet_size, b.packet_size);
        assert_eq!(a.oc, b.oc);
    }
}
