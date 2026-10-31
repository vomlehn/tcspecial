//! Which configuration a start runs, and loading a configuration by its
//! checksum.
//!
//! tcspecial is started with a configuration on its command line, which
//! provisioning installed and made read-only. That configuration's
//! `tcspecial` section says where the state file is, where configurations
//! are kept and how many tries one is given: what a start needs before it
//! can choose. Every start then loads the first of these that passes its
//! check:
//!
//! 1. the configuration the state file names for it -- the one the last
//!    taken RESTART named, or else the one the start before ran -- unless
//!    `load_trials` of its starts have ended before a matching CONNECT;
//! 2. else the *confirmed* configuration, the last one a matching CONNECT
//!    was taken on;
//! 3. else the configuration on its command line.
//!
//! Whichever it loads, it says in the log which and why, and writes the
//! state file before its first beacon: the configuration it runs is what
//! the next start loads, and this start is counted as one of its trials. A
//! state file that is missing or damaged names nothing, and the start loads
//! the configuration on its command line. See "Starting, and the restart
//! flag" in `docs/design.rst`.
//!
//! [`Boot::load`] is tcspecial's function that loads a configuration by its
//! checksum: a start calls it for each configuration the state file names,
//! and a RESTART calls it just before it is taken, to check that the
//! configuration it names would load.

use std::path::{Path, PathBuf};

use tcslibgs::config_digest::{digest_of_graph, ConfigDigest};
use tcslibgs::graph::Graph;
use tcslibgs::store::{single_file, Store};

use crate::state::{self, State};

/// What a start knows from the configuration on its command line before it
/// chooses: where to find what it may choose from.
#[derive(Debug, Clone)]
pub struct Boot {
    /// The configuration on the command line.
    pub command_line: PathBuf,
    /// Its checksum.
    pub command_line_digest: ConfigDigest,
    /// The state file its `tcspecial` section names, or `None` where it has
    /// no section: such a configuration takes no commands, so nothing can
    /// name another.
    pub state_file: Option<PathBuf>,
    /// Where configurations are kept, where its section says.
    pub store: Option<Store>,
    /// How many starts the configuration the state file names is given.
    pub load_trials: u32,
}

/// A configuration loaded and checked, ready to run.
#[derive(Debug, Clone)]
pub struct Loaded {
    /// The file it was read from.
    pub path: PathBuf,
    /// What it says.
    pub graph: Graph,
    /// Its checksum, computed from it.
    pub digest: ConfigDigest,
}

/// What a start chose, and why.
#[derive(Debug, Clone)]
pub struct Chosen {
    /// The configuration it runs.
    pub loaded: Loaded,
    /// Why that one, in words for the log: which of the three it was.
    pub why: String,
    /// Whether the ground commanded this start: the start before took an
    /// armed RESTART.
    pub commanded: bool,
    /// What it chose from, which a RESTART asks again.
    pub boot: Boot,
    /// What went wrong on the way, each for the log as a warning: a damaged
    /// state file, a configuration given up or that would not load.
    pub notes: Vec<String>,
}

/// Says what is wrong with a configuration, as a start checks it.
///
/// The same checks the ground tool runs, against the same code, and whether
/// this build contains every type the configuration has, which is
/// tcsverify's `--types` with the types this was built with. A
/// configuration that arrived here unchecked is exactly the configuration
/// that should not open a bus. And for now a configuration is a single
/// file: see [`single_file`].
///
/// # Arguments
///
/// * `path`  - The file it was read from, which each problem is said of.
/// * `graph` - What it says.
///
/// # Returns
///
/// Every problem, each a line beginning with `path`; none where it may run.
pub fn problems_of(path: &Path, graph: &Graph) -> Vec<String> {
    let mut problems: Vec<String> = graph
        .check()
        .iter()
        .chain(
            tcslibgs::graph::built::not_built(
                &graph.nodes,
                crate::node::payload::BUILT,
            )
            .iter(),
        )
        .map(|problem| format!("{}: {}", path.display(), problem.describe()))
        .collect();
    if let Err(why) = single_file(path, graph) {
        problems.push(why);
    }
    problems
}

impl Boot {
    /// Reads what a start needs from the configuration on its command line.
    ///
    /// # Arguments
    ///
    /// * `path` - The configuration on the command line.
    ///
    /// # Returns
    ///
    /// What the start chooses from, and the command line's configuration,
    /// loaded but not yet checked.
    ///
    /// # Errors
    ///
    /// Each a line beginning with `path`: the file not loading, its checksum
    /// not computing, or its `tcspecial` section not reading.
    pub fn of_command_line(path: &Path) -> Result<(Boot, Graph), Vec<String>> {
        let said = |why: String| vec![format!("{}: {why}", path.display())];
        let graph = Graph::load(path).map_err(|e| said(e.to_string()))?;
        let command_line_digest =
            digest_of_graph(&graph).map_err(|e| said(e.to_string()))?;
        let ci = match graph.tcspecial.as_ref() {
            Some(section) => Some(
                tcslibgs::graph::parameters::ci_config_of(
                    &tcslibgs::graph::parameters::flatten(section),
                )
                .and_then(|json| json.to_ci_config())
                .map_err(said)?,
            ),
            None => None,
        };
        Ok((
            Boot {
                command_line: path.to_path_buf(),
                command_line_digest,
                state_file: ci.as_ref().map(|ci| PathBuf::from(&ci.state_file)),
                store: ci
                    .as_ref()
                    .and_then(|ci| ci.configurations.as_ref())
                    .map(Store::at),
                load_trials: ci
                    .as_ref()
                    .map_or(tcslibgs::LOAD_TRIALS_DEFAULT, |ci| ci.load_trials),
            },
            graph,
        ))
    }

    /// Says whether a configuration of this checksum is held: it is the one
    /// on the command line, or is kept in the store. Only whether it is
    /// there: [`Boot::load`] is what checks it.
    ///
    /// # Arguments
    ///
    /// * `digest` - The configuration's checksum.
    ///
    /// # Returns
    ///
    /// Whether a start could look for it.
    pub fn holds(&self, digest: ConfigDigest) -> bool {
        digest == self.command_line_digest
            || self.store.as_ref().is_some_and(|store| store.holds(digest))
    }

    /// Loads the configuration whose checksum is `digest`, and checks it as
    /// a start would run it: tcspecial's function that loads a
    /// configuration by its checksum.
    ///
    /// The one on the command line is read from there, and any other from
    /// the store, whose checksum is computed again from its file and must
    /// be its name. Either is then checked by [`problems_of`].
    ///
    /// Allocates: it reads and checks a whole configuration. A start calls
    /// it before its loop, and a RESTART once, just before the process ends.
    ///
    /// # Arguments
    ///
    /// * `digest` - The configuration's checksum.
    ///
    /// # Returns
    ///
    /// The configuration, loaded and checked.
    ///
    /// # Errors
    ///
    /// Why it cannot run, in words for the log: not held, not loading, of
    /// another checksum than its name, or with problems.
    pub fn load(&self, digest: ConfigDigest) -> Result<Loaded, String> {
        let (graph, path) = if digest == self.command_line_digest {
            let graph = Graph::load(&self.command_line)
                .map_err(|e| format!("{}: {e}", self.command_line.display()))?;
            let computed =
                digest_of_graph(&graph).map_err(|e| e.to_string())?;
            if computed != digest {
                return Err(format!(
                    "{} has changed since tcspecial started: its checksum is \
                     {computed}, not {digest}",
                    self.command_line.display()
                ));
            }
            (graph, self.command_line.clone())
        } else {
            match self.store.as_ref() {
                Some(store) => store.load(digest)?,
                None => {
                    return Err(format!(
                        "no configuration {digest} is held: this vehicle keeps \
                         no configurations, its configuration on the command \
                         line saying no configurations directory, and runs \
                         {}",
                        self.command_line_digest
                    ))
                }
            }
        };
        let problems = problems_of(&path, &graph);
        if !problems.is_empty() {
            return Err(problems.join("; "));
        }
        Ok(Loaded {
            path,
            graph,
            digest,
        })
    }
}

/// Chooses the configuration this start runs, says why, and writes the
/// state file so that it is what the next start loads.
///
/// # Arguments
///
/// * `path` - The configuration on tcspecial's command line.
///
/// # Returns
///
/// What it chose, and why.
///
/// # Errors
///
/// Each a line beginning with the file it is about: the command line's
/// configuration not loading or having problems, where nothing the state
/// file names could be loaded instead, or the state file not being written.
pub fn choose(path: &Path) -> Result<Chosen, Vec<String>> {
    let (boot, command_line) = Boot::of_command_line(path)?;
    let Some(state_file) = boot.state_file.clone() else {
        // A configuration with no tcspecial section takes no commands, so
        // nothing can have named another: it runs what it was given.
        let problems = problems_of(path, &command_line);
        if !problems.is_empty() {
            return Err(problems);
        }
        return Ok(Chosen {
            loaded: Loaded {
                path: path.to_path_buf(),
                graph: command_line,
                digest: boot.command_line_digest,
            },
            why: "it is the configuration on tcspecial's command line, which \
                  has no tcspecial section and so no state file"
                .to_string(),
            commanded: false,
            boot,
            notes: Vec::new(),
        });
    };

    let found = state::read(&state_file);
    let mut notes = Vec::new();
    if let Some(why) = &found.damaged {
        notes.push(format!(
            "the state file was not one tcspecial can read ({why}), so it \
             names nothing, and this start is taken for one nobody commanded"
        ));
    }
    if let Some(generation) = found.dropped_generation {
        notes.push(format!(
            "the state file {} was written by a build that counted \
             generations, and said generation {generation}: there is no \
             generation now, so it was dropped",
            state_file.display()
        ));
    }

    // The configurations the state file names, in the order they are
    // tried, each with why.
    let was = &found.state;
    let mut given_up = None;
    let mut candidates: Vec<(ConfigDigest, String)> = Vec::new();
    if let Some(next) = was.next {
        if was.trials >= boot.load_trials {
            notes.push(format!(
                "configuration {next} is given up: {} of its starts ended \
                 before a matching CONNECT, which is load_trials, so this \
                 start does not load it",
                was.trials
            ));
            given_up = Some(next);
        } else {
            candidates.push((
                next,
                if was.commanded {
                    "the state file names it for this start, as the RESTART \
                     that commanded it did"
                        .to_string()
                } else {
                    "the state file names it for this start, as the one the \
                     start before ran"
                        .to_string()
                },
            ));
        }
    }
    if let Some(confirmed) = was.confirmed {
        if Some(confirmed) != given_up
            && !candidates.iter().any(|(digest, _)| *digest == confirmed)
        {
            candidates.push((
                confirmed,
                "it is the confirmed configuration, the last one a matching \
                 CONNECT was taken on"
                    .to_string(),
            ));
        }
    }

    let mut chosen = None;
    for (digest, why) in candidates {
        match boot.load(digest) {
            Ok(loaded) => {
                chosen = Some((loaded, why));
                break;
            }
            Err(failed) => notes.push(format!(
                "configuration {digest}, which {why}, cannot be loaded, so \
                 this start does not run it: {failed}"
            )),
        }
    }
    let (loaded, why) = match chosen {
        Some(chosen) => chosen,
        None => {
            let problems = problems_of(path, &command_line);
            if !problems.is_empty() {
                return Err(problems);
            }
            let because = if was.next.is_none() && was.confirmed.is_none() {
                "the state file names nothing"
            } else {
                "no configuration the state file names could be run"
            };
            (
                Loaded {
                    path: path.to_path_buf(),
                    graph: command_line,
                    digest: boot.command_line_digest,
                },
                format!(
                    "it is the configuration on tcspecial's command line, \
                     since {because}"
                ),
            )
        }
    };

    // This start is one of the trials of what it runs: a start that ends
    // before a matching CONNECT leaves it counted, and the CONNECT sets it
    // back to nought.
    let trials = if was.next == Some(loaded.digest) && given_up.is_none() {
        was.trials.saturating_add(1)
    } else {
        1
    };
    state::write(
        &state_file,
        &State {
            next: Some(loaded.digest),
            trials,
            confirmed: was.confirmed,
            commanded: false,
        },
    )
    .map_err(|why| vec![format!("{}: {why}", path.display())])?;

    Ok(Chosen {
        loaded,
        why,
        commanded: was.commanded,
        boot,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A vehicle on a desk: a command-line configuration naming a state file
    /// and a store, both in a directory of the test's.
    struct Desk {
        dir: tempfile::TempDir,
        command_line: PathBuf,
    }

    impl Desk {
        /// Makes the desk, its command-line configuration given `trials`
        /// as its `load_trials`.
        ///
        /// # Arguments
        ///
        /// * `trials` - What `load_trials` says.
        ///
        /// # Returns
        ///
        /// The desk.
        fn new(trials: u32) -> Desk {
            let dir = tempfile::tempdir().expect("a directory");
            std::fs::create_dir(dir.path().join("kept")).expect("a store");
            let command_line = dir.path().join("vehicle.yaml");
            std::fs::write(&command_line, Desk::configuration(&dir, 0, trials))
                .expect("written");
            Desk { dir, command_line }
        }

        /// A configuration of this desk's, `n` telling it from the others.
        ///
        /// # Arguments
        ///
        /// * `dir`    - The desk's directory.
        /// * `n`      - Which one: its gateway node's port.
        /// * `trials` - What `load_trials` says.
        ///
        /// # Returns
        ///
        /// Its YAML.
        fn configuration(
            dir: &tempfile::TempDir,
            n: u32,
            trials: u32,
        ) -> String {
            format!(
                "description: \"{n}\"
tcspecial:
  gateway: radio
  interface: \"{}\"
  tcs_cmd: {{ recv: \"127.0.0.1:1\", send: \"127.0.0.1:2\" }}
  payload_command: {{ source_base: \"127.0.0.1:3\", dest_base: \"127.0.0.1:4\" }}
  payload_telemetry: {{ source_base: \"127.0.0.1:5\", dest_base: \"127.0.0.1:6\" }}
  beacon: {{ default_interval: \"1 s\", reset_interval: \"1 s\", send: \"127.0.0.1:7\", recv: \"127.0.0.1:8\" }}
  state_file: \"{}\"
  configurations: \"{}\"
  load_trials: \"{trials}\"
gateway:
  - {{ name: g, type: gateway_radio, address: \"127.0.0.1\", port: \"{}\", \
ground_base: \"127.0.0.1:10\", payload-in: true, gateway-out: p }}
vehicle:
  - {{ name: p, type: ping, gateway-in: true, payload-out: g }}
",
                tcslibgs::InterfaceDigest::of_this_build(),
                dir.path().join("tcspecial.state").display(),
                dir.path().join("kept").display(),
                100 + n,
            )
        }

        /// Keeps configuration `n` in the store.
        ///
        /// # Arguments
        ///
        /// * `n` - Which one.
        ///
        /// # Returns
        ///
        /// Its checksum.
        fn keep(&self, n: u32) -> ConfigDigest {
            let path = self.dir.path().join(format!("other{n}.yaml"));
            std::fs::write(&path, Desk::configuration(&self.dir, n, 3))
                .expect("written");
            Store::at(self.dir.path().join("kept"))
                .keep(&path)
                .expect("kept")
                .0
        }

        fn state_file(&self) -> PathBuf {
            self.dir.path().join("tcspecial.state")
        }

        fn state(&self) -> State {
            state::read(&self.state_file()).state
        }

        fn write(&self, state: State) {
            state::write(&self.state_file(), &state).expect("written");
        }
    }

    /// A first start, with no state file, runs the command line's, says so,
    /// and names it for the next start, as one trial of it.
    #[test]
    fn a_first_start_runs_the_command_lines_and_names_it() {
        let desk = Desk::new(3);
        let chosen = choose(&desk.command_line).expect("it chooses");
        assert_eq!(chosen.loaded.path, desk.command_line);
        assert!(chosen.why.contains("command line"), "{}", chosen.why);
        assert!(chosen.why.contains("names nothing"), "{}", chosen.why);
        assert!(!chosen.commanded);
        assert_eq!(
            desk.state(),
            State {
                next: Some(chosen.loaded.digest),
                trials: 1,
                confirmed: None,
                commanded: false,
            }
        );
    }

    /// The configuration the state file names is the one loaded, from the
    /// store, and a commanded start says so; its trial is counted.
    #[test]
    fn the_named_configuration_is_loaded_from_the_store() {
        let desk = Desk::new(3);
        let other = desk.keep(1);
        desk.write(State {
            next: Some(other),
            trials: 0,
            confirmed: None,
            commanded: true,
        });
        let chosen = choose(&desk.command_line).expect("it chooses");
        assert_eq!(chosen.loaded.digest, other);
        assert_eq!(
            chosen.loaded.path,
            desk.dir.path().join(format!("kept/{other}.yaml"))
        );
        assert!(chosen.commanded);
        assert!(chosen.why.contains("RESTART"), "{}", chosen.why);
        assert_eq!(desk.state().trials, 1);
        assert!(!desk.state().commanded, "said once");
    }

    /// A named configuration that will not load -- not kept, or damaged --
    /// gives way to the confirmed one, and saying why.
    #[test]
    fn one_that_will_not_load_gives_way_to_the_confirmed_one() {
        let desk = Desk::new(3);
        let confirmed = desk.keep(1);
        let missing = ConfigDigest([0x77; 16]);
        desk.write(State {
            next: Some(missing),
            trials: 0,
            confirmed: Some(confirmed),
            commanded: true,
        });
        let chosen = choose(&desk.command_line).expect("it chooses");
        assert_eq!(chosen.loaded.digest, confirmed);
        assert!(chosen.why.contains("confirmed"), "{}", chosen.why);
        assert!(
            chosen
                .notes
                .iter()
                .any(|note| note.contains(&missing.to_string())),
            "{:?}",
            chosen.notes
        );
        assert_eq!(desk.state().next, Some(confirmed));
        assert_eq!(desk.state().trials, 1, "a trial of the confirmed one");
    }

    /// A configuration whose starts keep ending before a matching CONNECT
    /// is given up after load_trials of them, for the confirmed one; and
    /// the confirmed one, given up in its turn, for the command line's.
    #[test]
    fn a_configuration_that_keeps_failing_is_given_up() {
        let desk = Desk::new(2);
        let confirmed = desk.keep(1);
        let trying = desk.keep(2);
        desk.write(State {
            next: Some(trying),
            trials: 0,
            confirmed: Some(confirmed),
            commanded: true,
        });
        // Two starts of it, each ending before a CONNECT.
        for trial in 1..=2 {
            let chosen = choose(&desk.command_line).expect("it chooses");
            assert_eq!(chosen.loaded.digest, trying, "trial {trial}");
            assert_eq!(desk.state().trials, trial);
        }
        // The third start gives it up.
        let chosen = choose(&desk.command_line).expect("it chooses");
        assert_eq!(chosen.loaded.digest, confirmed);
        assert!(
            chosen.notes.iter().any(|note| note.contains("given up")),
            "{:?}",
            chosen.notes
        );
        assert_eq!(desk.state().trials, 1);

        // And the confirmed one, failing as often, gives way to the
        // command line's.
        choose(&desk.command_line).expect("its second");
        let chosen = choose(&desk.command_line).expect("it chooses");
        assert_eq!(chosen.loaded.path, desk.command_line);
        assert!(chosen.why.contains("command line"), "{}", chosen.why);
    }

    /// A matching CONNECT sets the trials back to nought, so a configuration
    /// that runs until it is matched is never given up.
    #[test]
    fn a_confirmed_start_is_never_counted_against_it() {
        let desk = Desk::new(1);
        let first = choose(&desk.command_line).expect("it chooses");
        state::confirmed(&desk.state_file(), first.loaded.digest)
            .expect("confirmed");
        for _ in 0..3 {
            let chosen = choose(&desk.command_line).expect("it chooses");
            assert_eq!(chosen.loaded.digest, first.loaded.digest);
            state::confirmed(&desk.state_file(), chosen.loaded.digest)
                .expect("confirmed");
        }
    }

    /// A damaged state file names nothing: the command line's runs, and the
    /// damage is said.
    #[test]
    fn a_damaged_state_file_runs_the_command_lines() {
        let desk = Desk::new(3);
        let other = desk.keep(1);
        desk.write(State {
            next: Some(other),
            trials: 0,
            confirmed: Some(other),
            commanded: true,
        });
        let mut text = std::fs::read(desk.state_file()).unwrap();
        text[10] ^= 1;
        std::fs::write(desk.state_file(), text).unwrap();
        let chosen = choose(&desk.command_line).expect("it chooses");
        assert_eq!(chosen.loaded.path, desk.command_line);
        assert!(!chosen.commanded);
        assert!(
            chosen.notes.iter().any(|note| note.contains("CRC")),
            "{:?}",
            chosen.notes
        );
    }

    /// The load function refuses what is not held, and what the store holds
    /// under a name that is not its checksum.
    #[test]
    fn loading_by_checksum_refuses_what_is_not_what_it_says() {
        let desk = Desk::new(3);
        let (boot, _) = Boot::of_command_line(&desk.command_line).unwrap();
        let other = desk.keep(1);
        assert!(boot.holds(other));
        assert!(boot.holds(boot.command_line_digest));
        boot.load(other).expect("it loads");

        let missing = ConfigDigest([0x77; 16]);
        assert!(!boot.holds(missing));
        assert!(boot.load(missing).is_err());

        let kept = desk.dir.path().join(format!("kept/{other}.yaml"));
        let mut perms = std::fs::metadata(&kept).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        std::fs::set_permissions(&kept, perms).unwrap();
        std::fs::write(&kept, Desk::configuration(&desk.dir, 9, 3)).unwrap();
        let why = boot.load(other).expect_err("damaged");
        assert!(why.contains("not the"), "{why}");
    }
}
