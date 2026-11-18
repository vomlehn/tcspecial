/// Compiles the station's window, `ui/station.slint`, with debug info.
///
/// # Panics
///
/// The markup does not compile.
fn main() {
    // The landscape and the lander are tcs-sim's pieces, imported by path,
    // so that the station draws them as tcs-sim does. With debug info the
    // compiled window keeps the names of its elements, which is how a test
    // finds the text on it.
    slint_build::compile_with_config(
        "ui/station.slint",
        slint_build::CompilerConfiguration::new().with_debug_info(true),
    )
    .unwrap();
}
