//! Which configuration a start runs, and loading a configuration by its
//! checksum.
//!
//! tcspecial is started with a configuration on its command line, which
//! provisioning installed and made read-only. That configuration's
//! `tcspecial` section says where the state file is, where configurations
//! are kept and how many tries one is given: what a start needs before it
//! can choose. A start -- a child of the supervisor -- runs the top of the
//! configuration stack the state file keeps, which is the command line's
//! where nothing is stacked above it. It does not fall back: a top that will
//! not load or check fails the start, and the supervisor pops it and starts
//! a fresh child on what is beneath (see [`crate::runtime::supervisor`]),
//! so that one rule gives every configuration up, whatever failed. A state
//! file that is missing or damaged names nothing, and the start runs the
//! command line's. See "The supervisor and the configuration stack" in
//! `docs/tcspecial-design.rst`.
//!
//! [`Boot::load`] is tcspecial's function that loads a configuration by its
//! checksum: a start calls it for the top of the stack, and a RESTART calls
//! it just before it is taken, to check that the configuration it names
//! would load.

use std::path::{Path, PathBuf};

use tcsprotocol::config_digest::{digest_of_graph, ConfigDigest};
use tcsprotocol::graph::Graph;
use tcsprotocol::store::Store;

use crate::state;

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

/// Why a start could not choose, and whether what it could not run is the
/// command line's configuration, which nothing is beneath.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// Each problem, a line beginning with the file it is about.
    pub problems: Vec<String>,
    /// Whether it is the command line's configuration that cannot run: a
    /// refusal that is the same at every start, which no other
    /// configuration is there to stand in for.
    pub of_the_bottom: bool,
}

impl Refused {
    /// A refusal of the command line's configuration.
    ///
    /// # Arguments
    ///
    /// * `problems` - What is wrong with it.
    ///
    /// # Returns
    ///
    /// The refusal.
    fn of_the_bottom(problems: Vec<String>) -> Refused {
        Refused {
            problems,
            of_the_bottom: true,
        }
    }

    /// A refusal of what is stacked above it, which the supervisor pops.
    ///
    /// # Arguments
    ///
    /// * `problems` - What is wrong with it.
    ///
    /// # Returns
    ///
    /// The refusal.
    fn of_the_top(problems: Vec<String>) -> Refused {
        Refused {
            problems,
            of_the_bottom: false,
        }
    }
}

/// Says what is wrong with a configuration, as a start checks it.
///
/// The same checks the ground tool runs, against the same code, and whether
/// this build contains every type the configuration has, which is
/// tcs-verify's `--types` with the types this was built with. A
/// configuration that arrived here unchecked is exactly the configuration
/// that should not open a bus.
///
/// # Arguments
///
/// * `path`  - The file it was read from, which each problem is said of.
/// * `graph` - What it says.
///
/// # Returns
///
/// Every problem that stops it, each a line beginning with `path`; none
/// where it may run. One that does not stop it is [`warnings_of`]'s.
pub fn problems_of(path: &Path, graph: &Graph) -> Vec<String> {
    graph
        .check()
        .iter()
        .filter(|problem| problem.refused_aboard())
        .chain(
            tcsprotocol::graph::built::not_built(
                &graph.nodes,
                crate::node::payload::BUILT,
            )
            .iter(),
        )
        .map(|problem| format!("{}: {}", path.display(), problem.describe()))
        .collect()
}

/// Says what is unfinished in a configuration that a start runs all the
/// same: a string or bytes with no most (see
/// [`tcsprotocol::graph::check::Problem::refused_aboard`]).
///
/// # Arguments
///
/// * `path`  - The file it was read from, which each warning is said of.
/// * `graph` - What it says.
///
/// # Returns
///
/// Each, a line beginning with `path`, for the log.
pub fn warnings_of(path: &Path, graph: &Graph) -> Vec<String> {
    graph
        .check()
        .iter()
        .filter(|problem| !problem.refused_aboard())
        .map(|problem| format!("{}: {}", path.display(), problem.describe()))
        .collect()
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
                tcsprotocol::graph::parameters::ci_config_of(
                    &tcsprotocol::graph::parameters::flatten(section),
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
                    .map_or(tcsprotocol::LOAD_TRIALS_DEFAULT, |ci| {
                        ci.load_trials
                    }),
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

/// Chooses the configuration this start runs, says why, and clears the
/// state file's word that this start was commanded, which is said once.
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
/// configuration not loading or having problems, where nothing is stacked
/// above it; the top of the stack not loading or having problems; or the
/// state file not being written.
pub fn choose(path: &Path) -> Result<Chosen, Refused> {
    let (boot, command_line) =
        Boot::of_command_line(path).map_err(Refused::of_the_bottom)?;
    state::set_bottom(boot.command_line_digest);
    let Some(state_file) = boot.state_file.clone() else {
        // A configuration with no tcspecial section takes no commands, so
        // nothing can have named another: it runs what it was given.
        let problems = problems_of(path, &command_line);
        if !problems.is_empty() {
            return Err(Refused::of_the_bottom(problems));
        }
        let notes = warnings_of(path, &command_line);
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
            notes,
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

    let was = &found.state;
    let (loaded, why) = match was.top() {
        Some(top) => {
            let loaded = boot.load(top).map_err(|failed| {
                Refused::of_the_top(vec![format!(
                    "{}: configuration {top}, the top of the configuration \
                     stack, cannot be run: {failed}",
                    path.display()
                )])
            })?;
            (
                loaded,
                if was.commanded {
                    "it is the top of the configuration stack, pushed by the \
                     RESTART that commanded this start"
                } else {
                    "it is the top of the configuration stack"
                }
                .to_string(),
            )
        }
        None => {
            let problems = problems_of(path, &command_line);
            if !problems.is_empty() {
                return Err(Refused::of_the_bottom(problems));
            }
            (
                Loaded {
                    path: path.to_path_buf(),
                    graph: command_line,
                    digest: boot.command_line_digest,
                },
                "it is the configuration on tcspecial's command line, the \
                 bottom of the configuration stack, with nothing above it"
                    .to_string(),
            )
        }
    };

    if was.commanded {
        state::update(&state_file, |state| state.commanded = false).map_err(
            |why| {
                Refused::of_the_top(vec![format!("{}: {why}", path.display())])
            },
        )?;
    }

    notes.extend(warnings_of(&loaded.path, &loaded.graph));
    Ok(Chosen {
        loaded,
        why,
        commanded: was.commanded,
        boot,
        notes,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::state::State;

    /// A vehicle on a desk: a command-line configuration naming a state file
    /// and a store, both in a directory of the test's. The command
    /// interpreter's tests use it too, for a vehicle that holds more than
    /// it runs.
    pub(crate) struct Desk {
        dir: tempfile::TempDir,
        pub(crate) command_line: PathBuf,
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
        pub(crate) fn new(trials: u32) -> Desk {
            let dir = tempfile::tempdir().expect("a directory");
            std::fs::create_dir(dir.path().join("kept")).expect("a store");
            let command_line = dir.path().join("vehicle.ytcs");
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
  payload_command: {{ source_base: \"127.0.0.1:3\", dest_base: \
            \"127.0.0.1:4\" }}
  payload_telemetry: {{ source_base: \"127.0.0.1:5\", dest_base: \
            \"127.0.0.1:6\" }}
  beacon: {{ default_interval: \"1 s\", reset_interval: \"1 s\", send: \
            \"127.0.0.1:7\", recv: \"127.0.0.1:8\" }}
  state_file: \"{}\"
  max_consolidator_inputs: \"32\"
  configurations: \"{}\"
  load_trials: \"{trials}\"
gateways:
  - type gateway_radio:
      - gateway: g
        address: \"127.0.0.1:{}\"
        ground_base: \"127.0.0.1:10\"
        moc-out: [p.g]
        world-in: [p]
vehicle:
  - type ping:
      - compute: p
        world-out: [g.p]
        moc-in: [g]
",
                tcsprotocol::InterfaceDigest::of_this_build(),
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
        pub(crate) fn keep(&self, n: u32) -> ConfigDigest {
            let path = self.dir.path().join(format!("other{n}.ytcs"));
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
    /// and writes nothing: nothing is stacked, and it was not commanded.
    #[test]
    fn a_first_start_runs_the_command_lines() {
        let desk = Desk::new(3);
        let chosen = choose(&desk.command_line).expect("it chooses");
        assert_eq!(chosen.loaded.path, desk.command_line);
        assert!(chosen.why.contains("command line"), "{}", chosen.why);
        assert!(chosen.why.contains("bottom"), "{}", chosen.why);
        assert!(!chosen.commanded);
        assert_eq!(desk.state(), State::default());
    }

    /// The top of the stack is the one loaded, from the store, and a
    /// commanded start says so, once.
    #[test]
    fn the_top_of_the_stack_is_loaded_from_the_store() {
        let desk = Desk::new(3);
        let beneath = desk.keep(1);
        let top = desk.keep(2);
        desk.write(State {
            stack: vec![beneath, top],
            trials: 1,
            commanded: true,
        });
        let chosen = choose(&desk.command_line).expect("it chooses");
        assert_eq!(chosen.loaded.digest, top);
        assert_eq!(
            chosen.loaded.path,
            desk.dir.path().join(format!("kept/{top}/{top}.ytcs"))
        );
        assert!(chosen.commanded);
        assert!(chosen.why.contains("RESTART"), "{}", chosen.why);
        assert!(!desk.state().commanded, "said once");
        assert_eq!(desk.state().stack, vec![beneath, top], "nothing popped");
        assert_eq!(desk.state().trials, 1, "the supervisor counts them");
    }

    /// A top that will not load -- not kept, or damaged -- fails the start,
    /// naming it, rather than running what is beneath: the supervisor pops
    /// it and starts afresh.
    #[test]
    fn a_top_that_will_not_load_fails_the_start() {
        let desk = Desk::new(3);
        let beneath = desk.keep(1);
        let missing = ConfigDigest([0x77; 16]);
        desk.write(State {
            stack: vec![beneath, missing],
            trials: 0,
            commanded: false,
        });
        let why = choose(&desk.command_line).expect_err("it fails");
        assert!(!why.of_the_bottom, "{why:?}");
        let why = &why.problems;
        assert!(why[0].contains(&missing.to_string()), "{why:?}");
        assert!(why[0].contains("top of the configuration stack"), "{why:?}");
        assert_eq!(desk.state().stack, vec![beneath, missing], "not popped");
    }

    /// A damaged state file names nothing: the command line's runs, and the
    /// damage is said.
    #[test]
    fn a_damaged_state_file_runs_the_command_lines() {
        let desk = Desk::new(3);
        let other = desk.keep(1);
        desk.write(State {
            stack: vec![other],
            trials: 0,
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

        let kept = desk.dir.path().join(format!("kept/{other}/{other}.ytcs"));
        let mut perms = std::fs::metadata(&kept).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        std::fs::set_permissions(&kept, perms).unwrap();
        std::fs::write(&kept, Desk::configuration(&desk.dir, 9, 3)).unwrap();
        let why = boot.load(other).expect_err("damaged");
        assert!(why.contains("not the"), "{why}");
    }
}
