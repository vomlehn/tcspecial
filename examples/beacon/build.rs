fn main() {
    // The window is one panel, and the panel is tcsui's: the markup imports
    // it by path, which is what Slint resolves relative to the importing
    // file. Rebuilt when either changes, which Slint's own build glue
    // arranges by telling cargo about the files it read.
    // With debug info the compiled window keeps the names of its elements,
    // which is how the window test finds the text on it.
    slint_build::compile_with_config(
        "ui/watch.slint",
        slint_build::CompilerConfiguration::new().with_debug_info(true),
    )
    .unwrap();
}
