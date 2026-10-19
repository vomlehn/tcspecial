//! The examples in `examples/` are checked as tcsverify checks any pair.
//!
//! Each example is a directory of `examples/` holding a tcspecial
//! configuration, `<name>.yaml` and the same in XML, `<name>.xml`, and the
//! simulator configuration beside it, `<name>-sim.yaml`. They are discovered
//! rather than listed, so an example added to the directory is checked
//! without anyone having to remember to name it here.

use std::path::{Path, PathBuf};

use tcslibgs::graph::Graph;
use tcsverify::verify;

const SIM_SUFFIX: &str = "-sim.yaml";

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples")
}

fn entries(dir: &Path) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.expect("a readable directory entry").path())
}

/// Every file of every example directory with the given extension.
fn example_files(extension: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = entries(&examples_dir())
        .filter(|path| path.is_dir())
        .flat_map(|dir| entries(&dir).collect::<Vec<_>>())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .collect();
    files.sort();
    files
}

fn yaml_files() -> Vec<PathBuf> {
    example_files("yaml")
}

fn is_sim(path: &Path) -> bool {
    path.to_str().is_some_and(|p| p.ends_with(SIM_SUFFIX))
}

fn sim_for(config: &Path) -> PathBuf {
    let stem = config.file_stem().and_then(|s| s.to_str()).expect("a name");
    config.with_file_name(format!("{stem}{SIM_SUFFIX}"))
}

/// Every example is a pair, and nothing is found wrong or unfinished in
/// either file. Warnings count: an example is something to copy, and one
/// that is not finished teaches somebody to leave theirs unfinished.
#[test]
fn every_example_verifies() {
    let configs: Vec<PathBuf> = yaml_files().into_iter().filter(|p| !is_sim(p)).collect();
    assert!(
        !configs.is_empty(),
        "no examples in {}",
        examples_dir().display()
    );

    for config in configs {
        let sim = sim_for(&config);
        assert!(
            sim.is_file(),
            "{} has no {}",
            config.display(),
            sim.display()
        );

        let report = verify(&config, Some(&sim), tcsverify::Limits::EVERYTHING);
        let findings: Vec<String> = report.findings.iter().map(|f| f.to_string()).collect();
        assert!(
            findings.is_empty(),
            "{}:\n{}",
            config.display(),
            findings.join("\n")
        );
        assert!(
            report.joined,
            "{} was not checked against {}",
            sim.display(),
            config.display()
        );
    }
}

/// Every example is written in both spellings, and the two read as one
/// configuration.
///
/// Each is parsed by its own parser, and a change made to one and not the
/// other leaves both files valid: nothing but this notices.
#[test]
fn every_example_reads_the_same_in_both_formats() {
    for yaml in yaml_files().into_iter().filter(|p| !is_sim(p)) {
        let xml = yaml.with_extension("xml");
        assert!(
            xml.is_file(),
            "{} has no {} beside it: an example is written in both formats",
            yaml.display(),
            xml.display()
        );

        // Where each was read from is the one thing the two differ in by
        // rights.
        let load = |path: &Path| {
            let mut graph = Graph::load(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            graph.files.clear();
            graph.homes.clear();
            graph
        };
        assert_eq!(
            load(&xml),
            load(&yaml),
            "{} and {} describe different configurations",
            xml.display(),
            yaml.display()
        );
    }
}

/// And every XML example has the YAML it is a spelling of, which is what the
/// tests above discover examples by.
#[test]
fn every_xml_example_has_its_yaml() {
    for xml in example_files("xml") {
        let yaml = xml.with_extension("yaml");
        assert!(
            yaml.is_file(),
            "{} has no {}",
            xml.display(),
            yaml.display()
        );
    }
}

/// And every simulator file has a configuration to go with it. A lone one is
/// usually a configuration renamed without its partner.
#[test]
fn every_simulator_file_has_its_configuration() {
    for sim in yaml_files().into_iter().filter(|p| is_sim(p)) {
        let name = sim.to_str().expect("a UTF-8 path");
        let config = PathBuf::from(format!("{}.yaml", &name[..name.len() - SIM_SUFFIX.len()]));
        assert!(
            config.is_file(),
            "{} has no {}",
            sim.display(),
            config.display()
        );
    }
}

/// Every example has a diagram, `<name>.svg`, and the diagram names every node
/// the configuration has.
///
/// The diagrams are drawn by hand, so nothing else keeps them true: a node
/// added to an example and not to its picture fails here, naming both.
#[test]
fn every_example_has_a_diagram_of_its_nodes() {
    for config in yaml_files().into_iter().filter(|p| !is_sim(p)) {
        let svg = config.with_extension("svg");
        let drawn = std::fs::read_to_string(&svg).unwrap_or_else(|e| {
            panic!(
                "{} has no diagram, {}: {e}",
                config.display(),
                svg.display()
            )
        });
        let graph = Graph::load(&config).unwrap_or_else(|e| panic!("{}: {e}", config.display()));
        for node in &graph.nodes {
            assert!(
                drawn.contains(&format!(">{}<", node.name)),
                "{} does not show the node \"{}\" of {}",
                svg.display(),
                node.name,
                config.display()
            );
        }
    }
}

/// And every example has a section in the manual, which shows its diagram.
#[test]
fn every_example_is_in_the_manual() {
    let manual = Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/manual.rst");
    let text =
        std::fs::read_to_string(&manual).unwrap_or_else(|e| panic!("{}: {e}", manual.display()));
    for config in yaml_files().into_iter().filter(|p| !is_sim(p)) {
        let name = config.file_stem().and_then(|s| s.to_str()).expect("a name");
        let figure = format!(".. figure:: ../examples/{name}/{name}.svg");
        assert!(
            text.contains(&figure),
            "{} has no section in {} showing its diagram: no \"{figure}\"",
            config.display(),
            manual.display()
        );
    }
}
