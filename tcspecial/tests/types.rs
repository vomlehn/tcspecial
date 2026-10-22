//! `--types` says which payload types this build contains, and the startup
//! summary says them too.
//!
//! The answer is read off the binary that will fly rather than recalled from
//! how it was built, and it is in the words `tcsverify --types=` takes, so
//! that one can be handed to the other.

use std::process::Command;

use tcslibgs::graph::built::{types_from, OPTIONAL_TYPES};

#[test]
fn types_prints_what_was_built_in_the_words_tcsverify_takes() {
    let ran = Command::new(env!("CARGO_BIN_EXE_tcspecial"))
        .arg("--types")
        .output()
        .expect("tcspecial runs");
    assert!(ran.status.success(), "asked for, so a success");
    let said = String::from_utf8_lossy(&ran.stdout);
    let said = said.trim_end();

    assert_eq!(said, tcspecial::node::payload::BUILT.join(","));
    // What tcsverify reads it as is what was built.
    assert_eq!(
        types_from(said).as_deref(),
        Ok(tcspecial::node::payload::BUILT)
    );
    // A test is built with the defaults, which are every type.
    assert_eq!(said, OPTIONAL_TYPES.join(","));
}

/// Said as it starts, before the configuration is read: a start refused for
/// want of a type is then read beside the list of what the build has.
#[test]
fn the_summary_says_which_types_were_built() {
    let ran = Command::new(env!("CARGO_BIN_EXE_tcspecial"))
        .arg("/nonexistent/tcspecial-types-test.yaml")
        .env_remove("RUST_LOG")
        .output()
        .expect("tcspecial runs");
    assert!(
        !ran.status.success(),
        "a file that is not there is not flown"
    );
    let said = String::from_utf8_lossy(&ran.stderr);
    assert!(
        said.contains(&format!(
            "payload types built: {}",
            tcspecial::node::payload::BUILT.join(", ")
        )),
        "{said}"
    );
}
