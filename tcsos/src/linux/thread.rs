//! What the system says of a thread that the standard library does not.

/// The calling thread's stack, as glibc has it.
///
/// # Returns
///
/// The lowest address it may use, one past its highest, and its size in
/// bytes; `None` where glibc cannot say. A thread's stack excludes the
/// guard below it. The main thread's lowest is as far down as `ulimit -s`
/// lets it grow.
pub fn stack_bounds() -> Option<(usize, usize, usize)> {
    // SAFETY: an attribute object that is initialised by the call that
    // fills it, read only once that call has said it succeeded, and
    // destroyed once read.
    unsafe {
        let mut attr: libc::pthread_attr_t = std::mem::zeroed();
        if libc::pthread_getattr_np(libc::pthread_self(), &mut attr) != 0 {
            return None;
        }
        let mut addr: *mut libc::c_void = std::ptr::null_mut();
        let mut size: libc::size_t = 0;
        let got = libc::pthread_attr_getstack(&attr, &mut addr, &mut size);
        libc::pthread_attr_destroy(&mut attr);
        (got == 0 && !addr.is_null())
            .then(|| (addr as usize, addr as usize + size, size))
    }
}
