//! The words a configuration used to say, and the words it says now.
//!
//! The side of the graph that commands tcspecial and takes its telemetry was
//! called `moc` while the ground was the only thing that could be there. It is
//! the gateway now, because a cFS or an F´ flight computer can stand where the
//! ground stood -- see "Gateways: what the vehicle answers to" in
//! `docs/design.rst` -- and every word a file wrote for that side was renamed
//! with it.
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
    /// A node's `type`.
    NodeType,
    /// A key of a node: one of its links, or what it is told.
    Key,
}

/// One old word, where it was written, and the word for it now.
struct Renamed {
    old: &'static str,
    place: Place,
    new: &'static str,
}

/// Every word that was renamed, and the word it is now: the one list, which
/// the reader and its tests both read.
const RENAMED: &[Renamed] = &[
    Renamed {
        old: "moc",
        place: Place::Section,
        new: "gateway",
    },
    Renamed {
        old: "moc",
        place: Place::NodeType,
        new: "gateway_radio",
    },
    Renamed {
        old: "tee-moc",
        place: Place::NodeType,
        new: "tee-gateway",
    },
    Renamed {
        old: "null-moc",
        place: Place::NodeType,
        new: "null-gateway",
    },
    Renamed {
        old: "app_moc_veh",
        place: Place::NodeType,
        new: "app_gateway_veh",
    },
    Renamed {
        old: "moc-in",
        place: Place::Key,
        new: "gateway-in",
    },
    Renamed {
        old: "moc-out",
        place: Place::Key,
        new: "gateway-out",
    },
    Renamed {
        old: "app-moc-arg",
        place: Place::Key,
        new: "app-gateway-arg",
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
        "at line {line}, {what} is called {} now: rename it",
        said.new
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
        // Every section of nodes is a list of them; anything else in the file
        // has neither types nor links.
        let nodes = value.as_sequence().into_iter().flatten();
        for node in nodes.filter_map(serde_norway::Value::as_mapping) {
            for (key, value) in node {
                let Some(key) = key.as_str() else {
                    continue;
                };
                if let Some(said) = renamed(Place::Key, key) {
                    found.push(said);
                }
                let typed = value.as_str().filter(|_| key == "type");
                if let Some(said) =
                    typed.and_then(|t| renamed(Place::NodeType, t.trim()))
                {
                    found.push(said);
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
            Place::NodeType => keys_and_values(line)
                .any(|(key, value)| key == "type" && value == said.old),
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
fn uncommented(line: &str) -> &str {
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
fn keys_and_values(line: &str) -> impl Iterator<Item = (&str, &str)> {
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
/// it, one for each node, and a node's type and links are elements in that.
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
    // Inside a node's `type` element, where its text is the type.
    let mut in_type = false;
    loop {
        let event = reader.read_event().ok()?;
        let at = line_at(reader.buffer_position());
        let element = |name: &str, depth: usize| match depth {
            2 => renamed(Place::Section, name),
            3 => renamed(Place::Key, name),
            _ => None,
        };
        match event {
            Event::Start(tag) => {
                depth += 1;
                if let Some(said) = element(tag.name().as_ref(), depth) {
                    return Some((at, said));
                }
                in_type = depth == 3 && tag.name().as_ref() == "type";
            }
            Event::Empty(tag) => {
                if let Some(said) = element(tag.name().as_ref(), depth + 1) {
                    return Some((at, said));
                }
            }
            Event::Text(said) if in_type => {
                let said = said.into_inner();
                if let Some(said) = renamed(Place::NodeType, said.trim()) {
                    return Some((at, said));
                }
            }
            Event::End(_) => {
                in_type = false;
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
                    gateway:\n  \
                    - name: down\n    \
                    type: gateway_radio\n    \
                    gateway-out:\n      \
                    - target: sensor\n      \
                    - target: echo\n      \
                    - target: drop\n      \
                    - target: app\n\
                    payloads:\n  \
                    - { name: sensor, type: i2c, gateway-in: true }\n\
                    vehicle:\n  \
                    - { name: echo, type: tee-gateway, gateway-in: true }\n  \
                    - { name: drop, type: null-gateway, gateway-in: true }\n  \
                    - name: app\n    \
                    type: app_gateway_veh\n    \
                    gateway-in: true\n    \
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
                    <gateway>\n\
                    <name>down</name>\n\
                    <type>gateway_radio</type>\n\
                    <gateway-out><target>sensor</target></gateway-out>\n\
                    <gateway-out><target>echo</target></gateway-out>\n\
                    <gateway-out><target>drop</target></gateway-out>\n\
                    <gateway-out><target>app</target></gateway-out>\n\
                    </gateway>\n\
                    <payloads>\n\
                    <name>sensor</name><type>i2c</type>\n\
                    <gateway-in>true</gateway-in>\n\
                    </payloads>\n\
                    <vehicle><name>echo</name><type>tee-gateway</type>\
                    <gateway-in>true</gateway-in></vehicle>\n\
                    <vehicle><name>drop</name><type>null-gateway</type>\
                    <gateway-in>true</gateway-in></vehicle>\n\
                    <vehicle><name>app</name><type>app_gateway_veh</type>\n\
                    <gateway-in>true</gateway-in>\n\
                    <app-gateway-arg>1</app-gateway-arg></vehicle>\n\
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
            [("yaml", yaml_with("", "")), ("xml", xml_with("", ""))]
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
            ("gateway:\n", "moc:\n", "the moc section", "gateway"),
            (
                "type: gateway_radio",
                "type: moc",
                "the node type moc",
                "gateway_radio",
            ),
            (
                "type: tee-gateway",
                "type: \"tee-moc\"",
                "the node type tee-moc",
                "tee-gateway",
            ),
            (
                "type: null-gateway",
                "type: null-moc",
                "the node type null-moc",
                "null-gateway",
            ),
            (
                "type: app_gateway_veh",
                "type: app_moc_veh",
                "the node type app_moc_veh",
                "app_gateway_veh",
            ),
            (
                "gateway-in: true }",
                "moc-in: true }",
                "moc-in",
                "gateway-in",
            ),
            ("gateway-out:", "moc-out:", "moc-out", "gateway-out"),
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
            assert!(said.ends_with("rename it"), "{said}");
        }
    }

    /// Each old word, in XML, is refused at its line with the new word.
    #[test]
    fn each_old_word_is_refused_in_xml_with_the_new_one() {
        for (new, old, word, now) in [
            ("gateway>", "moc>", "the moc section", "gateway"),
            (
                "<type>gateway_radio</type>",
                "<type>moc</type>",
                "the node type moc",
                "gateway_radio",
            ),
            (
                "<type>tee-gateway</type>",
                "<type> tee-moc </type>",
                "the node type tee-moc",
                "tee-gateway",
            ),
            (
                "<type>null-gateway</type>",
                "<type>null-moc</type>",
                "the node type null-moc",
                "null-gateway",
            ),
            (
                "<type>app_gateway_veh</type>",
                "<type>app_moc_veh</type>",
                "the node type app_moc_veh",
                "app_gateway_veh",
            ),
            ("gateway-in>", "moc-in>", "moc-in", "gateway-in"),
            (
                "<gateway-in>true</gateway-in>\n</payloads>",
                "<moc-in/>\n</payloads>",
                "moc-in",
                "gateway-in",
            ),
            ("gateway-out>", "moc-out>", "moc-out", "gateway-out"),
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
             description: \"moc: type: moc, moc-in: true\"\n{}",
            yaml_with("name: down", "name: moc")
        );
        assert_eq!(refused(&yaml, ConfigFormat::Yaml), None);

        let xml = xml_with("<name>down</name>", "<name>moc</name>").replace(
            "<description>a file</description>",
            "<!-- <moc><type>moc</type><moc-in/></moc> -->\n\
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
                [("yaml", yaml_with_old(said)), ("xml", xml_with_old(said))]
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
                        " is called {} now: rename it",
                        said.new
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
    /// * `extension` - `yaml` or `xml`.
    ///
    /// # Returns
    ///
    /// The piece, to be found in the file.
    fn old_piece(said: &Renamed, extension: &str) -> String {
        match (said.place, extension) {
            (Place::Section, "yaml") => format!("{}:", said.old),
            (Place::NodeType, "yaml") => format!("type: {}", said.old),
            (Place::Key, "yaml") => format!("{}:", said.old),
            (Place::Section, _) => format!("<{}>", said.old),
            (Place::NodeType, _) => format!("<type>{}</type>", said.old),
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
            Place::Section => yaml_with("gateway:\n", "moc:\n"),
            Place::NodeType => yaml_with(
                &format!("type: {}", said.new),
                &format!("type: {}", said.old),
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
            Place::Section => xml_with("gateway>", "moc>"),
            Place::NodeType => xml_with(
                &format!("<type>{}</type>", said.new),
                &format!("<type>{}</type>", said.old),
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
            "# Each node is told moc-out: below\n{}",
            yaml_with("gateway-out:", "moc-out:")
        );
        let said = refused(&text, ConfigFormat::Yaml).expect("refused");
        let line = line_of(&text, "    moc-out:");
        assert!(said.starts_with(&format!("at line {line}, ")), "{said}");
    }
}
