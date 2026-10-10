//! What is wrong with a configuration file, and what each thing is about.
//!
//! Every rule in this library is asked the same question twice. A program
//! loading a file wants the first answer and nothing else: it cannot run, and
//! which of several mistakes it names hardly matters. A person checking a file
//! wants all of them, each beside the line that has to change -- which is what
//! `tcsverify` is for.
//!
//! So the rules are stated once and collect their answers. The loaders --
//! [`crate::PayloadConfig::to_dh_configs`] and
//! [`crate::SimConfigFile::resolve`] -- return the first of them, which is
//! what they always returned; the verifier takes the lot.
//!
//! A problem says what it is about rather than where it is. Nothing here has
//! seen the file: a parser hands up a document, and by then the lines are
//! gone. Naming the payload or the group is enough, because a name is what a
//! reader looks for in the file, and it is what `tcsverify` looks for too.

/// What a problem is about, which is how the line it belongs to is found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum About {
    /// The file as a whole: a file that does not parse, or that describes
    /// nothing this reads.
    File,
    /// One of the file's sections, by the name the file calls it.
    Section(&'static str),
    /// A payload, by name.
    Payload(String),
    /// A group of payloads, by name.
    Group(String),
    /// A simulated payload, by name.
    SimPayload(String),
    /// A group of simulated payloads, by name.
    SimGroup(String),
}

/// Something wrong with a configuration file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// What it is about.
    pub about: About,
    /// What is wrong, in the words the loaders have always used.
    pub message: String,
}

impl Problem {
    /// A problem with the file itself.
    pub fn file(message: impl Into<String>) -> Self {
        Problem {
            about: About::File,
            message: message.into(),
        }
    }

    /// A problem with one of its sections.
    pub fn section(section: &'static str, message: impl Into<String>) -> Self {
        Problem {
            about: About::Section(section),
            message: message.into(),
        }
    }

    /// A problem with a payload.
    pub fn payload(name: &str, message: impl Into<String>) -> Self {
        Problem {
            about: About::Payload(name.to_string()),
            message: message.into(),
        }
    }

    /// A problem with a group of payloads.
    pub fn group(name: &str, message: impl Into<String>) -> Self {
        Problem {
            about: About::Group(name.to_string()),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
