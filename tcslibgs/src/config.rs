//! Configuration loading for payloads

use std::path::Path;

use serde::de::IgnoredAny;
use serde::Deserialize;

use crate::{ConfigFormat, TcsError};

/// Which environment variable names each program's payload configuration, and
/// what every one of them reads when its variable is unset.
///
/// Each program has its own variable because tcsmoc starts the other two as
/// subprocesses, which inherit its environment: one shared name could not
/// point tcsmoc at one file and its children at another. Tcsmoc itself takes
/// its file as a command line argument, for the same reason, and then sets
/// each child's variable on that child's own command.
///
/// They live here, in the library every one of them reads its configuration
/// through, rather than in the programs: the name tcsmoc sets for a child is then the same constant the
/// child reads, checked by the compiler rather than by two string literals
/// being kept equal by hand.
pub const PAYLOAD_CONFIG_PATH_VAR: &str = "PAYLOAD_CONFIG_PATH";

/// Which environment variable names tcssim's payload configuration.
///
/// See [`PAYLOAD_CONFIG_PATH_VAR`]. Tcssim's simulator configuration is a
/// separate file, named by a variable of its own that only tcssim knows:
/// tcsmoc never reads it, so there is nothing here to keep in step.
pub const SIM_PAYLOAD_CONFIG_PATH_VAR: &str = "SIM_PAYLOAD_CONFIG_PATH";

/// The configuration read when nothing names one.
///
/// The shipped fixture. A default at all is for running the programs from
/// the repository by hand -- every one of them takes the path as its
/// argument, and that is what a real run names.
pub const DEFAULT_PAYLOAD_CONFIG_PATH: &str = "tests/manual/thermostat.yaml";


/// The simulator file beside a configuration and named for it, where there is
/// one.
///
/// Two spellings are looked for: `<stem>-sim.yaml`, which the examples use,
/// and the older `<stem>sim.yaml`. The stem is the configuration's name
/// without `.yaml`, `.yml` or `.xml`, so either spelling of a configuration
/// finds the same simulator file. One function for every program that looks,
/// so that tcsmoc and the tcssim it starts cannot pick different files.
pub fn sim_path_beside(payload_path: &str) -> Option<String> {
    let stem = [".yaml", ".yml", ".xml"]
        .iter()
        .find_map(|ext| payload_path.strip_suffix(ext))?;
    [format!("{stem}-sim.yaml"), format!("{stem}sim.yaml")]
        .into_iter()
        .find(|candidate| Path::new(candidate).is_file())
}

/// The tcspecial configuration file a program was told to read.
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
            "unexpected argument \"{}\"\nusage: {} [tcspecial configuration file]",
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
/// every XML tcspecial configuration of more than one payload as unrecognisable
/// -- and said `duplicate field`, which is a complaint about this struct rather
/// than about the file.
///
/// There used to be a second set of sections here, and this said which of two
/// languages a file was written in. The other one -- `general`,
/// `endpoint_groups`, `endpoints` -- is gone, so the question is no longer
/// which language but whether this is one of these files at all.
#[derive(Deserialize)]
struct Sections {
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


/// What a file still written to an older spelling is told, if it is one.
///
/// Asked by the graph loader before it reads a file: those older files stated
/// no version, as a configuration now does not, so nothing else tells them
/// apart from a graph, and one is better told what became of its sections
/// than that it names fields a graph does not have.
pub fn names_what_is_gone(text: &str, format: ConfigFormat) -> Option<TcsError> {
    what_is_gone(&format.parse(text).ok()?)
}

fn what_is_gone(sections: &Sections) -> Option<TcsError> {
    if !sections.data_handlers.is_empty() || !sections.data_handler_groups.is_empty() {
        // The old spelling, named rather than ignored. A file keeping it would
        // otherwise be read as naming no section at all, and the honest
        // report of that -- it describes no payloads -- would say nothing
        // about the one word that has to change.
        Some(TcsError::Config(
            "names data_handlers, which is what the payload section was called \
             before: payloads is the section now, and payload_groups a group of \
             them"
                .to_string(),
        ))
    } else if !sections.endpoints.is_empty() || !sections.endpoint_groups.is_empty() {
        // The language that is gone, named for the same reason. Its files
        // described the same payloads in other words, and a payload states
        // what an endpoint and its group stated between them.
        Some(TcsError::Config(
            "names endpoints, which was a second way to describe payloads and \
             is gone: a payload states what an endpoint and its group stated \
             between them, in the payloads section"
                .to_string(),
        ))
    } else {
        None
    }
}






#[cfg(test)]
mod tests {
    use super::*;


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
        let message = names_what_is_gone(endpoints, ConfigFormat::Yaml)
            .expect("the language is gone")
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
            "data_handlers:\n  - ",
            "data_handler_groups:\n  - name: g\n",
        ] {
            let text = format!("version: \"1.0\"\n{old}");
            let message = names_what_is_gone(&text, ConfigFormat::Yaml)
                .expect("the old spelling is named")
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

}
