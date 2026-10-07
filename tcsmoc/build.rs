fn main() {
    // With debug info the compiled window keeps the names of its elements,
    // which is how the layout tests find them: they open the window
    // headlessly through Slint's testing backend and measure where things
    // landed. Without it the searches find nothing and those tests fail, so
    // this is not a thing to turn off to save a few bytes.
    slint_build::compile_with_config(
        "ui/main.slint",
        slint_build::CompilerConfiguration::new().with_debug_info(true),
    )
    .unwrap();
}
