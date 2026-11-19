//! What each program has one of, on Linux: a static, every program being a
//! process with statics of its own.
//!
//! In a web page every program shares one memory, and the browser side
//! keeps one value for each program instead: see `browser::local`. Written
//! as a [`Local`], a static is the one on Linux and the other in a page.

/// A static each program has its own of: on Linux, the static itself.
#[derive(Debug)]
pub struct Local<T: 'static> {
    /// The value.
    value: T,
}

impl<T: Send + Sync + 'static> Local<T> {
    /// Makes the static.
    ///
    /// # Arguments
    ///
    /// * `value` - Its value.
    /// * `_made` - What makes another program's, in a page: the same
    ///             value, made again. Written `|| value`, the same
    ///             expression.
    ///
    /// # Returns
    ///
    /// The static.
    pub const fn new(value: T, _made: fn() -> T) -> Local<T> {
        Local { value }
    }

    /// The calling program's value.
    ///
    /// # Returns
    ///
    /// It.
    pub fn get(&self) -> &T {
        &self.value
    }
}

impl<T: Send + Sync + 'static> std::ops::Deref for Local<T> {
    type Target = T;

    /// The calling program's value: see [`Local::get`].
    fn deref(&self) -> &T {
        self.get()
    }
}
