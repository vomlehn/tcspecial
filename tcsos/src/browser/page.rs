//! The page itself: what the windows are drawn on.
//!
//! Each window is drawn on a canvas of its own, which the page makes as the
//! window is made, inside a frame that is hidden until the window is shown.
//! A browser's windowing does nothing when a window is shown or hidden, so
//! the window's program says so here, and the frame is shown or hidden
//! with it.

use wasm_bindgen::JsCast;

/// Shows or hides the frame a window's canvas is in.
///
/// # Arguments
///
/// * `canvas` - The canvas.
/// * `shown`  - Whether the window is shown.
pub fn show_canvas(canvas: &web_sys::HtmlCanvasElement, shown: bool) {
    let element: &web_sys::Element = canvas.unchecked_ref();
    let frame = element.parent_element();
    let target = frame.as_ref().unwrap_or(element);
    let _ = target.toggle_attribute_with_force("hidden", !shown);
}
