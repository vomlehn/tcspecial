//! What the tests read out of the example's configuration through
//! `tcslibgs`: the Rust's answers, which the Python's and the Yamcs
//! wrapper's are held to.

use std::path::{Path, PathBuf};
use std::process::Command;

use quick_xml::escape::unescape;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use tcslibgs::config_digest::digest_of_file;
use tcslibgs::graph::parameters::{beacon_of, ci_config_of, flatten};
use tcslibgs::graph::reach::{apid_base_of, reach_of};
use tcslibgs::graph::telemetry::{self, Via};
use tcslibgs::graph::Graph;
use tcslibgs::EndpointConfig;

/// Where the example is.
///
/// # Returns
///
/// Its directory.
pub fn here() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// The address to reach a socket at from this host, as `ping_yamcs.py`'s
/// `local` gives it: Yamcs, on this host, reaches a socket bound on every
/// interface at this host's own address.
///
/// # Arguments
///
/// * `address` - The address the socket is bound at.
///
/// # Returns
///
/// The address to send to.
fn local(address: &str) -> String {
    match address {
        "0.0.0.0" | "::" | "" => "127.0.0.1".to_string(),
        other => other.to_string(),
    }
}

/// The Rust's answers to the same questions, from the YAML spelling.
///
/// # Returns
///
/// The ports, the APIDs and the digest, as `ping_yamcs.py --describe` names
/// them. Not the settings that are Yamcs's own -- its web port and its
/// directories -- which the configuration does not say.
///
/// # Panics
///
/// The example's configuration does not read or lacks any of these.
pub fn worked_out() -> Vec<(String, String)> {
    let path = here().join("ping-yamcs.ytcs");
    let graph = Graph::load(&path).expect("the example reads");

    let section = graph.tcspecial.as_ref().expect("a tcspecial section");
    let ci = ci_config_of(&flatten(section))
        .expect("its section reads")
        .to_ci_config()
        .expect("its section settles");
    let sockets = ci.sockets().expect("its links are sockets");
    let framing = sockets.framing.expect("its packets are framed");

    let beacon = beacon_of(&graph)
        .expect("its section reads")
        .expect("it has one");

    // The one gateway_yamcs node and the one ping node, as the Python finds
    // them: by their types.
    let one_of = |kind: &str| {
        let mut found =
            graph.nodes.iter().filter(|node| node.node_type == kind);
        let node = found.next().unwrap_or_else(|| panic!("a {kind} node"));
        assert!(found.next().is_none(), "one {kind} node");
        node
    };
    let gateway = one_of("gateway_yamcs");
    let ping = one_of("ping");

    let uplink = match reach_of(gateway).expect("it is reached").endpoint {
        EndpointConfig::Network(network) => network,
        other => panic!("a gateway node is a socket: {other:?}"),
    };
    let uplink_apid = apid_base_of(gateway)
        .expect("its apid_base reads")
        .expect("it has one");

    let answer = telemetry::of(&graph)
        .expect("its links to the ground work out")
        .into_iter()
        .find(|link| {
            link.source == ping.name
                && link.via == Via::Gateway(gateway.name.clone())
        })
        .expect("ping sends to the gateway node");
    let downlink = answer.ground().expect("a link over UDP").clone();
    let reply_apid = answer.apid.expect("its packets carry an APID");

    [
        ("cmd_host", local(&sockets.address)),
        ("cmd_port", sockets.port.to_string()),
        ("uplink_host", local(&uplink.address)),
        ("uplink_port", uplink.port.to_string()),
        ("downlink_port", downlink.port.to_string()),
        ("beacon_port", beacon.recv.port.to_string()),
        ("beacon_apid", framing.beacon_apid.to_string()),
        ("cmd_apid", framing.cmd_apid.to_string()),
        ("uplink_apid", uplink_apid.to_string()),
        ("reply_apid", reply_apid.to_string()),
        (
            "digest",
            digest_of_file(&path).expect("it digests").to_string(),
        ),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_string(), value))
    .collect()
}

/// One of the Rust's answers, by the name `ping_yamcs.py --describe`
/// gives it.
///
/// # Arguments
///
/// * `name` - Its name.
///
/// # Returns
///
/// The answer.
///
/// # Panics
///
/// There is no answer of that name.
// Each test file is a crate of its own that takes the share of this module
// it needs, so what one does not use is not dead.
#[allow(dead_code)]
pub fn worked_out_of(name: &str) -> String {
    worked_out()
        .into_iter()
        .find(|(said, _)| said == name)
        .map(|(_, value)| value)
        .unwrap_or_else(|| panic!("no {name} is worked out"))
}

/// One element of an XTCE document, with what is in it.
#[derive(Debug, Default)]
pub struct Element {
    /// Its name, as written, with any prefix.
    pub name: String,
    /// Its attributes, in order.
    pub attributes: Vec<(String, String)>,
    /// The elements in it, in order.
    pub children: Vec<Element>,
    /// Its text.
    pub text: String,
}

#[allow(dead_code)]
impl Element {
    /// One of its attributes.
    ///
    /// # Arguments
    ///
    /// * `name` - The attribute's name.
    ///
    /// # Returns
    ///
    /// Its value, or `None` where it has none of that name.
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(said, _)| said == name)
            .map(|(_, value)| value.as_str())
    }

    /// Its first child of a kind.
    ///
    /// # Arguments
    ///
    /// * `name` - The child's element name.
    ///
    /// # Returns
    ///
    /// The child, or `None`.
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|child| child.name == name)
    }

    /// Everything of a kind within it, at any depth, in document order.
    ///
    /// # Arguments
    ///
    /// * `name`  - The element name.
    /// * `found` - Where they are put.
    pub fn within<'a>(&'a self, name: &str, found: &mut Vec<&'a Element>) {
        for child in &self.children {
            if child.name == name {
                found.push(child);
            }
            child.within(name, found);
        }
    }

    /// The one element of a kind within it with a `name` attribute.
    ///
    /// # Arguments
    ///
    /// * `kind` - The element name.
    /// * `name` - Its `name` attribute.
    ///
    /// # Returns
    ///
    /// The element.
    ///
    /// # Panics
    ///
    /// There is none.
    pub fn named(&self, kind: &str, name: &str) -> &Element {
        let mut found = Vec::new();
        self.within(kind, &mut found);
        found
            .into_iter()
            .find(|element| element.attribute("name") == Some(name))
            .unwrap_or_else(|| panic!("the document has no {kind} {name}"))
    }
}

/// An element's name and attributes, as an [`Element`] with nothing in it.
///
/// # Arguments
///
/// * `start` - The element's start tag.
///
/// # Returns
///
/// The element, with no children or text.
fn opened(start: &BytesStart<'_>) -> Element {
    let text = |said: &str| unescape(said).expect("well escaped").into_owned();
    Element {
        name: text(start.name().as_ref()),
        attributes: start
            .attributes()
            .map(|attribute| {
                let attribute = attribute.expect("an attribute");
                (text(attribute.key.as_ref()), text(&attribute.value))
            })
            .collect(),
        ..Element::default()
    }
}

/// Reads an XTCE document.
///
/// # Arguments
///
/// * `text` - The document.
///
/// # Returns
///
/// Its root element.
///
/// # Panics
///
/// It is not XML.
#[allow(dead_code)]
pub fn read_xml(text: &str) -> Element {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut open = vec![Element::default()];
    loop {
        match reader.read_event().expect("the document is XML") {
            Event::Start(start) => open.push(opened(&start)),
            Event::Empty(start) => {
                let element = opened(&start);
                open.last_mut().expect("a parent").children.push(element);
            }
            Event::End(_) => {
                let element = open.pop().expect("an element");
                open.last_mut().expect("a parent").children.push(element);
            }
            Event::Text(said) => {
                let said: &str = said.as_ref();
                open.last_mut().expect("an element").text.push_str(said);
            }
            // An entity in text, `&lt;` in a comparison, comes apart from
            // the text around it.
            Event::GeneralRef(entity) => {
                let entity: &str = entity.as_ref();
                let said = unescape(&format!("&{entity};"))
                    .expect("an entity XML defines")
                    .into_owned();
                open.last_mut().expect("an element").text.push_str(&said);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut document = open.pop().expect("the document");
    assert!(open.is_empty(), "every element closed");
    document.children.pop().expect("a root element")
}

/// Runs Python with the tree's `python/` and the example's directory on its
/// path.
///
/// # Arguments
///
/// * `script`    - The program, which `sys.argv[1:]` gives `arguments` to.
/// * `arguments` - Its arguments.
///
/// # Returns
///
/// What it printed.
///
/// # Panics
///
/// python3 cannot be run, or the program fails.
pub fn python(script: &str, arguments: &[&Path]) -> String {
    let ran = Command::new("python3")
        .arg("-c")
        .arg(format!(
            "import sys\n\
             sys.path[:0] = [{:?}, {:?}]\n\
             {script}",
            here().join("../../python").display().to_string(),
            here().display().to_string()
        ))
        .args(arguments)
        .output()
        .expect("python3 runs");
    assert!(
        ran.status.success(),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    String::from_utf8(ran.stdout).expect("text")
}

/// The mission database's wrapper `tcslibgs/yamcs.py` generates for a
/// configuration, as `ping_yamcs.py` writes it for Yamcs.
///
/// # Arguments
///
/// * `configuration` - The configuration.
///
/// # Returns
///
/// The wrapper's root element.
///
/// # Panics
///
/// The Python fails, or what it writes is not XML.
#[allow(dead_code)]
pub fn generated_mdb(configuration: &Path) -> Element {
    let dir = tempfile::tempdir().expect("a directory");
    let written = dir.path().join("tcs-ccsds.xtce");
    python(
        "from pathlib import Path\n\
         from tcslibgs import configuration, yamcs\n\
         yamcs.write_mdb(configuration.load(sys.argv[1]), Path(sys.argv[2]))\n",
        &[configuration, &written],
    );
    read_xml(&std::fs::read_to_string(&written).expect("it was written"))
}

/// A copy of the example's configuration with a payload more drawn into
/// the gateway node, "pong", and two payloads nothing touches, "sensor" and
/// "tank": `test_ping_yamcs.with_more_nodes`, the one place it is written.
///
/// # Arguments
///
/// * `dir` - Where to write it.
///
/// # Returns
///
/// The copy's path.
///
/// # Panics
///
/// The Python fails.
#[allow(dead_code)]
pub fn with_more_nodes(dir: &Path) -> PathBuf {
    let said = python(
        "from pathlib import Path\n\
         import test_ping_yamcs\n\
         print(test_ping_yamcs.with_more_nodes(Path(sys.argv[1])))\n",
        &[dir],
    );
    PathBuf::from(said.trim())
}
