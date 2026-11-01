//! Configurations kept on board, named by their checksum.
//!
//! The vehicle keeps each configuration it holds in the directory the
//! `tcspecial` section's `configurations` attribute names, one directory
//! for each, named by its checksum in hex. In it, the top file is named
//! by the checksum too, with the extension that says how to read it --
//! `<checksum>/<checksum>.ytcs` or `<checksum>/<checksum>.xtcs` -- and each
//! file it includes beside it, under the name its include list gives it.
//! The checksum takes in those names, so it says the shape of the tree as
//! well as what is in it. A RESTART names the configuration to restart onto
//! by its checksum, and a start loads one by it. See "Keeping
//! configurations on board" in `docs/design.rst`.
//!
//! A directory for each, rather than every file in one, because the names
//! of included files are the configuration's own: two configurations may
//! each include a `zone`, and mean two different things by it.
//!
//! The rules about what may be kept and what may be loaded are here, once,
//! for tcspecial, which loads from the store, and tcsverify, which puts a
//! configuration into it for testing and first provisioning:
//!
//! * the name it is kept under is the checksum computed from it, never one
//!   anybody gives it;
//! * an include list names a file beside the top one, by a name that is
//!   not hidden and is not the checksum: one that names one elsewhere is
//!   refused, both when it is kept and when it is loaded;
//! * keeping one whose checksum is already held writes nothing, so a
//!   configuration once kept is never overwritten;
//! * its checksum is computed again from its files each time it is loaded,
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
const EXTENSIONS: [&str; 2] = ["ytcs", "xtcs"];

/// The directory configurations are kept in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    dir: PathBuf,
}

/// What [`Store::keep`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kept {
    /// The configuration was written to the store; its top file is at this
    /// path.
    Written(PathBuf),
    /// A configuration of the same checksum was held already, its top file
    /// at this path, and nothing was written: it means the same.
    Already(PathBuf),
}

impl Kept {
    /// Where the configuration's top file is kept, whichever was done.
    ///
    /// # Returns
    ///
    /// The kept top file's path.
    pub fn path(&self) -> &Path {
        match self {
            Kept::Written(path) | Kept::Already(path) => path,
        }
    }
}

/// Refuses a configuration whose include list names a file the store could
/// not keep beside its top file.
///
/// An included file is kept under the name the include list gives it, in
/// the directory its top file is kept in, so the name must be one file's
/// name there: not a path, not hidden -- the store's own files that are not
/// yet in place are -- and not the checksum, which is the top file's.
///
/// # Arguments
///
/// * `path`   - The configuration's top file.
/// * `graph`  - What it loads as.
/// * `digest` - Its checksum.
///
/// # Errors
///
/// A message naming the file and each include name it cannot keep.
pub fn keepable(
    path: &Path,
    graph: &Graph,
    digest: ConfigDigest,
) -> Result<(), String> {
    let top = digest.to_string();
    let refused: Vec<String> = graph
        .included()
        .into_iter()
        .filter(|name| {
            name.is_empty()
                || name.contains('/')
                || name.starts_with('.')
                || *name == top
        })
        .collect();
    if refused.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{} includes {}: a configuration kept on board includes files \
         beside it, each by a name that is not a path, does not begin with \
         a dot, and is not its checksum, {top}",
        path.display(),
        refused.join(", ")
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

    /// Finds the top file of a configuration, by its checksum.
    ///
    /// # Arguments
    ///
    /// * `digest` - The configuration's checksum.
    ///
    /// # Returns
    ///
    /// The file, `<checksum>/<checksum>.ytcs` or `<checksum>/<checksum>.xtcs`,
    /// or `None` where neither is there. A store holds a checksum once, so
    /// where both are, the YAML is the one found.
    pub fn find(&self, digest: ConfigDigest) -> Option<PathBuf> {
        let tree = self.dir.join(digest.to_string());
        EXTENSIONS
            .iter()
            .map(|ext| tree.join(format!("{digest}.{ext}")))
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
    /// Its checksum is computed again from its files, and one whose checksum
    /// is not its name is refused: a write cut short, storage that went bad,
    /// or a file somebody put there by hand under a name it does not have.
    /// One that includes a file from outside its directory is refused too.
    /// Nothing else is checked here; a start checks it as it checks any
    /// configuration.
    ///
    /// # Arguments
    ///
    /// * `digest` - The configuration's checksum.
    ///
    /// # Returns
    ///
    /// The configuration, and its top file.
    ///
    /// # Errors
    ///
    /// Why it cannot be loaded: not held, not readable, including a file
    /// from elsewhere, or of another checksum than its name.
    pub fn load(
        &self,
        digest: ConfigDigest,
    ) -> Result<(Graph, PathBuf), String> {
        let path = self.find(digest).ok_or_else(|| {
            format!(
                "no configuration {digest} is kept in {}: there is no \
                 {digest}/{digest}.ytcs or {digest}/{digest}.xtcs there",
                self.dir.display()
            )
        })?;
        let graph = Graph::load(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        keepable(&path, &graph, digest)?;
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

    /// Keeps the configuration whose top file is at `path`, under its
    /// checksum.
    ///
    /// The checksum is computed here, from what the files say and the names
    /// they are included by, and is the name the configuration is kept
    /// under: a name anybody gives it is not trusted. A checksum already
    /// held writes nothing, since the configuration held already means the
    /// same, so a configuration once kept is never overwritten. One written
    /// is written to a directory beside its name, each file flushed and
    /// read-only, mode 0444, and the directory renamed into place, so that a
    /// power failure part way leaves no configuration of that name rather
    /// than half of one. A directory left beside its name by a keep cut
    /// short like that is the store's own, and is removed first.
    ///
    /// What makes a configuration fit to fly is not decided here: tcsverify
    /// checks it before it keeps it, and the vehicle checks it again as it
    /// loads it.
    ///
    /// # Arguments
    ///
    /// * `path` - The configuration's top file, `.ytcs` or `.xtcs`.
    ///
    /// # Returns
    ///
    /// Its checksum, and whether it was written or was held already.
    ///
    /// # Errors
    ///
    /// Why it was not kept: it does not load, it includes a file the store
    /// cannot keep beside it, its extension is not one a configuration has,
    /// its checksum's directory is there without its top file, or the store
    /// cannot be written.
    pub fn keep(&self, path: &Path) -> Result<(ConfigDigest, Kept), String> {
        let graph = Graph::load(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let digest = digest_of_graph(&graph)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        keepable(path, &graph, digest)?;
        if let Some(held) = self.find(digest) {
            return Ok((digest, Kept::Already(held)));
        }
        let ext = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .filter(|ext| EXTENSIONS.contains(&ext.as_str()))
            .ok_or_else(|| {
                format!(
                    "{}: a configuration is kept as .ytcs or .xtcs, and this \
                     is neither",
                    path.display()
                )
            })?;
        let cannot = |what: &str, e: std::io::Error| {
            format!(
                "{} cannot be {what} in {} ({e}): the configurations \
                 attribute must name a directory that can be written",
                path.display(),
                self.dir.display()
            )
        };
        let tree = self.dir.join(digest.to_string());
        if tree.exists() {
            return Err(format!(
                "{} is there without its top file, {digest}.ytcs or \
                 {digest}.xtcs: what is kept there was damaged, and must be \
                 removed before {} can be kept",
                tree.display(),
                path.display()
            ));
        }
        let beside = self.dir.join(format!(".{digest}.new"));
        if beside.exists() {
            std::fs::remove_dir_all(&beside)
                .map_err(|e| cannot("cleared", e))?;
        }
        std::fs::create_dir(&beside).map_err(|e| cannot("written", e))?;
        let names = std::iter::once(digest.to_string()).chain(graph.included());
        for (file, name) in graph.files.iter().zip(names) {
            let bytes = std::fs::read(file).map_err(|e| {
                format!("{} cannot be read: {e}", file.display())
            })?;
            write_read_only(&beside.join(format!("{name}.{ext}")), &bytes)
                .map_err(|e| cannot("written", e))?;
        }
        sync_dir(&beside).map_err(|e| cannot("flushed", e))?;
        std::fs::rename(&beside, &tree).map_err(|e| cannot("kept", e))?;
        sync_dir(&self.dir).map_err(|e| cannot("flushed", e))?;
        Ok((digest, Kept::Written(tree.join(format!("{digest}.{ext}")))))
    }

    /// Lists the checksums the store holds, from the names of its
    /// directories.
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
                ConfigDigest::of_hex(entry.file_name().to_str()?)
            })
            .filter(|digest| self.holds(*digest))
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
    /// what was asked for already, and succeeds. Its directory is renamed
    /// out of the way first, so that a power failure part way leaves no
    /// configuration of that name rather than part of one.
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
        if !self.holds(digest) {
            return Ok(());
        }
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
        let tree = self.dir.join(digest.to_string());
        let going = self.dir.join(format!(".{digest}.old"));
        let cannot = |e: std::io::Error| {
            format!("{} cannot be removed: {e}", tree.display())
        };
        if going.exists() {
            std::fs::remove_dir_all(&going).map_err(cannot)?;
        }
        std::fs::rename(&tree, &going).map_err(cannot)?;
        sync_dir(&self.dir).map_err(cannot)?;
        std::fs::remove_dir_all(&going).map_err(cannot)
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

/// Flushes a directory, so that the names made and changed in it last.
///
/// # Arguments
///
/// * `dir` - The directory.
///
/// # Errors
///
/// What the system said, where it cannot be.
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::File::open(dir)?.sync_all()
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
    /// * `ext` - `ytcs` or `xtcs`.
    ///
    /// # Returns
    ///
    /// Its path.
    fn written(dir: &Path, n: u32, ext: &str) -> PathBuf {
        let path = dir.join(format!("vehicle{n}.{ext}"));
        let text = if ext == "xtcs" {
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
        let path = written(from.path(), 1, "ytcs");
        let digest = crate::config_digest::digest_of_file(&path).unwrap();

        assert!(!store.holds(digest));
        let (kept_as, kept) = store.keep(&path).expect("kept");
        assert_eq!(kept_as, digest, "the checksum computed from it");
        let at = store_dir
            .path()
            .join(digest.to_string())
            .join(format!("{digest}.ytcs"));
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
        let xml = written(from.path(), 1, "xtcs");
        assert_eq!(
            crate::config_digest::digest_of_file(&xml).unwrap(),
            digest,
            "the two spellings say the same"
        );
        assert!(!at.with_extension("xtcs").exists());
        assert_eq!(store.keep(&xml).unwrap(), (digest, Kept::Already(at)));
    }

    /// A file kept under a name that is not its checksum -- damaged, cut
    /// short, or put there by hand -- is not loaded, and says why.
    #[test]
    fn a_file_whose_checksum_is_not_its_name_is_not_loaded() {
        let from = tempfile::tempdir().expect("a directory");
        let store_dir = tempfile::tempdir().expect("a store");
        let store = Store::at(store_dir.path());
        let one = written(from.path(), 1, "ytcs");
        let two = written(from.path(), 2, "ytcs");
        let digest = crate::config_digest::digest_of_file(&one).unwrap();
        let tree = store_dir.path().join(digest.to_string());
        std::fs::create_dir(&tree).unwrap();
        std::fs::copy(&two, tree.join(format!("{digest}.ytcs"))).unwrap();

        assert!(store.holds(digest), "a file of the name is there");
        let why = store.load(digest).expect_err("but it is another");
        assert!(why.contains("not the"), "{why}");
        assert!(why.contains(&digest.to_string()), "{why}");

        let missing = ConfigDigest([7; 16]);
        let why = store.load(missing).expect_err("nothing of that name");
        assert!(why.contains(&missing.to_string()), "{why}");
    }

    /// A configuration of a top file that includes `name`, which holds a
    /// ping node, both written in `dir`.
    ///
    /// # Arguments
    ///
    /// * `dir`  - Where to write them.
    /// * `top`  - The top file's stem.
    /// * `name` - The name the top file includes the other by.
    ///
    /// # Returns
    ///
    /// The top file's path.
    fn tree(dir: &Path, top: &str, name: &str) -> PathBuf {
        std::fs::write(
            dir.join(format!("{name}.ytcs")),
            "vehicle:\n  - { name: p, type: ping }\n",
        )
        .unwrap();
        let path = dir.join(format!("{top}.ytcs"));
        std::fs::write(&path, format!("include:\n  - {name}\n")).unwrap();
        path
    }

    /// A configuration of several files is kept as a tree: its top file
    /// under its checksum, and each file it includes beside it, under the
    /// name it is included by. It loads by its checksum.
    #[test]
    fn a_configuration_of_several_files_is_kept_as_a_tree() {
        use std::os::unix::fs::PermissionsExt;

        let from = tempfile::tempdir().expect("a directory");
        let store_dir = tempfile::tempdir().expect("a store");
        let store = Store::at(store_dir.path());
        let top = tree(from.path(), "top", "zone");

        let (digest, kept) = store.keep(&top).expect("kept");
        let at = store_dir.path().join(digest.to_string());
        let top_name = format!("{digest}.ytcs");
        assert_eq!(kept, Kept::Written(at.join(&top_name)));
        let mut names: Vec<String> = std::fs::read_dir(&at)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, vec![top_name, "zone.ytcs".to_string()]);
        let mode = std::fs::metadata(at.join("zone.ytcs"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o444, "read-only");
        assert_eq!(store.held().unwrap(), vec![digest]);

        let (graph, read_from) = store.load(digest).expect("it loads");
        assert_eq!(read_from, *kept.path());
        assert_eq!(graph.included(), vec!["zone".to_string()]);
        assert_eq!(graph.nodes.len(), 1, "the included file's node");
        let again = store.keep(&top).expect("kept");
        assert_eq!(again, (digest, Kept::Already(read_from)));
    }

    /// The names a configuration includes its files by are in its
    /// checksum, so two trees of the same files differently named are two
    /// configurations, kept apart. The top file's own name is not.
    #[test]
    fn the_include_names_are_in_the_checksum() {
        let one = tempfile::tempdir().expect("a directory");
        let two = tempfile::tempdir().expect("a directory");
        let three = tempfile::tempdir().expect("a directory");
        let store_dir = tempfile::tempdir().expect("a store");
        let store = Store::at(store_dir.path());

        let (zone, _) = store.keep(&tree(one.path(), "top", "zone")).unwrap();
        let (hold, _) = store.keep(&tree(two.path(), "top", "hold")).unwrap();
        assert_ne!(zone, hold);
        let mut both = vec![zone, hold];
        both.sort_by_key(|digest| digest.0);
        assert_eq!(store.held().unwrap(), both);

        let renamed = tree(three.path(), "elsewhere", "zone");
        assert_eq!(
            crate::config_digest::digest_of_file(&renamed).unwrap(),
            zone
        );
    }

    /// A file included by a name the store cannot keep beside the top file
    /// is refused, and nothing is kept.
    #[test]
    fn an_include_name_that_is_not_a_file_beside_it_is_refused() {
        let from = tempfile::tempdir().expect("a directory");
        let store_dir = tempfile::tempdir().expect("a store");
        let store = Store::at(store_dir.path());
        std::fs::create_dir(from.path().join("sub")).unwrap();

        for name in ["sub/zone", ".zone"] {
            let why = store
                .keep(&tree(from.path(), "top", name))
                .expect_err("not a file beside it");
            assert!(why.contains(name), "{why}");
            assert!(why.contains("not a path"), "{why}");
        }
        assert!(store.held().unwrap().is_empty(), "and nothing was kept");
    }

    /// A keep cut short leaves its directory beside its name, which the
    /// next keep clears; a configuration removed leaves nothing.
    #[test]
    fn a_keep_cut_short_is_cleared_and_a_removal_leaves_nothing() {
        let from = tempfile::tempdir().expect("a directory");
        let store_dir = tempfile::tempdir().expect("a store");
        let store = Store::at(store_dir.path());
        let top = tree(from.path(), "top", "zone");
        let digest = crate::config_digest::digest_of_file(&top).unwrap();
        let cut_short = store_dir.path().join(format!(".{digest}.new"));
        std::fs::create_dir(&cut_short).unwrap();
        std::fs::write(cut_short.join("zone.ytcs"), "half").unwrap();
        assert!(store.held().unwrap().is_empty(), "it is not held");

        store.keep(&top).expect("kept");
        store.keep(&written(from.path(), 1, "ytcs")).expect("kept");
        assert_eq!(store.held().unwrap().len(), 2);

        store.remove(digest, &[], 1).expect("removed");
        let named = digest.to_string();
        let left: Vec<String> = std::fs::read_dir(store_dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.contains(&named))
            .collect();
        assert!(left.is_empty(), "{left:?}");
    }

    /// Removing keeps the floor the policy sets: never a protected one,
    /// never below `configurations_kept`.
    #[test]
    fn removing_never_goes_below_what_must_be_kept() {
        let from = tempfile::tempdir().expect("a directory");
        let store_dir = tempfile::tempdir().expect("a store");
        let store = Store::at(store_dir.path());
        let digests: Vec<ConfigDigest> = (1..=3)
            .map(|n| store.keep(&written(from.path(), n, "ytcs")).unwrap().0)
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
