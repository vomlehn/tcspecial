//! Files, in a page: a file system in the page's memory, with the standard
//! library's interface.
//!
//! A page has no files. What the TCS programs read -- configurations,
//! mission databases -- the page puts here first, with [`preload`]; what
//! they write -- a state file, a log -- is kept here for as long as the page
//! is open. Paths are as on Linux: absolute from `/`, or relative to the
//! program's directory (see [`crate::browser::env::current_dir`]).
//! Symbolic links are followed. A file's mode is kept, and `set_mode`
//! changes it, but nothing is refused for it: there is one user.

pub use std::fs::FileTimes;

use std::collections::BTreeMap;
use std::ffi::{CStr, OsString};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::browser::time::SystemTime;

/// The most bytes a path can have, its nul included, as on Linux.
pub const PATH_BYTES: usize = 4096;

/// What is at a path.
#[derive(Debug, Clone)]
enum Node {
    /// A file, shared by every open of it.
    File(Arc<Mutex<Vec<u8>>>),
    /// A directory.
    Dir,
    /// A symbolic link, to where it says.
    Link(PathBuf),
}

/// What is at a path, and its mode.
#[derive(Debug, Clone)]
struct Entry {
    /// What it is.
    node: Node,
    /// Its permission bits.
    mode: u32,
    /// When it last changed.
    modified: SystemTime,
}

/// The page's files, by absolute path.
static FILES: Mutex<BTreeMap<PathBuf, Entry>> = Mutex::new(BTreeMap::new());

/// Takes the files, spinning rather than waiting, as the page's own thread
/// may not wait: see [`crate::browser::net`].
///
/// # Returns
///
/// The files, with `/` and `/tmp` there.
fn files() -> MutexGuard<'static, BTreeMap<PathBuf, Entry>> {
    let mut held = loop {
        match FILES.try_lock() {
            Ok(held) => break held,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                break poisoned.into_inner()
            }
            Err(std::sync::TryLockError::WouldBlock) => std::hint::spin_loop(),
        }
    };
    for dir in ["/", "/tmp", "/bin"] {
        held.entry(PathBuf::from(dir)).or_insert_with(|| Entry {
            node: Node::Dir,
            mode: 0o755,
            modified: SystemTime::now(),
        });
    }
    held
}

/// A path made absolute against the program's directory, `.` and `..`
/// taken out.
///
/// # Arguments
///
/// * `path` - The path.
///
/// # Returns
///
/// The absolute path.
pub fn absolute(path: &Path) -> PathBuf {
    let full = if path.is_absolute() {
        path.to_path_buf()
    } else {
        crate::browser::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(path)
    };
    let mut out = PathBuf::from("/");
    for part in full.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(name) => out.push(name),
            _ => {}
        }
    }
    out
}

/// A path with every symbolic link along it followed.
///
/// # Arguments
///
/// * `held` - The files.
/// * `path` - The path, absolute.
/// * `last` - Whether a link at the last part is followed too.
///
/// # Returns
///
/// The path links lead to.
///
/// # Errors
///
/// `InvalidInput` where links lead round in a circle.
fn resolved(
    held: &BTreeMap<PathBuf, Entry>,
    path: &Path,
    last: bool,
) -> io::Result<PathBuf> {
    let mut path = path.to_path_buf();
    for _ in 0..40 {
        let mut out = PathBuf::from("/");
        let parts: Vec<_> = path.components().collect();
        let mut moved = false;
        for (at, part) in parts.iter().enumerate() {
            let Component::Normal(name) = part else {
                continue;
            };
            out.push(name);
            let is_last = at + 1 == parts.len();
            if is_last && !last {
                break;
            }
            if let Some(Entry {
                node: Node::Link(to),
                ..
            }) = held.get(&out)
            {
                let base = out.parent().unwrap_or(Path::new("/")).to_path_buf();
                let mut next = if to.is_absolute() {
                    to.clone()
                } else {
                    base.join(to)
                };
                for rest in &parts[at + 1..] {
                    next.push(rest.as_os_str());
                }
                path = normalised(&next);
                moved = true;
                break;
            }
        }
        if !moved {
            return Ok(out);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "symbolic links lead round in a circle",
    ))
}

/// An absolute path with `.` and `..` taken out.
///
/// # Arguments
///
/// * `path` - The path, read as though it starts at the root.
///
/// # Returns
///
/// The path from the root, with no `.` or `..` in it.
fn normalised(path: &Path) -> PathBuf {
    let mut out = PathBuf::from("/");
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(name) => out.push(name),
            _ => {}
        }
    }
    out
}

/// What is at a path, links followed.
///
/// # Arguments
///
/// * `path` - The path.
///
/// # Returns
///
/// Where it is, and what is there.
///
/// # Errors
///
/// `NotFound` where nothing is.
fn lookup(path: &Path) -> io::Result<(PathBuf, Entry)> {
    let held = files();
    let at = resolved(&held, &absolute(path), true)?;
    let entry = held.get(&at).cloned().ok_or_else(|| not_found(path))?;
    Ok((at, entry))
}

/// The error a missing path is, naming it.
///
/// # Arguments
///
/// * `path` - The path that is missing.
///
/// # Returns
///
/// A `NotFound` error that names the path.
fn not_found(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("{}: no such file or directory", path.display()),
    )
}

/// Checks that a path's directory is there.
///
/// # Arguments
///
/// * `held` - The files.
/// * `at`   - The path, absolute and resolved.
///
/// # Errors
///
/// `NotFound` where it is not.
fn parent_is_there(
    held: &BTreeMap<PathBuf, Entry>,
    at: &Path,
) -> io::Result<()> {
    let parent = at.parent().unwrap_or(Path::new("/"));
    match held.get(parent) {
        Some(Entry {
            node: Node::Dir, ..
        }) => Ok(()),
        _ => Err(not_found(parent)),
    }
}

/// Puts a file in the page's files, its directories made for it: what the
/// page does with the configurations and the rest a program reads, before
/// it starts one.
///
/// # Arguments
///
/// * `path`  - Where, absolute.
/// * `bytes` - What it holds.
pub fn preload(path: &str, bytes: &[u8]) {
    let at = absolute(Path::new(path));
    if let Some(parent) = at.parent() {
        let _ = create_dir_all(parent);
    }
    files().insert(
        at,
        Entry {
            node: Node::File(Arc::new(Mutex::new(bytes.to_vec()))),
            mode: 0o644,
            modified: SystemTime::now(),
        },
    );
}

/// Makes a symbolic link.
///
/// # Arguments
///
/// * `to`   - Where it leads.
/// * `link` - Where it is.
///
/// # Errors
///
/// `AlreadyExists` where something is there, `NotFound` where its
/// directory is not.
pub fn symlink(to: impl AsRef<Path>, link: impl AsRef<Path>) -> io::Result<()> {
    let mut held = files();
    let at = resolved(&held, &absolute(link.as_ref()), false)?;
    parent_is_there(&held, &at)?;
    if held.contains_key(&at) {
        return Err(io::Error::from(io::ErrorKind::AlreadyExists));
    }
    held.insert(
        at,
        Entry {
            node: Node::Link(to.as_ref().to_path_buf()),
            mode: 0o777,
            modified: SystemTime::now(),
        },
    );
    Ok(())
}

/// What kind of file something is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileType {
    /// Which.
    kind: Kind,
}

impl FileType {
    /// Whether it is a directory.
    pub fn is_dir(&self) -> bool {
        self.kind == Kind::Directory
    }

    /// Whether it is a file.
    pub fn is_file(&self) -> bool {
        self.kind == Kind::File
    }

    /// Whether it is a symbolic link.
    pub fn is_symlink(&self) -> bool {
        self.kind == Kind::Other
    }
}

/// A file's permission bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    /// The bits.
    mode: u32,
}

impl Permissions {
    /// Whether nobody may write it.
    pub fn readonly(&self) -> bool {
        self.mode & 0o222 == 0
    }

    /// Makes it read-only, or writable by its owner.
    ///
    /// # Arguments
    ///
    /// * `readonly` - Which.
    pub fn set_readonly(&mut self, readonly: bool) {
        if readonly {
            self.mode &= !0o222;
        } else {
            self.mode |= 0o200;
        }
    }
}

/// What is known of a file.
#[derive(Debug, Clone)]
pub struct Metadata {
    /// What it is.
    kind: Kind,
    /// How long, for a file.
    len: u64,
    /// Its permission bits.
    mode: u32,
    /// When it last changed.
    modified: SystemTime,
}

impl Metadata {
    /// Of an entry.
    ///
    /// # Arguments
    ///
    /// * `entry` - The entry.
    ///
    /// # Returns
    ///
    /// What is known of it. A link is neither a file nor a directory, and
    /// only a file has a length.
    fn of(entry: &Entry) -> Metadata {
        let (kind, len) = match &entry.node {
            Node::File(bytes) => {
                (Kind::File, bytes.lock().map_or(0, |b| b.len() as u64))
            }
            Node::Dir => (Kind::Directory, 0),
            Node::Link(_) => (Kind::Other, 0),
        };
        Metadata {
            kind,
            len,
            mode: entry.mode,
            modified: entry.modified,
        }
    }

    /// Whether it is a directory.
    pub fn is_dir(&self) -> bool {
        self.kind == Kind::Directory
    }

    /// Whether it is a file.
    pub fn is_file(&self) -> bool {
        self.kind == Kind::File
    }

    /// Whether it is a symbolic link.
    pub fn is_symlink(&self) -> bool {
        self.kind == Kind::Other
    }

    /// How long it is.
    pub fn len(&self) -> u64 {
        self.len
    }

    /// Whether it is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// What kind it is.
    pub fn file_type(&self) -> FileType {
        FileType { kind: self.kind }
    }

    /// Its permission bits.
    pub fn permissions(&self) -> Permissions {
        Permissions { mode: self.mode }
    }

    /// When it last changed.
    ///
    /// # Errors
    ///
    /// None.
    pub fn modified(&self) -> io::Result<SystemTime> {
        Ok(self.modified)
    }
}

/// An open file.
#[derive(Debug)]
pub struct File {
    /// Its bytes, shared by every open of it.
    bytes: Arc<Mutex<Vec<u8>>>,
    /// Where the next read or write is, shared by this open's clones.
    at: Arc<Mutex<u64>>,
    /// Whether it may be written.
    write: bool,
    /// Whether every write goes on the end.
    append: bool,
    /// Where it is, for its metadata.
    path: PathBuf,
}

impl File {
    /// Opens a file to read.
    ///
    /// # Arguments
    ///
    /// * `path` - Where.
    ///
    /// # Returns
    ///
    /// The file.
    ///
    /// # Errors
    ///
    /// `NotFound` where it is not there.
    pub fn open(path: impl AsRef<Path>) -> io::Result<File> {
        OpenOptions::new().read(true).open(path)
    }

    /// Makes a file, empty, to write.
    ///
    /// # Arguments
    ///
    /// * `path` - Where.
    ///
    /// # Returns
    ///
    /// The file.
    ///
    /// # Errors
    ///
    /// `NotFound` where its directory is not there.
    pub fn create(path: impl AsRef<Path>) -> io::Result<File> {
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
    }

    /// Makes a file that must not be there already.
    ///
    /// # Arguments
    ///
    /// * `path` - Where.
    ///
    /// # Returns
    ///
    /// The file.
    ///
    /// # Errors
    ///
    /// `AlreadyExists` where it is.
    pub fn create_new(path: impl AsRef<Path>) -> io::Result<File> {
        OpenOptions::new().write(true).create_new(true).open(path)
    }

    /// How a file is to be opened.
    pub fn options() -> OpenOptions {
        OpenOptions::new()
    }

    /// What is known of it.
    ///
    /// # Returns
    ///
    /// What is known of it, with now as when it last changed.
    ///
    /// # Errors
    ///
    /// None.
    pub fn metadata(&self) -> io::Result<Metadata> {
        Ok(Metadata {
            kind: Kind::File,
            len: self.bytes.lock().map_or(0, |b| b.len() as u64),
            mode: lookup(&self.path).map_or(0o644, |(_, entry)| entry.mode),
            modified: SystemTime::now(),
        })
    }

    /// Makes it a length, cut short or filled with nought.
    ///
    /// # Arguments
    ///
    /// * `len` - The length.
    ///
    /// # Errors
    ///
    /// None.
    pub fn set_len(&self, len: u64) -> io::Result<()> {
        self.bytes.lock().expect("a file").resize(len as usize, 0);
        Ok(())
    }

    /// Flushes it: everything is where it will be already.
    ///
    /// # Errors
    ///
    /// None.
    pub fn sync_all(&self) -> io::Result<()> {
        Ok(())
    }

    /// Flushes its data.
    ///
    /// # Errors
    ///
    /// None.
    pub fn sync_data(&self) -> io::Result<()> {
        Ok(())
    }

    /// A second open, sharing where reading and writing are.
    ///
    /// # Returns
    ///
    /// The second open.
    ///
    /// # Errors
    ///
    /// None.
    pub fn try_clone(&self) -> io::Result<File> {
        Ok(File {
            bytes: self.bytes.clone(),
            at: self.at.clone(),
            write: self.write,
            append: self.append,
            path: self.path.clone(),
        })
    }

    /// Sets its permission bits.
    ///
    /// # Arguments
    ///
    /// * `permissions` - The bits.
    ///
    /// # Errors
    ///
    /// As for [`set_permissions`].
    pub fn set_permissions(&self, permissions: Permissions) -> io::Result<()> {
        set_permissions(&self.path, permissions)
    }

    /// Takes it for this open alone: there is one program at a time that
    /// would ask, in a page.
    ///
    /// # Errors
    ///
    /// None.
    pub fn lock(&self) -> io::Result<()> {
        Ok(())
    }

    /// Takes it for this open alone, without waiting.
    ///
    /// # Errors
    ///
    /// None.
    pub fn try_lock(&self) -> Result<(), TryLockError> {
        Ok(())
    }

    /// Gives it up.
    ///
    /// # Errors
    ///
    /// None.
    pub fn unlock(&self) -> io::Result<()> {
        Ok(())
    }

    /// Reads into a buffer from where the open is.
    ///
    /// # Arguments
    ///
    /// * `into` - The buffer, filled as far as the file goes.
    ///
    /// # Returns
    ///
    /// How many bytes were read: nought at the end of the file.
    ///
    /// # Errors
    ///
    /// None.
    ///
    /// # Panics
    ///
    /// Where a thread panicked while it held the file or the place.
    fn read_into(&self, into: &mut [u8]) -> io::Result<usize> {
        let bytes = self.bytes.lock().expect("a file");
        let mut at = self.at.lock().expect("a place");
        let from = (*at as usize).min(bytes.len());
        let count = (bytes.len() - from).min(into.len());
        into[..count].copy_from_slice(&bytes[from..from + count]);
        *at += count as u64;
        Ok(count)
    }

    /// Writes from a buffer at where the open is, or on the end.
    ///
    /// # Arguments
    ///
    /// * `data` - What to write. The file grows to hold it.
    ///
    /// # Returns
    ///
    /// How many bytes were written: all of them.
    ///
    /// # Errors
    ///
    /// `PermissionDenied` where the file was not opened to write.
    ///
    /// # Panics
    ///
    /// Where a thread panicked while it held the file or the place.
    fn write_from(&self, data: &[u8]) -> io::Result<usize> {
        if !self.write {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        let mut bytes = self.bytes.lock().expect("a file");
        let mut at = self.at.lock().expect("a place");
        if self.append {
            *at = bytes.len() as u64;
        }
        let from = *at as usize;
        if bytes.len() < from + data.len() {
            bytes.resize(from + data.len(), 0);
        }
        bytes[from..from + data.len()].copy_from_slice(data);
        *at += data.len() as u64;
        Ok(data.len())
    }

    /// Moves where the open is.
    ///
    /// # Arguments
    ///
    /// * `to` - Where to, from the start, the end or where it is now.
    ///
    /// # Returns
    ///
    /// Where it is now, from the start.
    ///
    /// # Errors
    ///
    /// `InvalidInput` where that is before the start.
    ///
    /// # Panics
    ///
    /// Where a thread panicked while it held the file or the place.
    fn seek_to(&self, to: SeekFrom) -> io::Result<u64> {
        let len = self.bytes.lock().expect("a file").len() as i64;
        let mut at = self.at.lock().expect("a place");
        let next = match to {
            SeekFrom::Start(n) => n as i64,
            SeekFrom::End(n) => len + n,
            SeekFrom::Current(n) => *at as i64 + n,
        };
        if next < 0 {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        *at = next as u64;
        Ok(*at)
    }
}

impl Read for File {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        self.read_into(into)
    }
}

impl Read for &File {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        self.read_into(into)
    }
}

impl Write for File {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.write_from(data)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Write for &File {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.write_from(data)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for File {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.seek_to(to)
    }
}

impl Seek for &File {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.seek_to(to)
    }
}

/// Why a file could not be taken without waiting.
#[derive(Debug)]
pub enum TryLockError {
    /// Something went wrong.
    Error(io::Error),
    /// Another open holds it.
    WouldBlock,
}

/// How a file is to be opened.
#[derive(Debug, Clone, Default)]
pub struct OpenOptions {
    /// To read.
    read: bool,
    /// To write.
    write: bool,
    /// To write on the end.
    append: bool,
    /// Emptied as it is opened.
    truncate: bool,
    /// Made where it is not there.
    create: bool,
    /// Made, and refused where it is there.
    create_new: bool,
}

impl OpenOptions {
    /// Nothing asked for yet.
    pub fn new() -> OpenOptions {
        OpenOptions::default()
    }

    /// To read.
    pub fn read(&mut self, on: bool) -> &mut OpenOptions {
        self.read = on;
        self
    }

    /// To write.
    pub fn write(&mut self, on: bool) -> &mut OpenOptions {
        self.write = on;
        self
    }

    /// To write on the end.
    ///
    /// # Arguments
    ///
    /// * `on` - Whether to. Writing on the end is writing, so this asks to
    ///          write as well.
    ///
    /// # Returns
    ///
    /// The options, to ask for more.
    pub fn append(&mut self, on: bool) -> &mut OpenOptions {
        self.append = on;
        if on {
            self.write = true;
        }
        self
    }

    /// Emptied as it is opened.
    pub fn truncate(&mut self, on: bool) -> &mut OpenOptions {
        self.truncate = on;
        self
    }

    /// Made where it is not there.
    pub fn create(&mut self, on: bool) -> &mut OpenOptions {
        self.create = on;
        self
    }

    /// Made, and refused where it is there.
    pub fn create_new(&mut self, on: bool) -> &mut OpenOptions {
        self.create_new = on;
        self
    }

    /// Opens a file as asked.
    ///
    /// # Arguments
    ///
    /// * `path` - Where.
    ///
    /// # Returns
    ///
    /// The file.
    ///
    /// # Errors
    ///
    /// `NotFound` where it is not there and is not to be made,
    /// `AlreadyExists` where it is and must not be, and `IsADirectory`
    /// where a directory is.
    pub fn open(&self, path: impl AsRef<Path>) -> io::Result<File> {
        let path = path.as_ref();
        let mut held = files();
        let at = resolved(&held, &absolute(path), true)?;
        let bytes = match held.get(&at) {
            Some(Entry {
                node: Node::File(bytes),
                ..
            }) => {
                if self.create_new {
                    return Err(io::Error::from(io::ErrorKind::AlreadyExists));
                }
                let bytes = bytes.clone();
                if self.truncate && self.write {
                    bytes.lock().expect("a file").clear();
                }
                bytes
            }
            // A directory, opened only to read, as Linux allows: what a
            // program syncs a directory through. It has no bytes.
            Some(Entry {
                node: Node::Dir, ..
            }) if !(self.write || self.append) => {
                Arc::new(Mutex::new(Vec::new()))
            }
            Some(_) => {
                return Err(io::Error::from(io::ErrorKind::IsADirectory));
            }
            None if self.create || self.create_new => {
                parent_is_there(&held, &at)?;
                let bytes = Arc::new(Mutex::new(Vec::new()));
                held.insert(
                    at.clone(),
                    Entry {
                        node: Node::File(bytes.clone()),
                        mode: 0o644,
                        modified: SystemTime::now(),
                    },
                );
                bytes
            }
            None => return Err(not_found(path)),
        };
        Ok(File {
            bytes,
            at: Arc::new(Mutex::new(0)),
            write: self.write || self.append,
            append: self.append,
            path: at,
        })
    }
}

/// Reads a whole file.
///
/// # Arguments
///
/// * `path` - Where.
///
/// # Returns
///
/// Its bytes.
///
/// # Errors
///
/// `NotFound` where it is not there.
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Reads a whole file as text.
///
/// # Arguments
///
/// * `path` - Where.
///
/// # Returns
///
/// Its text.
///
/// # Errors
///
/// `NotFound` where it is not there, `InvalidData` where it is not UTF-8.
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    String::from_utf8(read(path)?)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Writes a whole file, made or emptied first.
///
/// # Arguments
///
/// * `path`  - Where.
/// * `bytes` - What it holds.
///
/// # Errors
///
/// `NotFound` where its directory is not there.
pub fn write(
    path: impl AsRef<Path>,
    bytes: impl AsRef<[u8]>,
) -> io::Result<()> {
    File::create(path)?.write_all(bytes.as_ref())
}

/// Takes a file, or a link, away.
///
/// # Arguments
///
/// * `path` - Where.
///
/// # Errors
///
/// `NotFound` where nothing is there, `IsADirectory` where a directory is.
pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    let mut held = files();
    let at = resolved(&held, &absolute(path), false)?;
    match held.get(&at) {
        Some(Entry {
            node: Node::Dir, ..
        }) => Err(io::Error::from(io::ErrorKind::IsADirectory)),
        Some(_) => {
            held.remove(&at);
            Ok(())
        }
        None => Err(not_found(path)),
    }
}

/// Makes a directory.
///
/// # Arguments
///
/// * `path` - Where.
///
/// # Errors
///
/// `AlreadyExists` where something is there, `NotFound` where its
/// directory is not.
pub fn create_dir(path: impl AsRef<Path>) -> io::Result<()> {
    let mut held = files();
    let at = resolved(&held, &absolute(path.as_ref()), true)?;
    if held.contains_key(&at) {
        return Err(io::Error::from(io::ErrorKind::AlreadyExists));
    }
    parent_is_there(&held, &at)?;
    held.insert(
        at,
        Entry {
            node: Node::Dir,
            mode: 0o755,
            modified: SystemTime::now(),
        },
    );
    Ok(())
}

/// Makes a directory and every one above it that is not there.
///
/// # Arguments
///
/// * `path` - Where.
///
/// # Errors
///
/// `AlreadyExists` where a file is in the way.
pub fn create_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let at = absolute(path.as_ref());
    let mut so_far = PathBuf::from("/");
    for part in at.components() {
        let Component::Normal(name) = part else {
            continue;
        };
        so_far.push(name);
        match metadata(&so_far) {
            Ok(found) if found.is_dir() => {}
            Ok(_) => return Err(io::Error::from(io::ErrorKind::AlreadyExists)),
            Err(_) => create_dir(&so_far)?,
        }
    }
    Ok(())
}

/// Takes a directory away, and everything in it.
///
/// # Arguments
///
/// * `path` - Where.
///
/// # Errors
///
/// `NotFound` where nothing is there.
pub fn remove_dir_all(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    let mut held = files();
    let at = resolved(&held, &absolute(path), false)?;
    if !held.contains_key(&at) {
        return Err(not_found(path));
    }
    held.retain(|key, _| !key.starts_with(&at));
    Ok(())
}

/// Takes an empty directory away.
///
/// # Arguments
///
/// * `path` - Where.
///
/// # Errors
///
/// `NotFound` where nothing is there, `DirectoryNotEmpty` where it is not
/// empty.
pub fn remove_dir(path: impl AsRef<Path>) -> io::Result<()> {
    if read_dir(path.as_ref())?.next().is_some() {
        return Err(io::Error::from(io::ErrorKind::DirectoryNotEmpty));
    }
    remove_dir_all(path)
}

/// One thing in a directory.
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Its path.
    path: PathBuf,
    /// What is known of it.
    meta: Metadata,
}

impl DirEntry {
    /// Its path.
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// Its name.
    pub fn file_name(&self) -> OsString {
        self.path.file_name().unwrap_or_default().to_os_string()
    }

    /// What kind it is.
    ///
    /// # Errors
    ///
    /// None.
    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(self.meta.file_type())
    }

    /// What is known of it.
    ///
    /// # Errors
    ///
    /// None.
    pub fn metadata(&self) -> io::Result<Metadata> {
        Ok(self.meta.clone())
    }
}

/// What is in a directory.
#[derive(Debug)]
pub struct ReadDir(std::vec::IntoIter<io::Result<DirEntry>>);

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// What is in a directory.
///
/// # Arguments
///
/// * `path` - Where.
///
/// # Returns
///
/// Its entries, by name, as the path was given rather than resolved.
///
/// # Errors
///
/// `NotFound` where it is not there, `NotADirectory` where it is not one.
pub fn read_dir(path: impl AsRef<Path>) -> io::Result<ReadDir> {
    let path = path.as_ref();
    let held = files();
    let at = resolved(&held, &absolute(path), true)?;
    match held.get(&at) {
        Some(Entry {
            node: Node::Dir, ..
        }) => {}
        Some(_) => return Err(io::Error::from(io::ErrorKind::NotADirectory)),
        None => return Err(not_found(path)),
    }
    let entries: Vec<_> = held
        .iter()
        .filter(|(key, _)| key.parent() == Some(at.as_path()) && **key != at)
        .map(|(key, entry)| {
            Ok(DirEntry {
                path: path.join(key.file_name().unwrap_or_default()),
                meta: Metadata::of(entry),
            })
        })
        .collect();
    Ok(ReadDir(entries.into_iter()))
}

/// What is known of a file, links followed.
///
/// # Arguments
///
/// * `path` - Where.
///
/// # Errors
///
/// `NotFound` where nothing is there.
pub fn metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
    lookup(path.as_ref()).map(|(_, entry)| Metadata::of(&entry))
}

/// What is known of a file, a link at the end not followed.
///
/// # Arguments
///
/// * `path` - Where.
///
/// # Returns
///
/// What is known of it, or of the link where it is one.
///
/// # Errors
///
/// `NotFound` where nothing is there.
pub fn symlink_metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
    let path = path.as_ref();
    let held = files();
    let at = resolved(&held, &absolute(path), false)?;
    held.get(&at)
        .map(Metadata::of)
        .ok_or_else(|| not_found(path))
}

/// Where a link leads.
///
/// # Arguments
///
/// * `path` - The link.
///
/// # Returns
///
/// The path the link holds, as it was made.
///
/// # Errors
///
/// `InvalidInput` where it is not a link.
pub fn read_link(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let path = path.as_ref();
    let held = files();
    let at = resolved(&held, &absolute(path), false)?;
    match held.get(&at) {
        Some(Entry {
            node: Node::Link(to),
            ..
        }) => Ok(to.clone()),
        Some(_) => Err(io::Error::from(io::ErrorKind::InvalidInput)),
        None => Err(not_found(path)),
    }
}

/// A path made absolute, every link along it followed.
///
/// # Arguments
///
/// * `path` - The path.
///
/// # Errors
///
/// `NotFound` where nothing is there.
pub fn canonicalize(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    lookup(path.as_ref()).map(|(at, _)| at)
}

/// Whether something is at a path.
///
/// # Arguments
///
/// * `path` - The path.
///
/// # Returns
///
/// `Ok(true)` where something is.
///
/// # Errors
///
/// None.
pub fn exists(path: impl AsRef<Path>) -> io::Result<bool> {
    Ok(metadata(path).is_ok())
}

/// Copies a file.
///
/// # Arguments
///
/// * `from` - The file.
/// * `to`   - Where to.
///
/// # Returns
///
/// How many bytes.
///
/// # Errors
///
/// `NotFound` where it is not there.
pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
    let bytes = read(from)?;
    write(to, &bytes)?;
    Ok(bytes.len() as u64)
}

/// Moves a file, or a directory and everything in it.
///
/// # Arguments
///
/// * `from` - Where it is.
/// * `to`   - Where it goes, replacing a file there.
///
/// # Errors
///
/// `NotFound` where it is not there.
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let from_path = from.as_ref();
    let mut held = files();
    let from = resolved(&held, &absolute(from_path), false)?;
    let to = resolved(&held, &absolute(to.as_ref()), false)?;
    if !held.contains_key(&from) {
        return Err(not_found(from_path));
    }
    parent_is_there(&held, &to)?;
    let moving: Vec<_> = held
        .keys()
        .filter(|key| key.starts_with(&from))
        .cloned()
        .collect();
    held.retain(|key, _| !key.starts_with(&to) || moving.contains(key));
    for key in moving {
        if let Some(entry) = held.remove(&key) {
            let rest = key.strip_prefix(&from).unwrap_or(Path::new(""));
            let next = if rest.as_os_str().is_empty() {
                to.clone()
            } else {
                to.join(rest)
            };
            held.insert(next, entry);
        }
    }
    Ok(())
}

/// Renames a file, from names already nul-terminated.
///
/// # Arguments
///
/// * `from` - The file's name.
/// * `to`   - Its new name.
///
/// # Errors
///
/// As for [`rename`].
pub fn rename_c(from: &CStr, to: &CStr) -> io::Result<()> {
    rename(
        Path::new(&*from.to_string_lossy()),
        Path::new(&*to.to_string_lossy()),
    )
}

/// Sets a file's permission bits.
///
/// # Arguments
///
/// * `path`        - Where.
/// * `permissions` - The bits.
///
/// # Errors
///
/// `NotFound` where nothing is there.
pub fn set_permissions(
    path: impl AsRef<Path>,
    permissions: Permissions,
) -> io::Result<()> {
    set_mode(path.as_ref(), permissions.mode)
}

/// Whether the page may run the program at a path: it may where a program
/// is registered under its name.
///
/// # Arguments
///
/// * `program` - The path.
///
/// # Returns
///
/// `true` where one is.
pub fn may_run(program: &Path) -> bool {
    crate::browser::process::is_registered(program)
}

/// Sets a file's mode.
///
/// # Arguments
///
/// * `path` - The file.
/// * `mode` - The bits.
///
/// # Errors
///
/// `NotFound` where nothing is there.
pub fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    let mut held = files();
    let at = resolved(&held, &absolute(path), true)?;
    let entry = held.get_mut(&at).ok_or_else(|| not_found(path))?;
    entry.mode = mode & 0o7777;
    Ok(())
}

/// A file's mode.
///
/// # Arguments
///
/// * `path` - The file.
///
/// # Returns
///
/// The bits.
///
/// # Errors
///
/// `NotFound` where nothing is there.
pub fn mode(path: &Path) -> io::Result<u32> {
    metadata(path).map(|found| found.mode)
}

/// Whether a lock was taken, by [`lock_one_byte`].
#[derive(Debug)]
pub enum Taken {
    /// It was.
    Yes,
    /// Another open of the file holds it.
    Elsewhere,
    /// The file cannot be locked.
    Cannot(io::Error),
}

/// Takes a byte of a file for this open: always, in a page.
///
/// # Arguments
///
/// * `_file` - The open file.
/// * `_at`   - Which byte.
///
/// # Returns
///
/// [`Taken::Yes`].
pub fn lock_one_byte(_file: &File, _at: u16) -> Taken {
    Taken::Yes
}

/// Whether a file is a device: never, in a page.
///
/// # Arguments
///
/// * `_meta` - What is known of it.
///
/// # Returns
///
/// `false`.
pub fn is_device(_meta: &Metadata) -> bool {
    false
}

/// What kind of file something is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A directory.
    Directory,
    /// An ordinary file.
    File,
    /// A named pipe: never, in a page.
    Pipe,
    /// A socket: never.
    Socket,
    /// A block device: never.
    BlockDevice,
    /// A character device: never.
    CharDevice,
    /// A symbolic link.
    Other,
}

/// What kind of file something is.
///
/// # Arguments
///
/// * `meta` - What is known of it.
///
/// # Returns
///
/// Its kind.
pub fn kind_of(meta: &Metadata) -> Kind {
    meta.kind
}

/// The device number of a character device: none, in a page.
///
/// # Arguments
///
/// * `_meta` - What is known of the file.
///
/// # Returns
///
/// `None`.
pub fn device_number(_meta: &Metadata) -> Option<u64> {
    None
}

/// Whether a path names a file, following links: what std's
/// `Path::is_file` says, asked of this file system.
///
/// # Arguments
///
/// * `path` - The path.
///
/// # Returns
///
/// `true` for a file; `false` for anything else, or nothing.
pub fn is_file(path: impl AsRef<Path>) -> bool {
    metadata(path).is_ok_and(|found| found.is_file())
}

/// Whether a path names a directory, following links: what std's
/// `Path::is_dir` says, asked of this file system.
///
/// # Arguments
///
/// * `path` - The path.
///
/// # Returns
///
/// `true` for a directory; `false` for anything else, or nothing.
pub fn is_dir(path: impl AsRef<Path>) -> bool {
    metadata(path).is_ok_and(|found| found.is_dir())
}

/// Whether a path names a symbolic link, not following it: what std's
/// `Path::is_symlink` says, asked of this file system.
///
/// # Arguments
///
/// * `path` - The path.
///
/// # Returns
///
/// `true` for a link; `false` for anything else, or nothing.
pub fn is_symlink(path: impl AsRef<Path>) -> bool {
    symlink_metadata(path).is_ok_and(|found| found.file_type().is_symlink())
}

/// Whether anything is at a path, following links: what std's
/// `Path::exists` says, asked of this file system.
///
/// # Arguments
///
/// * `path` - The path.
///
/// # Returns
///
/// `true` where something is; `false` where nothing is, or it cannot be
/// told.
pub fn path_exists(path: impl AsRef<Path>) -> bool {
    metadata(path).is_ok()
}
