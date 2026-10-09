//! Configuration loading for payloads

use std::fs;
use std::path::Path;

use serde::de::IgnoredAny;
use serde::Deserialize;

use crate::{
    endpoint_config, load_config_file, CIConfigJson, ConfigFormat, DHConfig, PayloadConfig,
    TcsError, TcsResult,
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
pub const DEFAULT_PAYLOAD_CONFIG_PATH: &str = "tests/manual/tcspecial1.yaml";

/// Load payload configuration from a YAML or XML file.
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

/// Which kind of configuration a file holds.
///
/// A file's extension says how it is spelled -- YAML or XML -- and not
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
///
/// Public under a longer name as well -- see [`handler_source_of_text`] --
/// because the digest of a configuration has to ask the same question before
/// it can parse one.
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

/// Which kind of configuration `text` holds, for a caller that has the text.
pub fn handler_source_of_text(text: &str, format: ConfigFormat) -> TcsResult<HandlerSource> {
    handler_source_of(text, format)
}

/// Which kind of configuration the file at `path` holds.
pub fn handler_source<P: AsRef<Path>>(path: P) -> TcsResult<HandlerSource> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)?;
    handler_source_of(&text, ConfigFormat::of_file(path)?)
}

/// What a payload set says about the command interpreter, if it says anything.
///
/// The `tcspecial` section of a payload configuration file. `None` for a file
/// that has no such section, and for an endpoint configuration file, which has
/// no section to have: either way the command interpreter is placed by its own
/// file alone.
///
/// Here rather than in `load_dh_configs` because the two answer different
/// questions and most callers want only the handlers -- tcsmoc and tcssim have
/// no use for a command interpreter's configuration. The file is read twice by
/// the one program that wants both, which is a file read twice at startup.
pub fn load_tcspecial_section<P: AsRef<Path>>(path: P) -> TcsResult<Option<CIConfigJson>> {
    let path = path.as_ref();
    let format = ConfigFormat::of_file(path)?;
    let text = fs::read_to_string(path)?;

    match handler_source_of(&text, format)? {
        HandlerSource::Payload => {
            let config: PayloadConfig = format.parse(&text)?;
            Ok(config.tcspecial)
        }
        HandlerSource::Endpoints => Ok(None),
    }
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
    let format = ConfigFormat::of_file(path)?;

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

    /// `text` in a temporary file with `ext`, so the loaders choose their
    /// parser from the name as they do for a real one.
    fn write_temp(ext: &str, text: &str) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut file = tempfile::Builder::new().suffix(ext).tempfile().unwrap();
        file.write_all(text.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

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

    /// A payload set's tcspecial section is read, and an endpoint set has
    /// none to read.
    ///
    /// The section used to be carried and checked and nothing more. The
    /// beacon address is read from it now, so a set states where its own
    /// ground station listens.
    #[test]
    fn a_payload_sets_tcspecial_section_is_read() {
        let stated = "version: \"1.0\"\ndescription: a set\n\
                      tcspecial:\n  address: 0.0.0.0\n  port: 4000\n  protocol: udp\n  \
                      beacon_interval_ms: 5000\n  beacon_address: 239.255.0.7:7550\n  beacon_interface: 127.0.0.1\n\
                      payloads:\n  - dh_id: 0\n    name: DH0\n    type: device\n    \
                      path: /dev/null\n    packet_size: 1\n";
        let file = write_temp(".yaml", stated);
        let section = load_tcspecial_section(file.path())
            .expect("it loads")
            .expect("the set states a section");
        assert_eq!(section.beacon_address, "239.255.0.7:7550");
        assert_eq!(section.beacon_interface, "127.0.0.1");

        // A payload set with no section at all.
        let bare = "version: \"1.0\"\ndescription: a set\n\
                    payloads:\n  - dh_id: 0\n    name: DH0\n    type: device\n    \
                    path: /dev/null\n    packet_size: 1\n";
        let file = write_temp(".yaml", bare);
        assert!(load_tcspecial_section(file.path()).expect("it loads").is_none());

        // And an endpoint configuration, which has no section to have.
        let endpoints = "endpoint_groups:\n  - name: g\n    type: network\n    protocol: udp\n\
                         endpoints:\n  - name: e\n    group: g\n    address: localhost\n    \
                         port: 5000\n";
        let file = write_temp(".yaml", endpoints);
        assert!(load_tcspecial_section(file.path()).expect("it loads").is_none());
    }

    /// A tcspecial section states where beacons go and how often, or it is
    /// refused.
    ///
    /// Neither has a default. Beacons are how the ground knows the spacecraft
    /// is alive, so a section that has not been asked the question has not
    /// answered it, and a section is not the place to find that out by the
    /// beacons going somewhere nobody is listening.
    #[test]
    fn a_tcspecial_section_states_both_beacon_attributes() {
        let whole = "version: \"1.0\"\ndescription: a set\n\
                     tcspecial:\n  address: 0.0.0.0\n  port: 4000\n  protocol: udp\n  \
                     beacon_interval_ms: 5000\n  beacon_address: 239.255.0.1:5550\n  \
                     beacon_interface: 127.0.0.1\n\
                     payloads:\n  - dh_id: 0\n    name: DH0\n    type: device\n    \
                     path: /dev/null\n    packet_size: 1\n";
        let file = write_temp(".yaml", whole);
        let section = load_tcspecial_section(file.path())
            .expect("it loads")
            .expect("a section");
        assert_eq!(section.beacon_interval_ms, 5000);
        assert_eq!(section.beacon_address, "239.255.0.1:5550");
        assert_eq!(section.beacon_interface, "127.0.0.1");

        // Each of the three in turn: a section that leaves any of them out is
        // a section that has not said where its beacons go.
        for missing in [
            "  beacon_interval_ms: 5000\n",
            "  beacon_address: 239.255.0.1:5550\n",
            "  beacon_interface: 127.0.0.1\n",
        ] {
            let without = whole.replace(missing, "");
            assert_ne!(without, whole, "the test removed nothing");
            let file = write_temp(".yaml", &without);
            assert!(
                load_tcspecial_section(file.path()).is_err(),
                "a section with no {missing:?} was accepted"
            );
        }
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
