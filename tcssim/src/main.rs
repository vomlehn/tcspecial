//! TCSpecial Payload Simulator (tcssim)
//!
//! A GUI application for simulating payloads that communicate with tcspecial.
//!
//! Two configuration files describe what to simulate. The payload file
//! describes the payloads themselves: which data handlers exist, how tcspecial
//! reaches each one, and how big its packets are. The simulator file adds what
//! simulating one takes and the payload file has no business knowing: how fast
//! a payload produces packets, and how a packet is divided into segments. The
//! two are joined by the data handler's name; see
//! [`tcslibgs::sim_config`].

use slint::{Model, ModelRc, SharedString, VecModel};
use std::env;
use std::process;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use tcslibgs::config::{load_dh_configs, payload_path_from_args, SIM_PAYLOAD_CONFIG_PATH_VAR};
use tcslibgs::{payload_parameters, DHConfig, DHSample, NO_TRANSFER_TIME};

mod endpoint;
mod grid;
mod payload;
mod payload_device;
mod payload_i2c;
mod payload_serial;
mod payload_spi;
mod payload_tcp;
mod payload_unix;
mod pty;
mod payload_udp;

use endpoint::{endpoint_description, payload_config_from};
use payload::{PayloadConfig, PayloadStats, SimulatedPayload};
use tcslibgs::{ResolvedSim, SimConfigFile};

slint::include_modules!();


/// What a panel's status says of a payload that is sending, and of one that
/// is not.
///
/// `ui/main.slint` labels the panel's one button from these and colours the
/// status line by them, and `handle_transfer_button` reads the same status
/// back to decide which way a press goes, so the button and what it does
/// cannot disagree. What the button actually says each way is the window's
/// own: the two words appear in Rust only in the test that holds the window
/// to this rule.
const RUNNING_STATUS: &str = "Running";
const STOPPED_STATUS: &str = "Stopped";

/// Which environment variable names the payload simulation configuration, and
/// what is read when that variable is unset.
const SIM_CONFIG_PATH_VAR: &str = "PAYLOAD_SIM_YAML";
const DEFAULT_SIM_CONFIG_PATH: &str = "tests/manual/tcspecial1sim.yaml";

/// The two strings a panel shows for one sample: when, and what.
///
/// The formatting is [`DHSample::panel_lines`], which tcsmoc's panels use
/// too: the two show the one transfer from opposite ends of a link and are
/// read side by side, so a byte or a time that read differently in the two
/// would look like a difference in the data.
fn sample_lines(sample: &DHSample) -> (SharedString, SharedString) {
    let (time, data) = sample.panel_lines();
    (SharedString::from(time), SharedString::from(data))
}

/// Take an edited setting to the payload it belongs to, and say whether the
/// two sizes still describe something that can be sent.
///
/// A panel holds no state of its own: what a spin box shows is what the model
/// says, so an edit has to come through here to be kept. The answer about the
/// sizes is decided here too, so there is one rule -- the window disables the
/// button while this says something and shows what it says, rather than
/// comparing the two numbers itself.
fn handle_config_edits(
    ui: &MainWindow,
    payloads: Arc<Mutex<Vec<SimulatedPayload>>>,
    model: Rc<VecModel<PayloadInfo>>,
) {
    ui.on_config_payload(
        move |row, packet_size, segment_size, packet_interval, segment_interval| {
            let guard = payloads.lock().unwrap();
            if let Some(payload) = guard.get(row as usize) {
                payload.set_packet_size(packet_size as u32);
                payload.set_segment_size(segment_size as u32);
                payload.set_packet_interval(packet_interval as u32);
                payload.set_segment_interval(segment_interval as u32);
            }
            drop(guard);

            let problem = sizes_problem(packet_size, segment_size);
            update_row(&model, row as usize, |info| {
                info.sizes_problem = SharedString::from(problem);
            });
        },
    );
}

/// What is wrong with a packet size and a segment size together, and nothing
/// when they agree.
///
/// A segment is a piece of a packet, so a segment larger than the packet
/// describes a piece larger than the whole it is a piece of. Equal is the
/// ordinary case -- a packet sent in one segment -- and is what a simulator
/// file that states no segment size asks for, so only a segment strictly
/// larger than the packet is wrong.
///
/// Returns the sentence the window shows, which says both numbers and either
/// way out of it: the two spin boxes are side by side and either can be the
/// one that was meant to change.
fn sizes_problem(packet_size: i32, segment_size: i32) -> String {
    if segment_size <= packet_size {
        return String::new();
    }

    format!(
        "A segment of {segment_size} bytes cannot be part of a packet of \
         {packet_size}: a segment is a piece of a packet. Raise the packet size \
         to {segment_size} or more, or lower the segment size to {packet_size} \
         or less."
    )
}

/// What a press of a panel's one button asks of the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transfer {
    /// Start it, so it sends.
    Transmit,
    /// Stop it, so it goes quiet.
    Silent,
}

/// Which way a press of a panel's one button goes.
///
/// Decided by what the button offered, read back from the status its label
/// comes from -- not by what the payload turns out to be doing. Anything that
/// is not the running status offers to transmit: a payload that failed to
/// start shows the stopped status, and the useful thing to offer then is the
/// start that failed rather than a stop of something that never began.
fn transfer_wanted(status: &str) -> Transfer {
    if status == RUNNING_STATUS {
        Transfer::Silent
    } else {
        Transfer::Transmit
    }
}

/// Put a payload's traffic into its panel.
///
/// The times are what the panel is for: a count tells how much has moved
/// since the payload started, and the time tells whether any of it is moving
/// now. A payload stopped ten minutes ago and one sending every second have
/// the same count a moment after they are looked at. The bytes beside each
/// time are the head of that packet, which is what says the traffic is the
/// traffic that was expected rather than merely traffic.
fn show_traffic(info: &mut PayloadInfo, stats: &PayloadStats) {
    info.packets_sent = i32::try_from(stats.packets_sent).unwrap_or(i32::MAX);
    info.packets_recv = i32::try_from(stats.packets_recv).unwrap_or(i32::MAX);
    (info.last_sent_time, info.last_sent_data) = sample_lines(&stats.last_sent);
    (info.last_recv_time, info.last_recv_data) = sample_lines(&stats.last_recv);
}

/// Turn a data handler and its settled simulator settings into the panel the
/// window shows for it.
fn payload_info_from(dh: &DHConfig, sim: &ResolvedSim) -> PayloadInfo {
    PayloadInfo {
        name: SharedString::from(dh.name.0.clone()),
        config: SharedString::from(endpoint_description(&dh.endpoint)),
        packet_size: i32::try_from(dh.packet_size).unwrap_or(i32::MAX),
        segment_size: sim.segment_size as i32,
        packet_interval: sim.packet_interval_ms as i32,
        segment_interval: sim.segment_interval_ms as i32,
        status: SharedString::from(STOPPED_STATUS),
        last_sent_time: SharedString::from(NO_TRANSFER_TIME),
        last_sent_data: SharedString::new(),
        last_recv_time: SharedString::from(NO_TRANSFER_TIME),
        last_recv_data: SharedString::new(),
        packets_sent: 0,
        packets_recv: 0,
        // Both files, as they were read. Tcssim has both in hand, so its
        // panels can show the whole of what a payload set says about a
        // payload.
        parameters: SharedString::from(payload_parameters(dh, Some(sim))),
        // Checked from the start, not only after an edit: a simulator file
        // may state a segment larger than the payload file's packet, and a
        // panel that said nothing about it until a spin box was touched would
        // have the payload refuse to start for no stated reason.
        // Whether anything is ever sent to this payload, which decides
        // whether its panel has a received line at all.
        triggered: sim.triggered,
        sizes_problem: SharedString::from(sizes_problem(
            i32::try_from(dh.packet_size).unwrap_or(i32::MAX),
            sim.segment_size as i32,
        )),
    }
}

/// Change one panel's contents, leaving the rest of it alone.
///
/// A Slint model row is a value rather than a place, so a field is changed by
/// reading the row, changing it, and putting it back.
fn update_row(model: &Rc<VecModel<PayloadInfo>>, row: usize, f: impl FnOnce(&mut PayloadInfo)) {
    if let Some(mut info) = model.row_data(row) {
        f(&mut info);
        model.set_row_data(row, info);
    }
}

fn main() {
//    env_logger::init();

    // Read both configuration files before the window exists, so a file that
    // cannot be read is reported plainly rather than as an empty window.
    let payload_path = match payload_path_from_args(
        env::args(),
        Some(SIM_PAYLOAD_CONFIG_PATH_VAR),
    ) {
        Ok(payload_path) => payload_path,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };
    let sim_path = env::var(SIM_CONFIG_PATH_VAR)
        .unwrap_or_else(|_| DEFAULT_SIM_CONFIG_PATH.to_string());

    let dh_configs = match load_dh_configs(&payload_path) {
        Ok(dh_configs) => dh_configs,
        Err(e) => {
            eprintln!("Error loading payload configuration from {}: {}", payload_path, e);
            process::exit(1);
        }
    };

    let sim_file = match SimConfigFile::load(&sim_path) {
        Ok(sim_file) => sim_file,
        Err(e) => {
            eprintln!("Error loading simulator configuration from {}: {}", sim_path, e);
            process::exit(1);
        }
    };

    // Join the two by name. Every data handler needs settings and every
    // settings entry needs a handler, so this is where a file naming a payload
    // that does not exist, or leaving one undriven, is caught.
    let sims = match sim_file.resolve(&dh_configs) {
        Ok(sims) => sims,
        Err(e) => {
            eprintln!("Error in {} against {}: {}", sim_path, payload_path, e);
            process::exit(1);
        }
    };

    let configs: Vec<PayloadConfig> = match dh_configs
        .iter()
        .zip(&sims)
        .map(|(dh, sim)| payload_config_from(dh, sim))
        .collect()
    {
        Ok(configs) => configs,
        Err(e) => {
            eprintln!("Error in payload configuration {}: {}", payload_path, e);
            process::exit(1);
        }
    };

    if configs.is_empty() {
        eprintln!("{} describes no payloads to simulate", payload_path);
        process::exit(1);
    }

    eprintln!(
        "Simulating {} payloads from {} and {}",
        configs.len(),
        payload_path,
        sim_path
    );
    if let Some(description) = &sim_file.description {
        eprintln!(
            "Simulator configuration: {} (version {})",
            description,
            sim_file.version.as_deref().unwrap_or("unstated")
        );
    }

    let ui = MainWindow::new().unwrap();
    let ui_weak = ui.as_weak();

    // One panel per payload, in a grid worked out from how many there are.
    let model: Rc<VecModel<PayloadInfo>> = Rc::new(VecModel::from(
        dh_configs
            .iter()
            .zip(&sims)
            .map(|(dh, sim)| payload_info_from(dh, sim))
            .collect::<Vec<_>>(),
    ));
    ui.set_payload_model(ModelRc::from(model.clone()));

    let shape = grid::grid_shape(configs.len());
    ui.set_columns(shape.columns as i32);
    ui.set_cell_width(grid::PANEL_WIDTH);
    ui.set_cell_height(grid::PANEL_HEIGHT);
    // Taller than the grid asks for where there is a third row to look
    // ahead for, and no taller than the payloads on hand need otherwise. It
    // is also why the window can open taller than it is wide, which the shape
    // itself never is: the shape rule chooses the grid, and the room rule
    // says how much of the window to give it.
    ui.window()
        .set_size(grid::window_size(&shape, configs.len()));

    // Create simulated payloads
    let payloads: Arc<Mutex<Vec<SimulatedPayload>>> = Arc::new(Mutex::new(
        configs.into_iter().map(|c| SimulatedPayload::new(c)).collect()
    ));

    // The one button each panel has, which starts and stops its payload.
    {
        let payloads = payloads.clone();
        let model = model.clone();
        ui.on_transfer_payload(move |row| {
            let row = row as usize;

            // The status the button's label came from, so what happens is
            // what the label offered.
            let showing = match model.row_data(row) {
                Some(info) => info.status.to_string(),
                None => return,
            };

            let mut guard = payloads.lock().unwrap();
            let payload = match guard.get_mut(row) {
                Some(payload) => payload,
                None => return,
            };

            match transfer_wanted(&showing) {
                Transfer::Transmit => match payload.start() {
                    Ok(_) => update_row(&model, row, |info| {
                        info.status = SharedString::from(RUNNING_STATUS);
                    }),
                    // The panel keeps saying it is not sending, which is
                    // true: a payload that could not be started has not
                    // started. Named as the payload configuration names it,
                    // because a row number is this window's business and not
                    // the reader's.
                    Err(e) => eprintln!("{}: cannot start: {}", payload.name(), e),
                },
                Transfer::Silent => {
                    payload.stop();
                    update_row(&model, row, |info| {
                        info.status = SharedString::from(STOPPED_STATUS);
                    });
                }
            }
        });
    }

    handle_config_edits(&ui, payloads.clone(), model.clone());

    // Quit handler
    {
        let payloads = payloads.clone();
        let ui_weak = ui_weak.clone();
        ui.on_quit_clicked(move || {
            // Stop all payloads
            let mut guard = payloads.lock().unwrap();
            for payload in guard.iter_mut() {
                payload.stop();
            }
            drop(guard);

            let ui = ui_weak.unwrap();
            ui.hide().unwrap();
        });
    }

    // Periodic update timer
    {
        let payloads = payloads.clone();
        let model = model.clone();
        let timer = slint::Timer::default();
        timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(500), move || {
            let guard = payloads.lock().unwrap();
            for (row, payload) in guard.iter().enumerate() {
                let stats = payload.stats();
                update_row(&model, row, |info| show_traffic(info, &stats));
            }
        });

        // Keep timer alive
        std::mem::forget(timer);
    }

    ui.run().unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tcslibgs::{DHId, DHName, EndpointConfig, NetworkConfig, NetworkProtocol};

    fn repo_file(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(name)
    }

    /// A sample of a transfer, as the stats hold it.
    fn sample_of(seconds: u64, data: &[u8]) -> DHSample {
        let mut sample = DHSample::new();
        sample.record(data);
        sample.time = Some(tcslibgs::Timestamp {
            seconds,
            nanoseconds: 0,
        });
        sample
    }

    /// The window size the design document states is the one the rule gives.
    ///
    /// It said 640x496 for a long time against the 664x722 the program opened
    /// at: the number went stale when the panels gained their three rows of
    /// data and the window its three-row floor, and nothing noticed because
    /// nothing read it. Anyone sizing a screen or a screenshot from the
    /// document was being told the wrong thing.
    #[test]
    fn the_design_document_states_the_size_the_window_opens_at() {
        let doc = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("docs/design.rst");
        let text = std::fs::read_to_string(&doc).unwrap();

        // The four payloads of tcspecial1.yaml, which is the set the document
        // describes.
        let shape = grid::grid_shape(4);
        let size = grid::window_size(&shape, 4);
        let said = format!("{}x{}", size.width, size.height);

        assert_eq!(
            (shape.columns, shape.rows),
            (2, 2),
            "the document describes a two-by-two grid"
        );
        assert!(
            text.contains(&said),
            "{} does not say the window opens at {said}",
            doc.display()
        );
    }

    /// An edit that makes the two sizes disagree says so and takes the
    /// button away, and an edit that fixes them gives it back.
    ///
    /// The whole path, from the spin box to the button: the window asks Rust,
    /// Rust answers in the row, and the panel obeys the answer. Checking the
    /// rule alone would say nothing about any of that, and a panel that
    /// quietly let an unsendable pair be started is what this is for.
    ///
    /// Not a test of its own: one test per binary may start the testing
    /// backend, and its windows belong to the thread that made them.
    fn sizes_that_cannot_be_sent_stop_the_button_and_say_so() {
        use i_slint_backend_testing::ElementHandle;

        let dh = handler("DH0");
        let settings = sim(1000, 1000, 12);
        let model: Rc<VecModel<PayloadInfo>> =
            Rc::new(VecModel::from(vec![payload_info_from(&dh, &settings)]));

        let ui = MainWindow::new().unwrap();
        ui.set_payload_model(ModelRc::from(model.clone()));
        ui.set_columns(1);

        // The handler the running program registers, over the payload the
        // files describe.
        let payloads: Arc<Mutex<Vec<SimulatedPayload>>> = Arc::new(Mutex::new(vec![
            SimulatedPayload::new(payload_config_from(&dh, &settings).expect("a payload")),
        ]));
        handle_config_edits(&ui, payloads, model.clone());
        ui.show().unwrap();

        let button = |ui: &MainWindow| {
            ElementHandle::find_by_accessible_label(ui, "Transmit")
                .next()
                .and_then(|b| b.accessible_enabled())
        };
        let said = |ui: &MainWindow| -> Vec<String> {
            ElementHandle::find_by_element_type_name(ui, "Text")
                .filter_map(|e| e.accessible_label())
                .map(|label| label.to_string())
                .filter(|line| line.contains("cannot be"))
                .collect()
        };

        // A packet of twelve bytes in segments of twelve: the ordinary case,
        // and nothing to say about it.
        assert_eq!(button(&ui), Some(true), "the button is not there to press");
        assert!(said(&ui).is_empty(), "something is said before anything is wrong");

        // The packet size, down one, which leaves the segment larger than the
        // packet it is a piece of. The first spin box of the panel is the
        // packet size; Slint's testing backend does not report the last child
        // of a layout, which is why the segment's is not reached here.
        let packet_size = ElementHandle::find_by_element_type_name(&ui, "SpinBox")
            .next()
            .expect("the panel has a packet size to edit");
        packet_size.invoke_accessible_decrement_action();
        assert_eq!(packet_size.accessible_value().as_deref(), Some("11"));

        assert_eq!(
            button(&ui),
            Some(false),
            "a pair that cannot be sent left the button there to press"
        );
        let shown = said(&ui);
        assert!(
            shown.iter().any(|line| line.contains("segment of 12")
                && line.contains("packet of 11")),
            "nothing on screen says what is wrong: {shown:?}"
        );

        // And back up again: the pair can be sent, so the button returns and
        // the popup stops saying otherwise. An open popup still saying these
        // sizes cannot be sent, after the edit that made them sendable, would
        // be the wrong answer left on the screen.
        packet_size.invoke_accessible_increment_action();
        assert_eq!(packet_size.accessible_value().as_deref(), Some("12"));
        assert_eq!(button(&ui), Some(true), "the button did not come back");
        assert!(said(&ui).is_empty(), "the popup is still saying it: {:?}", said(&ui));
    }

    /// A payload whose files disagree about the two sizes cannot be started,
    /// and says why before anything is touched.
    ///
    /// A simulator file may state a segment larger than the payload file's
    /// packet. A panel that said nothing about it until a spin box was
    /// touched would have the payload refuse to start for no stated reason.
    #[test]
    fn a_payload_whose_files_disagree_starts_out_unsendable() {
        let dh = handler("DH0");
        let info = payload_info_from(&dh, &sim(1000, 1000, 20));
        assert_eq!(dh.packet_size, 12, "the payload file says twelve bytes");
        assert!(
            info.sizes_problem.contains("segment of 20")
                && info.sizes_problem.contains("packet of 12"),
            "{}",
            info.sizes_problem
        );

        // And one whose files agree says nothing.
        let info = payload_info_from(&dh, &sim(1000, 1000, 12));
        assert_eq!(info.sizes_problem, "");
    }

    /// A segment larger than the packet it is a piece of is refused, and
    /// equal sizes are the ordinary case.
    #[test]
    fn a_segment_cannot_be_larger_than_its_packet() {
        // The ordinary case, which is what a simulator file stating no
        // segment size asks for: one segment, the size of the packet.
        assert_eq!(sizes_problem(12, 12), "");
        assert_eq!(sizes_problem(12, 5), "");
        assert_eq!(sizes_problem(1, 1), "");

        // And the one that cannot be sent. The sentence says both numbers and
        // either way out of it: the two spin boxes are side by side and
        // either can be the one that was meant to change.
        let said = sizes_problem(11, 12);
        assert!(said.contains("12") && said.contains("11"), "{said}");
        assert!(
            said.contains("Raise the packet size") && said.contains("lower the segment size"),
            "the error should say either way out of it: {said}"
        );
    }

    /// What the window's one panel button says each way.
    ///
    /// Here rather than beside the statuses because the window owns its
    /// labels: Slint builds the string, and these are how the test below
    /// holds it to the same rule Rust decides a press by.
    const SILENT_LABEL: &str = "Silent";
    const TRANSMIT_LABEL: &str = "Transmit";

    impl Transfer {
        /// The label a button offering this carries.
        fn label(&self) -> &'static str {
            match self {
                Transfer::Transmit => TRANSMIT_LABEL,
                Transfer::Silent => SILENT_LABEL,
            }
        }
    }

    /// A panel's one button offers what pressing it will do, and Rust does
    /// what the label offered.
    ///
    /// There were two buttons, Start and Stop, and either was pressable
    /// whatever the payload was doing: a Stop on a payload that was not
    /// sending did nothing and read as though it had. One button offers the
    /// one thing worth doing next -- and because Slint decides the label and
    /// Rust decides what a press does, both from the same status, the two
    /// have to be checked against each other or they drift into offering one
    /// thing and doing the other.
    #[test]
    fn the_transfer_button_does_what_its_label_offers() {
        // A payload that is sending offers to go silent, and one that is not
        // offers to transmit. Anything that is not the running status offers
        // to transmit: a payload that failed to start shows the stopped
        // status, and the useful thing to offer then is the start that
        // failed.
        assert_eq!(transfer_wanted(RUNNING_STATUS), Transfer::Silent);
        assert_eq!(transfer_wanted(STOPPED_STATUS), Transfer::Transmit);
        assert_eq!(transfer_wanted(""), Transfer::Transmit);

        assert_eq!(Transfer::Silent.label(), SILENT_LABEL);
        assert_eq!(Transfer::Transmit.label(), TRANSMIT_LABEL);

        // And the window labels it by the same rule, which is the half Rust
        // cannot fail on: the label is a Slint expression, so a drift between
        // the two files is a button that offers Transmit and stops the
        // payload.
        let window = Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/main.slint");
        let source = std::fs::read_to_string(&window).unwrap();
        let offered = format!(
            "payload-status == \"{}\" ? \"{}\" : \"{}\"",
            RUNNING_STATUS,
            transfer_wanted(RUNNING_STATUS).label(),
            transfer_wanted(STOPPED_STATUS).label()
        );
        assert!(
            source.contains(&offered),
            "{} does not label its transfer button {:?}",
            window.display(),
            offered
        );

        // The status line is coloured and weighted by the same status, so it
        // and the button cannot come to disagree either.
        assert!(
            source.contains(&format!("payload-status == \"{}\" ? black : red", RUNNING_STATUS)),
            "{} does not colour the status line on {:?}",
            window.display(),
            RUNNING_STATUS
        );

        // One button, not two: the pair they replaced took the same room and
        // left a press available that did nothing.
        for gone in ["text: \"Start\"", "text: \"Stop\""] {
            assert!(
                !source.contains(gone),
                "{} still has a panel button saying {gone}",
                window.display()
            );
        }
    }

    /// A panel shows when the last packet went each way and what it was, and
    /// says plainly when there has not been one.
    ///
    /// The time is the point of the two rows: a count says how much has moved
    /// since the payload started, and only a time says whether any of it is
    /// moving now -- a payload stopped ten minutes ago and one sending every
    /// second look identical by their counts. The bytes beside it are what
    /// say the traffic is the traffic that was expected rather than merely
    /// traffic.
    #[test]
    fn a_panel_shows_when_the_last_packet_went_and_what_it_was() {
        let mut info = payload_info_from(&handler("DH0"), &sim(1000, 1000, 12));
        assert_eq!(info.last_sent_time, NO_TRANSFER_TIME);
        assert_eq!(info.last_recv_time, NO_TRANSFER_TIME);

        // Nothing has moved yet, so neither row shows a time or any bytes: a
        // count of nought with a time beside it would be a transfer that
        // never happened.
        show_traffic(&mut info, &PayloadStats::default());
        assert_eq!(info.last_sent_time, NO_TRANSFER_TIME);
        assert_eq!(info.last_sent_data, "");
        assert_eq!(info.last_recv_time, NO_TRANSFER_TIME);
        assert_eq!(info.last_recv_data, "");

        // One each way, an hour apart, as the panel shows them.
        show_traffic(
            &mut info,
            &PayloadStats {
                packets_sent: 7,
                packets_recv: 2,
                last_sent: sample_of(3661, &[0x01, 0xAB, 0xFF]),
                last_recv: sample_of(7322, b"READ\r"),
                ..PayloadStats::default()
            },
        );
        assert_eq!(info.last_sent_time, "01:01:01");
        assert_eq!(info.last_sent_data, "01 AB FF");
        assert_eq!(info.last_recv_time, "02:02:02");
        assert_eq!(info.last_recv_data, "52 45 41 44 0D");
        assert_eq!((info.packets_sent, info.packets_recv), (7, 2));

        // A packet longer than the sample keeps shows a head and says so,
        // which is how a panel of eight bytes talks about a packet of twelve.
        show_traffic(
            &mut info,
            &PayloadStats {
                packets_sent: 8,
                last_sent: sample_of(86_399, &[0x11; 12]),
                ..PayloadStats::default()
            },
        );
        assert_eq!(info.last_sent_time, "23:59:59");
        assert_eq!(info.last_sent_data, "11 11 11 11 11 11 11 11...");

        // And one direction without the other: a payload that sends and is
        // never spoken to is the commonest kind there is.
        assert_eq!(
            (info.last_recv_time.as_str(), info.last_recv_data.as_str()),
            (NO_TRANSFER_TIME, ""),
            "a payload nobody has spoken to has no received line to show"
        );
    }

    /// Every panel, and each of the three rows of data it ends with, is
    /// inside the window tcssim opens for each shipped set.
    ///
    /// Where `CHROME_HEIGHT` and `CHROME_WIDTH` come from. The window is
    /// opened here as main() opens it, through Slint's testing backend: no
    /// display and no event loop, but a real layout with real font metrics,
    /// so what this measures is what the window does. A panel scrolled out of
    /// sight is not merely off-screen, it is never built, so the count of
    /// panels found is part of the check.
    #[test]
    fn every_panels_data_is_inside_the_window() {
        use i_slint_backend_testing as testing;
        use testing::ElementHandle;

        testing::init_integration_test_with_system_time();

        // Every shipped set, and then more payloads than the three-row floor
        // covers. Both are needed: the floor leaves the shipped sets rows to
        // spare, so only a grid taller than the floor can show whether the
        // panel and chrome sizes are right.
        let mut cases: Vec<(String, Vec<PayloadInfo>)> = Vec::new();
        for (payload_path, sim_path) in shipped_sets() {
            let dh_configs = load_dh_configs(&payload_path)
                .unwrap_or_else(|e| panic!("{} failed to load: {e}", payload_path.display()));
            let sim_file = SimConfigFile::load(&sim_path)
                .unwrap_or_else(|e| panic!("{} failed to load: {e}", sim_path.display()));
            let sims = sim_file
                .resolve(&dh_configs)
                .unwrap_or_else(|e| panic!("{} does not fit: {e}", sim_path.display()));

            cases.push((
                format!("{}", payload_path.display()),
                dh_configs
                    .iter()
                    .zip(&sims)
                    .map(|(dh, sim)| payload_info_from(dh, sim))
                    .collect(),
            ));
        }
        let one = cases[0].1[0].clone();
        cases.push((
            "sixteen payloads".to_string(),
            std::iter::repeat_with(|| one.clone()).take(16).collect(),
        ));

        // A case of panels filled with the longest lines they can show was
        // tried here and taken out again: Slint clips a line too long for its
        // panel rather than widening anything, and the testing backend
        // reports a Text's laid-out width rather than the width it wanted, so
        // such a case measures exactly what an empty one does. What the rows
        // hold is checked by the tests of show_traffic instead, and how much
        // room they have by the heights below.

        for (set, rows) in cases {
            let panels = rows.len();

            let ui = MainWindow::new().unwrap();
            ui.set_payload_model(ModelRc::from(Rc::new(VecModel::from(rows))));

            // Opened as main() opens it, floor and all.
            let shape = grid::grid_shape(panels);
            ui.set_columns(shape.columns as i32);
            ui.set_cell_width(grid::PANEL_WIDTH);
            ui.set_cell_height(grid::PANEL_HEIGHT);
            ui.window().set_size(grid::window_size(&shape, panels));
            ui.show().unwrap();

            let window = ui.window().size().to_logical(1.0);

            // The grid fits the area it is given, across and down: it stood
            // wider and taller than that area while the padding it keeps
            // inside the scrolling area was left out of the window size.
            let area = ElementHandle::find_by_element_id(&ui, "MainWindow::scroll")
                .next()
                .expect("the window has a scrolling area for the panels");
            let grid = ElementHandle::find_by_element_id(&ui, "MainWindow::grid")
                .next()
                .expect("the window has a grid of panels");
            assert!(
                grid.size().width <= area.size().width
                    && grid.size().height <= area.size().height,
                "{set}: the grid is {}x{} in an area {}x{}",
                grid.size().width,
                grid.size().height,
                area.size().width,
                area.size().height
            );

            // Room for as many rows as the rule says, and no more than
            // that: a set of fewer payloads than the window looks ahead for
            // opens only as tall as those payloads need.
            let rows = grid::rows_of_room(shape.rows, panels);
            let wanted = grid::height_for_rows(rows) - grid::CHROME_HEIGHT;
            assert!(
                area.size().height >= wanted,
                "{set}: the panels have {} of the {wanted} that {rows} rows need",
                area.size().height
            );
            assert!(
                area.size().height < wanted + grid::PANEL_HEIGHT,
                "{set}: the panels have {}, room for a row more than the {rows} wanted",
                area.size().height
            );

            // Every button is inside the window across, which is what
            // keeps a label from running off the side of its panel: a
            // Button reports the width it was given, where a Text that does
            // not fit is clipped and reports the width it had.
            for button in i_slint_backend_testing::ElementHandle::find_by_element_type_name(
                &ui, "Button",
            ) {
                let right = button.absolute_position().x + button.size().width;
                assert!(
                    right <= window.width,
                    "{set}: the {:?} button reaches {right}, past the {} the \
                     window is wide",
                    button.accessible_label(),
                    window.width
                );
            }

            for what in [
                "PayloadPanel",
                "PayloadPanel::sent-row",
                "PayloadPanel::recv-row",
                "PayloadPanel::packets-row",
            ] {
                let found: Vec<_> = if what.contains("::") {
                    ElementHandle::find_by_element_id(&ui, what).collect()
                } else {
                    ElementHandle::find_by_element_type_name(&ui, what).collect()
                };

                assert_eq!(
                    found.len(),
                    panels,
                    "{set}: {} of {panels} {what} were built, so the rest are \
                     out of sight",
                    found.len()
                );

                // Against the edges of the area the panels live in, which
                // is where they are cut off, rather than the edges of the
                // window: below the area is the row with Quit All in it.
                let area_bottom = area.absolute_position().y + area.size().height;
                let area_right = area.absolute_position().x + area.size().width;
                for (n, element) in found.iter().enumerate() {
                    let bottom = element.absolute_position().y + element.size().height;
                    let right = element.absolute_position().x + element.size().width;
                    assert!(
                        bottom <= area_bottom && right <= area_right,
                        "{set}: {what} {n} reaches ({right},{bottom}), past the \
                         ({area_right},{area_bottom}) the panels have in a \
                         window {}x{}",
                        window.width,
                        window.height
                    );
                }
            }
        }

        // The backend is up and this thread owns its windows, so the other
        // checks that need one run from here.
        the_panel_shows_the_time_and_bytes_it_was_given();
        the_params_button_shows_what_both_files_said();
        sizes_that_cannot_be_sent_stop_the_button_and_say_so();
    }

    /// Every shipped payload set: a payload file and the simulator file beside
    /// it.
    ///
    /// The sets are discovered rather than listed, so a set added to the
    /// repository is covered by the tests that already exist, instead of only
    /// by whichever one somebody remembers to extend. A payload file with no
    /// simulator file beside it fails here: tcssim needs both, so half a set
    /// is a set that cannot be run.
    fn shipped_sets() -> Vec<(std::path::PathBuf, std::path::PathBuf)> {
        let root = repo_file("tests/manual");
        let mut sets = Vec::new();

        for entry in std::fs::read_dir(&root).expect("the repository root is readable") {
            let path = entry.expect("a readable directory entry").path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name,
                None => continue,
            };

            // The simulator files are found through their payload file, not on
            // their own, so that one with no payload file is noticed too.
            let stem = match name
                .strip_prefix("tcspecial")
                .and_then(|rest| rest.strip_suffix(".yaml"))
            {
                Some(stem) if !stem.ends_with("sim") => stem,
                _ => continue,
            };

            let sim_path = repo_file(&format!("tests/manual/tcspecial{}sim.yaml", stem));
            assert!(
                sim_path.exists(),
                "{} has no {} beside it, so the set cannot be simulated",
                path.display(),
                sim_path.display()
            );
            sets.push((path, sim_path));
        }

        sets.sort();
        assert!(
            !sets.is_empty(),
            "no payload sets found in {}",
            root.display()
        );
        sets
    }

    /// A data handler, for the tests that only need one to exist.
    fn handler(name: &str) -> DHConfig {
        DHConfig {
            dh_id: DHId(0),
            name: DHName::new(name),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: "localhost".to_string(),
                port: 5000,
            }),
            packet_size: 12,
            oc: None,
            mode: Default::default(),
        }
    }

    fn sim(packet_interval_ms: u32, segment_interval_ms: u32, segment_size: u32) -> ResolvedSim {
        ResolvedSim {
            packet_interval_ms,
            segment_interval_ms,
            segment_size,
            triggered: false,
            faults: Default::default(),
            payload_address: None,
            payload_port: None,
        }
    }

    /// No shipped simulator file fits another set's payload file.
    ///
    /// Two sets of the repository differ only in how their one payload is
    /// reached -- one by datagram, one by stream -- and the files of a set are
    /// joined by handler name alone, which a copied set keeps. So the wrong
    /// pairing is a mistake that can actually be made, by passing the wrong
    /// path or by copying a set and editing one of its two files. It is caught
    /// because each simulated payload states the kind of payload it stands in
    /// for, and nothing else in either file would notice.
    ///
    /// Every crossed pairing being refused also says the sets are really
    /// different from one another: two that were the same but for their names
    /// would be one set shipped twice.
    #[test]
    fn a_simulator_file_does_not_fit_another_sets_payload_file() {
        let sets = shipped_sets();
        assert!(sets.len() > 1, "one set cannot be paired with another");

        for (payload_path, _) in &sets {
            let dh_configs = load_dh_configs(payload_path).expect("the payload file loads");

            for (other_payload, sim_path) in &sets {
                if other_payload == payload_path {
                    continue;
                }

                let sim_file = SimConfigFile::load(sim_path).expect("the simulator file loads");
                assert!(
                    sim_file.resolve(&dh_configs).is_err(),
                    "{} is accepted for {}, which it was not written for",
                    sim_path.display(),
                    payload_path.display()
                );
            }
        }
    }

    /// Every shipped set has to describe payloads the simulator can simulate,
    /// and its two files have to agree with each other.
    ///
    /// Nothing else checks that: the GUI only reveals files that disagree when
    /// someone runs it and finds the simulator refusing to start.
    #[test]
    fn the_shipped_files_become_simulator_configs() {
        for (payload_path, sim_path) in shipped_sets() {
            let dh_configs = load_dh_configs(&payload_path)
                .unwrap_or_else(|e| panic!("{} failed to load: {e}", payload_path.display()));
            let sim_file = SimConfigFile::load(&sim_path)
                .unwrap_or_else(|e| panic!("{} failed to load: {e}", sim_path.display()));

            let sims = sim_file.resolve(&dh_configs).unwrap_or_else(|e| {
                panic!(
                    "{} does not fit {}: {e}",
                    sim_path.display(),
                    payload_path.display()
                )
            });

            let configs: Vec<PayloadConfig> = dh_configs
                .iter()
                .zip(&sims)
                .map(|(dh, s)| payload_config_from(dh, s))
                .collect::<Result<_, _>>()
                .unwrap_or_else(|e| panic!("{} is not simulatable: {e}", payload_path.display()));

            assert!(
                !configs.is_empty(),
                "{} has no payloads to simulate",
                payload_path.display()
            );
        }
    }

    /// A shipped payload file states a packet interval only where a payload
    /// is triggered, and no other simulation setting at all.
    ///
    /// The interval a payload file may state is the rate a trigger goes out
    /// at, which is tcspecial's doing and so flight behaviour. Everything
    /// else about timing and segmentation is the simulator's, and a line of
    /// it that crept into the payload file would be read by nothing: the
    /// loader refuses an interval on a periodic payload, but a segment size
    /// there is an unknown attribute, and this says plainly which file the
    /// settings belong in.
    #[test]
    fn a_shipped_payload_file_states_only_a_triggered_payload_s_interval() {
        for (payload_path, sim_path) in shipped_sets() {
            let text = std::fs::read_to_string(&payload_path).unwrap();
            // Whether the payload whose attributes are being read is one that
            // answers requests. Each payload starts with its dh_id.
            let mut triggered = false;

            for (number, line) in text.lines().enumerate() {
                let line = line.trim().trim_start_matches("- ");
                if line.starts_with('#') {
                    continue;
                }
                if line.starts_with("dh_id") {
                    triggered = false;
                }
                if line.starts_with("mode") && line.contains("triggered") {
                    triggered = true;
                }

                for setting in ["segment_interval_ms", "segment_size"] {
                    assert!(
                        !line.starts_with(setting),
                        "{}:{} states {}, which belongs in {}",
                        payload_path.display(),
                        number + 1,
                        setting,
                        sim_path.display()
                    );
                }

                assert!(
                    !line.starts_with("packet_interval_ms") || triggered,
                    "{}:{} states packet_interval_ms for a payload that sends on \
                     its own, whose rate belongs in {}",
                    payload_path.display(),
                    number + 1,
                    sim_path.display()
                );
            }
        }
    }

    /// Every payload of every shipped set gets a panel, and they fill a grid
    /// with room for all of them.
    #[test]
    fn the_shipped_files_give_a_panel_each() {
        for (payload_path, sim_path) in shipped_sets() {
            let dh_configs = load_dh_configs(&payload_path).unwrap();
            let sim_file = SimConfigFile::load(&sim_path).unwrap();
            let sims = sim_file.resolve(&dh_configs).unwrap();

            let panels: Vec<PayloadInfo> = dh_configs
                .iter()
                .zip(&sims)
                .map(|(dh, s)| payload_info_from(dh, s))
                .collect();

            assert_eq!(
                panels.len(),
                dh_configs.len(),
                "{}",
                payload_path.display()
            );

            let shape = grid::grid_shape(panels.len());
            assert!(
                shape.columns * shape.rows >= panels.len(),
                "{} does not fit its grid",
                payload_path.display()
            );
        }
    }

    /// The panel on screen shows the time and bytes Rust put in the model.
    ///
    /// The rest of the chain is checked a piece at a time -- the payload
    /// records a sample, the refresh puts it in the row -- and none of that
    /// shows anything if the window does not read the field it was put in.
    /// A panel declares a property per value and the grid passes each one
    /// across by name, so a field the grid forgets is a panel that shows its
    /// own default for ever, with every test of the Rust side still passing.
    /// This reads the text the window actually laid out.
    ///
    /// Not a test of its own: the testing backend is started once per test
    /// binary and its windows belong to the thread that made them, so the
    /// checks that need a window are all reached from the one test below.
    fn the_panel_shows_the_time_and_bytes_it_was_given() {
        use i_slint_backend_testing::ElementHandle;

        /// Every line of a panel that begins with `start`, as laid out.
        fn lines_from(ui: &MainWindow, start: &str) -> Vec<String> {
            ElementHandle::find_by_element_type_name(ui, "Text")
                .filter_map(|e| e.accessible_label())
                .map(|label| label.to_string())
                .filter(|label| label.starts_with(start))
                .collect()
        }

        let shown = |info: PayloadInfo| {
            let ui = MainWindow::new().unwrap();
            ui.set_payload_model(ModelRc::from(Rc::new(VecModel::from(vec![info]))));
            ui.set_columns(1);
            ui.show().unwrap();
            (lines_from(&ui, "Sent: "), lines_from(&ui, "Recv: "))
        };

        // A payload that sends on its own and has moved nothing: it says so
        // in the direction it can move in, and has no received line at all --
        // nothing is ever sent to such a payload, so a line for it would read
        // the same from the first packet to the last.
        let quiet = payload_info_from(&handler("DH0"), &sim(1000, 1000, 12));
        let (sent, recv) = shown(quiet.clone());
        assert_eq!(sent, vec![format!("Sent: {NO_TRANSFER_TIME} ")]);
        assert!(
            recv.is_empty(),
            "a payload that sends on its own has a received line: {recv:?}"
        );

        // One that answers requests has both, and says the same thing in
        // each before anything has moved.
        let mut asked = sim(1000, 1000, 12);
        asked.triggered = true;
        asked.packet_interval_ms = 0;
        asked.segment_interval_ms = 0;
        let (sent, recv) = shown(payload_info_from(&handler("DH0"), &asked));
        assert_eq!(sent, vec![format!("Sent: {NO_TRANSFER_TIME} ")]);
        assert_eq!(recv, vec![format!("Recv: {NO_TRANSFER_TIME} ")]);

        // And one that has sent and been spoken to shows when each happened
        // and the head of what moved.
        let mut moving = payload_info_from(&handler("DH0"), &asked);
        show_traffic(
            &mut moving,
            &PayloadStats {
                packets_sent: 3,
                packets_recv: 1,
                last_sent: sample_of(3661, &[0xDE, 0xAD]),
                last_recv: sample_of(7322, b"ASK"),
                ..PayloadStats::default()
            },
        );
        let (sent, recv) = shown(moving);
        assert_eq!(
            sent,
            vec!["Sent: 01:01:01 DE AD".to_string()],
            "the panel does not show the time and bytes of the last packet sent"
        );
        assert_eq!(
            recv,
            vec!["Recv: 02:02:02 41 53 4B".to_string()],
            "the panel does not show the time and bytes of the last packet received"
        );
    }

    /// Pressing a panel's Configuration button shows what both files said about
    /// that payload.
    ///
    /// Everything up to the window is checked elsewhere -- the text itself by
    /// `tcslibgs::payload_parameters`, and that a panel carries it by the
    /// model -- and none of that puts anything on screen if the button is not
    /// wired to the popup or the popup not to the text. This presses the
    /// button and reads what the window then laid out.
    ///
    /// Not a test of its own, for the reason given above: one test per binary
    /// may start the backend.
    fn the_params_button_shows_what_both_files_said() {
        use i_slint_backend_testing::ElementHandle;

        let dh = handler("DH0");
        let sim = sim(1000, 1000, 12);
        let ui = MainWindow::new().unwrap();
        ui.set_payload_model(ModelRc::from(Rc::new(VecModel::from(vec![
            payload_info_from(&dh, &sim),
        ]))));
        ui.set_columns(1);
        ui.show().unwrap();

        // Every line of text the window has laid out.
        let on_screen = |ui: &MainWindow| -> Vec<String> {
            ElementHandle::find_by_element_type_name(ui, "Text")
                .filter_map(|e| e.accessible_label())
                .map(|label| label.to_string())
                .collect()
        };

        // Nothing of the parameters is on screen until the button is pressed:
        // a panel is a dozen lines tall and the parameters are thirty.
        assert!(
            !on_screen(&ui).iter().any(|line| line.contains("dh_id")),
            "the parameters are shown before anyone asked for them"
        );

        let button: Vec<_> = ElementHandle::find_by_accessible_label(&ui, "Configuration").collect();
        assert_eq!(
            button.len(),
            1,
            "the panel has no Configuration button to press"
        );
        button[0].invoke_accessible_default_action();

        let shown = on_screen(&ui);
        assert!(
            shown.iter().any(|line| line == "DH0 parameters"),
            "the popup does not say which payload it is describing: {shown:?}"
        );

        // And what it shows is what the two files said, which is the one
        // description both programs use.
        let wanted = payload_parameters(&dh, Some(&sim));
        assert!(
            shown.contains(&wanted),
            "the popup shows something other than the parameters of this \
             payload:\n{shown:?}"
        );
    }

    /// A panel shows the data handler's packet size and the simulator file's
    /// settings, so an edit starts from what the files said.
    #[test]
    fn a_panel_shows_both_files_values() {
        let dh = DHConfig {
            dh_id: DHId(3),
            name: DHName::new("DH3"),
            endpoint: EndpointConfig::Network(NetworkConfig {
                protocol: NetworkProtocol::Udp,
                address: "localhost".to_string(),
                port: 5003,
            }),
            packet_size: 15,
     oc: None,
      mode: Default::default(),
 };

        let info = payload_info_from(&dh, &sim(500, 250, 5));
        assert_eq!(info.name, SharedString::from("DH3"));
        assert_eq!(info.config, SharedString::from("UDP localhost:5003"));
        // From the payload file
        assert_eq!(info.packet_size, 15);
        // From the simulator file
        assert_eq!(info.packet_interval, 500);
        assert_eq!(info.segment_interval, 250);
        assert_eq!(info.segment_size, 5);
    }
}
