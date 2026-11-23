//! Windows with no screen: each drawn into memory by Slint's software
//! renderer.
//!
//! What `tcs-cfge --print` draws its pages in, so that a configuration is
//! printed to a PDF with no desktop at all -- the examples' pictures are
//! made so -- and what the manual's screenshots are taken in. Set as the
//! platform before any window is made, and only then: a process has one.

use std::cell::RefCell;
use std::rc::Rc;

use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter};

thread_local! {
    /// Every window made on this platform, in the order they were made:
    /// what [`drawn`] finds a window's buffer by.
    static MADE: RefCell<Vec<Rc<MinimalSoftwareWindow>>> =
        const { RefCell::new(Vec::new()) };
}

/// A platform with no screen: each window drawn into memory.
pub struct Headless;

impl Platform for Headless {
    /// Makes a window drawn into memory, and keeps it for [`drawn`].
    ///
    /// # Returns
    ///
    /// The window's adapter.
    ///
    /// # Errors
    ///
    /// None: a window in memory can always be made.
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        MADE.with(|made| made.borrow_mut().push(window.clone()));
        Ok(window)
    }
}

/// Draws a window into memory, at a size, after letting whatever was
/// waiting on a timer run.
///
/// # Arguments
///
/// * `window` - The window.
/// * `size`   - Its width and height, in pixels.
///
/// # Returns
///
/// The pixels, row by row, as red, green, blue and alpha.
///
/// # Panics
///
/// If the window was not made by [`Headless`].
pub fn drawn(window: &slint::Window, size: (u32, u32)) -> Vec<u8> {
    let (width, height) = size;
    let adapter = MADE.with(|made| {
        made.borrow()
            .iter()
            .find(|made| std::ptr::eq(made.window(), window))
            .cloned()
            .expect("a window the platform made")
    });
    adapter.set_size(slint::PhysicalSize::new(width, height));
    slint::platform::update_timers_and_animations();
    adapter.request_redraw();
    let mut pixels =
        vec![PremultipliedRgbaColor::default(); (width * height) as usize];
    adapter.draw_if_needed(|renderer| {
        renderer.render(&mut pixels, width as usize);
    });
    pixels
        .iter()
        .flat_map(|p| [p.red, p.green, p.blue, p.alpha])
        .collect()
}
