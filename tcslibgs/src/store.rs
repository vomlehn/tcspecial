//! Configurations kept on board, named by their checksum.
//!
//! The vehicle keeps each configuration it holds in the directory the
//! `tcspecial` section's `configurations` attribute names, under its
//! checksum in hex and the extension that says how to read it:
//! `<checksum>.yaml` or `<checksum>.xml`. A RESTART names the configuration
//! to restart onto by that checksum, and a start loads one by it. See
//! "Keeping configurations on board" in `docs/design.rst`.
//!
//! The rules about what may be kept and what may be loaded are here, once,
//! for tcspecial, which loads from the store, and tcsverify, which puts a
//! configuration into it for testing and first provisioning:
//!
//! * a configuration is a single file, and one that includes another is
//!   refused, both when it is kept and when it is loaded;
//! * the name it is kept under is the checksum computed from it, never one
//!   anybody gives it;
//! * keeping one whose checksum is already held writes nothing, so a
//!   configuration once kept is never overwritten;
//! * its checksum is computed again from its file each time it is loaded,
//!   and one whose checksum is not its name is not loaded, so that a write
//!   cut short by a crash or a power loss is refused rather than flown.
//!
//! Uplinking a configuration by CFDP, and the provisioning crate that is to
//! receive, check, keep and remove them in flight, are not built: see the
//! design.

use std::path::{Path, PathBuf};

use crate::config_digest::{digest_of_graph, ConfigDigest};
use crate::graph::Graph;

/// The extensions a kept configuration may have, which say how it is read:
/// the configuration formats, as [`crate::ConfigFormat`] takes them.
const EXTENSIONS: [&str; 2] = ["yaml", "xml"];

/// The directory configurations are kept in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    dir: PathBuf,
}

/// What [`Store::keep`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kept {
    /// The configuration was written to the store, at this path.
    Written(PathBuf),
    /// A configuration of the same checksum was held already, at this path,
    /// and nothing was written: it means the same.
    Already(PathBuf),
}

impl Kept {
    /// Where the configuration is kept, whichever was done.
    ///
    /// # Returns
    ///
    /// The kept file's path.
    pub fn path(&self) -> &Path {
        match self {
            Kept::Written(path) | Kept::Already(path) => path,
        }
    }
}

/// Refuses a configuration that is more than one file.
///
/// For now a configuration is a single file: one that includes another is
/// refused, saying so, when it is kept and when it is loaded. A
/// configuration spread across several files is to be handled later, and
/// until it is, the checksum a store names a file by must be the checksum of
/// that file alone, with nothing it reaches into from beside it.
///
/// # Arguments
///
/// * `path`  - The configuration file.
/// * `graph` - What it loads as.
///
/// # Errors
///
/// A message naming the file and the files it includes.
pub fn single_file(path: &Path, graph: &Graph) -> Result<(), String> {
    if graph.files.len() <= 1 {
        return Ok(());
    }
    let included: Vec<String> = graph.files[1..]
        .iter()
        .map(|file| file.display().to_string())
        .collect();
    Err(format!(
        "{} includes {}, and a configuration kept on board is a single \
         file: write what it includes into it, and drop its include list",
        path.display(),
        included.join(", ")
    ))
}

impl Store {
    /// The store in `dir`, which the `tcspecial` section's `configurations`
    /// names.
    ///
    /// # Arguments
    ///
    /// * `dir` - The directory.
    ///
    /// # Returns
    ///
    /// The store. Nothing is read until it is asked something.
    pub fn at(dir: impl Into<PathBuf>) -> Store {
        Store { dir: dir.into() }
    }

    /// The directory the store is.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Finds the file a configuration is kept in, by its checksum.
    ///
    /// # Arguments
    ///
    /// * `digest` - The configuration's checksum.
    ///
    /// # Returns
    ///
    /// The file, `<checksum>.yaml` or `<checksum>.xml`, or `None` where
    /// neither is there. A store holds a checksum once, so where both are,
    /// the YAML is the one found.
    pub fn find(&self, digest: ConfigDigest) -> Option<PathBuf> {
        EXTENSIONS
            .iter()
            .map(|ext| self.dir.join(format!("{digest}.{ext}")))
            .find(|path| path.is_file())
    }

    /// Says whether the store holds a configuration of this checksum. Only
    /// whether a file of that name is there: [`Store::load`] is what checks
    /// it.
    ///
    /// # Arguments
    ///
    /// * `digest` - The configuration's checksum.
    ///
    /// # Returns
    ///
    /// Whether one is kept under it.
    pub fn holds(&self, digest: ConfigDigest) -> bool {
        self.find(digest).is_some()
    }

    /// Loads the configuration kept under `digest`, checking that it is
    /// what its name says.
    ///
    /// Its checksum is computed again from its file, and one whose checksum
    /// is not its name is refused: a write cut short, storage that went bad,
    /// or a file somebody put there by hand under a name it does not have.
    /// One that includes another file is refused too. Nothing else is
    /// checked here; a start checks it as it checks any configuration.
    ///
    /// # Arguments
    ///
    /// * `digest` - The configuration's checksum.
    ///
    /// # Returns
    ///
    /// The configuration, and the file it was read from.
    ///
    /// # Errors
    ///
    /// Why it cannot be loaded: not held, not readable, more than one file,
    /// or of another checksum than its name.
    pub fn load(
        &self,
        digest: ConfigDigest,
    ) -> Result<(Graph, PathBuf), String> {
        let path = self.find(digest).ok_or_else(|| {
            format!(
                "no configuration {digest} is kept in {}: there is no \
                 {digest}.yaml or {digest}.xml there",
                self.dir.display()
            )
        })?;
        let graph = Graph::load(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        single_file(&path, &graph)?;
        let computed = digest_of_graph(&graph)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if computed != digest {
            return Err(format!(
                "{} has checksum {computed}, not the {digest} it is kept \
                 under: it was damaged, or cut short as it was written, and \
                 is not loaded",
                path.display()
            ));
        }
        Ok((graph, path))
    }

    /// Keeps the configuration in the file at `path`, under its checksum.
    ///
    /// The checksum is computed here, from what the file says, and is the
    /// name the file is kept under: a name anybody gives it is not trusted.
    /// A checksum already held writes nothing, since the configuration held
    /// already means the same, so a configuration once kept is never
    /// overwritten. One written is written to a file beside its name,
    /// flushed and renamed into place, so that a power failure part way
    /// leaves no file of that name rather than half of one, and is made
    /// read-only, mode 0444.
    ///
    /// What makes a configuration fit to fly is not decided here: tcsverify
    /// checks it before it keeps it, and the vehicle checks it again as it
    /// loads it.
    ///
    /// # Arguments
    ///
    /// * `path` - The configuration file, `.yaml` or `.xml`.
    ///
    /// # Returns
    ///
    /// Its checksum, and whether it was written or was held already.
    ///
    /// # Errors
    ///
    /// Why it was not kept: it does not load, it includes another file, its
    /// extension is not one a configuration has, or the store cannot be
    /// written.
    pub fn keep(&self, path: &Path) -> Result<(ConfigDigest, Kept), String> {
        let graph = Graph::load(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        single_file(path, &graph)?;
        let digest = digest_of_graph(&graph)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if let Some(held) = self.find(digest) {
            return Ok((digest, Kept::Already(held)));
        }
        let ext = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .map(|ext| {
                if ext == "yml" {
                    "yaml".to_string()
                } else {
                    ext
                }
            })
            .filter(|ext| EXTENSIONS.contains(&ext.as_str()))
            .ok_or_else(|| {
                format!(
                    "{}: a configuration is kept as .yaml or .xml, and this \
                     is neither",
                    path.display()
                )
            })?;
        let kept = self.dir.join(format!("{digest}.{ext}"));
        let cannot = |what: &str, e: std::io::Error| {
            format!(
                "{} cannot be {what} in {} ({e}): the configurations \
                 attribute must name a directory that can be written",
                path.display(),
                self.dir.display()
            )
        };
        let bytes = std::fs::read(path)
            .map_err(|e| format!("{} cannot be read: {e}", path.display()))?;
        let beside = self.dir.join(format!(".{digest}.{ext}.new"));
        write_read_only(&beside, &bytes).map_err(|e| cannot("written", e))?;
        std::fs::rename(&beside, &kept).map_err(|e| cannot("kept", e))?;
        std::fs::File::open(&self.dir)
            .and_then(|dir| dir.sync_all())
            .map_err(|e| cannot("flushed", e))?;
        Ok((digest, Kept::Written(kept)))
    }

    /// Lists the checksums the store holds, from the names of its files.
    ///
    /// # Returns
    ///
    /// Each checksum once, in order.
    ///
    /// # Errors
    ///
    /// Why the directory cannot be read.
    pub fn held(&self) -> Result<Vec<ConfigDigest>, String> {
        let entries = std::fs::read_dir(&self.dir).map_err(|e| {
            format!("{} cannot be read: {e}", self.dir.display())
        })?;
        let mut held: Vec<ConfigDigest> = entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                let ext = path.extension()?.to_str()?;
                if !EXTENSIONS.contains(&ext) {
                    return None;
                }
                ConfigDigest::of_hex(path.file_stem()?.to_str()?)
            })
            .collect();
        held.sort_by_key(|digest| digest.0);
        held.dedup();
        Ok(held)
    }

    /// Removes the configuration kept under `digest`, if the policy lets it.
    ///
    /// Which configurations are kept is the operators' policy; what is
    /// enforced here is the floor under it: never the configurations in
    /// `protected` -- the one running and the one a taken RESTART named --
    /// and never one that would leave fewer than `kept_at_least` held, the
    /// `tcspecial` section's `configurations_kept`. Removing one not held is
    /// what was asked for already, and succeeds.
    ///
    /// For the provisioning crate, which is not yet built, and for tests:
    /// nothing in flight removes a configuration yet.
    ///
    /// # Arguments
    ///
    /// * `digest`        - The checksum of the configuration to remove.
    /// * `protected`     - Configurations that may not be removed.
    /// * `kept_at_least` - How many must be held afterwards.
    ///
    /// # Errors
    ///
    /// Why it was not removed, in words that name the rule.
    pub fn remove(
        &self,
        digest: ConfigDigest,
        protected: &[ConfigDigest],
        kept_at_least: u32,
    ) -> Result<(), String> {
        let Some(path) = self.find(digest) else {
            return Ok(());
        };
        if protected.contains(&digest) {
            return Err(format!(
                "configuration {digest} is running, or is the one a RESTART \
                 named, and is not removed"
            ));
        }
        let held = self.held()?.len();
        if held <= kept_at_least as usize {
            return Err(format!(
                "configuration {digest} is not removed: {held} are held, and \
                 configurations_kept says at least {kept_at_least} must be"
            ));
        }
        std::fs::remove_file(&path)
            .map_err(|e| format!("{} cannot be removed: {e}", path.display()))
    }
}

/// Writes `bytes` to a new file at `path`, flushed, and read-only.
///
/// # Arguments
///
/// * `path`  - The file, which must not exist.
/// * `bytes` - What it holds.
///
/// # Errors
///
/// What the system said, where it cannot be written.
fn write_read_only(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    file.set_permissions(std::fs::Permissions::from_mode(0o444))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A configuration of one file, with nothing but its section, which is
    /// enough to have a checksum: `n`, in its node's name, makes each one
    /// different.
    ///
    /// # Arguments
    ///
    /// * `dir` - Where to write it.
    /// * `n`   - Which one.
    /// * `ext` - `yaml` or `xml`.
    ///
    /// # Returns
    ///
    /// Its path.
    fn written(dir: &Path, n: u32, ext: &str) -> PathBuf {
        let path = dir.join(format!("vehicle{n}.{ext}"));
        let text = if ext == "xml" {
            format!(
                "<configuration><description>{n}</description>\
                 <vehicle><name>p{n}</name><type>ping</type></vehicle>\
                 </configuration>\n"
            )
        } else {
            format!(
                "description: \"{n}\"\nvehicle:\n  - {{ name: p{n}, type: ping \
                 }}\n"
            )
        };
        std::fs::write(&path, text).expect("written");
        path
    }

    /// A configuration is kept under its checksum, read-only, and loads by
    /// it; keeping it again writes nothing.
    #[test]
    fn a_configuration_is_kept_under_its_checksum_and_loads_by_it() {
        use std::os::unix::fs::PermissionsExt;

        let from = tempfile::tempdir().expect("a directory");
        let store_dir = tempfile::tempdir().expect("a store");
        let store = Store::at(store_dir.path());
        let path = written(from.path(), 1, "yaml");
        let digest = crate::config_digest::digest_of_file(&path).unwrap();

        assert!(!store.holds(digest));
        let (kept_as, kept) = store.keep(&path).expect("kept");
        assert_eq!(kept_as, digest, "the checksum computed from it");
        let at = store_dir.path().join(format!("{digest}.yaml"));
        assert_eq!(kept, Kept::Written(at.clone()));
        assert!(store.holds(digest));
        let mode = std::fs::metadata(&at).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o444, "read-only");
        assert_eq!(store.held().unwrap(), vec![digest]);

        let (graph, read_from) = store.load(digest).expect("it loads");
        assert_eq!(read_from, at);
        assert_eq!(digest_of_graph(&graph).unwrap(), digest);

        // Again, and in the other spelling of the same configuration:
        // nothing is written, since the one held means the same.
        let again = store.keep(&path).expect("kept");
        assert_eq!(again, (digest, Kept::Already(at.clone())));
        let xml = written(from.path(), 1, "xml");
        assert_eq!(
            crate::config_digest::digest_of_file(&xml).unwrap(),
            digest,
            "the two spellings say the same"
        );
        assert_eq!(store.keep(&xml).unwrap(), (digest, Kept::Already(at)));
        assert!(!store_dir.path().join(format!("{digest}.xml")).exists());
    }

    /// A file kept under a name that is not its checksum -- damaged, cut
    /// short, or put there by hand -- is not loaded, and says why.
    #[test]
    fn a_file_whose_checksum_is_not_its_name_is_not_loaded() {
        let from = tempfile::tempdir().expect("a directory");
        let store_dir = tempfile::tempdir().expect("a store");
        let store = Store::at(store_dir.path());
        let one = written(from.path(), 1, "yaml");
        let two = written(from.path(), 2, "yaml");
        let digest = crate::config_digest::digest_of_file(&one).unwrap();
        std::fs::copy(&two, store_dir.path().join(format!("{digest}.yaml")))
            .unwrap();

        assert!(store.holds(digest), "a file of the name is there");
        let why = store.load(digest).expect_err("but it is another");
        assert!(why.contains("not the"), "{why}");
        assert!(why.contains(&digest.to_string()), "{why}");

        let missing = ConfigDigest([7; 16]);
        let why = store.load(missing).expect_err("nothing of that name");
        assert!(why.contains(&missing.to_string()), "{why}");
    }

    /// A configuration that includes another file is refused, kept or
    /// loaded.
    #[test]
    fn a_configuration_of_more_than_one_file_is_refused() {
        let from = tempfile::tempdir().expect("a directory");
        let store_dir = tempfile::tempdir().expect("a store");
        let store = Store::at(store_dir.path());
        std::fs::write(
            from.path().join("cabin.yaml"),
            "vehicle:\n  - { name: p, type: ping }\n",
        )
        .unwrap();
        let top = from.path().join("top.yaml");
        std::fs::write(&top, "include:\n  - cabin\n").unwrap();

        let why = store.keep(&top).expect_err("more than one file");
        assert!(why.contains("includes"), "{why}");
        assert!(why.contains("single"), "{why}");
        assert!(store.held().unwrap().is_empty(), "and nothing was kept");
    }

    /// Removing keeps the floor the policy sets: never a protected one,
    /// never below `configurations_kept`.
    #[test]
    fn removing_never_goes_below_what_must_be_kept() {
        let from = tempfile::tempdir().expect("a directory");
        let store_dir = tempfile::tempdir().expect("a store");
        let store = Store::at(store_dir.path());
        let digests: Vec<ConfigDigest> = (1..=3)
            .map(|n| store.keep(&written(from.path(), n, "yaml")).unwrap().0)
            .collect();

        let why = store
            .remove(digests[0], &[digests[0]], 1)
            .expect_err("protected");
        assert!(why.contains("running"), "{why}");
        let why = store.remove(digests[0], &[], 3).expect_err("the floor");
        assert!(why.contains("configurations_kept"), "{why}");
        store.remove(digests[0], &[], 2).expect("above the floor");
        assert_eq!(store.held().unwrap().len(), 2);
        store.remove(digests[0], &[], 2).expect("gone already");
    }
}
