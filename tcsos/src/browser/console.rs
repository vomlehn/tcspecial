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

#[wasm_bindgen]
extern "C" {
    /// The console's `log`, which shows a line plainly.
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn console_log(said: &str);
}

/// What every program thread prints, with `print!` and `eprint!`, until it
/// is shown: a page has no standard output or error of its own, and the
/// standard library's prints nothing there.
static PRINTED: std::sync::OnceLock<std::sync::Arc<std::sync::Mutex<Vec<u8>>>> =
    std::sync::OnceLock::new();

/// Whether the thread that shows what is printed has been started.
static SHOWING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// How often what is printed is shown.
const SHOW_EVERY: std::time::Duration = std::time::Duration::from_millis(100);

/// Has what the calling thread prints shown in the console, a line at a
/// time. Called as each program thread starts: the page's own thread is
/// left alone, since the capture is behind a lock it may not wait for.
pub fn capture_output() {
    let printed = PRINTED.get_or_init(Default::default).clone();
    std::io::set_output_capture(Some(printed));
    if !SHOWING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        crate::browser::thread::spawn(show_printed);
    }
}

/// Shows what has been printed, each whole line as it comes, for as long as
/// the page lasts.
fn show_printed() {
    let Some(printed) = PRINTED.get() else { return };
    let mut partial: Vec<u8> = Vec::new();
    loop {
        crate::browser::thread::sleep(SHOW_EVERY);
        if let Ok(mut held) = printed.lock() {
            partial.extend(held.drain(..));
        }
        if let Some(end) = partial.iter().rposition(|&b| b == b'\n') {
            let lines: Vec<u8> = partial.drain(..=end).collect();
            for line in String::from_utf8_lossy(&lines).lines() {
                console_log(line);
            }
        }
    }
}

/// Has what the page's own thread prints -- the windows' callbacks, which
/// run there -- shown in the console, a line at a time. Its own buffer,
/// which no other thread touches, so the page's thread never waits for its
/// lock; shown by the browser's timer, on the same thread.
pub fn capture_page_output() {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let printed: std::sync::Arc<std::sync::Mutex<Vec<u8>>> = Default::default();
    std::io::set_output_capture(Some(printed.clone()));
    let mut partial: Vec<u8> = Vec::new();
    let show = Closure::<dyn FnMut()>::new(move || {
        if let Ok(mut held) = printed.try_lock() {
            partial.extend(held.drain(..));
        }
        if let Some(end) = partial.iter().rposition(|&b| b == b'\n') {
            let lines: Vec<u8> = partial.drain(..=end).collect();
            for line in String::from_utf8_lossy(&lines).lines() {
                console_log(line);
            }
        }
    });
    if let Some(window) = web_sys::window() {
        let _ = window.set_interval_with_callback_and_timeout_and_arguments_0(
            show.as_ref().unchecked_ref(),
            SHOW_EVERY.as_millis() as i32,
        );
    }
    // Shown for as long as the page lasts.
    show.forget();
}
