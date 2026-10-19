//! The window's button starts the vehicle and stops it.
//!
//! Pressed as a person presses it, through Slint's testing backend, with
//! `sleep` standing in for tcspecial: what is tested is the button and the
//! process, and a real tcspecial would bind the beacon ports, which a test
//! run beside a real session must not.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use i_slint_backend_testing::ElementHandle;
use slint::ComponentHandle;

use beacon_example::{notice_exit, wire, Vehicle, WatchWindow, START, STOP};

/// The window, shown, with its button joined to this vehicle.
fn window_for(vehicle: &Rc<RefCell<Vehicle>>) -> WatchWindow {
    i_slint_backend_testing::init_no_event_loop();
    let window = WatchWindow::new().expect("the window opens");
    window.show().expect("it is shown");
    wire(&window, Rc::clone(vehicle));
    window
}

/// Press the button.
fn press(window: &WatchWindow) {
    ElementHandle::find_by_element_id(window, "WatchWindow::tcspecial-button")
        .next()
        .expect("the window has the button")
        .invoke_accessible_default_action();
}

/// Whether a process of this id is still there, reaped or not.
fn alive(id: u32) -> bool {
    Path::new(&format!("/proc/{id}")).exists()
}

/// Start, then stop, and back where it began: the label says what the next
/// press does, and the process is there exactly while the label says stop.
#[test]
fn the_button_starts_the_vehicle_and_stops_it() {
    let vehicle = Rc::new(RefCell::new(Vehicle::new("sleep", &["1000"])));
    let window = window_for(&vehicle);
    assert_eq!(window.get_tcspecial_label(), START, "it opens not running");
    assert!(!vehicle.borrow().is_running());

    press(&window);
    assert_eq!(window.get_tcspecial_label(), STOP);
    let id = vehicle.borrow().id().expect("it is running");
    assert!(alive(id), "the process is there");

    press(&window);
    assert_eq!(window.get_tcspecial_label(), START);
    assert!(!vehicle.borrow().is_running());
    assert!(!alive(id), "killed, and reaped rather than left a zombie");

    // And the initial state is the state again: it starts as it did before.
    press(&window);
    assert_eq!(window.get_tcspecial_label(), STOP);
    let again = vehicle.borrow().id().expect("running again");
    press(&window);
    assert_eq!(window.get_tcspecial_label(), START);
    assert!(!alive(again));
}

/// A vehicle that stops by itself puts the button back to start.
#[test]
fn a_vehicle_that_stops_by_itself_puts_the_button_back() {
    let vehicle = Rc::new(RefCell::new(Vehicle::new("true", &[])));
    let window = window_for(&vehicle);

    press(&window);
    assert_eq!(window.get_tcspecial_label(), STOP);

    let until = Instant::now() + Duration::from_secs(5);
    while window.get_tcspecial_label() == STOP && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
        notice_exit(&window, &mut vehicle.borrow_mut());
    }
    assert_eq!(window.get_tcspecial_label(), START);
    assert!(!vehicle.borrow().is_running());
}

/// A vehicle that cannot be started leaves the button saying start.
#[test]
fn a_vehicle_that_cannot_start_leaves_the_button_saying_start() {
    let vehicle = Rc::new(RefCell::new(Vehicle::new(
        "/nonexistent/tcspecial",
        &["beacon.yaml"],
    )));
    let window = window_for(&vehicle);

    press(&window);
    assert_eq!(window.get_tcspecial_label(), START);
    assert!(!vehicle.borrow().is_running());
}

/// A vehicle dropped while running is stopped: a window that closes leaves
/// nothing behind it.
#[test]
fn a_vehicle_dropped_while_running_is_stopped() {
    let mut vehicle = Vehicle::new("sleep", &["1000"]);
    vehicle.start().expect("it starts");
    let id = vehicle.id().expect("running");
    drop(vehicle);
    assert!(!alive(id));
}
