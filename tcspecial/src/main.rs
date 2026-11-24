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
//! `tcspecial::visual`. It is refused under `simulate: none`, the flight
//! setting and the one a run without a simulator file gets.
//!
//! The simulation mode, `none`, `some` or `all`, is not given on the
//! command line but stated in the run's simulator file, as `simulate` in
//! its `tcspecial` section: the file `PAYLOAD_SIM_YAML` names, or else the
//! one beside the configuration. With neither, or none stated, the run is
//! `none`, the flight setting. `--simulate`, which once gave it, is refused,
//! saying so. See `tcsprotocol::simulate`. Each payload is judged a real, a
//! dummy or a simulated device before anything is opened, and a run whose
//! payloads the mode does not allow is refused: see
//! `tcspecial::runtime::start`.
//!
//! Bit errors and a delay on the link to the ground are the ground's to
//! simulate, never tcspecial's: see `tcsmoc::loss` and `tcs-relay`.
//!
//! Every check that no test code is flying is made in
//! `tcspecial::flight_checks`, before anything is read, and what it found
//! goes in every beacon.
//!
//! `--no-memlock` runs the vehicle without locking its memory into RAM,
//! in any mode. Without it the vehicle's memory is always locked, and a
//! lock that fails is said and run without in flight, and stops tcspecial
//! under `simulate: some` or `all`: see
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
