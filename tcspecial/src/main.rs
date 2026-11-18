//! The TCSpecial Flight Executive's entry point: the flight software, as a
//! program.
//!
//! Reads the configuration it is given, says what it read, and runs the graph
//! that configuration describes: see [`tcspecial::runtime::start`].
//!
//! `-q` or `--quiet` leaves the startup summary unsaid and logs warnings and
//! worse rather than information, for a program that starts tcspecial to
//! show something else. What stops a start is said either way.
//!
//! `--visual` opens a window showing the graph as tcs-cfge draws it, each link
//! lit as data crosses it, in a tcspecial built with the `visual` feature;
//! one built without it says so and exits. Never in flight: see
//! `tcspecial::visual`. It is refused under `--simulate=none`, the flight
//! setting and the one a run without `--simulate` gets.
//!
//! `--simulate=<mode>` is the simulation mode, `none`, `some` or `all`: see
//! `tcsprotocol::simulate`. Each payload is judged a real, a dummy or a
//! simulated device before anything is opened, and a run whose payloads the
//! mode does not allow is refused: see `tcspecial::runtime::start`.
//!
//! `--no-memlock` runs the vehicle without locking its memory into RAM,
//! under any `--simulate`. Without it the vehicle's memory is always locked,
//! and a lock that fails is said and run without in flight, and stops
//! tcspecial under `--simulate=some` or `all`: see
//! `tcspecial::runtime::memory`.
//!
//! `--types` prints the payload types this build contains, separated by
//! commas, and exits: what `tcs-verify --types=` takes, so that a
//! configuration can be checked on the ground against the binary that will
//! fly, with `tcs-verify --types=$(tcspecial --types) <configuration>`.

use tcspecial::runtime;

/// The allocator, which ends the vehicle with a status that says so when
/// memory runs out, rather than aborting as though it had crashed: see
/// `tcspecial::runtime::memory`.
#[global_allocator]
static ALLOCATOR: runtime::memory::Allocator = runtime::memory::Allocator;

/// Runs tcspecial as the command line asks: see
/// [`tcspecial::command_line::main`].
fn main() {
    tcspecial::command_line::main()
}
