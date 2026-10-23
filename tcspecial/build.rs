//! Compiles the visual window's markup, for a build with the `visual`
//! feature and for no other: see src/visual. A tcspecial that flies has no
//! window, so its build has nothing to do here.

fn main() {
    #[cfg(feature = "visual")]
    slint_build::compile("ui/visual.slint").expect("the visual window's markup compiles");
}
