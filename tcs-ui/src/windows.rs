//! Where a program's windows are made and run.
//!
//! On Linux a program makes its windows on its own main thread and runs
//! them there until they close, which is what Slint's `run` does. In a
//! web page every program is a Web Worker, and windows can be made only on
//! the page's own thread, whose event loop never stops: so a program sends
//! the part of it that makes a window there, and waits for the window to
//! close. Written this way, the same code does both:
//!
//! ```text
//! tcs_ui::windows::on_window_thread(move |closed| {
//!     let ui = MainWindow::new().unwrap();
//!     ...
//!     tcs_ui::windows::run(ui, kept, closed).unwrap();
//! });
//! ```
//!
//! Whatever the part that is sent takes with it must therefore be `Send`;
//! what it makes there need not be.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, PlatformError};
use tcsos::sync::mpsc;
use tcsos::time::Duration;

/// How often a page looks whether a window it was sent has closed.
const LOOK: Duration = Duration::from_millis(100);

/// What a window's part calls once its window has closed, which lets the
/// program that sent it go on.
pub struct Closed(
    /// Says so to the program, where it waits on another thread; nothing
    /// where the part runs on the program's own.
    Option<mpsc::Sender<()>>,
);

impl Closed {
    /// Says that the windows have closed.
    fn say(self) {
        if let Some(sender) = self.0 {
            let _ = sender.send(());
        }
    }
}

/// Runs a part of the program on the thread its windows are made on, and
/// waits for the windows it runs to close.
///
/// # Arguments
///
/// * `part` - What makes the windows and runs them, with [`run`], given
///            what it says the windows have closed with.
pub fn on_window_thread(part: impl FnOnce(Closed) + Send + 'static) {
    if !tcsos::thread::windows_elsewhere() {
        part(Closed(None));
        return;
    }
    let (sender, closed) = mpsc::channel();
    let sent =
        slint::invoke_from_event_loop(move || part(Closed(Some(sender))));
    if sent.is_ok() {
        // Ended either way: closed, or the page's thread gone.
        let _ = closed.recv();
    }
}

/// Runs a window until it closes: until it is hidden, by the program or
/// by whoever is looking at it.
///
/// On Linux this is Slint's `run`, which returns once it has closed. In a
/// page it shows the window and returns at once, keeping the window, and
/// what it is given to keep, until the window closes; then it says so with
/// `closed`.
///
/// # Arguments
///
/// * `window` - The window.
/// * `kept`   - What lives as long as the window does: its timers, and
///              what they reach.
/// * `closed` - What [`on_window_thread`] gave the part this is called
///              from.
///
/// # Returns
///
/// Once the window has closed, on Linux; once it is shown, in a page.
///
/// # Errors
///
/// Where the window cannot be shown.
pub fn run<C, K>(
    window: C,
    kept: K,
    closed: Closed,
) -> Result<(), PlatformError>
where
    C: ComponentHandle + 'static,
    K: 'static,
{
    if closed.0.is_none() && !cfg!(target_arch = "wasm32") {
        let ran = window.run();
        drop(kept);
        return ran;
    }
    show(&window)?;
    watch(Rc::new(RefCell::new(Some((window, kept, closed)))));
    Ok(())
}

/// Shows a window, in a page as well as to Slint.
///
/// # Arguments
///
/// * `window` - The window.
///
/// # Errors
///
/// Where it cannot be shown.
pub fn show<C: ComponentHandle + 'static>(
    window: &C,
) -> Result<(), PlatformError> {
    window.show()?;
    on_page(window, true);
    Ok(())
}

/// Hides a window, in a page as well as to Slint.
///
/// # Arguments
///
/// * `window` - The window.
///
/// # Errors
///
/// Where it cannot be hidden.
pub fn hide<C: ComponentHandle + 'static>(
    window: &C,
) -> Result<(), PlatformError> {
    window.hide()?;
    on_page(window, false);
    Ok(())
}

/// Shows or hides the canvas a window is drawn on, in a page: a browser's
/// windowing does nothing with a window shown or hidden. See tcsos's
/// `browser::page`.
///
/// # Arguments
///
/// * `window` - The window.
/// * `shown`  - Whether it is shown.
#[cfg(target_arch = "wasm32")]
fn on_page<C: ComponentHandle + 'static>(window: &C, shown: bool) {
    use slint::winit_030::winit::platform::web::WindowExtWebSys;
    use slint::winit_030::WinitWindowAccessor;

    let found = window.window().with_winit_window(|winit| {
        if let Some(canvas) = winit.canvas() {
            tcsos::browser::page::show_canvas(&canvas, shown);
        }
    });
    // Slint makes a window's canvas its own on the event loop, after show
    // has returned: looked for again until it has, unless the window has
    // been hidden or shown again since.
    if found.is_none() {
        let weak = window.as_weak();
        slint::Timer::single_shot(LOOK, move || {
            if let Some(window) = weak.upgrade() {
                if window.window().is_visible() == shown {
                    on_page(&window, shown);
                }
            }
        });
    }
}

/// Nothing, but in a page: a window manager shows and hides what it draws.
///
/// # Arguments
///
/// * `_window` - The window.
/// * `_shown`  - Whether it is shown.
#[cfg(not(target_arch = "wasm32"))]
fn on_page<C: ComponentHandle + 'static>(_window: &C, _shown: bool) {}

/// Looks whether a window has closed, and again later until it has; then
/// drops it and what was kept with it, and says so.
///
/// # Arguments
///
/// * `shown` - The window, what was kept with it, and what to say it closed
///             with; empty once it has.
fn watch<C, K>(shown: Rc<RefCell<Option<(C, K, Closed)>>>)
where
    C: ComponentHandle + 'static,
    K: 'static,
{
    let open = match &*shown.borrow() {
        Some((window, _, _)) => window.window().is_visible(),
        None => return,
    };
    if open {
        slint::Timer::single_shot(LOOK, move || watch(shown));
        return;
    }
    if let Some((window, kept, closed)) = shown.borrow_mut().take() {
        on_page(&window, false);
        drop(kept);
        drop(window);
        closed.say();
    }
}
