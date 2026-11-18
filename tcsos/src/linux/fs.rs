//! What the TCS programs ask of the file system that the standard library
//! does not give, or gives only by allocating.

use std::ffi::CStr;
use std::path::Path;

/// The most bytes a path can have, its nul included: room enough for a
/// buffer that holds any path the system takes.
pub const PATH_BYTES: usize = libc::PATH_MAX as usize;

/// Renames a file, from names already nul-terminated, without allocating.
///
/// The standard library's `rename` makes a C string of each path, on the
/// stack where it is short and on the heap where it is not; this is for a
/// caller that must not allocate however long its names are.
///
/// # Arguments
///
/// * `from` - The file's name.
/// * `to`   - Its new name.
///
/// # Errors
///
/// What the system said.
pub fn rename(from: &CStr, to: &CStr) -> std::io::Result<()> {
    // SAFETY: both are nul-terminated buffers that outlive the call.
    if unsafe { libc::rename(from.as_ptr(), to.as_ptr()) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Whether this process may run the program at a path.
///
/// # Arguments
///
/// * `program` - The path.
///
/// # Returns
///
/// `true` where it may: the file is executable by this user.
pub fn may_run(program: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    let Ok(name) = std::ffi::CString::new(program.as_os_str().as_bytes())
    else {
        return false;
    };
    // SAFETY: a nul-terminated name that outlives the call.
    unsafe { libc::access(name.as_ptr(), libc::X_OK) == 0 }
}

/// Sets a file's mode, its permission bits.
///
/// # Arguments
///
/// * `path` - The file.
/// * `mode` - The bits, as `chmod` takes them: 0o755.
///
/// # Errors
///
/// What the system said.
pub fn set_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

/// A file's mode, its permission bits.
///
/// # Arguments
///
/// * `path` - The file.
///
/// # Returns
///
/// The bits, as `chmod` takes them.
///
/// # Errors
///
/// What the system said.
pub fn mode(path: &Path) -> std::io::Result<u32> {
    use std::os::unix::fs::PermissionsExt;

    Ok(std::fs::metadata(path)?.permissions().mode() & 0o7777)
}

/// Whether a lock was taken, by [`lock_one_byte`].
#[derive(Debug)]
pub enum Taken {
    /// It was, and is held for as long as the file is open.
    Yes,
    /// Another open of the file holds it.
    Elsewhere,
    /// The file cannot be locked, the system says why: a file system or a
    /// driver without locks.
    Cannot(std::io::Error),
}

/// Takes the byte at `at` of `file` for this open of it, without waiting:
/// an open file description lock, which the standard library's whole-file
/// `try_lock` cannot give, held by this open and not by the process, so
/// that two opens of one I2C bus can hold two of its addresses.
///
/// # Arguments
///
/// * `file` - The open file, opened for writing.
/// * `at`   - Which byte.
///
/// # Returns
///
/// Whether it was taken.
pub fn lock_one_byte(file: &std::fs::File, at: u16) -> Taken {
    use std::os::fd::AsRawFd;

    // SAFETY: flock is plain data, for which zero is a valid value.
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as libc::c_short;
    lock.l_whence = libc::SEEK_SET as libc::c_short;
    lock.l_start = libc::off_t::from(at);
    lock.l_len = 1;
    // l_pid stays nought, as an open file description lock requires.
    //
    // SAFETY: F_OFD_SETLK on a descriptor `file` holds open, with a pointer
    // to a flock that lives across the call.
    let said =
        unsafe { libc::fcntl(file.as_raw_fd(), libc::F_OFD_SETLK, &lock) };
    if said == 0 {
        return Taken::Yes;
    }
    let e = std::io::Error::last_os_error();
    match e.raw_os_error() {
        Some(libc::EWOULDBLOCK) | Some(libc::EACCES) => Taken::Elsewhere,
        _ => Taken::Cannot(e),
    }
}

/// Whether a file is a device: a character or a block device.
///
/// # Arguments
///
/// * `meta` - What the file system says of the file.
///
/// # Returns
///
/// `true` where it is one.
pub fn is_device(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::FileTypeExt;

    meta.file_type().is_char_device() || meta.file_type().is_block_device()
}

/// Makes a named pipe, readable and writable by this user alone. Linux
/// alone: for a test of a node on a pipe.
///
/// # Arguments
///
/// * `path` - Where.
///
/// # Errors
///
/// What the system said.
pub fn make_fifo(path: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    let name = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // SAFETY: a NUL-terminated path alive for the call.
    if unsafe { libc::mkfifo(name.as_ptr(), 0o600) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Opens a device for reading and writing, without its becoming this
/// process's controlling terminal.
///
/// A device that is a tty -- a real serial port, or a pty made to simulate
/// one -- becomes the controlling terminal of a process that opens it
/// without `O_NOCTTY`, and then a read from a background process group
/// raises SIGTTIN and a hangup raises SIGHUP: a program stopped or killed
/// for reasons that have nothing to do with its device.
///
/// # Arguments
///
/// * `path`  - The device.
/// * `read`  - Whether it is opened for reading.
/// * `write` - Whether it is opened for writing.
///
/// # Returns
///
/// The device, open.
///
/// # Errors
///
/// What the system said.
pub fn open_device(
    path: &Path,
    read: bool,
    write: bool,
) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    std::fs::OpenOptions::new()
        .read(read)
        .write(write)
        .custom_flags(libc::O_NOCTTY)
        .open(path)
}

/// What kind of file something is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A directory.
    Directory,
    /// An ordinary file.
    File,
    /// A named pipe.
    Pipe,
    /// A socket.
    Socket,
    /// A block device.
    BlockDevice,
    /// A character device.
    CharDevice,
    /// A symbolic link, or anything else.
    Other,
}

/// What kind of file the file system says something is.
///
/// # Arguments
///
/// * `meta` - What the file system says of it.
///
/// # Returns
///
/// Its kind.
pub fn kind_of(meta: &std::fs::Metadata) -> Kind {
    use std::os::unix::fs::FileTypeExt;

    let kind = meta.file_type();
    if kind.is_dir() {
        Kind::Directory
    } else if kind.is_file() {
        Kind::File
    } else if kind.is_fifo() {
        Kind::Pipe
    } else if kind.is_socket() {
        Kind::Socket
    } else if kind.is_block_device() {
        Kind::BlockDevice
    } else if kind.is_char_device() {
        Kind::CharDevice
    } else {
        Kind::Other
    }
}

/// The device number of a character device.
///
/// # Arguments
///
/// * `meta` - What the file system says of the file.
///
/// # Returns
///
/// Its device number, major and minor together as the kernel packs them;
/// `None` where it is not a character device.
pub fn device_number(meta: &std::fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;

    (kind_of(meta) == Kind::CharDevice).then(|| meta.rdev())
}

/// Makes a symbolic link, for a test of a tree of files with one in it.
pub use std::os::unix::fs::symlink;
