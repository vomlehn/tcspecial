//! Configurations kept on board, named by their checksum.
//!
//! The vehicle keeps each configuration it holds in the directory the
//! `tcspecial` section's `configurations` attribute names, one directory
//! for each, named by its checksum in hex: its tree. In it, the top file is
//! named by the checksum too, with the extension that says how to read it,
//! and each file it includes is where its include name, read from the top
//! file's directory, puts it, with the same extension. The checksum takes in
//! those names, so it says the shape of the tree as well as what is in it. A
//! RESTART names the configuration to restart onto by its checksum, and a
//! start loads one by it. See "Keeping configurations on board" in
//! `docs/design.rst`.
//!
//! A directory for each, rather than every file in one, because the names
//! of included files are the configuration's own: two configurations may
//! each include a `zone`, and mean two different things by it.
//!
//! An include name may climb out of the top file's directory with `..`, as
//! the thermostat's `../thermostat-common/thermostat` does, so the top file
//! is kept as far down in its tree as its includes climb: one directory
//! named [`TOP_DIR`], `top`, for each `..` the include that climbs furthest
//! takes above the top file's directory, worked out from the names alone.
//! The thermostat is kept as
//!
//! ```text
//! <checksum>/top/<checksum>.ytcs
//! <checksum>/thermostat-common/thermostat.ytcs
//! ```
//!
//! and an include of `../../shared/zone` puts its top file in
//! `<checksum>/top/top`. One name for every level, so that how deep the top
//! file is says nothing but how far its includes climb, and a plain one,
//! which says what is in it. A configuration whose includes are all beside
//! it, or that includes nothing, is the depth of none:
//! `<checksum>/<checksum>.ytcs`, with its includes beside it, which is how
//! every configuration was kept before an include could be a path, so a
//! tree kept then loads as it did.
//!
//! The rules about what may be kept and what may be loaded are here, once,
//! for tcspecial, which loads from the store, and tcsverify, which puts a
//! configuration into it for testing and first provisioning:
//!
//! * the name it is kept under is the checksum computed from it, never one
//!   anybody gives it;
//! * an include name puts its file inside the tree, at a name that is not
//!   hidden and is not the checksum, and at a place no other file of the
//!   configuration is: one that does not is refused, both when it is kept
//!   and when it is loaded;
//! * a top file is loaded only from the depth its includes need, so that
//!   none of them reads a file from outside its tree;
//! * keeping one whose checksum is already held writes nothing, so a
//!   configuration once kept is never overwritten;
//! * its checksum is computed again from its files each time it is loaded,
//!   and one whose checksum is not its name is not loaded, so that a write
//!   cut short by a crash or a power loss is refused rather than flown.
//!
//! Uplinking a configuration by CFDP, and the provisioning crate that is to
//! receive, check, keep and remove them in flight, are not built: see the
//! design.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::config_digest::{digest_of_graph, ConfigDigest};
use crate::graph::Graph;

/// The extensions a kept configuration may have, which say how it is read:
/// the configuration formats, as [`crate::ConfigFormat`] takes them.
const EXTENSIONS: [&str; 2] = ["ytcs", "xtcs"];

/// The name of each directory a kept top file is down in, one for each
/// level its includes climb above it.
pub const TOP_DIR: &str = "top";

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

/// One include name, read from the top file's directory a part at a time,
/// with nothing but the name: each `..` takes away the name before it, or,
/// where there is none, climbs a directory above the top file's.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Resolved {
    /// How many directories above the top file's the file is reached from.
    up: usize,
    /// The directories below that the file is in, outermost first.
    down: Vec<String>,
    /// Each directory the name passes into on the way, as `up` and `down`
    /// were there: a name such as `sub/../zone` passes through `sub`, which
    /// must be there for the system to read the name as it is written.
    passed: Vec<(usize, Vec<String>)>,
    /// The file's own name, without its extension.
    file: String,
}

/// Reads an include name as [`Resolved`] says, refusing one that does not
/// name a file the store can keep.
///
/// Every part must be a name that does not begin with a dot, or `..`, and
/// the last must be a name, and not the checksum. A hidden name is the
/// store's own kind, as the directory a keep is written to is. The
/// checksum is the top file's name, and [`Store::find`] takes the first
/// file of that name it comes to as the top file. An empty part, and `.`,
/// are refused when the configuration is read too (see
/// [`Graph::load`]); they are refused here as well because a configuration
/// in the store has its include list read before it is loaded.
///
/// # Arguments
///
/// * `path`   - The configuration's top file, for the complaint.
/// * `name`   - The include name, as the include list writes it.
/// * `digest` - The configuration's checksum.
///
/// # Returns
///
/// Where the name puts its file, from the top file's directory.
///
/// # Errors
///
/// A message naming the file and the include, and what an include name the
/// store keeps must be.
fn resolve(
    path: &Path,
    name: &str,
    digest: ConfigDigest,
) -> Result<Resolved, String> {
    let refused = || {
        format!(
            "{} includes {name}: a configuration kept on board includes a \
             file by its path from the top file's directory, each part of \
             it .. or a name that does not begin with a dot, the last a \
             name and not the checksum, {digest}",
            path.display()
        )
    };
    let is_name = |part: &str| !part.is_empty() && !part.starts_with('.');
    let mut parts: Vec<&str> = name.split('/').collect();
    let file = parts
        .pop()
        .filter(|file| is_name(file))
        .ok_or_else(refused)?;
    if file == digest.to_string() {
        return Err(refused());
    }
    let mut resolved = Resolved {
        up: 0,
        down: Vec::new(),
        passed: Vec::new(),
        file: file.to_string(),
    };
    for part in parts {
        if part == ".." {
            if resolved.down.pop().is_none() {
                resolved.up += 1;
            }
        } else if is_name(part) {
            resolved.down.push(part.to_string());
            resolved.passed.push((resolved.up, resolved.down.clone()));
        } else {
            return Err(refused());
        }
    }
    Ok(resolved)
}

/// Says how far down in its tree a configuration's top file is kept: as
/// many directories as the include that climbs furthest climbs above the
/// top file's.
///
/// # Arguments
///
/// * `path`   - The configuration's top file, for the complaint.
/// * `names`  - Its include names, as its include list writes them.
/// * `digest` - Its checksum.
///
/// # Returns
///
/// The depth, which is none for a configuration that includes nothing, or
/// only files beside it.
///
/// # Errors
///
/// What [`resolve`] says of a name it refuses.
fn depth(
    path: &Path,
    names: &[String],
    digest: ConfigDigest,
) -> Result<usize, String> {
    names.iter().try_fold(0, |deepest, name| {
        Ok(deepest.max(resolve(path, name, digest)?.up))
    })
}

/// Where each file of a kept configuration is, in its tree.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Layout {
    /// The top file, from the tree.
    top: PathBuf,
    /// Each included file, from the tree, in the include list's order.
    files: Vec<PathBuf>,
    /// Each directory inside the tree that a file is in or an include name
    /// passes through, from the tree. In the order a path sorts in, which
    /// puts each after the directory it is in.
    dirs: BTreeSet<PathBuf>,
}

/// Lays a configuration's files out in its tree, with its top file
/// `depth` directories down.
///
/// Each included file is where its name, read from the top file's
/// directory, puts it, with the top file's extension. Refused are a name
/// that climbs above the tree -- which the depth [`depth`] works out never
/// is, and which a top file put too high by hand would be -- and a name
/// that puts its file where another file, or a directory, of the
/// configuration is.
///
/// # Arguments
///
/// * `path`   - The configuration's top file, for the complaint.
/// * `names`  - Its include names, as its include list writes them.
/// * `digest` - Its checksum.
/// * `depth`  - How far down the top file is.
/// * `ext`    - The extension its files are kept with, `ytcs` or `xtcs`.
///
/// # Returns
///
/// Where each file, and each directory, is.
///
/// # Errors
///
/// A message naming the file and the include, for one [`resolve`] refuses,
/// one that climbs out of the tree, and one put where something else is.
fn layout(
    path: &Path,
    names: &[String],
    digest: ConfigDigest,
    depth: usize,
    ext: &str,
) -> Result<Layout, String> {
    let down_to = |level: usize| -> PathBuf {
        std::iter::repeat_n(TOP_DIR, level).collect()
    };
    let mut layout = Layout {
        top: down_to(depth).join(format!("{digest}.{ext}")),
        files: Vec::new(),
        dirs: (1..=depth).map(down_to).collect(),
    };
    for name in names {
        let resolved = resolve(path, name, digest)?;
        let at = |up: usize, down: &[String]| -> Result<PathBuf, String> {
            let level = depth.checked_sub(up).ok_or_else(|| {
                format!(
                    "{} includes {name}, which climbs {up} directories above \
                     the top file's, and the top file is {depth} down in its \
                     tree: it would read a file from outside the tree, and \
                     is refused",
                    path.display()
                )
            })?;
            Ok(down_to(level).join(down.iter().collect::<PathBuf>()))
        };
        for (up, down) in &resolved.passed {
            let passed = at(*up, down)?;
            layout
                .dirs
                .extend(passed.ancestors().map(Path::to_path_buf));
        }
        let dir = at(resolved.up, &resolved.down)?;
        layout.dirs.extend(dir.ancestors().map(Path::to_path_buf));
        layout
            .files
            .push(dir.join(format!("{}.{ext}", resolved.file)));
    }
    layout.dirs.remove(Path::new(""));
    if layout.dirs.contains(&layout.top) {
        return Err(format!(
            "{} includes a file by a name that passes through a directory \
             named {}, which is where its top file is kept",
            path.display(),
            layout.top.display()
        ));
    }
    let mut seen = BTreeSet::from([&layout.top]);
    for (name, file) in names.iter().zip(&layout.files) {
        if !seen.insert(file) || layout.dirs.contains(file) {
            return Err(format!(
                "{} includes {name}, which would be kept as {}, where \
                 another of its files or directories is",
                path.display(),
                file.display()
            ));
        }
    }
    Ok(layout)
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
    /// The file, `<checksum>.ytcs` or `<checksum>.xtcs` in the directory
    /// named `<checksum>` or one of the [`TOP_DIR`] directories down in it,
    /// or `None` where there is none. See [`Store::found`].
    pub fn find(&self, digest: ConfigDigest) -> Option<PathBuf> {
        self.found(digest).map(|(path, _)| path)
    }

    /// Finds the top file of a configuration, by its checksum, and how far
    /// down in its tree it is.
    ///
    /// The tree is looked in first, then each [`TOP_DIR`] down from it in
    /// turn, and the first file of the checksum's name is the one found:
    /// no include may be kept under that name, so there is no other. A
    /// store holds a checksum once, so where both extensions are, the YAML
    /// is the one found. Only a directory, not a link to one, is gone down
    /// into, so that a link back up cannot keep the search going for ever.
    ///
    /// # Arguments
    ///
    /// * `digest` - The configuration's checksum.
    ///
    /// # Returns
    ///
    /// The file and its depth, or `None` where there is none.
    fn found(&self, digest: ConfigDigest) -> Option<(PathBuf, usize)> {
        let mut dir = self.dir.join(digest.to_string());
        let mut depth = 0;
        loop {
            let top = EXTENSIONS
                .iter()
                .map(|ext| dir.join(format!("{digest}.{ext}")))
                .find(|path| path.is_file());
            if let Some(top) = top {
                return Some((top, depth));
            }
            dir.push(TOP_DIR);
            if !std::fs::symlink_metadata(&dir).is_ok_and(|m| m.is_dir()) {
                return None;
            }
            depth += 1;
        }
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
    /// Its include list is read before anything it includes, and one whose
    /// top file is not as far down its tree as the includes climb is
    /// refused, so that no file from outside the tree is read. Nothing else
    /// is checked here; a start checks it as it checks any configuration.
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
    /// from outside its tree or by a name the store does not keep, kept at
    /// another depth than its includes need, or of another checksum than
    /// its name.
    pub fn load(
        &self,
        digest: ConfigDigest,
    ) -> Result<(Graph, PathBuf), String> {
        let (path, found_at) = self.found(digest).ok_or_else(|| {
            format!(
                "no configuration {digest} is kept in {}: there is no \
                 {digest}.ytcs or {digest}.xtcs in {digest} or in a {TOP_DIR} \
                 directory down in it",
                self.dir.display()
            )
        })?;
        let names = crate::graph::read_one(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .include;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        layout(&path, &names, digest, found_at, ext)?;
        let needed = depth(&path, &names, digest)?;
        if found_at != needed {
            return Err(format!(
                "{} is {found_at} directories down in its tree, and its \
                 includes climb {needed}: it was not kept by the store, and \
                 is not loaded",
                path.display()
            ));
        }
        let graph = Graph::load(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
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
    /// is laid out as [`layout`] says, its top file as far down its tree as
    /// its includes climb. It is written to a directory beside its name,
    /// each file flushed and read-only, mode 0444, each directory in it
    /// flushed, and the directory renamed into place, so that a power
    /// failure part way leaves no configuration of that name rather than
    /// half of one. A directory left beside its name by a keep cut short
    /// like that is the store's own, and is removed first.
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
    /// Why it was not kept: it does not load, it includes a file by a name
    /// the store does not keep, its extension is not one a configuration has,
    /// its checksum's directory is there without its top file, or the store
    /// cannot be written.
    pub fn keep(&self, path: &Path) -> Result<(ConfigDigest, Kept), String> {
        let graph = Graph::load(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let digest = digest_of_graph(&graph)
            .map_err(|e| format!("{}: {e}", path.display()))?;
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
        let names = graph.included();
        let climbs = depth(path, &names, digest)?;
        let layout = layout(path, &names, digest, climbs, &ext)?;
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
                 {digest}.xtcs, in it or a {TOP_DIR} directory down in it: \
                 what is kept there was damaged, and must be removed before \
                 {} can be kept",
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
        // Each after the directory it is in, which is the order they sort in.
        for dir in &layout.dirs {
            std::fs::create_dir(beside.join(dir))
                .map_err(|e| cannot("written", e))?;
        }
        let kept_as = std::iter::once(&layout.top).chain(&layout.files);
        for (file, at) in graph.files.iter().zip(kept_as) {
            let bytes = std::fs::read(file).map_err(|e| {
                format!("{} cannot be read: {e}", file.display())
            })?;
            write_read_only(&beside.join(at), &bytes)
                .map_err(|e| cannot("written", e))?;
        }
        for dir in layout.dirs.iter().rev() {
            sync_dir(&beside.join(dir)).map_err(|e| cannot("flushed", e))?;
        }
        sync_dir(&beside).map_err(|e| cannot("flushed", e))?;
        std::fs::rename(&beside, &tree).map_err(|e| cannot("kept", e))?;
        sync_dir(&self.dir).map_err(|e| cannot("flushed", e))?;
        Ok((digest, Kept::Written(tree.join(&layout.top))))
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
                 <vehicle><type name=\"ping\"><compute name=\"p{n}\"/>\
                 </type></vehicle>\
                 </configuration>\n"
            )
        } else {
            format!(
                "description: \"{n}\"\nvehicle:\n  - type ping:\n      \
                 - {{ compute: p{n} }}\n"
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
            "vehicle:\n  - type ping:\n      - { compute: p }\n",
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

    /// A file included by a name the store does not keep -- hidden, or its
    /// checksum -- is refused, and nothing is kept.
    #[test]
    fn an_include_name_the_store_does_not_keep_is_refused() {
        let from = tempfile::tempdir().expect("a directory");
        let store_dir = tempfile::tempdir().expect("a store");
        let store = Store::at(store_dir.path());
        std::fs::create_dir(from.path().join(".sub")).unwrap();

        for name in [".zone", ".sub/zone"] {
            let why = store
                .keep(&tree(from.path(), "top", name))
                .expect_err("not a name the store keeps");
            assert!(why.contains(name), "{why}");
            assert!(why.contains("does not begin with a dot"), "{why}");
        }
        assert!(store.held().unwrap().is_empty(), "and nothing was kept");
    }

    /// Writes a file of a configuration, with whatever directories it is
    /// in.
    ///
    /// # Arguments
    ///
    /// * `path` - The file.
    /// * `text` - What it says.
    fn put(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// Copies the files of an example into `to`, each at its path from
    /// `examples`, so that the copy's includes reach one another as the
    /// example's do. Each is read through any link it is, as the store
    /// reads it.
    ///
    /// # Arguments
    ///
    /// * `to`    - Where to copy them.
    /// * `files` - Each file, from `examples`.
    fn copy_example(to: &Path, files: &[&str]) {
        let examples =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
        for file in files {
            let text = std::fs::read_to_string(examples.join(file))
                .expect("the example's file");
            put(&to.join(file), &text);
        }
    }

    /// The ping and thermostat examples, whose includes climb into a common
    /// directory beside theirs, are kept with their top files one `top`
    /// down and their common files where the includes put them. Each loads
    /// by its checksum, which is the example's own, in both spellings.
    #[test]
    fn the_ping_and_thermostat_examples_are_kept_and_load() {
        use std::os::unix::fs::PermissionsExt;

        let from = tempfile::tempdir().expect("a directory");
        let store_dir = tempfile::tempdir().expect("a store");
        let store = Store::at(store_dir.path());
        for (example, common) in
            [("ping", "ping"), ("thermostat", "thermostat")]
        {
            let mut files = Vec::new();
            for ext in EXTENSIONS {
                files.push(format!("{example}/{example}.{ext}"));
                files.push(format!("{example}-common/{common}.{ext}"));
            }
            let files: Vec<&str> = files.iter().map(String::as_str).collect();
            copy_example(from.path(), &files);
            let top = from.path().join(format!("{example}/{example}.ytcs"));
            let original = crate::config_digest::digest_of_file(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../examples/{example}/{example}.ytcs")),
            )
            .unwrap();

            let (digest, kept) = store.keep(&top).expect("kept");
            assert_eq!(digest, original, "the example's checksum");
            let at = store_dir.path().join(digest.to_string());
            assert_eq!(
                kept,
                Kept::Written(at.join("top").join(format!("{digest}.ytcs")))
            );
            let common_file =
                at.join(format!("{example}-common/{common}.ytcs"));
            let mode = std::fs::metadata(&common_file)
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o444, "read-only");
            assert_eq!(
                crate::config_digest::digest_of_file(kept.path()).unwrap(),
                original,
                "what is kept has the original's checksum"
            );

            let (graph, read_from) = store.load(digest).expect("it loads");
            assert_eq!(read_from, *kept.path());
            assert_eq!(
                graph.included(),
                vec![format!("../{example}-common/{common}")]
            );
            assert_eq!(
                graph.nodes.len(),
                Graph::load(&top).unwrap().nodes.len()
            );
            let xml = top.with_extension("xtcs");
            assert_eq!(
                store.keep(&xml).unwrap(),
                (digest, Kept::Already(read_from)),
                "the other spelling is the same configuration"
            );
        }
        assert_eq!(store.held().unwrap().len(), 2);
    }

    /// A configuration whose includes climb two directories is kept with
    /// its top file two `top`s down; one that passes into a directory and
    /// comes back out has that directory made for it. It loads, and its
    /// checksum is the original's.
    #[test]
    fn an_include_that_climbs_two_levels_is_kept_inside_the_tree() {
        let from = tempfile::tempdir().expect("a directory");
        let store_dir = tempfile::tempdir().expect("a store");
        let store = Store::at(store_dir.path());
        let node = "vehicle:\n  - type ping:\n      - { compute: ";
        put(
            &from.path().join("shared/zone.ytcs"),
            &format!("{node}z }}\n"),
        );
        put(&from.path().join("a/b/near.ytcs"), &format!("{node}n }}\n"));
        std::fs::create_dir(from.path().join("a/b/sub")).unwrap();
        let top = from.path().join("a/b/top.ytcs");
        put(&top, "include:\n  - ../../shared/zone\n  - sub/../near\n");
        let original = crate::config_digest::digest_of_file(&top).unwrap();

        let (digest, kept) = store.keep(&top).expect("kept");
        assert_eq!(digest, original);
        let at = store_dir.path().join(digest.to_string());
        let down = at.join("top/top");
        assert_eq!(kept, Kept::Written(down.join(format!("{digest}.ytcs"))));
        assert!(at.join("shared/zone.ytcs").is_file());
        assert!(down.join("near.ytcs").is_file());
        assert!(down.join("sub").is_dir(), "the directory passed through");
        assert_eq!(
            crate::config_digest::digest_of_file(kept.path()).unwrap(),
            original
        );
        let (graph, _) = store.load(digest).expect("it loads");
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(digest_of_graph(&graph).unwrap(), original);
    }

    /// A top file kept by hand higher in its tree than its includes climb
    /// would read a file from outside it, and is refused before that file
    /// is read; one kept lower than they need is not the store's, and is
    /// refused too. Laid out with too few levels, an include that climbs
    /// is refused rather than put outside the tree.
    #[test]
    fn an_include_that_would_escape_the_tree_is_refused() {
        let from = tempfile::tempdir().expect("a directory");
        let store_dir = tempfile::tempdir().expect("a store");
        let store = Store::at(store_dir.path());
        let node = "vehicle:\n  - type ping:\n      - { compute: z }\n";
        put(&from.path().join("common/zone.ytcs"), node);
        let include = "include:\n  - ../common/zone\n";
        let top = from.path().join("vehicle/top.ytcs");
        put(&top, include);
        let digest = crate::config_digest::digest_of_file(&top).unwrap();
        let names = vec!["../common/zone".to_string()];

        let why = layout(&top, &names, digest, 0, "ytcs")
            .expect_err("it climbs above the tree");
        assert!(why.contains("outside the tree"), "{why}");
        assert!(why.contains("../common/zone"), "{why}");
        assert!(layout(&top, &names, digest, 1, "ytcs").is_ok());

        // Its top file straight in its tree, and the file it includes
        // beside the tree, where an include read from there would find it.
        let tree = store_dir.path().join(digest.to_string());
        put(&tree.join(format!("{digest}.ytcs")), include);
        put(&store_dir.path().join("common/zone.ytcs"), node);
        let why = store.load(digest).expect_err("it would read outside");
        assert!(why.contains("outside the tree"), "{why}");
        std::fs::remove_dir_all(&tree).unwrap();

        put(&tree.join(format!("top/top/{digest}.ytcs")), include);
        put(&tree.join("top/common/zone.ytcs"), node);
        let why = store.load(digest).expect_err("deeper than it needs");
        assert!(why.contains("not kept by the store"), "{why}");
    }

    /// An include that would put its file where another of the
    /// configuration's is, or by the checksum's name, is refused.
    #[test]
    fn an_include_kept_where_another_file_is_is_refused() {
        let path = Path::new("top.ytcs");
        let digest = ConfigDigest([3; 16]);
        let names = |names: &[&str]| -> Vec<String> {
            names.iter().map(|name| name.to_string()).collect()
        };
        let why =
            layout(path, &names(&["../top/zone", "zone"]), digest, 1, "ytcs")
                .expect_err("two names for one place");
        assert!(why.contains("another of its files"), "{why}");
        let why =
            layout(path, &names(&["zone.ytcs/x", "zone"]), digest, 0, "ytcs")
                .expect_err("a file where a directory is");
        assert!(why.contains("another of its files"), "{why}");
        let why = depth(path, &names(&[&format!("../{digest}")]), digest)
            .expect_err("the checksum's name");
        assert!(why.contains("not the checksum"), "{why}");
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
