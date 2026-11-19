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

/// Says on a window's canvas which part of the window must fit on a small
/// screen, `data-fit`, and which part keeps the touches that start on it,
/// `data-hold`, each as `x y width height` in the window's logical pixels:
/// the page's script scales and scrolls by them. Written only where they
/// changed.
///
/// # Arguments
///
/// * `canvas` - The canvas.
/// * `fit`    - What must fit.
/// * `hold`   - What keeps its touches.
pub fn set_regions(
    canvas: &web_sys::HtmlCanvasElement,
    fit: [f32; 4],
    hold: [f32; 4],
) {
    let element: &web_sys::Element = canvas.unchecked_ref();
    for (name, area) in [("data-fit", fit), ("data-hold", hold)] {
        let said = area
            .iter()
            .map(|n| format!("{n:.0}"))
            .collect::<Vec<_>>()
            .join(" ");
        if element.get_attribute(name).as_deref() != Some(said.as_str()) {
            let _ = element.set_attribute(name, &said);
        }
    }
}
