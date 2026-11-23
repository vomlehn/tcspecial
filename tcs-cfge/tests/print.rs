//! `tcs-cfge --print`, run as a person runs it: every page of a
//! configuration printed to a PDF with no window on a screen, and nothing
//! else written.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The examples' directory in the checkout.
///
/// # Returns
///
/// Its path.
fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples")
}

/// Runs tcs-cfge with these words.
///
/// # Arguments
///
/// * `words` - The command line, without the program's name.
///
/// # Returns
///
/// What it did.
///
/// # Panics
///
/// If it cannot be started.
fn tcs_cfge(words: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tcs-cfge"))
        .args(words)
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DISPLAY")
        .output()
        .expect("tcs-cfge starts")
}

/// The thermostat, whose top page holds a subdiagram, is printed as two
/// pages, each headed by which page it is; and its files are as they were.
#[test]
fn every_page_of_a_configuration_is_printed() {
    let out = tempfile::tempdir().expect("a directory");
    let pdf = out.path().join("thermostat.pdf");
    let dir = examples().join("thermostat");
    let files = ["thermostat.ytcs", "thermostat.xtcs"]
        .map(|name| dir.join(name))
        .into_iter()
        .chain([dir.join("../thermostat-common/thermostat.ytcs")])
        .chain([dir.join("../thermostat-common/thermostat.xtcs")])
        .collect::<Vec<_>>();
    let before: Vec<Vec<u8>> = files
        .iter()
        .map(|file| std::fs::read(file).expect("the example"))
        .collect();

    let done = tcs_cfge(&[
        "--print".as_ref(),
        pdf.as_os_str(),
        dir.join("thermostat.ytcs").as_os_str(),
        dir.join("thermostat-sim.yaml").as_os_str(),
    ]);
    let said = String::from_utf8_lossy(&done.stderr);
    assert!(done.status.success(), "{said}");
    assert!(said.contains("2 pages"), "{said}");

    let bytes = std::fs::read(&pdf).expect("the PDF");
    let has = |what: &str| {
        bytes
            .windows(what.len())
            .any(|window| window == what.as_bytes())
    };
    assert!(bytes.starts_with(b"%PDF-1.4"));
    assert!(has("/Count 2"), "two pages");
    assert!(has("(thermostat.ytcs) Tj"), "the top page's heading");
    assert!(
        has("(thermostat.ytcs: subdiagram thermostat-common, from "),
        "the subdiagram's heading"
    );
    let after: Vec<Vec<u8>> = files
        .iter()
        .map(|file| std::fs::read(file).expect("the example"))
        .collect();
    assert!(before == after, "a print writes nothing but the PDF");
    let left: Vec<_> = std::fs::read_dir(out.path())
        .expect("the directory")
        .map(|entry| entry.expect("an entry").file_name())
        .collect();
    assert_eq!(left, ["thermostat.pdf"]);
}

/// A configuration that is not there is refused with status 1, and a
/// command line that names no PDF after `--print` with status 2, which
/// would otherwise write the picture over the configuration; neither
/// writes anything.
#[test]
fn a_print_of_nothing_is_refused() {
    let out = tempfile::tempdir().expect("a directory");
    let pdf = out.path().join("missing.pdf");
    let missing = out.path().join("missing.ytcs");
    let done =
        tcs_cfge(&["--print".as_ref(), pdf.as_os_str(), missing.as_os_str()]);
    assert_eq!(done.status.code(), Some(1));
    let said = String::from_utf8_lossy(&done.stderr);
    assert!(said.contains("nothing to print"), "{said}");

    let config = out.path().join("kept.ytcs");
    std::fs::write(&config, "payloads: []\n").expect("a configuration");
    let done = tcs_cfge(&["--print".as_ref(), config.as_os_str()]);
    assert_eq!(done.status.code(), Some(2));
    assert_eq!(
        std::fs::read_to_string(&config).expect("still there"),
        "payloads: []\n"
    );
    let mut left: Vec<_> = std::fs::read_dir(out.path())
        .expect("the directory")
        .map(|entry| entry.expect("an entry").file_name())
        .collect();
    left.sort();
    assert_eq!(left, ["kept.ytcs"]);
}

/// Copies a directory's files, links followed, into another.
///
/// # Arguments
///
/// * `from` - The directory copied.
/// * `to`   - Where its files go, made if it is not there.
///
/// # Panics
///
/// If a file cannot be copied.
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("the directory");
    for entry in std::fs::read_dir(from).expect("the directory") {
        let path = entry.expect("an entry").path();
        if tcsos::fs::metadata(&path).is_ok_and(|m| m.is_file()) {
            std::fs::copy(&path, to.join(path.file_name().unwrap()))
                .expect("the file");
        }
    }
}

/// Printed only if it has changed, a PDF is printed again when a file the
/// configuration includes changes, and not when nothing has: the digest it
/// records is of every file it was printed from, the included ones too.
#[test]
fn a_print_is_made_again_when_an_included_file_changes() {
    let work = tempfile::tempdir().expect("a directory");
    for name in ["thermostat", "thermostat-common"] {
        copy_dir(&examples().join(name), &work.path().join(name));
    }
    let dir = work.path().join("thermostat");
    let pdf = dir.join("thermostat.pdf");
    let _ = std::fs::remove_file(&pdf);
    let print = || {
        let done = tcs_cfge(&[
            "--print".as_ref(),
            pdf.as_os_str(),
            "--if-changed".as_ref(),
            dir.join("thermostat.ytcs").as_os_str(),
            dir.join("thermostat-sim.yaml").as_os_str(),
        ]);
        let said = String::from_utf8_lossy(&done.stderr).into_owned();
        assert!(done.status.success(), "{said}");
        said
    };

    assert!(print().contains("printed"), "no PDF yet: printed");
    assert!(print().contains("is up to date"), "nothing changed");

    let included = work.path().join("thermostat-common/thermostat.ytcs");
    let mut text = std::fs::read_to_string(&included).expect("the file");
    text.push_str("# A line added.\n");
    std::fs::write(&included, text).expect("changed");
    assert!(print().contains("printed"), "the included file changed");
    assert!(print().contains("is up to date"), "and is printed from now");
}
