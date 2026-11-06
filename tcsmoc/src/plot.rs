//! The plot window: every value that reaches the ground, over time, a panel
//! for each unit, and every switch's decisions under them.
//!
//! What is plotted is what the configuration says reaches the ground. A value
//! is anything `tcslibgs::graph::units::unit_of` gives a unit -- a
//! conversion, a vote, a program's answer -- read by the rule the vehicle
//! writes it by, `tcslibgs::node::vehicle::compute::from_value`, in that
//! unit. A switch is a node that sends decisions, a `thermostat`, read by
//! `from_decision`; or one line of a GPIO payload that watches lines, read
//! from the records its node sends -- a line sensing a heater is the
//! heater's real state, where a thermostat's decision is what it was told.
//! Nothing is guessed from a datum's width.
//!
//! Values in one unit share a panel, so that two temperatures are read off
//! one axis and a voltage is never read off a current's. Each panel is
//! scaled to what it shows: the lowest and highest reading in the span, and
//! a thermostat's setpoint where one reads values in that unit, rounded out
//! to a tick every one, two or five of a power of ten.
//!
//! Behind a panel read by a thermostat is that thermostat's [`Band`]: its
//! setpoint S and half band H mark out where the reading is held, S - H to
//! S + H, left white; a pale yellow H wide either side of that, where the
//! reading is off but near; and a pale red beyond, to the panel's edges.
//! Where several thermostats read one unit -- staged heating, a first stage
//! and a second with a lower setpoint -- the band drawn is that of the one
//! with the highest setpoint, the first stage, which is the one that states
//! the comfort band; the others' setpoints are drawn as rules, as every
//! setpoint is. The panel's scale takes in S - 2H to S + 2H, so that the
//! yellow is always seen, and the red wherever the readings or the scale's
//! rounding reach past it.
//!
//! The span is chosen in the window: the last ten seconds, minute or ten
//! minutes, which fill from the left and then scroll, the newest at the right
//! edge; or the whole run, the first reading at the left edge and the newest
//! at the right. Each value keeps its last [`RECENT`] readings for the first
//! three, and for the whole run a [`History`] of fixed size, which thins
//! itself as it fills. Neither allocates once the plot is made, so the
//! threads that receive the data allocate nothing to keep it. Where many
//! readings fall in one column of pixels, their lowest and highest are drawn,
//! so that thinning to the screen never hides a spike.
//!
//! A thermostat tells the ground only when its decision changes, so a
//! switch's state is drawn from the change that made it until the next, and
//! before the first change heard it is not drawn at all: a state nobody
//! reported is not shown as off.
//!
//! The colours are Okabe and Ito's, chosen to be told apart by readers with
//! each common kind of colour blindness, and never alone: every trace is
//! named, with its latest reading, in its panel's legend beside its colour,
//! and every switch is named on its strip, with its state in words. A band's
//! tints are their yellow and vermillion paled toward white, which pale
//! yellow and pale red readers with red-green colour blindness can confuse,
//! so each region is said in words on the panel -- "high", "too high",
//! "low", "too low" -- each edge is a line, and the legend names the band's
//! edges and its thermostat. A switch on is green, the colour people read
//! as on, and never by its shade alone: a switch is drawn only on its own
//! strip under the panels, in full bluish green with "on" written in it, and
//! nothing of a switch is drawn on a panel.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use slint::{
    Color, ComponentHandle, Model, ModelRc, SharedString, Timer, TimerMode,
    VecModel,
};
use tcslibgs::graph::units::{input_unit, unit_of};
use tcslibgs::graph::Graph;
use tcslibgs::node::payload::gpio::EVENT_BYTES;
use tcslibgs::node::vehicle::compute::{
    from_decision, from_value, params_of, Computation,
};
use tcslibgs::unit::Unit;

use crate::{
    MainWindow, PanelShown, PlotWindow, SpanShown, SwitchShown, TickShown,
    TraceShown,
};

/// How much time the plot spans, left edge to right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    TenSeconds,
    Minute,
    TenMinutes,
    /// From the first reading to now.
    Whole,
}

impl Span {
    /// Every span, in the order the window's buttons offer them.
    pub const ALL: [Span; 4] = [
        Span::TenSeconds,
        Span::Minute,
        Span::TenMinutes,
        Span::Whole,
    ];

    /// How long it is.
    ///
    /// # Returns
    ///
    /// Its length, or `None` for the whole run, whose length is however
    /// long the run has been.
    pub fn length(self) -> Option<Duration> {
        match self {
            Span::TenSeconds => Some(Duration::from_secs(10)),
            Span::Minute => Some(Duration::from_secs(60)),
            Span::TenMinutes => Some(Duration::from_secs(600)),
            Span::Whole => None,
        }
    }

    /// What the window says of it, over the plot.
    ///
    /// # Returns
    ///
    /// The words.
    fn said(self) -> &'static str {
        match self {
            Span::TenSeconds => "The last 10 s",
            Span::Minute => "The last minute",
            Span::TenMinutes => "The last 10 minutes",
            Span::Whole => "The whole run",
        }
    }
}

/// How long a value may be silent before its trace is broken rather than
/// joined across the silence: a straight line over a gap is a reading
/// nobody took. Longer than the slowest example's second between readings.
pub const GAP: Duration = Duration::from_millis(2500);

/// The most recent readings kept of one value: ten minutes of one every
/// tenth of a second, the descent example's radar. Fixed when the plot is
/// made, so that taking a reading allocates nothing; a faster value keeps
/// its newest readings, and its trace starts short of the left edge of the
/// longest spans.
const RECENT: usize = 6000;

/// The most readings one value's [`History`] keeps: about two for each
/// column of pixels across a wide window.
const HISTORY: usize = 4096;

/// The most changes kept of one switch: a thermostat's band exists so that
/// it changes rarely, but over ten minutes a tight one changes often. Past
/// this the oldest go, and the strip starts from the oldest kept.
const CHANGES: usize = 1024;

/// The traces' colours, in the order values are listed: Okabe and Ito's
/// blue, bluish green, reddish purple, sky blue and black; their orange and
/// yellow are left out, being too pale to read on white. A trace may share
/// its green with a switch on, because the two are never drawn in one place:
/// a trace is a line on a panel, and a switch a strip under the panels with
/// "on" written in it.
pub const TRACE_COLOURS: [u32; 5] =
    [0x0072B2, 0x009E73, 0xCC79A7, 0x56B4E9, 0x000000];

/// One reading kept for the whole run.
#[derive(Debug, Clone, Copy)]
struct Kept {
    at: Instant,
    value: f64,
    /// Whether the value was never silent for longer than [`GAP`] between
    /// the reading kept before this one and this one, so that the two are
    /// joined.
    joined: bool,
}

/// A value's readings over the whole run, in a fixed amount of room.
///
/// Every reading is kept until the room is full; then every other one is
/// let go, and from then on one reading in two is kept, then one in four,
/// and so on. The run is always covered from its first reading to its
/// latest, more coarsely the longer it runs, in at most [`HISTORY`]
/// readings.
#[derive(Debug)]
struct History {
    kept: Vec<Kept>,
    /// One reading in this many is kept.
    stride: u32,
    /// How many have arrived since the last one kept.
    since: u32,
    /// When the last reading arrived, kept or not.
    last: Option<Instant>,
    /// Whether the value has been silent for longer than [`GAP`] since the
    /// last reading kept.
    broken: bool,
}

impl History {
    /// An empty history, with its room.
    ///
    /// # Returns
    ///
    /// The history.
    fn new() -> History {
        History {
            kept: Vec::with_capacity(HISTORY),
            stride: 1,
            since: 0,
            last: None,
            broken: false,
        }
    }

    /// Takes a reading, keeping it if its turn has come. Allocates nothing:
    /// a full history is thinned in place.
    ///
    /// # Arguments
    ///
    /// * `at`    - When it arrived.
    /// * `value` - What it was.
    fn take(&mut self, at: Instant, value: f64) {
        if self.last.is_some_and(|was| at.duration_since(was) > GAP) {
            self.broken = true;
        }
        self.last = Some(at);
        self.since += 1;
        if !self.kept.is_empty() && self.since < self.stride {
            return;
        }
        if self.kept.len() == HISTORY {
            self.thin();
        }
        let joined = !self.kept.is_empty() && !self.broken;
        self.kept.push(Kept { at, value, joined });
        self.since = 0;
        self.broken = false;
    }

    /// Lets every other reading go, the first kept, and keeps half as many
    /// from now on. A reading is joined to the one now before it only where
    /// it was joined to the one let go between them, and that one to the
    /// one before.
    fn thin(&mut self) {
        let mut to = 0;
        for from in (0..self.kept.len()).step_by(2) {
            let mut kept = self.kept[from];
            if from > 0 {
                kept.joined &= self.kept[from - 1].joined;
            }
            self.kept[to] = kept;
            to += 1;
        }
        self.kept.truncate(to);
        self.stride *= 2;
    }
}

/// One value: its readings, oldest first, timed as they arrived.
#[derive(Debug)]
struct Trace {
    name: String,
    unit: Unit,
    recent: VecDeque<(Instant, f64)>,
    history: History,
}

/// The values in one unit, drawn on one axis.
#[derive(Debug)]
struct Panel {
    unit: Unit,
    /// Its traces, by their place in [`Plot::traces`].
    traces: Vec<usize>,
    /// The setpoints of the thermostats that read values in this unit, each
    /// drawn as a rule.
    setpoints: Vec<f64>,
    /// The band drawn behind it: see [`Band::of`].
    band: Option<Band>,
}

/// A thermostat's band: the setpoint it holds a reading at, and how far
/// either side of it it switches.
#[derive(Debug, Clone, PartialEq)]
pub struct Band {
    /// The thermostat's name, for the legend.
    pub name: String,
    pub setpoint: f64,
    /// Its `half_band`, more than nought: the checking refuses any other.
    pub half: f64,
}

impl Band {
    /// The band drawn behind a panel: that of the thermostat with the
    /// highest setpoint among those that read values in the panel's unit.
    /// With staged heating that is the first stage, whose band is the
    /// comfort band; a later stage, set lower, comes on only when the first
    /// cannot hold it. Of two with one setpoint, the first by name.
    ///
    /// # Arguments
    ///
    /// * `unit`        - The panel's unit.
    /// * `thermostats` - Every thermostat plotted, with the unit it reads,
    ///                   in order of name.
    ///
    /// # Returns
    ///
    /// The band, or `None` where no thermostat reads values in the unit.
    fn of(unit: Unit, thermostats: &[(Unit, Band)]) -> Option<Band> {
        let mut chosen: Option<&Band> = None;
        for (read, band) in thermostats {
            if *read == unit
                && chosen.is_none_or(|was| band.setpoint > was.setpoint)
            {
                chosen = Some(band);
            }
        }
        chosen.cloned()
    }

    /// Its edges, from the top down: S + 2H, where too high begins; S + H,
    /// where high begins; S - H, where low begins; and S - 2H, where too
    /// low begins.
    ///
    /// # Returns
    ///
    /// The four edges, in the panel's unit.
    pub fn edges(&self) -> [f64; 4] {
        let (s, h) = (self.setpoint, self.half);
        [s + 2.0 * h, s + h, s - h, s - 2.0 * h]
    }
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
    /// A value, in this unit: the place in [`Plot::traces`].
    Trace(usize, Unit),
    /// A switch's decision: the place in [`Plot::switches`].
    Switch(usize),
    /// A GPIO payload's lines: the place in [`Plot::line_sets`].
    Lines(usize),
}

/// A GPIO payload whose lines reach the ground, each line a switch: a line
/// sensing a heater is the heater's real state.
#[derive(Debug)]
struct LineSet {
    name: String,
    /// The place in [`Plot::switches`] of its first line's strip; the rest
    /// follow it, in its node's order.
    first: usize,
    /// Its lines' offsets on the chip, in its node's order, which is how a
    /// record says which line it is of.
    offsets: Vec<u32>,
}

/// Everything the plot shows, kept by the threads that receive it and drawn
/// by the window's timer.
#[derive(Debug)]
pub struct Plot {
    traces: Vec<Trace>,
    panels: Vec<Panel>,
    switches: Vec<Switch>,
    line_sets: Vec<LineSet>,
    /// When the first reading arrived: the left edge, until the span has
    /// filled, and always for the whole run.
    started: Option<Instant>,
    /// The span chosen in the window.
    pub span: Span,
}

impl Plot {
    /// Makes the plot of a configuration: a trace for each value that
    /// reaches the ground, in a panel for its unit, and a strip for each
    /// switch that does.
    ///
    /// # Arguments
    ///
    /// * `graph`   - The configuration.
    /// * `reaches` - Whether a node's data reaches the ground.
    ///
    /// # Returns
    ///
    /// The plot, or `None` where nothing that reaches the ground is a value
    /// or a switch, and there is nothing to plot.
    pub fn of(graph: &Graph, reaches: impl Fn(&str) -> bool) -> Option<Plot> {
        let mut traces = Vec::new();
        let mut panels: Vec<Panel> = Vec::new();
        let mut switches = Vec::new();
        let mut line_sets = Vec::new();
        let mut thermostats: Vec<(Unit, Band)> = Vec::new();
        // By name, the one order a configuration has: the panels, traces
        // and switches are drawn in it.
        let mut nodes: Vec<&tcslibgs::graph::Node> =
            graph.nodes.iter().collect();
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
        for node in nodes {
            // A thermostat's setpoint and band are drawn behind the values
            // it reads whether or not its decision reaches the ground, being
            // what those values are held to; its strip is drawn only where
            // the decision does.
            let reaching = reaches(&node.name);
            if !reaching && node.node_type != "compute" {
                continue;
            }
            if let Some(unit) = unit_of(graph, &node.name) {
                if !reaching {
                    continue;
                }
                let panel = match panels.iter().position(|p| p.unit == unit) {
                    Some(panel) => panel,
                    None => {
                        panels.push(Panel {
                            unit,
                            traces: Vec::new(),
                            setpoints: Vec::new(),
                            band: None,
                        });
                        panels.len() - 1
                    }
                };
                panels[panel].traces.push(traces.len());
                traces.push(Trace {
                    name: node.name.clone(),
                    unit,
                    recent: VecDeque::with_capacity(RECENT),
                    history: History::new(),
                });
                continue;
            }
            // A GPIO payload's watched lines, each a switch whose state is
            // the line's: what a line sensing a heater says the heater is
            // doing. And a gpio_inout's report, what a heater was told. A
            // node whose reach does not settle is refused before the window
            // opens.
            if let Ok(tcslibgs::graph::reach::Reach {
                endpoint: tcslibgs::EndpointConfig::Gpio(gpio),
                ..
            }) = tcslibgs::graph::reach::reach_of(node)
            {
                if !gpio.inputs.is_empty() {
                    line_sets.push(LineSet {
                        name: node.name.clone(),
                        first: switches.len(),
                        offsets: gpio.inputs.clone(),
                    });
                    for offset in &gpio.inputs {
                        switches.push(Switch {
                            name: format!("{} line {offset}", node.name),
                            drives: Vec::new(),
                            changes: VecDeque::with_capacity(CHANGES),
                        });
                    }
                } else if gpio.kind.reports() {
                    // A gpio_inout's report is what its lines were last
                    // told, a decision, drawn as a thermostat's is: a strip
                    // of its own, on and off.
                    switches.push(Switch {
                        name: node.name.clone(),
                        drives: Vec::new(),
                        changes: VecDeque::with_capacity(CHANGES),
                    });
                }
                continue;
            }
            if node.node_type != "compute" {
                continue;
            }
            // A computation the checking refuses is refused before the
            // window opens; one that is not settled here is not plotted.
            if let Ok(Computation::Thermostat(params)) =
                params_of(&node.name, &node.attributes)
            {
                if let Some(unit) = input_unit(graph, &node.name) {
                    thermostats.push((
                        unit,
                        Band {
                            name: node.name.clone(),
                            setpoint: params.setpoint,
                            half: params.half_band,
                        },
                    ));
                }
                if !reaching {
                    continue;
                }
                switches.push(Switch {
                    name: node.name.clone(),
                    drives: drives(graph, &node.name),
                    changes: VecDeque::with_capacity(CHANGES),
                });
            }
        }
        if traces.is_empty() && switches.is_empty() {
            return None;
        }
        // A setpoint is drawn where values in its unit are; where none
        // reaches the ground there is nothing for it to be read against.
        for (unit, band) in &thermostats {
            if let Some(panel) = panels.iter_mut().find(|p| p.unit == *unit) {
                panel.setpoints.push(band.setpoint);
            }
        }
        for panel in &mut panels {
            panel.band = Band::of(panel.unit, &thermostats);
        }
        Some(Plot {
            traces,
            panels,
            switches,
            line_sets,
            started: None,
            span: Span::TenSeconds,
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
        if let Some(i) = self.traces.iter().position(|t| t.name == source) {
            return Some(Plotted::Trace(i, self.traces[i].unit));
        }
        if let Some(k) = self.line_sets.iter().position(|l| l.name == source) {
            return Some(Plotted::Lines(k));
        }
        self.switches
            .iter()
            .position(|s| s.name == source)
            .map(Plotted::Switch)
    }

    /// Keeps what a GPIO payload says one of its lines is, where it is a
    /// change. Allocates nothing.
    ///
    /// # Arguments
    ///
    /// * `set`    - Its place in the plot's line sets.
    /// * `offset` - The line's offset on the chip, as its record says it.
    /// * `at`     - When it arrived.
    /// * `on`     - Whether the line is high.
    pub fn line(&mut self, set: usize, offset: u32, at: Instant, on: bool) {
        let Some(set) = self.line_sets.get(set) else {
            return;
        };
        // A record of a line the node does not name is the vehicle's
        // mistake, and is not drawn as one it does.
        if let Some(n) = set.offsets.iter().position(|&o| o == offset) {
            self.decision(set.first + n, at, on);
        }
    }

    /// Keeps a value's reading. Allocates nothing: the oldest recent
    /// reading makes room where there is none, and the history thins
    /// itself.
    ///
    /// # Arguments
    ///
    /// * `trace` - Its place in the plot's traces.
    /// * `at`    - When it arrived.
    /// * `value` - The value, in its unit.
    pub fn reading(&mut self, trace: usize, at: Instant, value: f64) {
        let Some(trace) = self.traces.get_mut(trace) else {
            return;
        };
        if trace.recent.len() == RECENT {
            trace.recent.pop_front();
        }
        trace.recent.push_back((at, value));
        trace.history.take(at, value);
        self.started.get_or_insert(at);
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
        // Heard, as a reading is: a plot of switches alone -- a set of lines
        // and nothing measured -- would otherwise wait for a reading that
        // never comes, and say it had heard nothing.
        self.started.get_or_insert(at);
    }

    /// How the window is laid out: the traces in each panel, and how many
    /// switches there are.
    ///
    /// # Returns
    ///
    /// Each panel's unit and its traces' places in the plot's traces, and
    /// the number of switches.
    pub fn layout(&self) -> (Vec<(Unit, Vec<usize>)>, usize) {
        let panels = self
            .panels
            .iter()
            .map(|panel| (panel.unit, panel.traces.clone()))
            .collect();
        (panels, self.switches.len())
    }

    /// The time at the plot's left edge, and how long the plot spans: for
    /// a span of fixed length, the first reading until the span has filled
    /// and the span's length ago after; for the whole run, the first
    /// reading, and the time since.
    ///
    /// # Arguments
    ///
    /// * `now` - The time at the right edge, once the span is full.
    ///
    /// # Returns
    ///
    /// The left edge and the span, or `None` before the first reading.
    pub fn window(&self, now: Instant) -> Option<(Instant, Duration)> {
        let started = self.started?;
        Some(match self.span.length() {
            Some(length) => match now.checked_sub(length) {
                Some(back) if back > started => (back, length),
                _ => (started, length),
            },
            // Never nought wide, which would put every reading at one x.
            None => (
                started,
                now.saturating_duration_since(started)
                    .max(Duration::from_secs(1)),
            ),
        })
    }

    /// Hands each reading of a trace to be drawn in the current span, from
    /// the last before the left edge, with whether it is joined to the one
    /// before.
    ///
    /// # Arguments
    ///
    /// * `trace` - The trace.
    /// * `left`  - The plot's left edge.
    /// * `visit` - Called with each reading's time, value and joining.
    fn each(
        &self,
        trace: &Trace,
        left: Instant,
        mut visit: impl FnMut(Instant, f64, bool),
    ) {
        if self.span == Span::Whole {
            let history = &trace.history;
            for kept in &history.kept {
                visit(kept.at, kept.value, kept.joined);
            }
            // The newest reading, which the history may not have kept yet,
            // so that the trace reaches the right edge.
            if let (Some(kept), Some(&(at, value))) =
                (history.kept.last(), trace.recent.back())
            {
                if at > kept.at {
                    visit(at, value, !history.broken);
                }
            }
            return;
        }
        // From the last reading before the left edge, the point the trace
        // comes in from, so that it runs to the edge rather than starting
        // short of it; those before that are off the plot.
        let first = trace.recent.partition_point(|&(at, _)| at < left);
        let mut last: Option<Instant> = None;
        for &(at, value) in trace.recent.range(first.saturating_sub(1)..) {
            let joined = last.is_some_and(|was| at.duration_since(was) <= GAP);
            visit(at, value, joined);
            last = Some(at);
        }
    }

    /// Draws the plot into `drawn`, whose strings and lists are reused from
    /// one drawing to the next.
    ///
    /// # Arguments
    ///
    /// * `now`    - The time to draw it at.
    /// * `width`  - Each panel's width, in pixels.
    /// * `height` - Each panel's height, in pixels.
    /// * `drawn`  - Where it is drawn.
    pub fn draw(
        &self,
        now: Instant,
        width: f32,
        height: f32,
        drawn: &mut Drawn,
    ) {
        drawn.clear(self);
        let Some((left, span)) = self.window(now) else {
            drawn.said.push_str("Nothing heard yet");
            for (panel, shown) in self.panels.iter().zip(&mut drawn.panels) {
                for (&i, latest) in panel.traces.iter().zip(&mut shown.latest) {
                    latest.push_str(&self.traces[i].name);
                }
                // Said before anything is heard, though it cannot be drawn
                // on a panel with no scale yet.
                if let Some(band) = &panel.band {
                    band_said(&mut shown.band_said, band, panel.unit);
                }
            }
            for switch in &self.switches {
                drawn.switches.push(SwitchDrawn::unheard(switch));
            }
            return;
        };
        drawn.said.push_str(self.span.said());
        if self.span == Span::Whole {
            drawn.said.push_str(": ");
            how_long(&mut drawn.said, now.saturating_duration_since(left));
        }
        // Signed, so that a reading from before the left edge is drawn to
        // the left of it, and the line from it comes in across the edge; the
        // window clips what falls outside.
        let x_of = |at: Instant| -> f32 {
            let since = if at >= left {
                at.duration_since(left).as_secs_f32()
            } else {
                -left.duration_since(at).as_secs_f32()
            };
            since / span.as_secs_f32() * width
        };

        for (panel, shown) in self.panels.iter().zip(&mut drawn.panels) {
            // The panel's scale: what is on the plot, the setpoints, and
            // the band's edges, so that its yellow is always on the panel.
            let mut low = f64::INFINITY;
            let mut high = f64::NEG_INFINITY;
            for &i in &panel.traces {
                self.each(&self.traces[i], left, |at, value, _| {
                    if at >= left {
                        low = low.min(value);
                        high = high.max(value);
                    }
                });
            }
            for &setpoint in &panel.setpoints {
                low = low.min(setpoint);
                high = high.max(setpoint);
            }
            if let Some(band) = &panel.band {
                let edges = band.edges();
                low = low.min(edges[3]);
                high = high.max(edges[0]);
            }
            // As many steps as fit with room for a tick's value in each, so
            // that a short panel's values are never written over each
            // other.
            let steps = ((height / TICK_ROOM) as usize).clamp(1, 5);
            // A reading that is not a number would never fit a scale: a
            // panel of only those is drawn as one with nothing yet.
            let scale = (low <= high && low.is_finite() && high.is_finite())
                .then(|| Scale::of(low, high, steps));
            let y_of = |value: f64| -> f32 {
                scale.map_or(height / 2.0, |scale| {
                    ((scale.top - value) / (scale.top - scale.bottom)) as f32
                        * height
                })
            };

            if let Some(scale) = scale {
                scale.ticks(height, &mut shown.ticks);
            }
            for &setpoint in &panel.setpoints {
                shown.setpoints.push(y_of(setpoint));
            }
            if let Some(band) = &panel.band {
                shown.band = Some(band.edges().map(y_of));
                band_said(&mut shown.band_said, band, panel.unit);
            }
            for (((&i, path), latest), dots) in panel
                .traces
                .iter()
                .zip(&mut shown.paths)
                .zip(&mut shown.latest)
                .zip(&mut shown.dots)
            {
                let trace = &self.traces[i];
                let mut pen = Pen { path, column: None };
                // A dot where each reading is, when it arrived and what it
                // said, so that the line between readings is not read as
                // readings; one to a column of pixels, where they crowd.
                let mut dotted: Option<i64> = None;
                self.each(trace, left, |at, value, joined| {
                    let (x, y) = (x_of(at), y_of(value));
                    pen.point(x, y, joined);
                    let column = x.floor() as i64;
                    if at >= left && dotted != Some(column) {
                        dot(dots, x, y);
                        dotted = Some(column);
                    }
                });
                pen.flush();
                latest.push_str(&trace.name);
                if let Some(&(_, value)) = trace.recent.back() {
                    latest.push(' ');
                    value_said(latest, value, trace.unit);
                }
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
                        shown.spans.push(SwitchSpan {
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
                    shown.spans.push(SwitchSpan {
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

/// Writes a trace's path, a reading at a time, as SVG path commands.
///
/// Readings that fall in one column of pixels are drawn as that column's
/// lowest and highest and then its last, rather than each: no more is
/// visible, and a long span would otherwise draw thousands of segments to a
/// few hundred columns.
struct Pen<'a> {
    path: &'a mut String,
    /// The column being drawn: its first reading's x, its index, the
    /// lowest and highest y in it, the last, and how many readings.
    column: Option<(f32, i64, f32, f32, f32, u32)>,
}

impl Pen<'_> {
    /// Draws a reading.
    ///
    /// # Arguments
    ///
    /// * `x`      - Where it is across, in pixels.
    /// * `y`      - Where it is down, in pixels.
    /// * `joined` - Whether a line is drawn to it from the one before.
    fn point(&mut self, x: f32, y: f32, joined: bool) {
        let index = x.floor() as i64;
        if joined {
            if let Some(column) = &mut self.column {
                if column.1 == index {
                    column.2 = column.2.min(y);
                    column.3 = column.3.max(y);
                    column.4 = y;
                    column.5 += 1;
                    return;
                }
            }
        }
        self.flush();
        let verb = if joined { 'L' } else { 'M' };
        let _ = write!(self.path, "{verb} {x:.1} {y:.1} ");
        self.column = Some((x, index, y, y, y, 1));
    }

    /// Finishes the column being drawn.
    fn flush(&mut self) {
        if let Some((x, _, low, high, last, count)) = self.column.take() {
            if count > 1 {
                let _ = write!(
                    self.path,
                    "L {x:.1} {low:.1} L {x:.1} {high:.1} L {x:.1} {last:.1} "
                );
            }
        }
    }
}

/// How big a reading's dot is: its radius, in pixels.
const DOT: f32 = 2.5;

/// Writes a dot, a circle of radius [`DOT`] about a point, as SVG path
/// commands: two half circles.
///
/// # Arguments
///
/// * `out` - Where it is written.
/// * `x`   - The point across, in pixels.
/// * `y`   - The point down, in pixels.
fn dot(out: &mut String, x: f32, y: f32) {
    let _ = write!(
        out,
        "M {:.1} {y:.1} A {DOT} {DOT} 0 1 0 {:.1} {y:.1} \
         A {DOT} {DOT} 0 1 0 {:.1} {y:.1} Z ",
        x - DOT,
        x + DOT,
        x - DOT
    );
}

/// A panel's scale: its top and bottom, and the step between its ticks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scale {
    pub top: f64,
    pub bottom: f64,
    pub step: f64,
}

impl Scale {
    /// The scale that shows everything from `low` to `high` with a little
    /// room either side, its ends on ticks one, two or five of a power of
    /// ten apart, as many as there is room for up to `steps`: fewer and the
    /// rounding out to the next tick can leave half the panel empty.
    ///
    /// # Arguments
    ///
    /// * `low`   - The lowest value it must show.
    /// * `high`  - The highest.
    /// * `steps` - The most steps between ticks there is room for, one at
    ///             least.
    ///
    /// # Returns
    ///
    /// The scale.
    pub fn of(low: f64, high: f64, steps: usize) -> Scale {
        // One value alone is shown with a tenth of itself either side, or a
        // whole one either side of nought.
        let (low, high) = if high - low > f64::EPSILON * high.abs().max(1.0) {
            (low, high)
        } else {
            let pad = if low == 0.0 { 1.0 } else { low.abs() * 0.1 };
            (low - pad, high + pad)
        };
        let room = (high - low) * 0.05;
        // Two at least: values either side of nought are a tick below it
        // and a tick above, however big the step.
        let steps = steps.max(2) as f64;
        let mut step = nice((high - low + 2.0 * room) / steps);
        let mut scale = Scale::around(low, high, room, step);
        // Rounding out to a tick at each end can add a step at each; where
        // that is more than there is room for, the next step up. Each try
        // at least doubles the step, so a few dozen reach any range a
        // double holds, and the tries are bounded all the same.
        for _ in 0..64 {
            if ((scale.top - scale.bottom) / step).round() <= steps {
                break;
            }
            step = nice(step * 1.01);
            scale = Scale::around(low, high, room, step);
        }
        scale
    }

    /// The scale with ticks `step` apart whose ends are the ticks beyond
    /// `low` and `high` with `room` either side.
    ///
    /// # Arguments
    ///
    /// * `low`  - The lowest value it must show.
    /// * `high` - The highest.
    /// * `room` - How much more it shows either side.
    /// * `step` - Between ticks.
    ///
    /// # Returns
    ///
    /// The scale.
    fn around(low: f64, high: f64, room: f64, step: f64) -> Scale {
        Scale {
            top: ((high + room) / step).ceil() * step,
            bottom: ((low - room) / step).floor() * step,
            step,
        }
    }

    /// Its ticks, from the top down, in a panel `height` pixels tall.
    ///
    /// # Arguments
    ///
    /// * `height` - The panel's height, in pixels.
    /// * `ticks`  - Where they are written, its strings reused.
    fn ticks(&self, height: f32, ticks: &mut Vec<Tick>) {
        let count = ((self.top - self.bottom) / self.step).round() as usize;
        let places = (-self.step.log10().floor()).max(0.0) as usize;
        ticks.resize_with(count + 1, Tick::default);
        for (k, tick) in ticks.iter_mut().enumerate() {
            let mut value = self.top - k as f64 * self.step;
            // So that nought is never said as minus nought.
            if value.abs() < self.step * 1e-6 {
                value = 0.0;
            }
            tick.y = k as f32 / count as f32 * height;
            tick.said.clear();
            let _ = write!(tick.said, "{value:.places$}");
        }
    }
}

/// The least room between ticks, in pixels: a line of the axis's type and
/// some space.
const TICK_ROOM: f32 = 36.0;

/// The step of one, two or five of a power of ten at least as big as `x`.
///
/// # Arguments
///
/// * `x` - The least the step may be, more than nought.
///
/// # Returns
///
/// The step.
fn nice(x: f64) -> f64 {
    let power = 10f64.powf(x.log10().floor());
    [1.0, 2.0, 5.0]
        .into_iter()
        .map(|m| m * power)
        .find(|&step| step >= x)
        .unwrap_or(10.0 * power)
}

/// Writes a value with its unit, to five figures and no more than three
/// places: a ratio is a number alone.
///
/// # Arguments
///
/// * `out`   - Where it is written.
/// * `value` - The value.
/// * `unit`  - Its unit.
fn value_said(out: &mut String, value: f64, unit: Unit) {
    number_said(out, value);
    if unit != Unit::Ratio {
        let _ = write!(out, " {}", unit.symbol());
    }
}

/// Writes a number to five figures and no more than three places.
///
/// # Arguments
///
/// * `out`   - Where it is written.
/// * `value` - The number.
fn number_said(out: &mut String, value: f64) {
    let figures = if value == 0.0 || !value.is_finite() {
        1
    } else {
        value.abs().log10().floor() as i32 + 1
    };
    let places = (5 - figures).clamp(0, 3) as usize;
    let _ = write!(out, "{value:.places$}");
}

/// Writes what the legend says of a band: where the reading is held, and
/// whose band it is, such as `band 291.15–295.15 K (thermostat)`.
///
/// # Arguments
///
/// * `out`  - Where it is written, emptied first.
/// * `band` - The band.
/// * `unit` - The panel's unit.
fn band_said(out: &mut String, band: &Band, unit: Unit) {
    let edges = band.edges();
    out.clear();
    out.push_str("band ");
    number_said(out, edges[2]);
    out.push('\u{2013}');
    value_said(out, edges[1], unit);
    let _ = write!(out, " ({})", band.name);
}

/// Writes how long the run has been: seconds, or minutes and seconds, or
/// hours and minutes.
///
/// # Arguments
///
/// * `out`  - Where it is written.
/// * `long` - How long.
fn how_long(out: &mut String, long: Duration) {
    let seconds = long.as_secs();
    let _ = match seconds {
        0..=59 => write!(out, "{seconds} s"),
        60..=3599 => write!(out, "{} min {} s", seconds / 60, seconds % 60),
        _ => write!(out, "{} h {} min", seconds / 3600, seconds / 60 % 60),
    };
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
/// The names of the nodes its data goes to, but the ground, sorted.
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
    driven.sort();
    driven
}

/// A stretch of time a switch was in one state, in pixels across the plot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchSpan {
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
    pub spans: Vec<SwitchSpan>,
}

impl SwitchDrawn {
    /// A switch as it is drawn before anything is known of it: labelled,
    /// with no state and nothing drawn.
    ///
    /// # Arguments
    ///
    /// * `switch` - The switch.
    ///
    /// # Returns
    ///
    /// The switch, drawn as unheard; [`Plot::draw`] adds what was heard.
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

/// One tick of a panel's axis: how far down, and its value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tick {
    pub y: f32,
    pub said: String,
}

/// One panel, drawn: a path for each trace, as SVG path commands in pixels,
/// what each last read, the ticks, the setpoints' rules and the band.
#[derive(Debug, Default)]
pub struct PanelDrawn {
    pub paths: Vec<String>,
    /// Each trace's dots, one where each reading is, as SVG path commands.
    pub dots: Vec<String>,
    pub latest: Vec<String>,
    pub ticks: Vec<Tick>,
    pub setpoints: Vec<f32>,
    /// How far down the band's edges are, in pixels, from the top down as
    /// [`Band::edges`] gives them, or `None` for a panel with no band.
    pub band: Option<[f32; 4]>,
    /// What the legend says of the band; empty where there is none.
    pub band_said: String,
}

/// The plot, drawn: what its span is, each panel, and each switch's spans.
#[derive(Debug, Default)]
pub struct Drawn {
    pub said: String,
    pub panels: Vec<PanelDrawn>,
    pub switches: Vec<SwitchDrawn>,
}

impl Drawn {
    /// Empties it for the next drawing, keeping what its strings and lists
    /// hold room for.
    ///
    /// # Arguments
    ///
    /// * `plot` - The plot to be drawn into it, which says how many panels
    ///            and traces there are.
    fn clear(&mut self, plot: &Plot) {
        self.said.clear();
        self.panels
            .resize_with(plot.panels.len(), PanelDrawn::default);
        for (panel, drawn) in plot.panels.iter().zip(&mut self.panels) {
            drawn.paths.resize_with(panel.traces.len(), String::new);
            drawn.dots.resize_with(panel.traces.len(), String::new);
            drawn.latest.resize_with(panel.traces.len(), String::new);
            for text in drawn
                .paths
                .iter_mut()
                .chain(&mut drawn.dots)
                .chain(&mut drawn.latest)
            {
                text.clear();
            }
            drawn.ticks.clear();
            drawn.setpoints.clear();
            drawn.band = None;
            drawn.band_said.clear();
        }
        self.switches.clear();
    }
}

/// How often the window is redrawn: often enough that the scrolling reads
/// as movement rather than as steps.
pub const REDRAW: Duration = Duration::from_millis(100);

/// Hands what arrived from a plotted source to the plot: a value, or a
/// switch's decision. What cannot be read as one is not plotted; the
/// source's panel shows it as it came.
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
        Plotted::Trace(i, unit) => {
            if let (Ok(value), Ok(mut plot)) =
                (from_value(data, unit, ""), plot.lock())
            {
                plot.reading(i, at, value);
            }
        }
        Plotted::Switch(i) => {
            if let (Ok(on), Ok(mut plot)) =
                (from_decision(data, ""), plot.lock())
            {
                plot.decision(i, at, on);
            }
        }
        // One record for each line reported, each the time, the line's
        // offset and what it is, as `tcslibgs::node::payload::gpio` lays
        // them out; a datum that is not whole records is not plotted.
        Plotted::Lines(k) => {
            let (records, rest) = data.as_chunks::<EVENT_BYTES>();
            if records.is_empty() || !rest.is_empty() {
                return;
            }
            let Ok(mut plot) = plot.lock() else {
                return;
            };
            for record in records {
                let word = |at: usize| {
                    u32::from_be_bytes([
                        record[at],
                        record[at + 1],
                        record[at + 2],
                        record[at + 3],
                    ])
                };
                plot.line(k, word(8), at, word(12) != 0);
            }
        }
    }
}

/// Sets the plot window up and opens it: the Plot button that opens it
/// again once closed, the span buttons, and the timer that redraws it.
///
/// Opened from the start, because a configuration with values reaching the
/// ground is one whose values are the reason to be watching it.
///
/// # Arguments
///
/// * `ui`     - The main window, whose button opens it.
/// * `window` - The plot window.
/// * `plot`   - What it plots.
///
/// # Returns
///
/// The timer, which redraws the window for as long as it is kept.
pub fn show(
    ui: &MainWindow,
    window: &PlotWindow,
    plot: Arc<Mutex<Plot>>,
) -> Timer {
    ui.set_can_plot(true);
    {
        let window = window.as_weak();
        ui.on_plot_clicked(move || {
            if let Some(window) = window.upgrade() {
                let _ = window.show();
            }
        });
    }
    {
        let plot = plot.clone();
        window.on_span_chosen(move |chosen| {
            let span = usize::try_from(chosen)
                .ok()
                .and_then(|i| Span::ALL.get(i).copied());
            if let (Some(span), Ok(mut plot)) = (span, plot.lock()) {
                plot.span = span;
            }
        });
    }

    let (layout, switches) =
        plot.lock().map(|p| p.layout()).unwrap_or((Vec::new(), 0));
    struct Models {
        traces: Rc<VecModel<TraceShown>>,
        ticks: Rc<VecModel<TickShown>>,
        setpoints: Rc<VecModel<f32>>,
    }
    let models: Vec<Models> = layout
        .iter()
        .map(|(_, traces)| Models {
            traces: Rc::new(VecModel::from(
                traces
                    .iter()
                    .map(|&i| TraceShown {
                        said: SharedString::new(),
                        commands: SharedString::new(),
                        dots: SharedString::new(),
                        colour: colour(TRACE_COLOURS[i % TRACE_COLOURS.len()]),
                    })
                    .collect::<Vec<_>>(),
            )),
            ticks: Rc::new(VecModel::default()),
            setpoints: Rc::new(VecModel::default()),
        })
        .collect();
    let panels: Rc<VecModel<PanelShown>> = Rc::new(VecModel::from(
        layout
            .iter()
            .zip(&models)
            .map(|((unit, _), models)| PanelShown {
                unit: SharedString::from(unit.name()),
                traces: ModelRc::from(models.traces.clone()),
                ticks: ModelRc::from(models.ticks.clone()),
                setpoints: ModelRc::from(models.setpoints.clone()),
                banded: false,
                band_said: SharedString::new(),
                too_high: 0.0,
                high: 0.0,
                low: 0.0,
                too_low: 0.0,
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
    window.set_panels(ModelRc::from(panels.clone()));
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
        let height = window.get_panel_height();
        {
            let Ok(plot) = plot.lock() else {
                return;
            };
            plot.draw(Instant::now(), width, height, &mut drawn);
        }

        // Slint takes what it draws as its own strings and models, so each
        // redraw makes them: the one place in this window that allocates as
        // it runs, and unavoidably, a window being drawn by Slint. What is
        // worked out to draw -- the paths, ticks and spans -- is in `drawn`,
        // which is reused.
        window.set_said(SharedString::from(drawn.said.as_str()));
        for (p, (panel, models)) in drawn.panels.iter().zip(&models).enumerate()
        {
            if let Some(mut row) = panels.row_data(p) {
                row.banded = panel.band.is_some();
                [row.too_high, row.high, row.low, row.too_low] =
                    panel.band.unwrap_or_default();
                if row.band_said.as_str() != panel.band_said {
                    row.band_said =
                        SharedString::from(panel.band_said.as_str());
                }
                panels.set_row_data(p, row);
            }
            for (i, ((commands, dots), said)) in panel
                .paths
                .iter()
                .zip(&panel.dots)
                .zip(&panel.latest)
                .enumerate()
            {
                if let Some(mut trace) = models.traces.row_data(i) {
                    trace.commands = SharedString::from(commands.as_str());
                    trace.dots = SharedString::from(dots.as_str());
                    trace.said = SharedString::from(said.as_str());
                    models.traces.set_row_data(i, trace);
                }
            }
            models.ticks.set_vec(
                panel
                    .ticks
                    .iter()
                    .map(|tick| TickShown {
                        y: tick.y,
                        said: SharedString::from(tick.said.as_str()),
                    })
                    .collect::<Vec<_>>(),
            );
            models.setpoints.set_vec(panel.setpoints.clone());
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

/// One of [`TRACE_COLOURS`] as Slint takes it.
fn colour(rgb: u32) -> Color {
    Color::from_rgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::path::Path;

    /// An example's configuration, as shipped.
    ///
    /// # Arguments
    ///
    /// * `name` - The example's directory, which names its file too.
    ///
    /// # Returns
    ///
    /// The configuration.
    fn example(name: &str) -> Graph {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        Graph::load(root.join(format!("examples/{name}/{name}.ytcs")))
            .expect("the example reads")
    }

    /// What reaches the ground in a configuration: the sources of its links
    /// to the ground, as tcsmoc asks.
    ///
    /// # Arguments
    ///
    /// * `graph` - The configuration.
    ///
    /// # Returns
    ///
    /// Whether a node's data reaches the ground.
    fn reaching(graph: &Graph) -> impl Fn(&str) -> bool {
        let sources: Vec<String> = tcslibgs::graph::telemetry::of(graph)
            .expect("its links to the ground")
            .into_iter()
            .map(|link| link.source)
            .collect();
        move |name| sources.iter().any(|source| source == name)
    }

    /// The thermostat example's three sensors and their vote are traced, in
    /// one panel in kelvin, with the thermostat's setpoint, though its
    /// decision does not reach the ground; what does is the lines sensing
    /// the two heaters, each a strip; and a sensor's own counts are not
    /// plotted.
    #[test]
    fn the_thermostat_example_plots_its_temperatures_and_its_heaters() {
        let graph = example("thermostat");
        let plot = Plot::of(&graph, reaching(&graph)).expect("a plot");
        let (panels, switches) = plot.layout();
        assert_eq!(panels.len(), 1);
        assert_eq!(panels[0].0, Unit::Kelvin);
        assert_eq!(panels[0].1.len(), 4);
        assert_eq!(switches, 2);
        assert_eq!(plot.panels[0].setpoints, vec![293.15]);
        assert_eq!(
            plot.plotted("ambient"),
            Some(Plotted::Trace(0, Unit::Kelvin))
        );
        assert_eq!(
            plot.plotted("temp2"),
            Some(Plotted::Trace(2, Unit::Kelvin))
        );
        // Each heater's switch, reporting what it was last told.
        assert_eq!(plot.plotted("gpio-out-tmass1"), Some(Plotted::Switch(0)));
        assert_eq!(plot.plotted("gpio-out-tmass2"), Some(Plotted::Switch(1)));
        assert_eq!(plot.plotted("thermostat"), None);
        assert_eq!(plot.plotted("temp_1"), None);

        let mut drawn = Drawn::default();
        plot.draw(Instant::now(), 100.0, 60.0, &mut drawn);
        assert_eq!(drawn.said, "Nothing heard yet");
        let labels: Vec<&str> =
            drawn.switches.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["gpio-out-tmass1", "gpio-out-tmass2"]);
        assert!(drawn.switches.iter().all(|s| s.on.is_none()));
        assert!(drawn.panels[0].paths.iter().all(String::is_empty));
    }

    /// A GPIO payload's records, one for each line, each set that line's
    /// strip; a record of a line the node does not name, or a datum that is
    /// not whole records, changes nothing. And a plot of switches alone, as
    /// the gpio example's is, is drawn once one has been heard.
    #[test]
    fn each_lines_record_sets_its_strip() {
        let graph = example("gpio");
        let plot = Mutex::new(Plot::of(&graph, reaching(&graph)).unwrap());
        let record = |line: u32, on: u32| {
            let mut record = [0u8; EVENT_BYTES];
            record[8..12].copy_from_slice(&line.to_be_bytes());
            record[12..].copy_from_slice(&on.to_be_bytes());
            record
        };
        let t0 = Instant::now();
        // The switch, line 0, high; then a line it does not name, and a
        // record and a piece.
        take(&plot, Plotted::Lines(0), &record(0, 1), t0);
        take(&plot, Plotted::Lines(0), &record(7, 0), t0);
        let mut ragged = record(0, 0).to_vec();
        ragged.extend(&record(0, 0)[..4]);
        take(&plot, Plotted::Lines(0), &ragged, t0);
        let mut drawn = Drawn::default();
        plot.lock().unwrap().draw(
            t0 + Duration::from_secs(1),
            100.0,
            60.0,
            &mut drawn,
        );
        let states: Vec<Option<bool>> =
            drawn.switches.iter().map(|s| s.on).collect();
        assert_eq!(states, [Some(true)]);
    }

    /// The descent example's values are in three units, so three panels,
    /// and it has no switch.
    #[test]
    fn the_descent_example_has_a_panel_for_each_unit() {
        let plot = Plot::of(&example("descent"), |_| true).expect("a plot");
        let (panels, switches) = plot.layout();
        let units: Vec<Unit> = panels.iter().map(|(unit, _)| *unit).collect();
        assert_eq!(units, vec![Unit::Metre, Unit::MetrePerSecond, Unit::Ratio]);
        assert_eq!(switches, 0);
    }

    /// The battery example's shedding switch reads the charge, a ratio, so
    /// its setpoint is drawn on the ratio panel.
    #[test]
    fn a_setpoint_is_drawn_where_its_unit_is() {
        let plot = Plot::of(&example("battery"), |_| true).expect("a plot");
        let ratio = plot
            .panels
            .iter()
            .find(|panel| panel.unit == Unit::Ratio)
            .expect("a panel of ratios");
        assert_eq!(ratio.setpoints, vec![0.4]);
        assert_eq!(plot.layout().1, 1);
    }

    /// A configuration whose values do not reach the ground has nothing to
    /// plot.
    #[test]
    fn nothing_reaching_the_ground_is_no_plot() {
        assert!(Plot::of(&example("thermostat"), |_| false).is_none());
    }

    /// A panel is scaled to what it shows, its ends on ticks; one value
    /// alone has room either side.
    #[test]
    fn a_scale_fits_what_it_shows_on_round_ticks() {
        assert_eq!(
            Scale::of(0.0, 10.0, 5),
            Scale {
                top: 15.0,
                bottom: -5.0,
                step: 5.0
            }
        );
        let scale = Scale::of(293.15, 293.15, 5);
        assert!(scale.bottom < 293.15 && scale.top > 293.15, "{scale:?}");
        let scale = Scale::of(0.0, 0.0, 5);
        assert!(scale.bottom < 0.0 && scale.top > 0.0, "{scale:?}");
        let mut ticks = Vec::new();
        Scale::of(0.0, 10.0, 5).ticks(400.0, &mut ticks);
        let said: Vec<&str> = ticks.iter().map(|t| t.said.as_str()).collect();
        assert_eq!(said, vec!["15", "10", "5", "0", "-5"]);
        assert_eq!(ticks[2].y, 200.0);
    }

    /// A short panel has fewer ticks, so that their values are never written
    /// over each other.
    #[test]
    fn a_short_panel_has_fewer_ticks() {
        let mut plot = Plot::of(&example("descent"), |_| true).expect("a plot");
        let t0 = Instant::now();
        plot.reading(0, t0, 0.0);
        plot.reading(0, t0 + Duration::from_secs(1), 10.0);
        let mut drawn = Drawn::default();
        for (height, most) in [(600.0, 6), (120.0, 4), (80.0, 3)] {
            plot.draw(t0 + Duration::from_secs(1), 100.0, height, &mut drawn);
            let ticks = &drawn.panels[0].ticks;
            assert!(ticks.len() <= most, "{height}: {ticks:?}");
            for pair in ticks.windows(2) {
                assert!(pair[1].y - pair[0].y >= TICK_ROOM, "{ticks:?}");
            }
        }
    }

    /// The plot fills from the left, and once the span has been drawn the
    /// newest is at the right edge and the oldest has scrolled off; a
    /// reading is placed on the panel's scale.
    #[test]
    fn it_fills_from_the_left_and_then_scrolls() {
        let mut plot = Plot::of(&example("descent"), |_| true).expect("a plot");
        let t0 = Instant::now();
        plot.reading(0, t0, 0.0);
        plot.reading(0, t0 + Duration::from_secs(2), 10.0);
        let mut drawn = Drawn::default();

        // From -5 m at the bottom to 15 m at the top, 600 pixels.
        plot.draw(t0 + Duration::from_secs(2), 1000.0, 600.0, &mut drawn);
        assert_eq!(drawn.panels[0].paths[0], "M 0.0 450.0 L 200.0 150.0 ");
        assert_eq!(drawn.panels[0].latest[0], "altitude 10.000 m");
        assert_eq!(drawn.said, "The last 10 s");

        plot.reading(0, t0 + Duration::from_secs(12), 0.0);
        plot.draw(t0 + Duration::from_secs(12), 1000.0, 600.0, &mut drawn);
        // The left edge is at two seconds: the reading there is at the
        // edge, the one before it comes in from off the plot, and the ten
        // seconds to the newest, at the right edge, are a gap.
        assert_eq!(
            drawn.panels[0].paths[0],
            "M -200.0 450.0 L 0.0 150.0 M 1000.0 450.0 "
        );
    }

    /// Readings further apart than GAP are not joined.
    #[test]
    fn a_silence_breaks_the_trace() {
        let mut plot = Plot::of(&example("descent"), |_| true).expect("a plot");
        let t0 = Instant::now();
        plot.reading(0, t0, 0.0);
        plot.reading(0, t0 + Duration::from_secs(1), 10.0);
        plot.reading(0, t0 + Duration::from_secs(4), 10.0);
        let mut drawn = Drawn::default();
        plot.draw(t0 + Duration::from_secs(4), 100.0, 600.0, &mut drawn);
        assert_eq!(
            drawn.panels[0].paths[0],
            "M 0.0 450.0 L 10.0 150.0 M 40.0 150.0 "
        );
    }

    /// Each reading on the plot has a dot where it is, and readings crowded
    /// into one column of pixels share one.
    #[test]
    fn each_reading_has_a_dot() {
        let mut plot = Plot::of(&example("descent"), |_| true).expect("a plot");
        let t0 = Instant::now();
        plot.reading(0, t0, 0.0);
        plot.reading(0, t0 + Duration::from_secs(2), 10.0);
        // A tenth of a pixel after the last, in the same column.
        plot.reading(0, t0 + Duration::from_millis(2001), 10.0);
        let mut drawn = Drawn::default();
        plot.draw(t0 + Duration::from_secs(2), 1000.0, 600.0, &mut drawn);
        assert_eq!(
            drawn.panels[0].dots[0],
            "M -2.5 450.0 A 2.5 2.5 0 1 0 2.5 450.0 A 2.5 2.5 0 1 0 -2.5 \
             450.0 Z M 197.5 150.0 A 2.5 2.5 0 1 0 202.5 150.0 A 2.5 2.5 0 1 \
             0 197.5 150.0 Z "
        );
    }

    /// Readings in one column of pixels are drawn as its lowest, highest and
    /// last, so that a spike among them is not lost.
    #[test]
    fn a_crowded_column_is_drawn_as_its_range() {
        let mut path = String::new();
        let mut pen = Pen {
            path: &mut path,
            column: None,
        };
        pen.point(0.2, 10.0, false);
        pen.point(0.5, 50.0, true);
        pen.point(0.7, 5.0, true);
        pen.point(0.9, 20.0, true);
        pen.point(1.5, 30.0, true);
        pen.flush();
        assert_eq!(
            path,
            "M 0.2 10.0 L 0.2 5.0 L 0.2 50.0 L 0.2 20.0 L 1.5 30.0 "
        );
    }

    /// The whole run is drawn from the first reading at the left edge to
    /// now at the right.
    #[test]
    fn the_whole_run_spans_the_plot() {
        let mut plot = Plot::of(&example("descent"), |_| true).expect("a plot");
        plot.span = Span::Whole;
        let t0 = Instant::now();
        plot.reading(0, t0, 0.0);
        plot.reading(0, t0 + Duration::from_secs(1), 10.0);
        plot.reading(0, t0 + Duration::from_secs(2), 0.0);
        let mut drawn = Drawn::default();
        plot.draw(t0 + Duration::from_secs(80), 800.0, 600.0, &mut drawn);
        assert_eq!(
            drawn.panels[0].paths[0],
            "M 0.0 450.0 L 10.0 150.0 L 20.0 450.0 "
        );
        assert_eq!(drawn.said, "The whole run: 1 min 20 s");
    }

    /// A full history thins itself in place, keeping the first reading and
    /// every other after, and a silence in what it lets go still breaks the
    /// trace where it was.
    #[test]
    fn a_full_history_thins_itself_and_keeps_its_silences() {
        let mut history = History::new();
        let t0 = Instant::now();
        let second = Duration::from_secs(1);
        for i in 0..HISTORY {
            // A silence before the fourth reading, which thinning lets go.
            let late = if i >= 3 { 3 * second } else { Duration::ZERO };
            history.take(t0 + second * i as u32 + late, i as f64);
        }
        assert_eq!(history.kept.len(), HISTORY);
        assert!(!history.kept[3].joined, "the silence is kept");

        let at = history.last.expect("a reading") + second;
        history.take(at, -1.0);
        assert_eq!(history.kept.capacity(), HISTORY, "nothing allocated");
        assert_eq!(history.kept.len(), HISTORY / 2 + 1);
        assert_eq!(history.stride, 2);
        let values: Vec<f64> =
            history.kept[..3].iter().map(|kept| kept.value).collect();
        assert_eq!(values, vec![0.0, 2.0, 4.0]);
        // Joined to 2 through 3, the reading let go, and the silence was
        // between 2 and 3.
        assert!(history.kept[1].joined);
        assert!(!history.kept[2].joined, "the silence survives thinning");
        assert_eq!(history.kept.last().map(|kept| kept.value), Some(-1.0));
    }

    /// A switch is drawn from each change to the next, in its state, and to
    /// now from the last; a decision that repeats the last changes nothing.
    #[test]
    fn a_switch_is_drawn_from_change_to_change() {
        let mut plot =
            Plot::of(&example("thermostat"), |_| true).expect("a plot");
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

    /// A band named for its thermostat, for the tests.
    ///
    /// # Arguments
    ///
    /// * `name`     - The thermostat's name.
    /// * `setpoint` - Its setpoint.
    /// * `half`     - Its half band.
    ///
    /// # Returns
    ///
    /// The band.
    fn band(name: &str, setpoint: f64, half: f64) -> Band {
        Band {
            name: name.to_string(),
            setpoint,
            half,
        }
    }

    /// A band's edges are the setpoint plus and minus one and two half
    /// bands, from the top down.
    #[test]
    fn a_band_has_four_edges() {
        assert_eq!(band("t", 293.0, 2.0).edges(), [297.0, 295.0, 291.0, 289.0]);
    }

    /// Of the thermostats reading a unit, the band drawn is that with the
    /// highest setpoint, the first of two with one setpoint; one reading
    /// another unit is not drawn, and with none there is no band.
    #[test]
    fn the_band_drawn_is_the_highest_setpoints() {
        let thermostats = vec![
            (Unit::Kelvin, band("stage_2", 290.15, 1.0)),
            (Unit::Ratio, band("shed", 400.0, 0.1)),
            (Unit::Kelvin, band("stage_1", 293.15, 2.0)),
            (Unit::Kelvin, band("stage_1b", 293.15, 3.0)),
        ];
        assert_eq!(
            Band::of(Unit::Kelvin, &thermostats),
            Some(band("stage_1", 293.15, 2.0))
        );
        assert_eq!(
            Band::of(Unit::Ratio, &thermostats),
            Some(band("shed", 400.0, 0.1))
        );
        assert_eq!(Band::of(Unit::Metre, &thermostats), None);
        assert_eq!(Band::of(Unit::Kelvin, &[]), None);
    }

    /// Staged heating: a second thermostat reading the zone temperature, set
    /// lower and driving one heater, has its setpoint drawn as a rule and its
    /// own strip, but the band drawn is the first stage's.
    #[test]
    fn staged_heating_draws_the_first_stages_band() {
        use tcslibgs::endpoint_config::Scalar;
        use tcslibgs::graph::{Destinations, Link};

        let mut graph = example("thermostat");
        let mut second = graph
            .node("thermostat")
            .expect("the example's thermostat")
            .clone();
        second.name = "thermostat_low".to_string();
        second
            .attributes
            .insert("setpoint".to_string(), Scalar::of("290.15"));
        graph.nodes.push(second);
        graph.links.push(Link {
            from: "ambient".to_string(),
            to: Destinations::of(vec!["thermostat_low".to_string()]),
            comment: None,
        });
        graph.links.push(Link {
            from: "thermostat_low".to_string(),
            to: Destinations::of(vec!["gpio-out-tmass2".to_string()]),
            comment: None,
        });
        let plot = Plot::of(&graph, |_| true).expect("a plot");
        assert_eq!(plot.panels[0].setpoints, vec![293.15, 290.15]);
        assert_eq!(plot.panels[0].band, Some(band("thermostat", 293.15, 2.0)));
        let mut drawn = Drawn::default();
        plot.draw(Instant::now(), 100.0, 60.0, &mut drawn);
        let labels: Vec<&str> =
            drawn.switches.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                "gpio-out-tmass1",
                "gpio-out-tmass2",
                "thermostat \u{2192} gpio-out-tmass1, gpio-out-tmass2",
                "thermostat_low \u{2192} gpio-out-tmass2",
            ]
        );
    }

    /// A thermostat's band is drawn behind its panel, its edges in order
    /// down the panel and on it, the scale taking in two half bands either
    /// side of the setpoint however close the readings are to it, and the
    /// legend naming the band and its thermostat.
    #[test]
    fn a_band_is_drawn_and_the_scale_takes_it_in() {
        let mut plot =
            Plot::of(&example("thermostat"), |_| true).expect("a plot");
        assert_eq!(plot.panels[0].band, Some(band("thermostat", 293.15, 2.0)));
        let mut drawn = Drawn::default();
        plot.draw(Instant::now(), 100.0, 600.0, &mut drawn);
        assert_eq!(drawn.panels[0].band, None, "no scale yet");
        assert_eq!(
            drawn.panels[0].band_said,
            "band 291.15\u{2013}295.15 K (thermostat)"
        );

        let t0 = Instant::now();
        plot.reading(0, t0, 293.0);
        plot.draw(t0, 100.0, 600.0, &mut drawn);
        let edges = drawn.panels[0].band.expect("a band");
        for pair in edges.windows(2) {
            assert!(pair[0] < pair[1], "{edges:?}");
        }
        assert!(edges[0] >= 0.0 && edges[3] <= 600.0, "{edges:?}");
        let ticks = &drawn.panels[0].ticks;
        let top: f64 = ticks[0].said.parse().expect("a number");
        let bottom: f64 =
            ticks[ticks.len() - 1].said.parse().expect("a number");
        assert!(top >= 297.15 && bottom <= 289.15, "{top} {bottom}");
        assert_eq!(
            drawn.panels[0].band_said,
            "band 291.15\u{2013}295.15 K (thermostat)"
        );
    }

    /// A panel no thermostat reads has no band, and says none.
    #[test]
    fn no_thermostat_is_no_band() {
        let mut plot = Plot::of(&example("descent"), |_| true).expect("a plot");
        assert!(plot.panels.iter().all(|panel| panel.band.is_none()));
        let t0 = Instant::now();
        plot.reading(0, t0, 10.0);
        let mut drawn = Drawn::default();
        plot.draw(t0, 100.0, 600.0, &mut drawn);
        for panel in &drawn.panels {
            assert_eq!(panel.band, None);
            assert!(panel.band_said.is_empty());
        }
    }

    /// The thermostat example's plot window, opened as tcsmoc opens it and
    /// redrawn once by its timer, says each region of the band in words, in
    /// order down the panel, names the band in the legend, and writes "on"
    /// on a switch's strip while it is on.
    ///
    /// Not a test of its own: one test per binary may start the testing
    /// backend, and its windows belong to the thread that made them. Called
    /// from main.rs's `every_panels_data_is_inside_the_window`.
    pub(crate) fn the_band_is_said_in_words_in_the_window() {
        use i_slint_backend_testing::ElementHandle;

        let ui = MainWindow::new().expect("a main window");
        let window = PlotWindow::new().expect("a plot window");
        window
            .window()
            .set_size(slint::LogicalSize::new(820.0, 620.0));
        let plot = Arc::new(Mutex::new(
            Plot::of(&example("thermostat"), |_| true).expect("a plot"),
        ));
        let _timer = show(&ui, &window, plot.clone());
        {
            let mut plot = plot.lock().expect("the plot");
            // Five seconds back, so that the switch's span is wide enough
            // to say "on" in.
            let t0 = Instant::now()
                .checked_sub(Duration::from_secs(5))
                .expect("a time five seconds back");
            plot.reading(0, t0, 292.0);
            plot.decision(0, t0, true);
        }
        std::thread::sleep(REDRAW + Duration::from_millis(50));
        slint::platform::update_timers_and_animations();

        let said = |text: &str| -> Vec<f32> {
            ElementHandle::find_by_element_type_name(&window, "Text")
                .filter(|e| e.accessible_label().as_deref() == Some(text))
                .map(|e| e.absolute_position().y)
                .collect()
        };
        let mut down = Vec::new();
        for words in ["too high", "high", "low", "too low"] {
            let found = said(words);
            assert_eq!(found.len(), 1, "{words:?} is said once: {found:?}");
            down.push(found[0]);
        }
        for pair in down.windows(2) {
            assert!(pair[0] < pair[1], "the regions out of order: {down:?}");
        }
        assert_eq!(
            said("band 291.15\u{2013}295.15 K (thermostat)").len(),
            1,
            "the legend names the band"
        );
        // Once beside the strip, as its state, and once on it.
        assert_eq!(said("on").len(), 2, "the switch on is said on its strip");
    }
}
