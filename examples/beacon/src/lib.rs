//! The example's window, and the one thing this program does to it.
//!
//! A library beside the binary so that the window can be opened by a test as
//! well as by a person: what the test checks is the wiring below, which is the
//! whole of what this example adds to the two halves it is made of -- see
//! `src/bin/beacon_watch.rs`.

use slint::ComponentHandle;
use tcsui::Shown;

slint::include_modules!();

/// Put what the beacon loop reported into the window.
///
/// All three at once, from the one reading: a light that said beacons were
/// arriving beside a time that said none had would be two answers to one
/// question. The window passes them down to the panel, which is `tcsui`'s.
pub fn show(window: &WatchWindow, shown: &Shown) {
    window.set_indicator_color(shown.indicator);
    window.set_last_received(shown.received.clone().into());
    window.set_last_message(shown.message.clone().into());
}

/// And the same for a window this does not own, which is what a background
/// thread has.
pub fn show_on(window: &slint::Weak<WatchWindow>, shown: Shown) {
    let window = window.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(window) = window.upgrade() {
            show(&window, &shown);
        }
    });
}

/// What the button says while tcspecial is not running, which is how the
/// window opens.
pub const START: &str = "Start tcspecial";

/// And while it is.
pub const STOP: &str = "Stop tcspecial";

/// The vehicle, as a program this window starts and stops.
///
/// The beacon is what the vehicle sends of its own accord, so a button that
/// starts it and stops it is the whole of what there is to watch: the light
/// goes out a little after the vehicle stops, and comes back with its first
/// beacon.
///
/// The program is run itself and not through `cargo run`. Stopping is
/// `Child::kill`, and killing cargo would leave the tcspecial it started
/// running, holding the beacon port, with nothing left that knows it is
/// there.
pub struct Vehicle {
    program: std::path::PathBuf,
    args: Vec<String>,
    running: Option<std::process::Child>,
}

impl Vehicle {
    /// A vehicle that is this program with these arguments, not running yet.
    pub fn new(program: impl Into<std::path::PathBuf>, args: &[&str]) -> Vehicle {
        Vehicle {
            program: program.into(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            running: None,
        }
    }

    /// tcspecial on this configuration, being the `tcspecial` beside this
    /// program: `cargo build` puts the two in one directory.
    pub fn beside_this_program(configuration: &str) -> Result<Vehicle, String> {
        let here = std::env::current_exe()
            .map_err(|e| format!("cannot tell where this program is, to find tcspecial: {e}"))?;
        let program = here.with_file_name("tcspecial");
        if !program.is_file() {
            return Err(format!(
                "there is no tcspecial at {}: build it with `cargo build --bin tcspecial`",
                program.display()
            ));
        }
        Ok(Vehicle::new(program, &[configuration]))
    }

    /// Whether it is running, as far as this window knows.
    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    /// The process id of the vehicle while it is running.
    pub fn id(&self) -> Option<u32> {
        self.running.as_ref().map(|child| child.id())
    }

    /// What the button says now.
    pub fn label(&self) -> &'static str {
        if self.is_running() {
            STOP
        } else {
            START
        }
    }

    /// What a click does: start it if it is not running, and stop it if it
    /// is.
    pub fn toggle(&mut self) -> Result<(), String> {
        if self.is_running() {
            self.stop()
        } else {
            self.start()
        }
    }

    /// Start it.
    ///
    /// Told to die with this program, by the kernel: a window closed is
    /// stopped by [`Drop`], but one killed or crashed runs no code at all,
    /// and the vehicle it left would go on beaconing with no button to stop
    /// it.
    pub fn start(&mut self) -> Result<(), String> {
        use std::os::unix::process::CommandExt;

        if self.is_running() {
            return Ok(());
        }
        let mut command = std::process::Command::new(&self.program);
        command.args(&self.args);
        // This program, as the child will see it as its parent: if the two
        // differ once the death signal is set, this program died between the
        // fork and the prctl, and the child would never be told.
        let parent = std::process::id() as libc::pid_t;
        // SAFETY: prctl and getppid are async-signal-safe, and touch nothing
        // of this process's but the child's own death signal.
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() != parent {
                    return Err(std::io::Error::other(
                        "the window that started it has already gone",
                    ));
                }
                Ok(())
            });
        }
        let child = command
            .spawn()
            .map_err(|e| format!("{} cannot be started: {e}", self.program.display()))?;
        self.running = Some(child);
        Ok(())
    }

    /// Stop it, with `kill()`, and wait for it to be gone.
    ///
    /// Waited for, so that it is reaped rather than left a zombie, and so
    /// that the beacon port is free by the time the button offers to start it
    /// again.
    pub fn stop(&mut self) -> Result<(), String> {
        let Some(mut child) = self.running.take() else {
            return Ok(());
        };
        let killed = child.kill();
        let waited = child.wait();
        killed.map_err(|e| format!("tcspecial cannot be stopped: {e}"))?;
        waited.map_err(|e| format!("tcspecial was stopped and cannot be waited for: {e}"))?;
        Ok(())
    }

    /// Whether it stopped of its own accord, and how, where it did.
    ///
    /// A vehicle that exits by itself -- a configuration it refuses, a port
    /// already taken -- leaves this as it was before it was started, so the
    /// button does not go on offering to stop something that is not there.
    pub fn exited(&mut self) -> Option<String> {
        let child = self.running.as_mut()?;
        match child.try_wait() {
            Ok(None) => None,
            Ok(Some(status)) => {
                self.running = None;
                Some(format!("tcspecial stopped by itself: {status}"))
            }
            Err(e) => {
                let _ = self.stop();
                Some(format!(
                    "tcspecial could not be asked whether it is running, so it was stopped: {e}"
                ))
            }
        }
    }
}

impl Drop for Vehicle {
    /// A window that closes stops the vehicle it started.
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Join the window's button to a vehicle: a click starts or stops it and the
/// button says what the next click will do.
///
/// Here rather than in the program so that a test can press the button.
/// Something that cannot be done is said on stderr, and the button says
/// what is true afterwards whichever way it went.
pub fn wire(window: &WatchWindow, vehicle: std::rc::Rc<std::cell::RefCell<Vehicle>>) {
    window.set_tcspecial_label(vehicle.borrow().label().into());
    let weak = window.as_weak();
    window.on_tcspecial_clicked(move || {
        let mut vehicle = vehicle.borrow_mut();
        if let Err(why) = vehicle.toggle() {
            eprintln!("{why}");
        }
        if let Some(window) = weak.upgrade() {
            window.set_tcspecial_label(vehicle.label().into());
        }
    });
}

/// Look whether the vehicle has stopped of its own accord, and put the
/// button back if it has.
pub fn notice_exit(window: &WatchWindow, vehicle: &mut Vehicle) {
    if let Some(how) = vehicle.exited() {
        eprintln!("{how}");
        window.set_tcspecial_label(vehicle.label().into());
    }
}
