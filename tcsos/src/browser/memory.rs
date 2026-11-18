//! Memory, in a web page: what the page's WebAssembly memory is, and the
//! lock a page cannot take.
//!
//! A web page cannot lock memory into RAM: the browser keeps its pages as
//! it pleases. [`lock_all`] says so, as a refusal, and tcspecial treats it
//! as it treats any refused lock -- under `--simulate`, as fatal -- so the
//! page starts tcspecial with `--no-memlock`, as a desk does on Linux whose
//! limit on locked memory is too small.

/// What the page has of memory, in bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Figures {
    /// The WebAssembly memory's size.
    pub mapped: u64,
    /// The same: the browser does not say what of it is in RAM.
    pub resident: u64,
    /// Nought: nothing is locked.
    pub locked: u64,
}

/// Refuses: a web page cannot lock memory into RAM.
///
/// # Errors
///
/// Always, saying why.
pub fn lock_all() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "a web page cannot lock memory into RAM",
    ))
}

/// What to change for a refused lock, in words for the person who starts
/// the program: in a web page, nothing can be.
pub const RAISE_THE_LIMIT: &str = "A web page cannot lock its memory";

/// The limit on what may be locked: there is none to read.
///
/// # Returns
///
/// `None`.
pub fn lock_limit() -> Option<u64> {
    None
}

/// What the page has of memory now.
///
/// # Returns
///
/// The size of the WebAssembly memory, as both mapped and resident.
pub fn figures() -> Figures {
    let bytes = (core::arch::wasm32::memory_size(0) as u64) * 65536;
    Figures {
        mapped: bytes,
        resident: bytes,
        locked: 0,
    }
}

/// Writes a message to the browser's console and ends the program, which
/// in a page is to stop the WebAssembly where it is.
///
/// # Arguments
///
/// * `said`    - The message.
/// * `_status` - What a process would exit with; a page has no status.
pub fn say_and_end(said: &[u8], _status: i32) -> ! {
    crate::browser::console::error_bytes(said);
    core::arch::wasm32::unreachable()
}
