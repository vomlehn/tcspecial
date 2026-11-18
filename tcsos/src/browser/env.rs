//! A program's environment, in a page, where every program is threads of
//! one: its arguments, its variables and its directory are its own, kept in
//! [`crate::browser::context`], and the standard library's, which a page
//! does not have, are not asked.

pub use std::env::{consts, JoinPathsError, SplitPaths, VarError};

use std::ffi::OsString;
use std::path::PathBuf;

use crate::browser::context;

/// The program's arguments, its name first.
///
/// # Returns
///
/// An iterator over them.
pub fn args() -> std::vec::IntoIter<String> {
    context::current().args.clone().into_iter()
}

/// The program's arguments, as `OsString`s.
///
/// # Returns
///
/// An iterator over them.
pub fn args_os() -> std::vec::IntoIter<OsString> {
    args().map(OsString::from).collect::<Vec<_>>().into_iter()
}

/// One of the program's variables.
///
/// # Arguments
///
/// * `name` - Its name.
///
/// # Returns
///
/// Its value.
///
/// # Errors
///
/// `VarError::NotPresent` where it has none.
pub fn var(name: impl AsRef<std::ffi::OsStr>) -> Result<String, VarError> {
    let name = name.as_ref().to_string_lossy();
    context::current()
        .env
        .lock()
        .ok()
        .and_then(|env| env.get(name.as_ref()).cloned())
        .ok_or(VarError::NotPresent)
}

/// One of the program's variables, as an `OsString`.
///
/// # Arguments
///
/// * `name` - Its name.
///
/// # Returns
///
/// Its value, where it has one.
pub fn var_os(name: impl AsRef<std::ffi::OsStr>) -> Option<OsString> {
    var(name).ok().map(OsString::from)
}

/// Every one of the program's variables.
///
/// # Returns
///
/// An iterator over them, names and values.
pub fn vars() -> std::vec::IntoIter<(String, String)> {
    context::current()
        .env
        .lock()
        .map(|env| env.clone().into_iter().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
}

/// Every one of the program's variables, as `OsString`s.
///
/// # Returns
///
/// An iterator over them, names and values.
pub fn vars_os() -> std::vec::IntoIter<(OsString, OsString)> {
    vars()
        .map(|(name, value)| (OsString::from(name), OsString::from(value)))
        .collect::<Vec<_>>()
        .into_iter()
}

/// Sets one of the program's variables.
///
/// # Arguments
///
/// * `name`  - Its name.
/// * `value` - Its value.
///
pub fn set_var(
    name: impl AsRef<std::ffi::OsStr>,
    value: impl AsRef<std::ffi::OsStr>,
) {
    if let Ok(mut env) = context::current().env.lock() {
        env.insert(
            name.as_ref().to_string_lossy().into_owned(),
            value.as_ref().to_string_lossy().into_owned(),
        );
    }
}

/// Takes one of the program's variables away.
///
/// # Arguments
///
/// * `name` - Its name.
///
pub fn remove_var(name: impl AsRef<std::ffi::OsStr>) {
    if let Ok(mut env) = context::current().env.lock() {
        env.remove(name.as_ref().to_string_lossy().as_ref());
    }
}

/// Where the program is: its name, under `/bin`, which is where the page's
/// programs are registered -- see [`crate::browser::process::register`].
///
/// # Returns
///
/// The path.
///
/// # Errors
///
/// None.
pub fn current_exe() -> std::io::Result<PathBuf> {
    let name = args().next().unwrap_or_default();
    let name = name.rsplit('/').next().unwrap_or_default().to_string();
    Ok(PathBuf::from("/bin").join(name))
}

/// The program's directory.
///
/// # Returns
///
/// It.
///
/// # Errors
///
/// None.
pub fn current_dir() -> std::io::Result<PathBuf> {
    Ok(context::current()
        .cwd
        .lock()
        .map(|cwd| cwd.clone())
        .unwrap_or_else(|_| PathBuf::from("/")))
}

/// Changes the program's directory.
///
/// # Arguments
///
/// * `to` - The directory.
///
/// # Errors
///
/// None.
pub fn set_current_dir(to: impl AsRef<std::path::Path>) -> std::io::Result<()> {
    let to = crate::browser::fs::absolute(to.as_ref());
    if let Ok(mut cwd) = context::current().cwd.lock() {
        *cwd = to;
    }
    Ok(())
}

/// Where temporary files go: `/tmp`, in the page's memory.
///
/// # Returns
///
/// The directory.
pub fn temp_dir() -> PathBuf {
    PathBuf::from("/tmp")
}
