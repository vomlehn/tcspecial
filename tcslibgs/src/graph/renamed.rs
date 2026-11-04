//! The words a configuration used to say, and the words it says now.
//!
//! The side of the graph that commands tcspecial and takes its telemetry was
//! called `moc` while the ground was the only thing that could be there. It is
//! the gateway now, because a cFS or an F´ flight computer can stand where the
//! ground stood -- see "Gateways: what the vehicle answers to" in
//! `docs/design.rst` -- and every word a file wrote for that side was renamed
//! with it.
//!
//! Then the terminals: `payload-` and `gateway-` became `world-` and `moc-`,
//! for the two sides of the graph each stream travels between -- the world a
//! payload reads, and the MOC that commands it -- and their shape changed
//! with them. A sink lists names, a link is written to one, and a file in the
//! old words is told both.
//!
//! A file still written in the old words is refused here, by line, and told
//! the word to write instead. Refused rather than read as before, because one
//! word for two things is what the rename was for, and a reader that took
//! both would leave every file free to say either for good. And here rather
//! than in serde's words, because "unknown field `moc`" says what is wrong and
//! not what to do about it, and a node type nothing knows reads as a typing
//! mistake when it is a file that was right once.
//!
//! What a file says is read as the format reads it, so that a word in a
//! comment or inside a description is not taken for a key; the text is
//! searched only for the line to point at.

use quick_xml::events::Event;

use crate::format::ConfigFormat;

/// Where in a file an old word was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    /// A section of its own, at the top of the file.
    Section,
    /// A type a section groups its nodes by.
    NodeType,
    /// A key of a node: one of its links, or what it is told.
    Key,
}

/// One old word, where it was written, and the word for it now.
struct Renamed {
    old: &'static str,
    place: Place,
    new: &'static str,
    /// What else changed with the word, said after it, where anything did.
    then: &'static str,
}

/// What a sink holds now, said after its new name.
const SINK: &str = ", and list under it the names a line may arrive by, as \
                    `[readings]`";

/// And what a source holds: each line as the node it reaches and the name
/// it arrives by.
const SOURCE: &str = ", and write each line under it as the node it reaches \
                      and a name that node lists, as `[gateway.readings]`";

/// Every word that was renamed, and the word it is now: the one list, which
/// the reader and its tests both read.
const RENAMED: &[Renamed] = &[
    Renamed {
        old: "moc",
        place: Place::Section,
        new: "gateways",
        then: "",
    },
    Renamed {
        old: "moc",
        place: Place::NodeType,
        new: "gateway_radio",
        then: "",
    },
    Renamed {
        old: "tee-moc",
        place: Place::NodeType,
        new: "tee-gateway",
        then: "",
    },
    Renamed {
        old: "null-moc",
        place: Place::NodeType,
        new: "null-gateway",
        then: "",
    },
    Renamed {
        old: "app_moc_veh",
        place: Place::NodeType,
        new: "app_gateway_veh",
        then: "",
    },
    // The four terminals, named for the side of the graph each stream
    // belongs to: the world a payload reads, and the MOC that commands it.
    // Their shape changed with them: a sink lists the names a line may
    // arrive by, and a link is written to one of those.
    Renamed {
        old: "payload-in",
        place: Place::Key,
        new: "world-in",
        then: SINK,
    },
    Renamed {
        old: "payload-out",
        place: Place::Key,
        new: "world-out",
        then: SOURCE,
    },
    Renamed {
        old: "gateway-in",
        place: Place::Key,
        new: "moc-in",
        then: SINK,
    },
    Renamed {
        old: "gateway-out",
        place: Place::Key,
        new: "moc-out",
        then: SOURCE,
    },
    Renamed {
        old: "app-moc-arg",
        place: Place::Key,
        new: "app-gateway-arg",
        then: "",
    },
];

/// The renamed word, if any, a word in this place is.
///
/// # Arguments
///
/// * `place` - Where the word was written.
/// * `word`  - The word.
///
/// # Returns
///
/// The entry for it, or `None` for a word that was never renamed.
fn renamed(place: Place, word: &str) -> Option<&'static Renamed> {
    RENAMED.iter().find(|r| r.place == place && r.old == word)
}

/// Says what a file must call something it still calls by its old name.
///
/// # Arguments
///
/// * `text`   - The file's text.
/// * `format` - What it is written in.
///
/// # Returns
///
/// A sentence naming the line the first old word is on, the word, and the
/// word to write instead; or `None` for a file that writes none of them, or
/// that does not parse, which the parser has its own words for.
pub fn refused(text: &str, format: ConfigFormat) -> Option<String> {
    let (line, said) = match format {
        ConfigFormat::Yaml => in_yaml(text),
        ConfigFormat::Xml => in_xml(text),
    }?;
    let what = match said.place {
        Place::Section => format!("the {} section", said.old),
        Place::NodeType => format!("the node type {}", said.old),
        Place::Key => said.old.to_string(),
    };
    Some(format!(
        "at line {line}, {what} is called {} now: rename it{}",
        said.new, said.then
    ))
}

/// Finds the first old word a YAML file writes, by line.
///
/// # Arguments
///
/// * `text` - The file's text.
///
/// # Returns
///
/// The line, from 1, and the word; or `None` where there is none or the
/// text is not YAML.
fn in_yaml(text: &str) -> Option<(usize, &'static Renamed)> {
    let file: serde_norway::Value = serde_norway::from_str(text).ok()?;
    let sections = file.as_mapping()?;
    let mut found: Vec<&'static Renamed> = Vec::new();
    for (key, value) in sections {
        if let Some(said) =
            key.as_str().and_then(|k| renamed(Place::Section, k))
        {
            found.push(said);
        }
        // Every section of nodes is a list of types, each an entry of one
        // key, `type` and the type, holding a list of nodes; anything else in
        // the file has neither types nor links.
        let entries = value.as_sequence().into_iter().flatten();
        for entry in entries.filter_map(serde_norway::Value::as_mapping) {
            for (key, nodes) in entry {
                let typed = key.as_str().and_then(|k| k.strip_prefix("type "));
                if let Some(said) =
                    typed.and_then(|t| renamed(Place::NodeType, t.trim()))
                {
                    found.push(said);
                }
                let nodes = nodes.as_sequence().into_iter().flatten();
                for node in nodes.filter_map(serde_norway::Value::as_mapping) {
                    let keys = node.keys().filter_map(|key| key.as_str());
                    found.extend(keys.filter_map(|k| renamed(Place::Key, k)));
                }
            }
        }
    }
    found
        .into_iter()
        .map(|said| (yaml_line(text, said), said))
        .min_by_key(|(line, _)| *line)
}

/// Finds the line of a YAML file an old word is written on.
///
/// A scan and not a parse: the parse has already said the word is there, and
/// this only points at it. A comment is passed over, so that a line that only
/// talks about the word is not the one pointed at.
///
/// # Arguments
///
/// * `text` - The file's text.
/// * `said` - The old word, and where it is written.
///
/// # Returns
///
/// The first line it is on, from 1; or the first line of the file where it
/// cannot be found, which is where a reader opens it.
fn yaml_line(text: &str, said: &Renamed) -> usize {
    let quoted = |rest: &str, word: &str| -> Option<usize> {
        let rest = rest.trim_start_matches(['"', '\'']);
        let after = rest.strip_prefix(word)?;
        let after = after.trim_start_matches(['"', '\'']);
        Some(rest.len() - after.len())
    };
    for (at, line) in text.lines().enumerate() {
        let line = uncommented(line);
        let hit = match said.place {
            // A section is a key at the top, flush with the margin.
            Place::Section => quoted(line, said.old)
                .is_some_and(|n| line[n..].trim_start().starts_with(':')),
            // A key, anywhere a node writes one: after the margin, a list's
            // dash, or a flow mapping's brace or comma.
            Place::Key => keys(line).any(|key| key == said.old),
            // An entry's key: `- type moc:`, quoted or not.
            Place::NodeType => {
                let entry = line.trim_start().trim_start_matches("- ");
                entry.trim_end().strip_suffix(':').is_some_and(|key| {
                    unquoted(key)
                        .strip_prefix("type ")
                        .is_some_and(|t| t.trim() == said.old)
                })
            }
        };
        if hit {
            return at + 1;
        }
    }
    1
}

/// A line of YAML without its comment, if it has one.
///
/// # Arguments
///
/// * `line` - The line.
///
/// # Returns
///
/// What is before a `#` that begins a comment: one at the start of the line
/// or after a space, and outside quotes.
pub(crate) fn uncommented(line: &str) -> &str {
    let mut quote = None;
    let mut before = ' ';
    for (at, c) in line.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            (None, '#') if before.is_whitespace() => return &line[..at],
            _ => {}
        }
        before = c;
    }
    line
}

/// The keys a line of YAML writes, and what each says, where it says it on
/// the same line, unquoted.
///
/// # Arguments
///
/// * `line` - The line, without its comment.
///
/// # Returns
///
/// Each key and the value after it, which is empty for a key whose value is
/// on the lines below.
pub(crate) fn keys_and_values(
    line: &str,
) -> impl Iterator<Item = (&str, &str)> {
    line.split(['{', ',', '}'])
        .filter_map(|part| {
            let part = part.trim_start().trim_start_matches("- ").trim_start();
            let (key, value) = part.split_once(':')?;
            Some((unquoted(key), unquoted(value)))
        })
        .filter(|(key, _)| !key.is_empty() && !key.contains(' '))
}

/// A word without the quotes and the spaces around it.
///
/// # Arguments
///
/// * `word` - The word, as a line writes it.
///
/// # Returns
///
/// The word itself.
fn unquoted(word: &str) -> &str {
    word.trim().trim_matches(['"', '\'']).trim()
}

/// The keys a line of YAML writes.
///
/// # Arguments
///
/// * `line` - The line, without its comment.
///
/// # Returns
///
/// Each key.
fn keys(line: &str) -> impl Iterator<Item = &str> {
    keys_and_values(line).map(|(key, _)| key)
}

/// Finds the first old word an XML file writes, by line.
///
/// The configuration is the root element; its sections are the elements in
/// it, their types the `<type>` elements in those, named by an attribute,
/// and a node's links and attributes the elements in the node.
///
/// # Arguments
///
/// * `text` - The file's text.
///
/// # Returns
///
/// The line, from 1, and the word; or `None` where there is none or the
/// text is not well-formed XML.
fn in_xml(text: &str) -> Option<(usize, &'static Renamed)> {
    let mut reader = quick_xml::Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let line_at = |at: u64| {
        let at = usize::try_from(at).unwrap_or(usize::MAX).min(text.len());
        text.as_bytes()[..at]
            .iter()
            .filter(|b| **b == b'\n')
            .count()
            + 1
    };
    let mut depth = 0usize;
    loop {
        let event = reader.read_event().ok()?;
        let at = line_at(reader.buffer_position());
        // A section is a child of the root, a type a child of a section, a
        // node a child of a type, and a node's links and attributes children
        // of the node.
        let element = |tag: &quick_xml::events::BytesStart, depth: usize| {
            let name = tag.name().as_ref().to_string();
            match depth {
                2 => renamed(Place::Section, &name),
                3 if name == "type" => {
                    tag.attributes().flatten().find_map(|attribute| {
                        (attribute.key.as_ref() == "name")
                            .then(|| {
                                renamed(Place::NodeType, attribute.value.trim())
                            })
                            .flatten()
                    })
                }
                5 => renamed(Place::Key, &name),
                _ => None,
            }
        };
        match event {
            Event::Start(tag) => {
                depth += 1;
                if let Some(said) = element(&tag, depth) {
                    return Some((at, said));
                }
            }
            Event::Empty(tag) => {
                if let Some(said) = element(&tag, depth + 1) {
                    return Some((at, said));
                }
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
            }
            Event::Eof => return None,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A configuration in the words it has now, in YAML, with `old` put in
    /// place of `new` on the line that says it.
    ///
    /// # Arguments
    ///
    /// * `new` - A line of the file as it is written now.
    /// * `old` - The same line in the old words.
    ///
    /// # Returns
    ///
    /// The file's text.
    fn yaml_with(new: &str, old: &str) -> String {
        let text = "description: a file\n\
                    gateways:\n  \
                    - type gateway_radio:\n      \
                    - gateway: down\n        \
                    world-in: [readings]\n        \
                    moc-out:\n          \
                    - sensor.asked\n          \
                    - echo.asked\n          \
                    - drop.asked\n          \
                    - app.asked\n\
                    payloads:\n  \
                    - type i2c:\n      \
                    - { payload: sensor, moc-in: [asked] }\n\
                    vehicle:\n  \
                    - type tee-gateway:\n      \
                    - { compute: echo, moc-in: [asked] }\n  \
                    - type null-gateway:\n      \
                    - { compute: drop, moc-in: [asked] }\n  \
                    - type app_gateway_veh:\n      \
                    - compute: app\n        \
                    moc-in: [asked]\n        \
                    world-out: [down.readings]\n        \
                    app-gateway-arg: \"1\"\n";
        assert!(text.contains(new), "{new}");
        text.replacen(new, old, 1)
    }

    /// The same in XML.
    ///
    /// # Arguments
    ///
    /// * `new` - A piece of the file as it is written now.
    /// * `old` - The same in the old words.
    ///
    /// # Returns
    ///
    /// The file's text.
    fn xml_with(new: &str, old: &str) -> String {
        let text = "<?xml version=\"1.0\"?>\n\
                    <configuration>\n\
                    <description>a file</description>\n\
                    <gateways>\n\
                    <type name=\"gateway_radio\">\n\
                    <gateway name=\"down\">\n\
                    <world-in>readings</world-in>\n\
                    <moc-out>sensor.asked</moc-out>\n\
                    <moc-out>echo.asked</moc-out>\n\
                    <moc-out>drop.asked</moc-out>\n\
                    <moc-out>app.asked</moc-out>\n\
                    </gateway>\n\
                    </type>\n\
                    </gateways>\n\
                    <payloads>\n\
                    <type name=\"i2c\">\n\
                    <payload name=\"sensor\">\n\
                    <moc-in>asked</moc-in>\n\
                    </payload>\n\
                    </type>\n\
                    </payloads>\n\
                    <vehicle>\n\
                    <type name=\"tee-gateway\"><compute name=\"echo\">\
                    <moc-in>asked</moc-in></compute></type>\n\
                    <type name=\"null-gateway\"><compute name=\"drop\">\
                    <moc-in>asked</moc-in></compute></type>\n\
                    <type name=\"app_gateway_veh\"><compute name=\"app\">\n\
                    <moc-in>asked</moc-in>\n\
                    <world-out>down.readings</world-out>\n\
                    <app-gateway-arg>1</app-gateway-arg></compute></type>\n\
                    </vehicle>\n\
                    </configuration>\n";
        assert!(text.contains(new), "{new}");
        text.replace(new, old)
    }

    /// The line a piece of text is on, from 1.
    ///
    /// # Arguments
    ///
    /// * `text`  - The whole text.
    /// * `piece` - What to find in it, which must be there.
    ///
    /// # Returns
    ///
    /// The line it starts on.
    fn line_of(text: &str, piece: &str) -> usize {
        text[..text.find(piece).expect("it is there")]
            .matches('\n')
            .count()
            + 1
    }

    /// The files as written now are not refused, in either spelling: the
    /// check is for the old words and nothing else.
    #[test]
    fn a_file_in_the_new_words_is_taken() {
        assert_eq!(refused(&yaml_with("", ""), ConfigFormat::Yaml), None);
        assert_eq!(refused(&xml_with("", ""), ConfigFormat::Xml), None);

        // And loads, so that a file that does not is refused for its old word
        // and for nothing else.
        let dir = tempfile::tempdir().expect("a directory");
        for (extension, text) in
            [("ytcs", yaml_with("", "")), ("xtcs", xml_with("", ""))]
        {
            let path = dir.path().join(format!("new.{extension}"));
            std::fs::write(&path, text).expect("written");
            crate::graph::Graph::load(&path)
                .unwrap_or_else(|e| panic!("{extension}: {e}"));
        }
    }

    /// Each old word, in YAML, is refused at its line with the new word.
    #[test]
    fn each_old_word_is_refused_in_yaml_with_the_new_one() {
        for (new, old, word, now) in [
            ("gateways:\n", "moc:\n", "the moc section", "gateways"),
            (
                "type gateway_radio:",
                "type moc:",
                "the node type moc",
                "gateway_radio",
            ),
            (
                "type tee-gateway:",
                "\"type tee-moc\":",
                "the node type tee-moc",
                "tee-gateway",
            ),
            (
                "type null-gateway:",
                "type null-moc:",
                "the node type null-moc",
                "null-gateway",
            ),
            (
                "type app_gateway_veh:",
                "type app_moc_veh:",
                "the node type app_moc_veh",
                "app_gateway_veh",
            ),
            (
                "moc-in: [asked] }",
                "gateway-in: [asked] }",
                "gateway-in",
                "moc-in",
            ),
            ("moc-out:", "gateway-out:", "gateway-out", "moc-out"),
            ("world-in:", "payload-in:", "payload-in", "world-in"),
            ("world-out:", "payload-out:", "payload-out", "world-out"),
            (
                "app-gateway-arg:",
                "app-moc-arg:",
                "app-moc-arg",
                "app-gateway-arg",
            ),
        ] {
            let text = yaml_with(new, old);
            let said = refused(&text, ConfigFormat::Yaml)
                .unwrap_or_else(|| panic!("{old} should be refused"));
            let line = line_of(&text, old);
            assert!(
                said.starts_with(&format!(
                    "at line {line}, {word} is called {now} now"
                )),
                "{said}"
            );
            assert!(said.contains("now: rename it"), "{said}");
        }
    }

    /// Each old word, in XML, is refused at its line with the new word.
    #[test]
    fn each_old_word_is_refused_in_xml_with_the_new_one() {
        for (new, old, word, now) in [
            ("gateways>", "moc>", "the moc section", "gateways"),
            (
                "<type name=\"gateway_radio\">",
                "<type name=\"moc\">",
                "the node type moc",
                "gateway_radio",
            ),
            (
                "<type name=\"tee-gateway\">",
                "<type name=\" tee-moc \">",
                "the node type tee-moc",
                "tee-gateway",
            ),
            (
                "<type name=\"null-gateway\">",
                "<type name=\"null-moc\">",
                "the node type null-moc",
                "null-gateway",
            ),
            (
                "<type name=\"app_gateway_veh\">",
                "<type name=\"app_moc_veh\">",
                "the node type app_moc_veh",
                "app_gateway_veh",
            ),
            ("moc-in>", "gateway-in>", "gateway-in", "moc-in"),
            ("moc-out>", "gateway-out>", "gateway-out", "moc-out"),
            ("world-in>", "payload-in>", "payload-in", "world-in"),
            ("world-out>", "payload-out>", "payload-out", "world-out"),
            (
                "app-gateway-arg>",
                "app-moc-arg>",
                "app-moc-arg",
                "app-gateway-arg",
            ),
        ] {
            let text = xml_with(new, old);
            let said = refused(&text, ConfigFormat::Xml)
                .unwrap_or_else(|| panic!("{old} should be refused"));
            let line = line_of(&text, old);
            assert!(
                said.starts_with(&format!(
                    "at line {line}, {word} is called {now} now"
                )),
                "{said}"
            );
        }
    }

    /// An old word that is only talked about -- in a comment, a description
    /// or a node's name -- is not a word the file writes, and is taken.
    #[test]
    fn an_old_word_only_talked_about_is_taken() {
        let yaml = format!(
            "# moc: was the section, and type: moc the radio\n\
             description: \"moc: type: moc, gateway-in: true\"\n{}",
            yaml_with("gateway: down", "gateway: moc")
        );
        assert_eq!(refused(&yaml, ConfigFormat::Yaml), None);

        let xml = xml_with("<gateway name=\"down\">", "<gateway name=\"moc\">")
            .replace(
                "<description>a file</description>",
                "<!-- <moc><type>moc</type><gateway-in/></moc> -->\n\
             <description>&lt;moc&gt; type moc</description>",
            );
        assert_eq!(refused(&xml, ConfigFormat::Xml), None);
    }

    /// Every old word, in both spellings, stops the file from loading, and
    /// the complaint names the file, the line and the word to write: the
    /// whole of the rule, as a program reading a configuration meets it.
    #[test]
    fn a_file_in_an_old_word_does_not_load_and_is_told_the_new_one() {
        let dir = tempfile::tempdir().expect("a directory");
        let mut asked = 0;
        for said in RENAMED {
            for (extension, text) in
                [("ytcs", yaml_with_old(said)), ("xtcs", xml_with_old(said))]
            {
                let path = dir.path().join(format!("old.{extension}"));
                std::fs::write(&path, &text).expect("written");
                let why = crate::graph::Graph::load(&path)
                    .expect_err("an old word is refused")
                    .to_string();
                assert!(why.contains(&path.display().to_string()), "{why}");
                assert!(
                    why.contains(&format!(
                        "at line {}, ",
                        line_of(&text, &old_piece(said, extension))
                    )),
                    "{why}"
                );
                assert!(
                    why.contains(&format!(
                        " is called {} now: rename it{}",
                        said.new, said.then
                    )),
                    "{why}"
                );
                asked += 1;
            }
        }
        assert_eq!(asked, 2 * RENAMED.len());
    }

    /// The piece of a file that writes an old word, as the two helpers below
    /// write it.
    ///
    /// # Arguments
    ///
    /// * `said`      - The old word.
    /// * `extension` - `ytcs` or `xtcs`.
    ///
    /// # Returns
    ///
    /// The piece, to be found in the file.
    fn old_piece(said: &Renamed, extension: &str) -> String {
        match (said.place, extension) {
            (Place::Section, "ytcs") => format!("{}:", said.old),
            (Place::NodeType, "ytcs") => format!("type {}:", said.old),
            (Place::Key, "ytcs") => format!("{}:", said.old),
            (Place::Section, _) => format!("<{}>", said.old),
            (Place::NodeType, _) => format!("<type name=\"{}\">", said.old),
            (Place::Key, _) => format!("<{}", said.old),
        }
    }

    /// The YAML file with one old word in it.
    ///
    /// # Arguments
    ///
    /// * `said` - The old word.
    ///
    /// # Returns
    ///
    /// The file's text.
    fn yaml_with_old(said: &Renamed) -> String {
        match said.place {
            Place::Section => yaml_with("gateways:\n", "moc:\n"),
            Place::NodeType => yaml_with(
                &format!("type {}:", said.new),
                &format!("type {}:", said.old),
            ),
            Place::Key => {
                yaml_with(&format!("{}:", said.new), &format!("{}:", said.old))
            }
        }
    }

    /// The XML file with one old word in it.
    ///
    /// # Arguments
    ///
    /// * `said` - The old word.
    ///
    /// # Returns
    ///
    /// The file's text.
    fn xml_with_old(said: &Renamed) -> String {
        match said.place {
            Place::Section => xml_with("gateways>", "moc>"),
            Place::NodeType => xml_with(
                &format!("<type name=\"{}\">", said.new),
                &format!("<type name=\"{}\">", said.old),
            ),
            Place::Key => {
                xml_with(&format!("<{}", said.new), &format!("<{}", said.old))
                    .replace(
                        &format!("</{}", said.new),
                        &format!("</{}", said.old),
                    )
            }
        }
    }

    /// A comment that mentions the word on an earlier line does not move
    /// the line pointed at.
    #[test]
    fn a_comment_is_not_the_line_pointed_at() {
        let text = format!(
            "# Each node is told gateway-out: below\n{}",
            yaml_with("moc-out:", "gateway-out:")
        );
        let said = refused(&text, ConfigFormat::Yaml).expect("refused");
        let line = line_of(&text, "    gateway-out:");
        assert!(said.starts_with(&format!("at line {line}, ")), "{said}");
    }
}
