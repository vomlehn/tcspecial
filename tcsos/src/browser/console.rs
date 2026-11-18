//! The browser's console, which stands in for a program's error output.

use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    /// The console's `error`, which shows a line in red.
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(said: &str);
}

/// Shows bytes in the console as an error, as much of them as is text.
///
/// # Arguments
///
/// * `said` - What to show.
pub fn error_bytes(said: &[u8]) {
    console_error(&String::from_utf8_lossy(said));
}
