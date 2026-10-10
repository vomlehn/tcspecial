//! Configuration loading for payloads

use std::fs;
use std::path::Path;

use serde::de::IgnoredAny;
use serde::Deserialize;

use crate::{
    load_config_file, CIConfigJson, ConfigFormat, ConfigVersion, DHConfig, PayloadConfig,
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

/// Just enough of a file to tell whether it describes payloads.
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
///
/// There used to be a second set of sections here, and this said which of two
/// languages a file was written in. The other one -- `general`,
/// `endpoint_groups`, `endpoints` -- is gone, so the question is no longer
/// which language but whether this is one of these files at all.
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
    /// The sections of the language that is gone, for the same reason again:
    /// a file still written in it is told what became of it rather than told
    /// it describes no payloads.
    #[serde(default, alias = "endpoint-groups")]
    endpoint_groups: Vec<IgnoredAny>,
    #[serde(default)]
    endpoints: Vec<IgnoredAny>,
}

/// Whether `text` describes payloads, and what is wrong with it if not.
///
/// Public under a longer name as well -- see [`describes_payloads_text`] --
/// because the digest of a configuration has to ask the same question before
/// it can parse one.
fn describes_payloads(text: &str, format: ConfigFormat) -> TcsResult<()> {
    let sections: Sections = format.parse(text)?;

    if !sections.payloads.is_empty() {
        Ok(())
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
    } else if !sections.endpoints.is_empty() || !sections.endpoint_groups.is_empty() {
        // The language that is gone, named for the same reason. Its files
        // described the same payloads in other words, and a payload states
        // what an endpoint and its group stated between them.
        Err(TcsError::Config(
            "names endpoints, which was a second way to describe payloads and \
             is gone: a payload states what an endpoint and its group stated \
             between them, in the payloads section"
                .to_string(),
        ))
    } else {
        Err(TcsError::Config(
            "names no payloads section, so it describes no payloads".to_string(),
        ))
    }
}

/// Whether `text` describes payloads, for a caller that has the text.
pub fn describes_payloads_text(text: &str, format: ConfigFormat) -> TcsResult<()> {
    describes_payloads(text, format)
}

/// Whether the file at `path` describes payloads.
pub fn file_describes_payloads<P: AsRef<Path>>(path: P) -> TcsResult<()> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)?;
    describes_payloads(&text, ConfigFormat::of_file(path)?)
}

/// What a payload set says about the command interpreter, if it says anything.
///
/// The `tcspecial` section of a payload configuration file. `None` for a file
/// that has no such section, in which case the command interpreter is placed
/// by its own file alone.
///
/// Here rather than in `load_dh_configs` because the two answer different
/// questions and most callers want only the handlers -- tcsmoc and tcssim have
/// no use for a command interpreter's configuration. The file is read twice by
/// the one program that wants both, which is a file read twice at startup.
pub fn load_tcspecial_section<P: AsRef<Path>>(path: P) -> TcsResult<Option<CIConfigJson>> {
    let path = path.as_ref();
    let format = ConfigFormat::of_file(path)?;
    let text = fs::read_to_string(path)?;

    describes_payloads(&text, format)?;
    let config: PayloadConfig = format.parse(&text)?;
    Ok(config.tcspecial)
}

/// The version a payload set states, as the three bytes a beacon carries.
///
/// Here beside the section loader and for the same reason: tcspecial wants one
/// fact out of the file that the handlers do not carry, and reading the file
/// again at startup is cheaper than threading the whole document through every
/// program that only wants the handlers.
///
/// A version that is not one, two or three decimal parts is an error rather
/// than a nought: it goes out in every beacon, where a wrong version is worse
/// than no program at all.
pub fn load_config_version<P: AsRef<Path>>(path: P) -> TcsResult<ConfigVersion> {
    let path = path.as_ref();
    let format = ConfigFormat::of_file(path)?;
    let text = fs::read_to_string(path)?;

    describes_payloads(&text, format)?;
    let config: PayloadConfig = format.parse(&text)?;
    config.config_version().ok_or_else(|| {
        TcsError::Config(format!(
            "{}: version \"{}\" is not one, two or three decimal parts, and a \
             payload set says its version in every beacon",
            path.display(),
            config.version
        ))
    })
}

/// Load data handlers from a payload configuration file.
///
/// Every program that needs data handlers reads them through this, so that all
/// of them agree about what a given file describes. That matters more than it sounds: tcsmoc's
/// panels, tcssim's payloads and tcspecial's handlers have to be the same
/// handlers, and one program reading a file a different way is the drift the
/// payload set mechanism exists to prevent.
pub fn load_dh_configs<P: AsRef<Path>>(path: P) -> TcsResult<Vec<DHConfig>> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)?;
    let format = ConfigFormat::of_file(path)?;

    describes_payloads(&text, format)?;
    let config: PayloadConfig = format.parse(&text)?;
    config.to_dh_configs().map_err(TcsError::Config)
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

        describes_payloads(xml, ConfigFormat::Xml).expect("an XML payload file");

        // And the whole file still loads, which is the thing the sniffer was
        // standing in the way of.
        let config: PayloadConfig = ConfigFormat::Xml.parse(xml).expect("it parses");
        assert_eq!(config.payloads.len(), 2);
    }

    /// A payload set's tcspecial section is read.
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
        assert!(load_tcspecial_section(file.path())
            .expect("it loads")
            .is_none());
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
    fn a_file_is_recognised_by_its_payloads_section() {
        describes_payloads(PAYLOAD, ConfigFormat::Yaml).expect("a payload file");
    }

    /// A file written in the language that is gone is told what became of it.
    ///
    /// Its files described the same payloads in other words -- a group of
    /// endpoints and the endpoints in it -- so one still written that way
    /// names no section this reads. Saying it describes no payloads would be
    /// true and would not say what to do about it.
    #[test]
    fn a_file_naming_endpoints_is_told_the_language_is_gone() {
        let endpoints = "endpoint_groups:\n  - name: g\n    type: network\n    protocol: udp\n\
                         endpoints:\n  - name: e\n    group: g\n    address: localhost\n    \
                         port: 5000\n";
        let message = describes_payloads(endpoints, ConfigFormat::Yaml)
            .expect_err("the language is gone")
            .to_string();
        assert!(
            message.contains("endpoints") && message.contains("payloads"),
            "the error should name what it was and what to write instead, but \
             said: {message}"
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
        for old in [
            "data_handlers:\n  - dh_id: 0\n",
            "data_handler_groups:\n  - name: g\n",
        ] {
            let text = format!("version: \"1.0\"\n{old}");
            let message = describes_payloads(&text, ConfigFormat::Yaml)
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
    fn a_file_naming_no_payloads_section_is_refused() {
        let message = describes_payloads("version: \"1.0\"\n", ConfigFormat::Yaml)
            .expect_err("must be rejected")
            .to_string();
        assert!(
            message.contains("payloads"),
            "the error should name the section it wanted, but said: {message}"
        );
    }
}
