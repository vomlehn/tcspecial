//! Which program a thread belongs to, in a page where every program is
//! threads of one.
//!
//! On Linux, tcs-moc, tcs-sim and tcspecial are processes, each with its
//! own arguments, environment, directory, signals and standard descriptors.
//! In a page they are threads of one WebAssembly instance, so each is given
//! a [`Program`] holding those things, which every thread it starts
//! inherits: see [`crate::browser::thread`]. What the rest of `tcsos`
//! answers from the system on Linux -- `env::args`, a stop signal, an
//! interface handed over -- it answers from here.

use crate::browser::sync::{Arc, Mutex, OnceLock};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::browser::net::TcpStream;

/// One program in the page.
#[derive(Debug)]
pub struct Program {
    /// Its number, as a process id would be.
    pub id: u32,
    /// Its arguments, its name first.
    pub args: Vec<String>,
    /// Its environment.
    pub env: Mutex<BTreeMap<String, String>>,
    /// The directory relative paths are taken from.
    pub cwd: Mutex<PathBuf>,
    /// Whether it has been asked to stop, as SIGTERM asks on Linux.
    pub stop: AtomicBool,
    /// The interfaces it was handed, by slot: 0, 1 and 2.
    pub slots: Mutex<[Option<TcpStream>; 3]>,
    /// The program that started it, nought for the page.
    pub parent: u32,
    /// What it ended with, once it has.
    pub ended: Mutex<Option<i32>>,
    /// Whether it is asked to stop when its parent ends, as the kernel's
    /// parent-death signal stops a process on Linux.
    pub dies_with_parent: AtomicBool,
}

impl Program {
    /// Notes that the program has ended, and with what: the first ending
    /// is the one kept.
    ///
    /// # Arguments
    ///
    /// * `code` - What it ended with.
    pub fn end(&self, code: i32) {
        if let Ok(mut ended) = self.ended.lock() {
            ended.get_or_insert(code);
        }
        crate::browser::net::close_owned_by(self.id);
        for child in dependents(self.id) {
            child.stop.store(true, Ordering::SeqCst);
        }
    }

    /// What the program ended with, where it has.
    ///
    /// # Returns
    ///
    /// The status, or `None` while it runs.
    pub fn ended(&self) -> Option<i32> {
        self.ended.lock().ok().and_then(|ended| *ended)
    }
}

/// The next program's number. The page is 1.
static NEXT: AtomicU32 = AtomicU32::new(2);

/// The page itself, as the program every thread belongs to until it is
/// started as another.
///
/// # Returns
///
/// The page's program, made the first time it is asked for.
fn page() -> &'static Arc<Program> {
    static PAGE: OnceLock<Arc<Program>> = OnceLock::new();
    PAGE.get_or_init(|| {
        Arc::new(Program {
            id: 1,
            args: vec!["page".to_string()],
            env: Mutex::new(BTreeMap::new()),
            cwd: Mutex::new(PathBuf::from("/")),
            stop: AtomicBool::new(false),
            slots: Mutex::new([None, None, None]),
            parent: 0,
            ended: Mutex::new(None),
            dies_with_parent: AtomicBool::new(false),
        })
    })
}

thread_local! {
    /// The program this thread belongs to, where it has been told.
    static CURRENT: RefCell<Option<Arc<Program>>> = const { RefCell::new(None) };
    /// Whether this thread is a Web Worker, which may wait; the page's own
    /// thread may not.
    static WORKER: Cell<bool> = const { Cell::new(false) };
}

/// The program the calling thread belongs to.
///
/// # Returns
///
/// It: the page, where the thread was never told another.
pub fn current() -> Arc<Program> {
    CURRENT
        .with(|current| current.borrow().clone())
        .unwrap_or_else(|| page().clone())
}

/// Says that the calling thread belongs to a program, and whether it may
/// wait. The first thing a thread started by [`crate::browser::thread`]
/// does.
///
/// # Arguments
///
/// * `program` - The program.
/// * `worker`  - Whether the thread is a Web Worker.
pub fn enter(program: Arc<Program>, worker: bool) {
    CURRENT.with(|current| *current.borrow_mut() = Some(program));
    WORKER.with(|is| is.set(worker));
    if worker {
        crate::browser::console::capture_output();
    }
}

/// Whether the calling thread may wait: a Web Worker may, and the page's
/// own thread, which draws the windows, may not -- the browser refuses
/// `Atomics.wait` there.
///
/// # Returns
///
/// `true` in a worker.
pub fn may_wait() -> bool {
    WORKER.with(|is| is.get())
}

/// Makes a program, to be started.
///
/// # Arguments
///
/// * `args` - Its arguments, its name first.
/// * `env`  - Its environment.
/// * `cwd`  - Its directory.
///
/// # Returns
///
/// The program, started by nobody yet, its parent the calling thread's.
pub fn new_program(
    args: Vec<String>,
    env: BTreeMap<String, String>,
    cwd: PathBuf,
) -> Arc<Program> {
    let program = Arc::new(Program {
        id: NEXT.fetch_add(1, Ordering::Relaxed),
        args,
        env: Mutex::new(env),
        cwd: Mutex::new(cwd),
        stop: AtomicBool::new(false),
        slots: Mutex::new([None, None, None]),
        parent: current().id,
        ended: Mutex::new(None),
        dies_with_parent: AtomicBool::new(false),
    });
    note(&program);
    program
}

/// Every program started, by number, for asking one to stop: held weakly,
/// so a program that has ended and been forgotten goes.
fn started() -> &'static Mutex<BTreeMap<u32, std::sync::Weak<Program>>> {
    static STARTED: OnceLock<Mutex<BTreeMap<u32, std::sync::Weak<Program>>>> =
        OnceLock::new();
    STARTED.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Notes a program as started, for [`ask_to_stop`] to find.
///
/// # Arguments
///
/// * `program` - The program.
fn note(program: &Arc<Program>) {
    if let Ok(mut started) = started().lock() {
        started.retain(|_, weak| weak.strong_count() > 0);
        started.insert(program.id, Arc::downgrade(program));
    }
}

/// Asks a program to stop, by its number, as SIGTERM asks a process.
///
/// # Arguments
///
/// * `id` - Its number.
///
/// # Returns
///
/// Whether there is such a program.
pub fn ask_to_stop(id: u32) -> bool {
    if id == page().id {
        page().stop.store(true, Ordering::SeqCst);
        return true;
    }
    let found = started()
        .lock()
        .ok()
        .and_then(|started| started.get(&id).and_then(|weak| weak.upgrade()));
    match found {
        Some(program) => {
            program.stop.store(true, Ordering::SeqCst);
            true
        }
        None => false,
    }
}

/// The programs a program started that die with it, still running.
///
/// # Arguments
///
/// * `parent` - The program's number.
///
/// # Returns
///
/// Them.
fn dependents(parent: u32) -> Vec<Arc<Program>> {
    let Ok(started) = started().lock() else {
        return Vec::new();
    };
    started
        .values()
        .filter_map(|weak| weak.upgrade())
        .filter(|program| {
            program.parent == parent
                && program.dies_with_parent.load(Ordering::SeqCst)
                && program.ended().is_none()
        })
        .collect()
}

/// Whether a program has started anything that dies with it and has not
/// yet ended, at any depth: what is waited for with it.
///
/// # Arguments
///
/// * `parent` - The program's number.
///
/// # Returns
///
/// `true` while one runs.
pub fn has_dependents(parent: u32) -> bool {
    dependents(parent)
        .iter()
        .any(|child| child.ended().is_none() || has_dependents(child.id))
}

/// Asks every program in the page to stop, as the page's Stop does.
pub fn ask_everything_to_stop() {
    page().stop.store(true, Ordering::SeqCst);
    if let Ok(started) = started().lock() {
        for program in started.values().filter_map(|weak| weak.upgrade()) {
            program.stop.store(true, Ordering::SeqCst);
        }
    }
}
