//! The window is just large enough for what it holds, and a border.
//!
//! Its size is measured from what it shows rather than stated, because the
//! text is whatever size the one window font makes it: 460 pixels held the
//! panel at ten-pixel type and cut a beacon's message off at twelve points.

use i_slint_backend_testing::ElementHandle;
use slint::ComponentHandle;

use beacon_example::WatchWindow;

/// What a beacon says, at the length tcspecial says it: the build, the
/// configuration's version and a sixteen-byte digest.
const A_BEACON: &str = "tcspecial v0.1.0 md5 e2dd2b9ec2b0b8b0a8607777639ceac1";

/// The window's contents reach to within one border of its right and bottom
/// edges: no less, which would cut them off, and no more, which would be a
/// window larger than it need be.
fn fits_exactly(window: &WatchWindow, what: &str) {
    let border = window.get_border();
    let panel = ElementHandle::find_by_element_id(window, "WatchWindow::panel")
        .next()
        .expect("the window has the panel");
    let button = ElementHandle::find_by_element_id(window, "WatchWindow::tcspecial-button")
        .next()
        .expect("the window has the button");

    let right = panel.absolute_position().x + panel.size().width;
    let bottom = button.absolute_position().y + button.size().height;
    let (width, height) = (window.get_window_width(), window.get_window_height());
    assert!(
        (width - border - right).abs() <= 1.0,
        "{what}: the panel ends at {right} in a window {width} wide, border {border}"
    );
    assert!(
        (height - border - bottom).abs() <= 1.0,
        "{what}: the button ends at {bottom} in a window {height} tall, border {border}"
    );
    assert_eq!(
        panel.absolute_position().x,
        border,
        "{what}: the border on the left"
    );
    assert_eq!(
        panel.absolute_position().y,
        border,
        "{what}: the border on top"
    );
}

#[test]
fn the_window_is_just_large_enough_for_what_it_holds() {
    use beacon_example::{prepare, widest_message};

    i_slint_backend_testing::init_no_event_loop();

    let window = WatchWindow::new().expect("the window opens");
    assert_eq!(
        window.get_widest_message(),
        widest_message(),
        "the markup's own copy of the widest message is not the one the \
         library writes"
    );
    assert_eq!(
        widest_message(),
        "tcspecial v0.0.0 md5 d41d8cd98f00b204e9800998ecf8427e"
    );
    prepare(&window);
    window.show().expect("it is shown");
    assert!(window.get_border() > 0.0, "there is a border");

    // Before any beacon, already as wide as a beacon's message wants.
    fits_exactly(&window, "before any beacon");
    let before = window.get_window_width();

    // And a message of that length arriving changes nothing: the window
    // opened at the width it keeps.
    window.set_last_message(widest_message().into());
    assert_eq!(
        window.get_window_width(),
        before,
        "the window grew when a beacon's message arrived"
    );
    fits_exactly(&window, "with a beacon's message");
}

/// The button is as wide as the longer of its two labels, and not as the
/// window: the same width whichever it says, so that pressing it does not
/// move its edge.
#[test]
fn the_button_is_as_wide_as_its_longer_label() {
    use std::cell::RefCell;
    use std::rc::Rc;

    use beacon_example::{wire, Vehicle};

    i_slint_backend_testing::init_no_event_loop();
    let window = WatchWindow::new().expect("the window opens");
    window.show().expect("it is shown");
    let vehicle = Rc::new(RefCell::new(Vehicle::new("sleep", &["1000"])));
    wire(&window, Rc::clone(&vehicle));

    // With a beacon's message showing, which is what the window is as wide
    // as once the vehicle is heard from.
    window.set_last_message(A_BEACON.into());

    let needs = window.get_button_needs();
    let width = window.get_button_width();
    assert_eq!(
        width, needs,
        "the button is not the width of its longer label"
    );
    assert!(
        width < window.get_window_width() / 2.0,
        "the button is {width} wide in a window {} wide",
        window.get_window_width()
    );

    // And centred across the window: as much room on its left as on its
    // right, to within a pixel of rounding.
    let button = ElementHandle::find_by_element_id(&window, "WatchWindow::tcspecial-button")
        .next()
        .expect("the window has the button");
    let left = button.absolute_position().x;
    let right = window.get_window_width() - (left + button.size().width);
    assert!(
        (left - right).abs() <= 1.0,
        "the button has {left} to its left and {right} to its right"
    );

    // Pressed, it says the other thing, at the same width.
    ElementHandle::find_by_element_id(&window, "WatchWindow::tcspecial-button")
        .next()
        .expect("the window has the button")
        .invoke_accessible_default_action();
    assert_eq!(window.get_tcspecial_label(), beacon_example::STOP);
    assert_eq!(window.get_button_width(), width);
    vehicle.borrow_mut().stop().expect("it stops");
}
