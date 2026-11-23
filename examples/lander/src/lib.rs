//! The lander example's ground station in Rust: two tcspecials flying a
//! lunar lander, watched from the Earth.
//!
//! The vehicles are `lander-a`, which flies the X axis, and `lander-b`,
//! which flies the Y: each reads its axis of the joystick in tcs-sim's
//! lander pane and drives that axis's thrust with it, and sends its axis's
//! position, velocity and acceleration down. One tcs-sim simulates the
//! payloads of both, in one world. See docs/lander-design.rst.
//!
//! This is the same station as `lander_yamcs.py`, without Yamcs, so that it
//! runs where Yamcs cannot -- in a web page -- as well as on Linux:
//!
//! * it makes a landscape at random from a seed, `--seed`, or one it
//!   chooses and says, and writes it into the run's copy of the simulator
//!   file, which tcs-sim and this program's window both read
//!   ([`landscape`]);
//! * it starts tcs-sim with both configurations and waits for it to be
//!   ready, then both tcspecials, each ended when this program ends;
//! * it sends each vehicle CONNECT carrying its configuration's checksum
//!   until the vehicle answers with it, takes what each sends down through
//!   its gateways, and sends up what the window's buttons ask for and the
//!   quench the sky asks for once it is whole ([`station`]), all of it held
//!   `--delay` seconds each way, 1.3 by default, the time light takes
//!   between the Moon and the Earth;
//! * it shows what has come down in a window: the landscape and the lander
//!   where it was said to be, drawn as tcs-sim draws it, the values, the
//!   fuel, where the joystick was held, and whether it has landed or
//!   crashed.

pub mod landscape;
pub mod readings;
pub mod station;
mod window;

use std::path::{Path, PathBuf};

use tcsos::process::exit;
use tcsos::time::Duration;
use tcsprotocol::ready::SetUpFor;
use tcsprotocol::sim_config::SimConfigFile;
use tcsprotocol::simulate::Mode;

use crate::readings::Readings;
use crate::station::{Craft, Station};

slint::include_modules!();

/// What `--help` prints.
const USAGE: &str = "\
usage: lander-station [options]

Run from the repository's root, as the paths below are taken from it.

  --seed=N        make the landscape from seed N; one is chosen and said
                  where this is left out
  --delay=S       hold everything S seconds each way, the light time between
                  the Moon and the Earth: 1.3 where this is left out
  --no-memlock    have the tcspecials run without locking their memory
  -h, --help      print this, and exit
";

/// The vehicles' configurations, from the repository's root.
const VEHICLES: [&str; 2] = [
    "examples/lander-a/lander-a.ytcs",
    "examples/lander-b/lander-b.ytcs",
];

/// The shipped simulator file, whose landscape each run replaces.
const SHIPPED_SIM: &str = "examples/lander-common/lander-sim.yaml";

/// The light time between the Moon and the Earth, in seconds, where
/// `--delay` does not say.
const DELAY: f64 = 1.3;

/// How long tcs-sim is given to be ready: built by `cargo run` first, out
/// of a source tree, which is most of it.
const WAIT_FOR_SIMULATOR: Duration = Duration::from_secs(300);

/// What the command line asks for.
#[derive(Debug, Clone, PartialEq)]
struct Asked {
    /// The landscape's seed, where one was given.
    seed: Option<u64>,
    /// The light time each way.
    delay: Duration,
    /// Whether the tcspecials run without locking their memory.
    no_memlock: bool,
}

/// Reads the command line.
///
/// # Arguments
///
/// * `args` - The arguments, after the program's name.
///
/// # Returns
///
/// What they ask for, or `None` for `--help`.
///
/// # Errors
///
/// What is wrong with an argument.
fn asked(args: &[String]) -> Result<Option<Asked>, String> {
    let mut asked = Asked {
        seed: None,
        delay: Duration::from_secs_f64(DELAY),
        no_memlock: false,
    };
    for arg in args {
        if arg == "-h" || arg == "--help" {
            return Ok(None);
        } else if arg == "--no-memlock" {
            asked.no_memlock = true;
        } else if let Some(seed) = arg.strip_prefix("--seed=") {
            asked.seed = Some(seed.parse().map_err(|_| {
                format!("--seed={seed} is not a seed: give a whole number")
            })?);
        } else if let Some(delay) = arg.strip_prefix("--delay=") {
            let seconds: f64 = delay
                .parse()
                .ok()
                .filter(|s: &f64| s.is_finite() && *s >= 0.0)
                .ok_or_else(|| {
                    format!(
                        "--delay={delay} is not a number of seconds: give \
                         --delay=1.3, or --delay=0 for none"
                    )
                })?;
            asked.delay = Duration::from_secs_f64(seconds);
        } else {
            return Err(format!("{arg} is not an option lander-station takes"));
        }
    }
    Ok(Some(asked))
}

/// Writes the run's simulator file, with a landscape made from the seed.
///
/// # Arguments
///
/// * `seed` - The landscape's seed.
///
/// # Returns
///
/// Where it is.
///
/// # Errors
///
/// Where the shipped file does not read or the run's cannot be written.
fn write_run(seed: u64) -> Result<PathBuf, String> {
    let shipped = tcsos::fs::read_to_string(SHIPPED_SIM)
        .map_err(|e| format!("{SHIPPED_SIM}: {e}"))?;
    let lander = landscape::lander_of(&shipped)
        .map_err(|e| format!("{SHIPPED_SIM}: {e}"))?;
    let (least, most, _, _) = lander.extent();
    let made = landscape::made(seed, lander.width, lander.height, most - least);
    let text = landscape::run_sim_text(&shipped, seed, &made)
        .map_err(|e| format!("{SHIPPED_SIM}: {e}"))?;
    let dir = tcsos::env::temp_dir().join("lander-station");
    tcsos::fs::create_dir_all(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join("lander-sim.yaml");
    tcsos::fs::write(&path, text)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// Starts tcs-sim on both configurations and the run's simulator file, and
/// waits for it to say it is ready.
///
/// # Arguments
///
/// * `sim_path` - The run's simulator file.
///
/// # Returns
///
/// tcs-sim, started and ready.
///
/// # Errors
///
/// Where it cannot be started, or is not ready in time.
fn start_simulator(sim_path: &Path) -> Result<tcsos::process::Child, String> {
    let sim = sim_path.to_string_lossy().into_owned();
    let mode = Mode::All;
    let ours = SetUpFor {
        configuration: tcsprotocol::config_digest::digest_of_files(&VEHICLES)
            .map_err(|e| format!("{}: {e}", VEHICLES.join(", ")))?,
        simulator: tcsprotocol::config_digest::digest_of_simulator_file(&sim)
            .map_err(|e| format!("{sim}: {e}"))?,
        mode,
    };
    let at = SimConfigFile::load(&sim)
        .map_err(|e| format!("{sim}: {e}"))?
        .ready()
        .map_err(|e| format!("{sim}: {e}"))?;
    let mut args = vec![
        "--sending".to_string(),
        format!("{}={mode}", tcsprotocol::simulate::FLAG),
        format!("--simulator={sim}"),
    ];
    args.extend(VEHICLES.iter().map(|v| v.to_string()));
    let child = station::start("tcs-sim", &args)?;
    eprintln!("lander-station: waiting for tcs-sim to say at {at} it is ready");
    tcsmoc::simulator::wait(at, &ours, WAIT_FOR_SIMULATOR)
        .map_err(|e| e.to_string())?;
    Ok(child)
}

/// Runs the station: see the module's documentation. Exits once its window
/// is closed, ending what it started.
pub fn main() {
    let args: Vec<String> = tcsos::env::args().skip(1).collect();
    let asked = match asked(&args) {
        Ok(Some(asked)) => asked,
        Ok(None) => {
            print!("{USAGE}");
            exit(0);
        }
        Err(e) => {
            eprintln!("lander-station: {e}\n{USAGE}");
            exit(2);
        }
    };
    if let Err(e) = run(&asked) {
        eprintln!("lander-station: {e}");
        exit(1);
    }
    exit(0);
}

/// Runs the station until its window is closed.
///
/// # Arguments
///
/// * `asked` - What the command line asks for.
///
/// # Errors
///
/// What stopped it before its window opened.
fn run(asked: &Asked) -> Result<(), String> {
    let seed = asked.seed.unwrap_or_else(landscape::any_seed);
    eprintln!("lander-station: the landscape is made from seed {seed}");
    let sim_path = write_run(seed)?;
    let figures = landscape::lander_of(
        &tcsos::fs::read_to_string(&sim_path)
            .map_err(|e| format!("{}: {e}", sim_path.display()))?,
    )?;
    let crafts = VEHICLES
        .iter()
        .map(|path| Craft::read(Path::new(path)))
        .collect::<Result<Vec<Craft>, String>>()?;
    let units = crafts.iter().flat_map(Craft::units).collect();
    let station = Station::open(&crafts, asked.delay)?;

    let mut children = vec![start_simulator(&sim_path)?];
    for craft in &crafts {
        let mut args =
            vec![format!("{}={}", tcsprotocol::simulate::FLAG, Mode::All)];
        if asked.no_memlock {
            args.push("--no-memlock".to_string());
        }
        args.push(craft.path.to_string_lossy().into_owned());
        children.push(station::start("tcspecial", &args)?);
    }

    let note = format!(
        "Everything here is {} s behind the lander",
        asked.delay.as_secs_f64()
    );
    tcs_ui::windows::on_window_thread(move |closed| {
        if let Err(e) =
            window::show(figures, station, Readings::new(units), note, closed)
        {
            eprintln!("lander-station: the window: {e}");
        }
    });
    for child in &mut children {
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What some arguments ask for.
    ///
    /// # Arguments
    ///
    /// * `said` - Each argument.
    ///
    /// # Returns
    ///
    /// What they ask, or what is wrong.
    fn asked_of(said: &[&str]) -> Result<Option<Asked>, String> {
        let args: Vec<String> = said.iter().map(|s| s.to_string()).collect();
        asked(&args)
    }

    /// The options are read, and the defaults are the Python station's.
    #[test]
    fn the_command_line_is_read() {
        let plain = asked_of(&[]).expect("read").expect("asked");
        assert_eq!(plain.seed, None);
        assert_eq!(plain.delay, Duration::from_secs_f64(1.3));
        assert!(!plain.no_memlock);
        let given = asked_of(&["--seed=7", "--delay=0", "--no-memlock"])
            .expect("read")
            .expect("asked");
        assert_eq!(given.seed, Some(7));
        assert_eq!(given.delay, Duration::ZERO);
        assert!(given.no_memlock);
        assert_eq!(asked_of(&["--help"]), Ok(None));
        assert!(asked_of(&["--seed=x"]).is_err());
        assert!(asked_of(&["--delay=-1"]).is_err());
        assert!(asked_of(&["--yamcs"]).is_err());
    }
}
