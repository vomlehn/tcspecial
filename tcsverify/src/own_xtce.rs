//! Writing a configuration's own XTCE beside it, for `--xtce`, and saying
//! whether the one there is what would be written, for `--xtce-check`.
//!
//! What the document says is [`tcslibgs::graph::own_xtce`]'s. This names
//! its file -- the configuration's, with `.xtce` for its extension, so that
//! both spellings of a configuration name one document -- and writes it, or
//! compares it.

use std::path::{Path, PathBuf};

use tcslibgs::graph::{own_xtce, Graph};

/// What was found of a configuration's own XTCE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// The file says what would be written.
    Current,
    /// It said something else, or was not there, and was written.
    Written,
    /// It says something else, or is not there, and was left alone.
    Stale,
}

/// Names a configuration's own XTCE.
///
/// # Arguments
///
/// * `configuration` - The configuration's file.
///
/// # Returns
///
/// The file beside it, of its name with the extension `.xtce`.
pub fn path_of(configuration: &Path) -> PathBuf {
    configuration.with_extension("xtce")
}

/// Writes a configuration's own XTCE beside it, or says whether the one
/// there is current.
///
/// # Arguments
///
/// * `configuration` - The configuration's file.
/// * `write`         - Whether to write a file that is not current, rather
///                     than only saying so.
///
/// # Returns
///
/// What was found, and done.
///
/// # Errors
///
/// A message, naming the file, when the configuration does not read, its
/// document cannot be made, or the document cannot be read or written.
pub fn of(configuration: &Path, write: bool) -> Result<Found, String> {
    let shown = configuration.display();
    let graph =
        Graph::load(configuration).map_err(|why| format!("{shown}: {why}"))?;
    let name = configuration
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{shown}: names no file"))?;
    let text = own_xtce::written(&graph, &name)
        .map_err(|why| format!("{shown}: {why}"))?;
    let path = path_of(configuration);
    match std::fs::read_to_string(&path) {
        Ok(there) if there == text => return Ok(Found::Current),
        Ok(_) => {}
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {}
        Err(why) => return Err(format!("{}: {why}", path.display())),
    }
    if !write {
        return Ok(Found::Stale);
    }
    std::fs::write(&path, text)
        .map_err(|why| format!("{}: {why}", path.display()))?;
    Ok(Found::Written)
}
