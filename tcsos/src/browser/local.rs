//! What each program has one of, in a page: a value for each program.
//!
//! On Linux every program is a process with statics of its own. In a page
//! every program is threads of one, sharing its memory, so a static would
//! be shared by every program that names it -- two tcspecials, a
//! supervisor and its child, the one before a restart and the one after --
//! and what one set, such as that it is stopping, the other would read. A
//! [`Local`] keeps a value for each program instead, made the first time
//! that program asks for it.

use std::collections::BTreeMap;

use crate::browser::context;
use crate::browser::sync::Mutex;

/// A static each program has its own of.
#[derive(Debug)]
pub struct Local<T: 'static> {
    /// Makes a program's value.
    made: fn() -> T,
    /// Each program's value, by its number: made once, and kept for as long
    /// as the page is, which a reference to it may be.
    values: Mutex<BTreeMap<u32, &'static T>>,
}

impl<T: Send + Sync + 'static> Local<T> {
    /// Makes the static.
    ///
    /// # Arguments
    ///
    /// * `value` - The value Linux's static is given, which here no
    ///             program has.
    /// * `made`  - What makes each program's: the same value, made again.
    ///
    /// # Returns
    ///
    /// The static.
    pub const fn new(value: T, made: fn() -> T) -> Local<T> {
        // Made at compile time and never used: nothing to drop.
        std::mem::forget(value);
        Local {
            made,
            values: Mutex::new(BTreeMap::new()),
        }
    }

    /// The calling program's value, made where it has none yet.
    ///
    /// # Returns
    ///
    /// It.
    pub fn get(&self) -> &T {
        let program = context::current().id;
        let mut values = self.values.lock().unwrap_or_else(|e| e.into_inner());
        values
            .entry(program)
            .or_insert_with(|| Box::leak(Box::new((self.made)())))
    }
}

impl<T: Send + Sync + 'static> std::ops::Deref for Local<T> {
    type Target = T;

    /// The calling program's value: see [`Local::get`].
    fn deref(&self) -> &T {
        self.get()
    }
}
