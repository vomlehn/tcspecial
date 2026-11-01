//! The temperatures window: each sensor's temperature, and whether the
//! heaters are on, over the last ten seconds, in one plot.
//!
//! What is plotted is what the configuration says reaches the ground. A
//! sensor is a `to_kelvin` compute node -- the conversion of one part's
//! counts, which is one sensor's temperature, a vote or a program being made
//! out of several -- and a heater switch is a node that sends decisions, a
//! `hysteresis`, whose decision is what its lines are driven to. Both are
//! read by the rules the vehicle writes them by,
//! `tcslibgs::node::vehicle::compute::from_value` and `from_decision`, and
//! only from the nodes the configuration says send them, never guessed from a
//! datum's width.
//!
//! The plot is ten seconds wide and sixty kelvin tall, centred on the
//! thermostat's setpoint where the configuration has one -- the temperature
//! everything here is being held to -- and on the first reading otherwise.
//! It fills from the left, and once ten seconds have been drawn it scrolls,
//! the newest at the right edge. A thermostat tells the ground only when its
//! decision changes, so a switch's state is drawn from the change that made
//! it until the next, and before the first change heard it is not drawn at
//! all: a state nobody reported is not shown as off.
//!
//! The colours are Okabe and Ito's, chosen to be told apart by readers with
//! each common kind of colour blindness, and never alone: every trace is
//! named, with its latest reading, in the legend beside its colour, and every
//! switch is named on its strip, with its state in words.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use slint::{
    Color, ComponentHandle, Model, ModelRc, SharedString, Timer, TimerMode,
    VecModel,
};
use tcslibgs::graph::Graph;
use tcslibgs::node::vehicle::compute::{
    from_decision, from_value, params_of, Computation,
};

use crate::{MainWindow, SpanShown, SwitchShown, ThermalWindow, TraceShown};

/// How much time the plot spans, left edge to right.
pub const SPAN: Duration = Duration::from_secs(10);

/// How far above and below its centre the plot reaches, in kelvin.
pub const HALF_RANGE: f64 = 30.0;

/// How long a sensor may be silent before its trace is broken rather than
/// joined across the silence: a straight line over a gap is a reading
/// nobody took. Longer than the slowest example's second between readings.
pub const GAP: Duration = Duration::from_millis(2500);

/// The most readings kept of one sensor: ten seconds of one every ten
/// milliseconds. Fixed when the plot is made, so that taking a reading
/// allocates nothing; a faster sensor keeps its newest readings, and the
/// trace starts short of the left edge.
const READINGS: usize = 1000;

/// The most changes kept of one switch. A thermostat's band exists so that
/// it changes rarely; more than this in ten seconds and the oldest go.
const CHANGES: usize = 64;

/// The traces' colours, in the order sensors are listed: Okabe and Ito's
/// blue, bluish green, reddish purple, sky blue and black. Their orange and
/// vermillion are kept for the heaters, so a trace is never the colour of a
/// heater, and their yellow is left out, being too pale to read on white.
pub const TRACE_COLOURS: [u32; 5] =
    [0x0072B2, 0x009E73, 0xCC79A7, 0x56B4E9, 0x000000];

/// One sensor's readings, oldest first, timed as they arrived.
#[derive(Debug)]
struct Sensor {
    name: String,
    readings: VecDeque<(Instant, f64)>,
}

/// One switch's changes of state, oldest first, timed as they arrived.
#[derive(Debug)]
struct Switch {
    name: String,
    /// What it drives, by name, for its label.
    drives: Vec<String>,
    changes: VecDeque<(Instant, bool)>,
}

/// What a source is to the plot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plotted {
    /// A sensor's temperature: the place in [`Plot::sensors`].
    Sensor(usize),
    /// A switch's decision: the place in [`Plot::switches`].
    Switch(usize),
}

/// Everything the plot shows, kept by the threads that receive it and drawn
/// by the window's timer.
#[derive(Debug)]
pub struct Plot {
    sensors: Vec<Sensor>,
    switches: Vec<Switch>,
    /// The temperature at the middle of the plot, once there is one.
    centre: Option<f64>,
    /// When the first reading arrived: the left edge, until the plot has
    /// filled.
    started: Option<Instant>,
}

impl Plot {
    /// Makes the plot of a configuration: a trace for each sensor that
    /// reaches the ground and a strip for each switch that does.
    ///
    /// # Arguments
    ///
    /// * `graph`   - The configuration.
    /// * `reaches` - Whether a node's data reaches the ground.
    ///
    /// # Returns
    ///
    /// The plot, or `None` where nothing that reaches the ground is a
    /// sensor, and there is nothing to plot.
    pub fn of(graph: &Graph, reaches: impl Fn(&str) -> bool) -> Option<Plot> {
        let mut sensors = Vec::new();
        let mut switches = Vec::new();
        let mut setpoint = None;
        for node in &graph.nodes {
            if node.node_type != "compute" || !reaches(&node.name) {
                continue;
            }
            // A computation the checking refuses is refused before the
            // window opens; one that is not settled here is not plotted.
            match params_of(&node.name, &node.attributes) {
                Ok(Computation::ToKelvin(_)) => sensors.push(Sensor {
                    name: node.name.clone(),
                    readings: VecDeque::with_capacity(READINGS),
                }),
                Ok(Computation::Hysteresis(params)) => {
                    setpoint.get_or_insert(params.setpoint);
                    switches.push(Switch {
                        name: node.name.clone(),
                        drives: drives(graph, &node.name),
                        changes: VecDeque::with_capacity(CHANGES),
                    });
                }
                _ => {}
            }
        }
        if sensors.is_empty() {
            return None;
        }
        Some(Plot {
            sensors,
            switches,
            centre: setpoint,
            started: None,
        })
    }

    /// Says what a source is to the plot.
    ///
    /// # Arguments
    ///
    /// * `source` - The node's name.
    ///
    /// # Returns
    ///
    /// Its trace or its strip, or `None` where it has neither.
    pub fn plotted(&self, source: &str) -> Option<Plotted> {
        if let Some(i) = self.sensors.iter().position(|s| s.name == source) {
            return Some(Plotted::Sensor(i));
        }
        self.switches
            .iter()
            .position(|s| s.name == source)
            .map(Plotted::Switch)
    }

    /// Keeps a sensor's reading. Allocates nothing: the oldest reading
    /// makes room where there is none.
    ///
    /// # Arguments
    ///
    /// * `sensor` - Its place in the plot's sensors.
    /// * `at`     - When it arrived.
    /// * `kelvin` - The temperature.
    pub fn reading(&mut self, sensor: usize, at: Instant, kelvin: f64) {
        let Some(sensor) = self.sensors.get_mut(sensor) else {
            return;
        };
        if sensor.readings.len() == READINGS {
            sensor.readings.pop_front();
        }
        sensor.readings.push_back((at, kelvin));
        self.started.get_or_insert(at);
        self.centre.get_or_insert(kelvin.round());
    }

    /// Keeps a switch's decision, where it is a change. Allocates nothing.
    ///
    /// # Arguments
    ///
    /// * `switch` - Its place in the plot's switches.
    /// * `at`     - When it arrived.
    /// * `on`     - The decision.
    pub fn decision(&mut self, switch: usize, at: Instant, on: bool) {
        let Some(switch) = self.switches.get_mut(switch) else {
            return;
        };
        if switch.changes.back().is_some_and(|&(_, was)| was == on) {
            return;
        }
        if switch.changes.len() == CHANGES {
            switch.changes.pop_front();
        }
        switch.changes.push_back((at, on));
    }

    /// How many sensors and switches there are, which is how many traces and
    /// strips are drawn.
    pub fn counts(&self) -> (usize, usize) {
        (self.sensors.len(), self.switches.len())
    }

    /// The temperature at the middle of the plot, once there is one.
    pub fn centre(&self) -> Option<f64> {
        self.centre
    }

    /// The time at the plot's left edge: the first reading until the plot
    /// has filled, and ten seconds ago after.
    ///
    /// # Arguments
    ///
    /// * `now` - The time at the right edge, once the plot is full.
    pub fn left(&self, now: Instant) -> Option<Instant> {
        let started = self.started?;
        Some(match now.checked_sub(SPAN) {
            Some(back) if back > started => back,
            _ => started,
        })
    }

    /// Draws the plot into `drawn`, whose strings and lists are reused from
    /// one drawing to the next.
    ///
    /// # Arguments
    ///
    /// * `now`    - The time to draw it at.
    /// * `width`  - The plot's width, in pixels.
    /// * `height` - The plot's height, in pixels.
    /// * `drawn`  - Where it is drawn.
    pub fn draw(
        &self,
        now: Instant,
        width: f32,
        height: f32,
        drawn: &mut Drawn,
    ) {
        drawn.clear(self.sensors.len());
        let (Some(left), Some(centre)) = (self.left(now), self.centre) else {
            for (i, sensor) in self.sensors.iter().enumerate() {
                drawn.latest[i].push_str(&sensor.name);
            }
            for switch in &self.switches {
                drawn.switches.push(SwitchDrawn::unheard(switch));
            }
            return;
        };
        // Signed, so that a reading from before the left edge is drawn to
        // the left of it, and the line from it comes in across the edge; the
        // window clips what falls outside.
        let x_of = |at: Instant| -> f32 {
            let since = if at >= left {
                at.duration_since(left).as_secs_f32()
            } else {
                -left.duration_since(at).as_secs_f32()
            };
            since / SPAN.as_secs_f32() * width
        };
        let y_of = |kelvin: f64| -> f32 {
            let above = ((kelvin - centre) / HALF_RANGE) as f32;
            height / 2.0 - above * height / 2.0
        };

        for (i, sensor) in self.sensors.iter().enumerate() {
            let path = &mut drawn.paths[i];
            // From the last reading before the left edge, the point the
            // trace comes in from, so that it runs to the edge rather than
            // starting short of it; those before that are off the plot.
            let first = sensor.readings.partition_point(|&(at, _)| at < left);
            let mut last: Option<Instant> = None;
            for &(at, kelvin) in
                sensor.readings.range(first.saturating_sub(1)..)
            {
                let joined =
                    last.is_some_and(|was| at.duration_since(was) <= GAP);
                let verb = if joined { 'L' } else { 'M' };
                let _ =
                    write!(path, "{verb} {:.1} {:.1} ", x_of(at), y_of(kelvin));
                last = Some(at);
            }
            let latest = &mut drawn.latest[i];
            latest.push_str(&sensor.name);
            if let Some(&(_, kelvin)) = sensor.readings.back() {
                let _ = write!(latest, " {kelvin:.2} K");
            }
        }

        for switch in &self.switches {
            let mut shown = SwitchDrawn::unheard(switch);
            // The state at the left edge is the last change before it.
            let mut state: Option<(f32, bool)> = None;
            for &(at, on) in &switch.changes {
                let x = if at < left { 0.0 } else { x_of(at) };
                if let Some((from, was)) = state {
                    if x > from {
                        shown.spans.push(Span {
                            x: from,
                            width: x - from,
                            on: was,
                        });
                    }
                }
                state = Some((x, on));
                shown.on = Some(on);
            }
            if let Some((from, was)) = state {
                let to = x_of(now).min(width);
                if to > from {
                    shown.spans.push(Span {
                        x: from,
                        width: to - from,
                        on: was,
                    });
                }
            }
            drawn.switches.push(shown);
        }
    }
}

/// What a switch drives: the nodes its links lead to.
///
/// # Arguments
///
/// * `graph`  - The configuration.
/// * `switch` - The switch's name.
///
/// # Returns
///
/// The names of the nodes its data goes to, but the ground.
fn drives(graph: &Graph, switch: &str) -> Vec<String> {
    let mut driven = Vec::new();
    for link in &graph.links {
        if link.origin().node != switch {
            continue;
        }
        for to in link.to.each() {
            let node = tcslibgs::graph::Destination::parse(to).node;
            let is_payload = graph.node(&node).is_some_and(|n| {
                !tcslibgs::gateway::is_gateway(&n.node_type)
                    && n.node_type != "log"
                    && n.node_type != "compute"
            });
            if is_payload && !driven.contains(&node) {
                driven.push(node);
            }
        }
    }
    driven
}

/// A stretch of time a switch was in one state, in pixels across the plot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub x: f32,
    pub width: f32,
    pub on: bool,
}

/// One switch, drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct SwitchDrawn {
    /// Its name, and what it drives.
    pub label: String,
    /// Its state as last heard, or `None` before any.
    pub on: Option<bool>,
    pub spans: Vec<Span>,
}

impl SwitchDrawn {
    fn unheard(switch: &Switch) -> SwitchDrawn {
        let mut label = switch.name.clone();
        if !switch.drives.is_empty() {
            label.push_str(" \u{2192} ");
            label.push_str(&switch.drives.join(", "));
        }
        SwitchDrawn {
            label,
            on: None,
            spans: Vec::new(),
        }
    }
}

/// The plot, drawn: a path for each sensor, as SVG path commands in pixels,
/// what each last read, and each switch's spans.
#[derive(Debug, Default)]
pub struct Drawn {
    pub paths: Vec<String>,
    pub latest: Vec<String>,
    pub switches: Vec<SwitchDrawn>,
}

impl Drawn {
    /// Empties it for the next drawing, keeping what its strings hold room
    /// for.
    fn clear(&mut self, sensors: usize) {
        self.paths.resize_with(sensors, String::new);
        self.latest.resize_with(sensors, String::new);
        for path in &mut self.paths {
            path.clear();
        }
        for latest in &mut self.latest {
            latest.clear();
        }
        self.switches.clear();
    }
}

/// How often the window is redrawn: often enough that the scrolling reads
/// as movement rather than as steps.
pub const REDRAW: Duration = Duration::from_millis(100);

/// Hands what arrived from a plotted source to the plot: a sensor's
/// temperature, or a switch's decision. What cannot be read as one is not
/// plotted; the source's panel shows it as it came.
///
/// # Arguments
///
/// * `plot`    - The plot.
/// * `plotted` - What the source is to it.
/// * `data`    - What arrived.
/// * `at`      - When.
pub fn take(plot: &Mutex<Plot>, plotted: Plotted, data: &[u8], at: Instant) {
    // Each reader allocates only to say what is wrong with what arrived,
    // which is never what a working node sends; on the way that does, they
    // allocate nothing.
    match plotted {
        Plotted::Sensor(i) => {
            if let (Ok(kelvin), Ok(mut plot)) =
                (from_value(data, ""), plot.lock())
            {
                plot.reading(i, at, kelvin);
            }
        }
        Plotted::Switch(i) => {
            if let (Ok(on), Ok(mut plot)) =
                (from_decision(data, ""), plot.lock())
            {
                plot.decision(i, at, on);
            }
        }
    }
}

/// Sets the temperatures window up and opens it: the Temperatures button
/// that opens it again once closed, and the timer that redraws it.
///
/// Opened from the start, because a configuration with a sensor reaching the
/// ground is one whose temperatures are the reason to be watching it.
///
/// # Arguments
///
/// * `ui`     - The main window, whose button opens it.
/// * `window` - The temperatures window.
/// * `plot`   - What it plots.
///
/// # Returns
///
/// The timer, which redraws the window for as long as it is kept.
pub fn show(
    ui: &MainWindow,
    window: &ThermalWindow,
    plot: Arc<Mutex<Plot>>,
) -> Timer {
    ui.set_can_plot(true);
    {
        let window = window.as_weak();
        ui.on_temperatures_clicked(move || {
            if let Some(window) = window.upgrade() {
                let _ = window.show();
            }
        });
    }

    let (sensors, switches) = plot.lock().map(|p| p.counts()).unwrap_or((0, 0));
    let traces: Rc<VecModel<TraceShown>> = Rc::new(VecModel::from(
        (0..sensors)
            .map(|i| TraceShown {
                said: SharedString::new(),
                commands: SharedString::new(),
                colour: colour(TRACE_COLOURS[i % TRACE_COLOURS.len()]),
            })
            .collect::<Vec<_>>(),
    ));
    let spans: Vec<Rc<VecModel<SpanShown>>> = (0..switches)
        .map(|_| Rc::new(VecModel::default()))
        .collect();
    let shown: Rc<VecModel<SwitchShown>> = Rc::new(VecModel::from(
        spans
            .iter()
            .map(|spans| SwitchShown {
                label: SharedString::new(),
                state: SharedString::new(),
                spans: ModelRc::from(spans.clone()),
            })
            .collect::<Vec<_>>(),
    ));
    window.set_traces(ModelRc::from(traces.clone()));
    window.set_switches(ModelRc::from(shown.clone()));
    let _ = window.show();

    let timer = Timer::default();
    let window = window.as_weak();
    let mut drawn = Drawn::default();
    timer.start(TimerMode::Repeated, REDRAW, move || {
        let Some(window) = window.upgrade() else {
            return;
        };
        let width = window.get_plot_width();
        let height = window.get_plot_height();
        let centre = {
            let Ok(plot) = plot.lock() else {
                return;
            };
            plot.draw(Instant::now(), width, height, &mut drawn);
            plot.centre()
        };
        if let Some(centre) = centre {
            window.set_top_said(kelvin_said(centre + HALF_RANGE));
            window.set_centre_said(kelvin_said(centre));
            window.set_bottom_said(kelvin_said(centre - HALF_RANGE));
        }

        // Slint takes what it draws as its own strings and models, so each
        // redraw makes them: the one place in this window that allocates as
        // it runs, and unavoidably, a window being drawn by Slint. What is
        // worked out to draw -- the paths and spans -- is in `drawn`, which
        // is reused.
        for (i, (commands, said)) in
            drawn.paths.iter().zip(&drawn.latest).enumerate()
        {
            if let Some(mut trace) = traces.row_data(i) {
                trace.commands = SharedString::from(commands.as_str());
                trace.said = SharedString::from(said.as_str());
                traces.set_row_data(i, trace);
            }
        }
        for (i, switch) in drawn.switches.iter().enumerate() {
            let Some(mut row) = shown.row_data(i) else {
                continue;
            };
            row.label = SharedString::from(switch.label.as_str());
            row.state = SharedString::from(match switch.on {
                Some(true) => "on",
                Some(false) => "off",
                None => "not heard",
            });
            shown.set_row_data(i, row);
            spans[i].set_vec(
                switch
                    .spans
                    .iter()
                    .map(|span| SpanShown {
                        x: span.x,
                        width: span.width,
                        on: span.on,
                    })
                    .collect::<Vec<_>>(),
            );
        }
    });
    timer
}

/// A temperature as the plot's axis says it.
fn kelvin_said(kelvin: f64) -> SharedString {
    SharedString::from(format!("{kelvin:.2} K"))
}

/// One of [`TRACE_COLOURS`] as Slint takes it.
fn colour(rgb: u32) -> Color {
    Color::from_rgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The thermostat example, as shipped.
    fn thermostat() -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Graph::load(root.join("examples/thermostat/thermostat.ytcs"))
            .expect("the example reads")
    }

    /// The thermostat example's three sensors are traced, its thermostat is
    /// a switch driving both heaters, the vote is neither, and the plot is
    /// centred on the setpoint.
    #[test]
    fn the_thermostat_example_plots_three_sensors_and_a_switch() {
        let plot = Plot::of(&thermostat(), |_| true).expect("a plot");
        assert_eq!(plot.counts(), (3, 1));
        assert_eq!(plot.plotted("k_2"), Some(Plotted::Sensor(1)));
        assert_eq!(plot.plotted("thermostat"), Some(Plotted::Switch(0)));
        assert_eq!(plot.plotted("cabin_temp"), None);
        assert_eq!(plot.plotted("temp_1"), None);
        assert_eq!(plot.centre(), Some(293.15));

        let mut drawn = Drawn::default();
        plot.draw(Instant::now(), 100.0, 60.0, &mut drawn);
        assert_eq!(
            drawn.switches[0].label,
            "thermostat \u{2192} heaters_1, heaters_2"
        );
        assert_eq!(drawn.switches[0].on, None, "nothing heard yet");
        assert!(drawn.paths.iter().all(String::is_empty));
    }

    /// A configuration whose sensors do not reach the ground has nothing to
    /// plot.
    #[test]
    fn nothing_reaching_the_ground_is_no_plot() {
        assert!(Plot::of(&thermostat(), |_| false).is_none());
    }

    /// The plot fills from the left edge, and once ten seconds have been
    /// drawn the newest is at the right edge and the oldest has scrolled
    /// off; a reading at the setpoint is at the middle, and one 30 K above
    /// it at the top.
    #[test]
    fn it_fills_from_the_left_and_then_scrolls() {
        let mut plot = Plot::of(&thermostat(), |_| true).expect("a plot");
        let t0 = Instant::now();
        plot.reading(0, t0, 293.15);
        plot.reading(0, t0 + Duration::from_secs(2), 323.15);
        let mut drawn = Drawn::default();

        plot.draw(t0 + Duration::from_secs(2), 1000.0, 600.0, &mut drawn);
        assert_eq!(drawn.paths[0], "M 0.0 300.0 L 200.0 0.0 ");
        assert_eq!(drawn.latest[0], "k_1 323.15 K");

        plot.reading(0, t0 + Duration::from_secs(12), 263.15);
        plot.draw(t0 + Duration::from_secs(12), 1000.0, 600.0, &mut drawn);
        // The left edge is at two seconds: the reading there is at the
        // edge, the one before it comes in from off the plot, and the ten
        // seconds to the newest, at the right edge, are a gap.
        assert_eq!(drawn.paths[0], "M -200.0 300.0 L 0.0 0.0 M 1000.0 600.0 ");
    }

    /// Readings further apart than GAP are not joined.
    #[test]
    fn a_silence_breaks_the_trace() {
        let mut plot = Plot::of(&thermostat(), |_| true).expect("a plot");
        let t0 = Instant::now();
        plot.reading(1, t0, 293.15);
        plot.reading(1, t0 + Duration::from_secs(1), 293.15);
        plot.reading(1, t0 + Duration::from_secs(4), 293.15);
        let mut drawn = Drawn::default();
        plot.draw(t0 + Duration::from_secs(4), 100.0, 60.0, &mut drawn);
        assert_eq!(drawn.paths[1], "M 0.0 30.0 L 10.0 30.0 M 40.0 30.0 ");
    }

    /// A switch is drawn from each change to the next, in its state, and to
    /// now from the last; a decision that repeats the last changes nothing.
    #[test]
    fn a_switch_is_drawn_from_change_to_change() {
        let mut plot = Plot::of(&thermostat(), |_| true).expect("a plot");
        let t0 = Instant::now();
        plot.reading(0, t0, 290.0);
        plot.decision(0, t0 + Duration::from_secs(1), true);
        plot.decision(0, t0 + Duration::from_secs(2), true);
        plot.decision(0, t0 + Duration::from_secs(4), false);
        let mut drawn = Drawn::default();
        plot.draw(t0 + Duration::from_secs(6), 100.0, 60.0, &mut drawn);
        let switch = &drawn.switches[0];
        assert_eq!(switch.on, Some(false));
        // To the hundredth of a pixel: an Instant's arithmetic is not exact
        // in an f32.
        let spans: Vec<(i32, i32, bool)> = switch
            .spans
            .iter()
            .map(|s| {
                (
                    (s.x * 100.0).round() as i32,
                    (s.width * 100.0).round() as i32,
                    s.on,
                )
            })
            .collect();
        assert_eq!(spans, vec![(1000, 3000, true), (4000, 2000, false)]);
    }
}
