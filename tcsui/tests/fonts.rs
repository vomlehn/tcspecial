//! Every window takes its default type size from `ui/fonts.slint`.
//!
//! The windows are in several crates, and each says its default-font-size
//! itself, so a window added without it would be set in Slint's own default
//! and look like it belonged to another program. Read from the files rather
//! than from a list here, so that a new window is held to it without anyone
//! naming it; and the Python windows' figure is held to the same one.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Finds every `.slint` file under a directory, but for build output and
/// hidden directories.
///
/// # Arguments
///
/// * `dir`   - The directory to search, recursively.
/// * `found` - Where each file's path is added.
///
/// # Panics
///
/// If a directory or one of its entries cannot be read.
fn slint_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("a directory") {
        let path = entry.expect("an entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if name != "target" && !name.starts_with('.') {
                slint_files(&path, found);
            }
        } else if name.ends_with(".slint") {
            found.push(path);
        }
    }
}

#[test]
fn every_window_is_set_in_the_one_default_size() {
    let mut files = Vec::new();
    slint_files(&root(), &mut files);

    let mut windows = 0;
    for file in &files {
        let text = std::fs::read_to_string(file).expect("it reads");
        let lines: Vec<&str> = text.lines().collect();
        for (at, line) in lines.iter().enumerate() {
            if line.contains(" inherits Window {") {
                windows += 1;
                assert_eq!(
                    lines.get(at + 1).map(|next| next.trim()),
                    Some("default-font-size: Fonts.window;"),
                    "{}:{}: a window that is not set in Fonts.window",
                    file.display(),
                    at + 1
                );
            }
        }
    }
    // At least one, so that a walk that went wrong and found nothing
    // fails rather than passing; not a count, which would change with
    // every window added or taken away.
    assert!(windows > 0, "no windows were found");
}

#[test]
fn the_python_windows_use_the_same_size() {
    let slint = std::fs::read_to_string(root().join("tcsui/ui/fonts.slint"))
        .expect("it reads");
    let python = std::fs::read_to_string(root().join("python/tcsui/fonts.py"))
        .expect("it reads");

    let points = slint
        .lines()
        .find_map(|line| {
            line.trim().strip_prefix("out property <length> window: ")
        })
        .and_then(|rest| rest.strip_suffix("pt;"))
        .expect("fonts.slint states the size in points");
    assert_eq!(points, "12");
    assert!(
        python
            .lines()
            .any(|line| line == format!("POINTS = {points}")),
        "python/tcsui/fonts.py does not say POINTS = {points}"
    );
}

/// Every text box and every button is set in the one size too: it either
/// names no size, and so takes the window's, or names `Fonts.window`.
///
/// A box or a button with a size of its own was how they came to be smaller
/// than the text around them -- tcsmoc's attribute fields were 11 pixels --
/// so a size is allowed only if it is the one size.
#[test]
fn every_text_box_and_button_is_set_in_the_one_size() {
    let mut files = Vec::new();
    slint_files(&root(), &mut files);

    let mut checked = 0;
    for file in &files {
        let text = std::fs::read_to_string(file).expect("it reads");
        let lines: Vec<&str> = text.lines().collect();
        for (at, line) in lines.iter().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            let opens =
                ["Button {", "LineEdit {", "TextEdit {"]
                    .iter()
                    .any(|widget| {
                        code.trim_start().starts_with(widget)
                            || code.contains(&format!(":= {widget}"))
                    });
            if !opens {
                continue;
            }
            checked += 1;
            // The widget's own block, to its closing brace.
            let mut depth = 0i32;
            for (offset, inner) in lines[at..].iter().enumerate() {
                let inner = inner.split("//").next().unwrap_or("");
                if depth == 1 {
                    if let Some(size) = inner.trim().strip_prefix("font-size:")
                    {
                        assert_eq!(
                            size.trim(),
                            "Fonts.window;",
                            "{}:{}: a text box or button set in a size of \
                             its own",
                            file.display(),
                            at + offset + 1
                        );
                    }
                }
                depth += inner.matches('{').count() as i32;
                depth -= inner.matches('}').count() as i32;
                if depth <= 0 && offset > 0 {
                    break;
                }
                if depth <= 0 && inner.contains('}') {
                    break;
                }
            }
        }
    }
    assert!(
        checked >= 20,
        "only {checked} text boxes and buttons were found"
    );
}

/// What a size written in a `.slint` file comes to, in points, where it is a
/// length written out: `16px` or `12pt`.
///
/// # Arguments
///
/// * `size` - The size as written, a number followed by `px` or `pt`.
///
/// # Returns
///
/// The size in points, or `None` where it is not a length written out,
/// such as `Fonts.window`.
fn points(size: &str) -> Option<f32> {
    if let Some(px) = size.strip_suffix("px") {
        // Slint's logical pixel is a 96th of an inch, a point a 72nd.
        return px.trim().parse::<f32>().ok().map(|px| px * 72.0 / 96.0);
    }
    size.strip_suffix("pt")?.trim().parse().ok()
}

/// No text in any window is set smaller than the one size.
///
/// Every `font-size` in every `.slint` file is `Fonts.window`, a length of
/// at least that, or one of tcsgui's `Type` names -- whose definitions are
/// held to the same rule here. The one exception is a node's label on
/// tcsgui's canvas, whose size is one of the ways node types are told apart
/// (see `tcsgui/src/style.rs`). And the Python windows are held to it too:
/// a font they name with a size names the size from `tcsui.fonts`.
#[test]
fn no_text_is_set_smaller_than_the_one_size() {
    let mut files = Vec::new();
    slint_files(&root(), &mut files);

    for file in &files {
        let text = std::fs::read_to_string(file).expect("it reads");
        for (at, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("").trim();
            // A Type definition, `small: Fonts.window;`, and every use.
            let size = if let Some(size) = code.strip_prefix("font-size:") {
                size
            } else if let Some(rest) =
                code.strip_prefix("out property <length> ")
            {
                match rest.split_once(':') {
                    Some((name, size))
                        if ["small", "body", "strong", "title"]
                            .contains(&name) =>
                    {
                        size
                    }
                    _ => continue,
                }
            } else {
                continue;
            };
            let size = size.trim().trim_end_matches(';').trim();
            let allowed = size == "Fonts.window"
                || size.starts_with("Type.")
                || size == "node.label-size"
                || points(size).is_some_and(|pt| pt >= 12.0);
            assert!(
                allowed,
                "{}:{}: text set in {size}, smaller than 12 points",
                file.display(),
                at + 1
            );
        }
    }

    for python in [
        "python/tcsui/beacon_panel.py",
        "examples/ping/ping_gateway.py",
    ] {
        let text =
            std::fs::read_to_string(root().join(python)).expect("it reads");
        for (at, line) in text.lines().enumerate() {
            if let Some(font) = line.split("font=(").nth(1) {
                let size = font.split(',').nth(1).unwrap_or("").trim();
                assert!(
                    size.starts_with("fonts.POINTS"),
                    "{python}:{}: a font named with a size other than \
                     fonts.POINTS",
                    at + 1
                );
            }
        }
        assert!(!text.contains(", 9)"), "{python}: a nine-point font");
    }
}
