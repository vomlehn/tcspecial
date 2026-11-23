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

/// What of a window a page shows: the part of its canvas on the screen,
/// as `x y width height` in the window's logical pixels -- nothing wide for
/// none -- and whether any other window's canvas is on the screen too.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InView {
    /// The part of this window on the screen.
    pub seen: [f32; 4],
    /// Whether any of another window is on the screen.
    pub others: bool,
}

/// Works out what of a window the page shows now.
///
/// # Arguments
///
/// * `canvas` - The window's canvas.
///
/// # Returns
///
/// What is shown; none where there is no page to ask.
fn in_view(canvas: &web_sys::HtmlCanvasElement) -> Option<InView> {
    let page = web_sys::window()?;
    let across = page.inner_width().ok()?.as_f64()?;
    let down = page.inner_height().ok()?.as_f64()?;
    // What of a box on the screen is on the screen, in the page's pixels.
    let shown = |element: &web_sys::Element| {
        let r = element.get_bounding_client_rect();
        let (x0, y0) = (r.left().max(0.0), r.top().max(0.0));
        let (x1, y1) = (r.right().min(across), r.bottom().min(down));
        (r, (x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)))
    };
    let element: &web_sys::Element = canvas.unchecked_ref();
    let (whole, (x, y, w, h)) = shown(element);
    // The canvas is scaled by the page to fit, its own size untouched: what
    // the page shows is said in the window's own pixels.
    let scale = whole.width() / f64::from(element.client_width().max(1));
    let seen = if w > 0.0 && h > 0.0 && scale > 0.0 {
        [
            ((x - whole.left()) / scale) as f32,
            ((y - whole.top()) / scale) as f32,
            (w / scale) as f32,
            (h / scale) as f32,
        ]
    } else {
        [0.0; 4]
    };
    let mut others = false;
    if let Ok(all) = page.document()?.query_selector_all("canvas") {
        for n in 0..all.length() {
            let Some(other) = all.item(n) else { continue };
            let other: web_sys::Element = other.unchecked_into();
            if other == *element {
                continue;
            }
            let (_, (_, _, w, h)) = shown(&other);
            others |= w > 0.0 && h > 0.0;
        }
    }
    Some(InView { seen, others })
}

/// Tells a window what of it the page shows, now and whenever that may
/// change: as anything on the page scrolls -- the page itself, or a frame
/// scrolled across a window wider than the screen -- as the page is
/// resized, and as the window's canvas is. Told on those events, and never
/// asked for between them.
///
/// # Arguments
///
/// * `canvas` - The window's canvas.
/// * `told`   - What is told: see [`InView`].
pub fn watch_view(
    canvas: &web_sys::HtmlCanvasElement,
    told: impl FnMut(InView) + 'static,
) {
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen::closure::Closure;

    let told = Rc::new(RefCell::new(told));
    let watched = canvas.clone();
    let say = {
        let told = Rc::clone(&told);
        move || {
            if let Some(view) = in_view(&watched) {
                (told.borrow_mut())(view);
            }
        }
    };
    say();
    let changed = Closure::<dyn FnMut()>::new(say);
    let callback = changed
        .as_ref()
        .unchecked_ref::<web_sys::js_sys::Function>();
    if let Some(page) = web_sys::window() {
        // Capturing, at the document: a scroll does not rise from the frame
        // it happens in.
        if let Some(document) = page.document() {
            let _ = document.add_event_listener_with_callback_and_bool(
                "scroll", callback, true,
            );
        }
        let _ = page.add_event_listener_with_callback("resize", callback);
    }
    if let Ok(observer) = web_sys::ResizeObserver::new(callback) {
        observer.observe(canvas);
        // Watched for as long as the page lives, as the window is.
        std::mem::forget(observer);
    }
    changed.forget();
}
