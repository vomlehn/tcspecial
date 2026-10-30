/// Gives `libtcs.so` its soname, `libtcs.so.<ABI>`.
///
/// A shared library without one is linked into a program by whatever name it
/// was found under, so a program built against `libtcs.so` would ask for
/// `libtcs.so` at run time -- the development symlink, which points at
/// whichever library was installed last. With the soname, a program asks for
/// the ABI it was built against, and is refused by the loader, rather than
/// misbehaving, on a machine that has only an incompatible one.
///
/// The ABI version is `abi-version`, a whole number on one line, kept apart
/// from the crate's version because it changes for another reason: it goes
/// up by one when, and only when, a program built against the old `tcs.h`
/// would no longer work with the new library -- a function's signature or
/// behaviour changing incompatibly, a function removed, or a struct or enum
/// in `tcs.h` changing its layout or its values. Adding a function, or
/// anything else an old program cannot notice, leaves it alone. The
/// Makefile reads the same file for the names it installs the library
/// under, so the two cannot disagree.
///
/// Only the cdylib is told: the static library and the rlib have no soname,
/// and the crate's own tests link with those.
///
/// # Panics
///
/// If `abi-version` cannot be read or does not hold a whole number.
fn main() {
    println!("cargo:rerun-if-changed=abi-version");
    let said = std::fs::read_to_string("abi-version")
        .expect("tcslib-c/abi-version, the library's ABI version, is readable");
    let abi: u32 = said.trim().parse().unwrap_or_else(|_| {
        panic!(
            "tcslib-c/abi-version holds {:?}: it must be one whole number, \
             the library's ABI version",
            said.trim()
        )
    });
    println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libtcs.so.{abi}");
}
