//! What each node type looks like, and the rules the looks obey.
//!
//! Two rules, and both are checked by the tests at the bottom rather than
//! trusted to whoever edits the table next.
//!
//! **Nothing is told apart by colour alone.** Every node type differs from
//! every other in at least two channels that are not colour -- its outline
//! shape, how wide its border is, how its inside is shaded, how large it is,
//! and how its label is set. A reader who sees no colour at all still sees
//! twelve distinguishable nodes, and a reader who sees colour gets the same
//! information twice. Two channels rather than one on purpose: a single
//! distinguishing channel is one print, one projector or one pair of eyes away
//! from being no channel.
//!
//! Type is one of those channels. A label in a monospaced face, a heavier
//! weight or an italic is a difference that survives a monochrome print and a
//! reader who cannot pick the shape out at a small size, and it is read
//! without being decoded -- which is more than can be said for a legend.
//!
//! **Red means something is wrong.** No node type, no selection, no hover and
//! no link is red. It is reserved so that red anywhere on the canvas means an
//! error and can be looked for, which is worth more than a tenth palette
//! entry.

/// The outline a node is drawn with.
///
/// Slint draws all of these from its own primitives -- a `Rectangle` with and
/// without `border-radius`, and one with a radius of half its shorter side for
/// a fully rounded end -- so none of this needs an image or a font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outline {
    /// Square corners. Hardware: what is physically there.
    Square,
    /// Rounded corners. Software aboard: a computation.
    Rounded,
    /// Fully rounded ends. An end of the graph: the ground, or a log.
    ///
    /// Drawn as a radius of half the shorter side, so it is a circle where the
    /// sides are equal and a rounded-off pill where they are not. Every node
    /// being one width now means these are pills, which reads as the same
    /// shape family and still shares nothing with the other three outlines.
    Circle,
    /// A rectangle with one pair of corners rounded and one square, which
    /// reads as a direction. Interfaces that are driven rather than read.
    Keyed,
}

/// How the inside is filled.
///
/// Shading rather than colour, so it survives a monochrome rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shading {
    /// Flat fill.
    Solid,
    /// Diagonal hatching, drawn as narrow lines.
    Hatched,
    /// No fill at all: the outline and the label only.
    Open,
    /// A second outline inside the first.
    Double,
}

/// How wide the outline is drawn, in pixels.
///
/// A third non-colour channel, and the one that reads at a glance across a
/// whole canvas: the nodes a control loop runs through are drawn heavily.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Narrow,
    Wide,
}

/// How a node's label is set.
///
/// The fifth channel, and the one that works at a size where a shape has
/// stopped being legible. `Mono` is for the types whose attributes are
/// addresses and register numbers -- a bus, a chip select -- because the file
/// shows those in a monospaced column and the canvas should agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    /// The system's ordinary face.
    Plain,
    /// The system's monospaced face.
    Mono,
}

/// What a node type looks like.
///
/// `tint` is never the whole of the distinction -- see the rule at the top of
/// this file -- and there is one per type. It is named rather than given as a
/// value so the window can hold the colours and this table can hold which
/// type has which.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Appearance {
    pub outline: Outline,
    pub shading: Shading,
    pub weight: Weight,
    /// The nominal width and height, in pixels, before a label widens it.
    ///
    /// Width is [`WIDTH`] for every type, so what varies here is the height:
    /// size is still the fourth channel -- a MOC link is visibly taller than a
    /// sensor -- and it is one channel whichever of the two carries it.
    pub size: (u32, u32),
    /// How the label is set: the face, its size in pixels, its weight, and
    /// whether it is italic. The fifth channel.
    pub face: Face,
    pub label_size: u32,
    pub label_weight: u32,
    pub label_italic: bool,
    pub tint: Tint,
}

/// The palette: one tint per node type, in three families.
///
/// A node's colour says two things at once. Which family it is in -- the
/// ground link, the vehicle's own graph, what it carries -- is the *hue*, and
/// it is the hue of that family's working pane: a payload is a plum or a
/// maroon on a pale blush ground, a vehicle node a green on a pale green one,
/// the ground link an azure on pale blue. Which type it is, within the family,
/// is the step: lightness mostly, chroma and a little hue beside it.
///
/// Darker than its own pane, always, and by enough to read: every tint has a
/// contrast ratio of at least three to one against the ground it is drawn on,
/// which is the threshold for a shape rather than for text. A node has to be
/// legible on its own pane, and nowhere else.
///
/// # Colour blindness
///
/// The fifteen were searched for, not chosen: the search maximised the
/// smallest perceived difference between any two of them, taken in CIE
/// L\*a\*b\* under normal vision and under simulated protanopia, deuteranopia
/// and tritanopia at once, subject to each tint staying in its pane's hue
/// family and keeping its contrast. The worst pair of the fifteen is 14 CIE
/// units apart under every one of the four, where two or three is "the same
/// colour in two printings" and ten is "clearly two colours". So a pair
/// anybody can tell apart is a pair everybody can tell apart, which is what
/// was asked for.
///
/// It is 14 and not the 17 the old palette managed, and the three units are
/// the price of the families. A palette free to put its colours anywhere can
/// spread them further than one whose reds must stay red; what the families
/// buy is that the colour now says which pane a node belongs in, which is a
/// thing a reader needs and a thing no amount of separation would have told
/// them.
///
/// The families are told apart by more than hue, because hue is what a
/// dichromacy takes: the payload family is dark (lightness 16 to 49), the
/// vehicle family light (36 to 58), and the ground link's one tint sits where
/// nothing else is. Lightness is the one channel every kind of colour
/// blindness keeps, the total ones included.
///
/// Named by colour, as the earlier palette was, because with one per type the
/// meaning is the type: a name like `I2cTint` would be the enum repeating
/// [`NodeType`].
///
/// No red in the sense that matters: error colouring does not come from this
/// table at all -- see [`ERROR_TINT`] -- which is what keeps the one red in
/// the program out of the ordinary drawing by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tint {
    /// The ground link: an azure on its pale blue pane.
    Azure,
    // The vehicle's own graph: greens and an olive on a pale green pane.
    Pine,
    Olive,
    Gold,
    Moss,
    // What the vehicle carries: plums, maroons and a clay on a pale blush
    // pane, darkest first.
    Aubergine,
    Oxblood,
    Crimson,
    Magenta,
    Mulberry,
    Rose,
    Plum,
    Orchid,
    Heather,
    Clay,
}

/// The ground each working pane is painted, by the pane.
///
/// Light and neutral, as asked: the three are 15 CIE units apart under every
/// one of the four ways of seeing, and none has a chroma above 14, so a pane
/// reads as a tinted sheet of paper rather than as a coloured panel. The nodes
/// drawn on one are darker members of its own hue family -- see [`Tint`] --
/// which is what makes a colour say which pane a node belongs in.
///
/// Here as a meaning and painted in `main.rs`, as the tints are: this file
/// says which pane has which ground, and the window says what that ground is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ground {
    Moc,
    Vehicle,
    Payloads,
}

/// Which ground a pane is painted.
pub fn ground_of(pane: crate::pane::Pane) -> Ground {
    match pane {
        crate::pane::Pane::Moc => Ground::Moc,
        crate::pane::Pane::Vehicle => Ground::Vehicle,
        crate::pane::Pane::Payloads => Ground::Payloads,
    }
}

/// How wide every node is drawn, in pixels.
///
/// One width for all of them, and the reason is alignment: a column of nodes
/// of four different widths has no edge to line up on, so a diagram built from
/// them looks approximate however carefully each node was placed. Height is
/// still per type -- it is one of the channels that tells two types apart --
/// and height alone is enough, as the accessibility test below proves by
/// still passing.
///
/// Wide enough for the longest type name at the largest label size, which is
/// what sets the floor: a label that elides is a label that has stopped being
/// a channel.
pub const WIDTH: u32 = 152;

/// The one red in the program, and the only thing it is used for.
///
/// A node whose configuration is refused, a link with an unbound input, a
/// compute process that will not start. Nothing else.
pub const ERROR_TINT: &str = "#c0392b";

/// A type's terminals, as the canvas needs them.
///
/// The same two lists [`tcslibgs::graph::terminals::Terminals`] holds, and for
/// most types literally those: this is a borrowed copy without the merge rule,
/// which is a checking question rather than a drawing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Terminals {
    pub sinks: &'static [&'static str],
    pub sources: &'static [&'static str],
}

impl Terminals {
    /// Whether this type has the named terminal at either end.
    pub fn has(&self, terminal: &str) -> bool {
        self.sinks.contains(&terminal) || self.sources.contains(&terminal)
    }

    /// Every terminal of this type, sinks and then sources.
    ///
    /// For the tests, which ask questions about all of them at once. The
    /// window asks about each by name, because the markup draws each in its
    /// own corner.
    #[cfg(test)]
    pub fn each(&self) -> impl Iterator<Item = &&'static str> {
        self.sinks.iter().chain(self.sources.iter())
    }
}

/// Where a terminal is drawn, and which way its arrow points.
///
/// Two rows and two columns, and the arrangement is the two streams: the
/// vehicleward one along the top, left to right, arriving from the ground at
/// `from_moc` and leaving for the payload at `to_payload`; the groundward one
/// along the bottom, right to left, arriving from the payload at
/// `from_payload` and leaving for the ground at `to_moc`.
///
/// Which means the arrows all point the way the data actually travels across
/// the canvas, because the canvas is laid out the same way: the ground is the
/// leftmost pane and the payloads the rightmost. A reader who has noticed that
/// once does not have to read the names again.
///
/// `None` for a name that is not one of the four, which is a computation's
/// own input: those are drawn where the link arrives and not as a terminal.
pub fn corner_of(terminal: &str) -> Option<Corner> {
    match terminal {
        // The upper left says nothing: see `Corner::says`.
        "from_moc" => Some(Corner {
            top: true,
            left: true,
            rightward: true,
            says: "",
        }),
        "to_payload" => Some(Corner {
            top: true,
            left: false,
            rightward: true,
            says: "out",
        }),
        "from_payload" => Some(Corner {
            top: false,
            left: false,
            rightward: false,
            says: "in",
        }),
        "to_moc" => Some(Corner {
            top: false,
            left: true,
            rightward: false,
            says: "in",
        }),
        _ => None,
    }
}

/// Which corner of a node a terminal is drawn in, and which way its arrow
/// points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Corner {
    pub top: bool,
    pub left: bool,
    /// Whether the data goes right here, which is to say towards the payloads.
    ///
    /// Not a glyph any more -- see [`Corner::says`] -- but still the direction
    /// a line leaves or arrives by, which is what [`attachment`] needs.
    pub rightward: bool,
    /// What is written in the box: `in`, `out`, or nothing.
    ///
    /// Words rather than arrows. An arrow says which way the data goes, which
    /// a reader has to read against the canvas to make anything of; the word
    /// says what the terminal *is* for the node it is on, which is the
    /// question somebody drawing a line is asking. `out` where the data goes
    /// right, `in` where it goes left.
    ///
    /// Nothing at all in the upper left. That one is `from_moc` -- where what
    /// the ground sends arrives -- and it was the arrow a reader took for "a
    /// line starts here"; the box is still a box and still answers a press,
    /// and what has gone is only what is drawn inside it.
    pub says: &'static str,
}

/// How large a terminal is drawn, in pixels.
///
/// It was 24 by 10 -- wide enough for an arrow and no wider -- and that is a
/// strip a person has to aim at, which is the complaint this answers: a press
/// has to land on a terminal exactly, since the terminal is what says which
/// stream the line carries, where the release only has to land on a node.
/// Twice the area, and both ways about, so the hand has room in whichever
/// direction it is short.
///
/// What bounds it is the smallest node, which is the `null` one at 40 tall and
/// the one with all four: two rows of terminals and their insets have to fit
/// inside that height and leave the rows apart, and the two columns have to
/// fit inside [`WIDTH`] -- see
/// `tests::the_terminals_of_every_type_fit_inside_it`, which is what says so
/// rather than this comment. The name is clear of them either way: a label is
/// padded away from whichever side carries terminals, so the boxes and the
/// name cannot collide however tall they grow.
pub const TERMINAL: (u32, u32) = (36, 14);

/// How far a terminal sits from the corner it is in.
///
/// Inside the node and inside its border, so that the box a person presses and
/// the box a link arrives at are the same box.
pub const TERMINAL_INSET: u32 = 3;

/// Where a terminal is drawn on a node of this size, relative to the node.
///
/// Here rather than in the markup for the reason the link routes are here:
/// geometry is arithmetic, and arithmetic in a layout language cannot be
/// tested. The test below is what says the four of them land in four
/// different corners and clear each other.
pub fn terminal_at(corner: Corner, (width, height): (u32, u32)) -> (i32, i32) {
    let inset = TERMINAL_INSET as i32;
    let far = |whole: u32, box_: u32| whole as i32 - box_ as i32 - inset;
    (
        if corner.left { inset } else { far(width, TERMINAL.0) },
        if corner.top { inset } else { far(height, TERMINAL.1) },
    )
}

/// Which terminal a link end means on a node of this type, for the drawing.
///
/// `named` is what the link wrote, where it wrote one, and `sink` says which
/// end of the link this is. Three answers in order of authority: the terminal
/// the link named, where the type has it; the only terminal of that kind,
/// where there is one; and otherwise the groundward one -- `from_payload`
/// where data arrives and `to_moc` where it leaves -- because telemetry on its
/// way to the ground is what a node does unless the file says otherwise.
///
/// `None` only for a type this build does not know, which is drawn as a node
/// with no terminals at all.
///
/// This is the drawing's question and not the checking's. A link into a
/// computation that named `in0` resolves here to the sink the line should be
/// drawn to; what `in0` *means* is the computation's business, and
/// [`tcslibgs::graph::terminals::terminal_at`] is what the checking asks.
pub fn terminal_for(node_type: &str, named: Option<&str>, sink: bool) -> Option<&'static str> {
    let terminals = NodeType::ALL
        .iter()
        .find(|t| t.spelling() == node_type)?
        .terminals();
    let ours = if sink {
        terminals.sinks
    } else {
        terminals.sources
    };

    if let Some(named) = named {
        if let Some(found) = ours.iter().find(|terminal| **terminal == named) {
            return Some(found);
        }
    }
    match ours {
        [only] => Some(only),
        _ => {
            let groundward = if sink { "from_payload" } else { "to_moc" };
            ours.iter().find(|terminal| **terminal == groundward).copied()
        }
    }
}

/// Where a line joins a node at this terminal, and which way it points there.
///
/// The outer edge of the terminal's own box, halfway down it: a line that
/// stopped at the node's edge beside the box would read as a line that misses
/// it. The direction is the box's own arrow -- a terminal on the left side
/// faces left -- which is what makes the line, the arrowhead and the arrow
/// drawn inside the terminal all say the same thing.
///
/// `at` is where the node is. `None` for a type this build does not know, or a
/// name that is not one of the four.
pub fn attachment(
    node_type: &str,
    at: (i32, i32),
    terminal: &str,
) -> Option<((f32, f32), bool)> {
    let found = NodeType::ALL.iter().find(|t| t.spelling() == node_type)?;
    let corner = corner_of(terminal)?;
    let (width, height) = found.appearance().size;
    let (_, y) = terminal_at(corner, (width, height));

    let edge = if corner.left {
        at.0 as f32
    } else {
        (at.0 + width as i32) as f32
    };
    Some((
        (edge, at.1 as f32 + y as f32 + TERMINAL.1 as f32 / 2.0),
        corner.rightward,
    ))
}

/// The box a terminal is drawn in, relative to the node.
///
/// For the tests, which let a line go of over a terminal because that is where
/// a person naturally aims. The gesture itself does not ask: a line is drawn
/// from the terminal that was pressed, and where it lands is a question about
/// the node under the pointer and not about these boxes -- see `landed` in
/// `main.rs`, which is what made aiming at them unnecessary.
///
/// `None` for a type this build does not know or a name that is not one of the
/// four.
#[cfg(test)]
pub fn terminal_box(
    node_type: &str,
    at: (i32, i32),
    terminal: &str,
) -> Option<(f32, f32, f32, f32)> {
    let found = NodeType::ALL.iter().find(|t| t.spelling() == node_type)?;
    let corner = corner_of(terminal)?;
    let (x, y) = terminal_at(corner, found.appearance().size);
    Some((
        (at.0 + x) as f32,
        (at.1 + y) as f32,
        TERMINAL.0 as f32,
        TERMINAL.1 as f32,
    ))
}

/// How much of a node's width a terminal takes up, with room to spare.
///
/// What the label is padded by on whichever side carries terminals: the name
/// stays in the middle of the node and stops short of the corner boxes, rather
/// than a row being reserved at the top and the bottom -- which would leave
/// a few pixels for a name on the shortest node.
pub const TERMINAL_COLUMN: u32 = TERMINAL.0 + 2 * TERMINAL_INSET;

/// Every node type the editor can place.
///
/// The order is the order the palette shows them in: the devices a vehicle has,
/// then what happens to their data, then where it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    I2c,
    Spi,
    Serial,
    /// A synchronous serial line, which Linux drives as a network interface.
    Hdlc,
    Can,
    Iio,
    Rpmsg,
    Network,
    DeviceFile,
    Gpio,
    Compute,
    /// A vehicle node that carries both streams through and does nothing to
    /// what passes. The one type with all four terminals, which is what makes
    /// it the statement of what a vehicle node is.
    Null,
    Log,
    Moc,
    /// A diagram of its own, kept in its own file and shown here as one box.
    Subdiagram,
}

impl NodeType {
    /// Every type, for the palette and for the tests.
    pub const ALL: [NodeType; 15] = [
        NodeType::I2c,
        NodeType::Spi,
        NodeType::Serial,
        NodeType::Hdlc,
        NodeType::Can,
        NodeType::Iio,
        NodeType::Rpmsg,
        NodeType::Network,
        NodeType::DeviceFile,
        NodeType::Gpio,
        NodeType::Compute,
        NodeType::Null,
        NodeType::Log,
        NodeType::Moc,
        NodeType::Subdiagram,
    ];

    /// The word the configuration file uses, which is the word the editor
    /// shows: an operator reading the file and an operator reading the canvas
    /// should not have to translate.
    pub fn spelling(self) -> &'static str {
        match self {
            NodeType::I2c => "i2c",
            NodeType::Spi => "spi",
            NodeType::Serial => "serial",
            NodeType::Hdlc => "hdlc",
            NodeType::Can => "can",
            NodeType::Iio => "iio",
            NodeType::Rpmsg => "rpmsg",
            NodeType::Network => "network",
            NodeType::DeviceFile => "device-file",
            NodeType::Gpio => "gpio",
            NodeType::Compute => "compute",
            NodeType::Null => "null",
            NodeType::Log => "log",
            NodeType::Moc => "moc",
            NodeType::Subdiagram => "subdiagram",
        }
    }

    /// What a node of this type says about itself.
    ///
    /// The attributes the configuration gives each type, in the order the
    /// manual lists them, with the ones that must be stated marked. Two things
    /// come of having them written down rather than inferred from what a node
    /// happens to hold:
    ///
    /// the editor can show a node's fields before any of them are filled in,
    /// so a person placing an I2C device is told it needs a bus and an address
    /// rather than having to know; and a required attribute left empty is a
    /// fault the editor can mark, which is the same check `tcsverify` makes on
    /// the ground and tcspecial makes again aboard.
    ///
    /// Not exhaustive for `compute`, whose parameters depend on which function
    /// it runs: those are whatever the function wants, so the editor takes
    /// what it is given there and the validation belongs to the function.
    pub fn attributes(self) -> &'static [(&'static str, bool)] {
        match self {
            NodeType::I2c => &[
                ("path", true),
                ("address", true),
                ("group", false),
                ("trigger", false),
                ("read_size", false),
                ("ten_bit", false),
                ("pec", false),
                ("retries", false),
                ("timeout", false),
            ],
            NodeType::Spi => &[
                ("path", true),
                ("max_speed", true),
                ("spi_mode", true),
                ("group", false),
                ("trigger", false),
                ("read_size", false),
                ("bits_per_word", false),
                ("bit_order", false),
                ("cs_active", false),
            ],
            NodeType::Serial => &[
                ("path", true),
                ("datarate", true),
                ("byte_length", true),
                ("stop_bits", true),
                ("parity", false),
                ("group", false),
                ("trigger", false),
            ],
            // An interface, not a path: see tcslibgs::node::payload::hdlc. The
            // standard and the clock are required because a line carries no
            // negotiation; the rate is required only where this end
            // generates the clock, which the palette cannot say, so it is
            // shown as optional and the checking has the rule.
            NodeType::Hdlc => &[
                ("interface", true),
                ("standard", true),
                ("clock_type", true),
                ("clock_rate", false),
                ("encoding", false),
                ("frame_check", false),
                ("loopback", false),
                ("group", false),
                ("trigger", false),
            ],
            NodeType::Can => &[
                ("interface", true),
                ("can_id", true),
                ("can_fd", false),
                ("loopback", false),
                ("group", false),
                ("trigger", false),
            ],
            NodeType::Iio => &[
                ("path", true),
                ("group", false),
                ("trigger", false),
            ],
            NodeType::Rpmsg => &[
                ("path", true),
                ("service", true),
                ("group", false),
                ("trigger", false),
            ],
            NodeType::Network => &[
                ("protocol", true),
                ("address", true),
                ("port", true),
                ("group", false),
                ("trigger", false),
            ],
            NodeType::DeviceFile => &[
                ("path", true),
                ("group", false),
                ("trigger", false),
            ],
            NodeType::Gpio => &[
                ("path", true),
                ("lines", true),
                ("direction", false),
                ("edge", false),
                ("bias", false),
                ("debounce_us", false),
                ("group", false),
                ("trigger", false),
            ],
            NodeType::Compute => &[
                ("function", true),
                ("inputs", false),
                ("program", false),
                ("timeout", false),
                ("restart", false),
            ],
            // Nothing at all, and it is the one type with nothing. A null
            // node carries what arrives through: its name and its type are
            // the whole of what there is to say, and the configuration check
            // refuses any attribute written on one.
            NodeType::Null => &[],
            NodeType::Log => &[("directory", true), ("format", true)],
            NodeType::Moc => &[("address", true), ("port", true)],
            NodeType::Subdiagram => &[("file", true)],
        }
    }

    /// Which terminals a node of this type has, as the canvas draws them.
    ///
    /// The four names and what each one is belong to
    /// [`tcslibgs::graph::terminals`], and for every type that module has an
    /// answer for -- a payload kind, the ground link, a `null` vehicle node --
    /// this reads it rather than repeating it: a terminal added there appears
    /// on the canvas without this file being touched, and the two cannot
    /// disagree about what a payload has.
    ///
    /// The rest are the types whose terminals that module calls their own
    /// business. The canvas has to draw something, because a terminal is the
    /// only thing a link can be dragged from or to -- a node with none drawn
    /// is a node nothing can be linked to -- and what is drawn is this:
    ///
    /// * a computation may be on either stream in either direction. The
    ///   readings arrive at it, its result goes to the ground, and the shipped
    ///   thermostat drives a line with it -- so it has all four. Its named
    ///   inputs, `in0` and the rest, are a mean's own affair and are not
    ///   terminals: a line dragged to a computation that declares inputs fills
    ///   the next free one, and no terminal is written;
    /// * a log is written to and read by nobody aboard, so it has the one
    ///   arriving terminal and no leaving one. Nothing can be dragged out of a
    ///   log, which is the rule the flight software enforces anyway;
    /// * a page of its own has all four, because what is inside it may have
    ///   any of them and nothing out here knows which.
    ///
    /// What is *drawn* and what is *written* are not the same question. A
    /// terminal is written into the file only where the type has more than one
    /// of that kind -- see [`NodeType::must_name`] -- so the file keeps saying
    /// `{ from: temp_fore, to: moc }`, which is what the schema reads it as
    /// anyway.
    pub fn terminals(self) -> Terminals {
        // What tcslibgs says, where it says anything.
        if let Some(terminals) = tcslibgs::graph::terminals::terminals_of(self.spelling()) {
            return Terminals {
                sinks: terminals.sinks,
                sources: terminals.sources,
            };
        }
        match self {
            NodeType::Log => Terminals {
                sinks: &["from_payload"],
                sources: &[],
            },
            // A computation and a page: both ends of both streams.
            _ => Terminals {
                sinks: &["from_payload", "from_moc"],
                sources: &["to_moc", "to_payload"],
            },
        }
    }

    /// Whether a link leaving or arriving here has to say which terminal.
    ///
    /// Only where the type has more than one of that kind, which is the
    /// schema's own rule for what a link end may leave unwritten -- and the
    /// rule that keeps the editor from writing a name into every link in every
    /// file it saves. `sink` says which end: `true` for where data arrives.
    ///
    /// Asked of [`NodeType::terminals`], which is the table the canvas drew
    /// from: the question is about the gesture, and what the gesture pointed at
    /// is one of the boxes on the screen. Asking the schema's table instead
    /// threw away what a person had just said -- a computation has two data
    /// sinks drawn on it and the schema calls its sinks its own business, so a
    /// line dragged to the `from_moc` in its upper left was recorded as a link
    /// naming no terminal, and then drawn arriving at the other corner.
    pub fn must_name(self, sink: bool) -> bool {
        let terminals = self.terminals();
        let how_many = if sink {
            terminals.sinks.len()
        } else {
            terminals.sources.len()
        };
        how_many > 1
    }

    /// Which family this type belongs to.    /// Which family this type belongs to.
    ///
    /// The families are the shape grammar: square corners are hardware,
    /// rounded are software aboard, a circle is an end of the graph, a keyed
    /// rectangle is an interface that is driven, and a page is a diagram of
    /// its own. The palette shows them under these headings, which is how the
    /// grammar gets taught -- a flat list of thirteen shapes hides the pattern
    /// that would make them learnable.
    pub fn family(self) -> &'static str {
        match self {
            NodeType::I2c
            | NodeType::Spi
            | NodeType::Serial
            | NodeType::Hdlc
            | NodeType::Can
            | NodeType::Iio
            | NodeType::Rpmsg
            | NodeType::Network
            | NodeType::DeviceFile => "Devices",
            NodeType::Gpio => "Driven",
            NodeType::Compute => "Computation",
            NodeType::Null => "Pass-through",
            NodeType::Log | NodeType::Moc => "Ends of the graph",
            NodeType::Subdiagram => "Pages",
        }
    }

    /// Whether this type is a payload.
    ///
    /// A payload is something the vehicle carries and reads: a device on a
    /// bus, a line it drives, a link to something aboard. The rest of the
    /// graph is not -- a computation, a log, the ground station, a page of its
    /// own are all parts of the telemetry and control system itself rather
    /// than things bolted to the vehicle.
    ///
    /// The editor keeps the two apart on the canvas: payloads live in the
    /// payload working pane and everything else in the vehicle working pane,
    /// which is what makes a diagram readable as "here is the vehicle, and
    /// here is what it is carrying this flight". The flight is the thing that
    /// changes between missions, and a drawing that puts the changing part in
    /// one place is a drawing that can be compared with last mission's.
    ///
    /// Listed rather than derived from [`NodeType::family`] so that a new type
    /// has to say which side it is on. The test below checks the two agree.
    pub fn is_payload(self) -> bool {
        match self {
            NodeType::I2c
            | NodeType::Spi
            | NodeType::Serial
            | NodeType::Hdlc
            | NodeType::Can
            | NodeType::Iio
            | NodeType::Rpmsg
            | NodeType::Network
            | NodeType::DeviceFile
            | NodeType::Gpio => true,
            NodeType::Compute
            | NodeType::Null
            | NodeType::Log
            | NodeType::Moc
            | NodeType::Subdiagram => false,
        }
    }

    /// How this type is drawn.
    ///
    /// The groupings are meant to be learnable rather than arbitrary: square
    /// corners are hardware, rounded corners are software, circles are ends of
    /// the graph, and a keyed rectangle is an interface that is driven. Within
    /// a shape the types differ by shading, weight and size.
    pub fn appearance(self) -> Appearance {
        use Outline::*;
        use Shading::*;
        use Weight::*;
        match self {
            // Buses: square, and told apart by shading and size.
            NodeType::I2c => Appearance {
                face: Face::Mono,
                label_size: 12,
                label_weight: 700,
                label_italic: false,
                outline: Square,
                shading: Solid,
                weight: Narrow,
                size: (WIDTH, 48),
                tint: Tint::Aubergine,
            },
            NodeType::Spi => Appearance {
                face: Face::Mono,
                label_size: 12,
                label_weight: 400,
                label_italic: false,
                outline: Square,
                shading: Hatched,
                weight: Narrow,
                size: (WIDTH, 56),
                tint: Tint::Oxblood,
            },
            NodeType::Serial => Appearance {
                face: Face::Mono,
                label_size: 10,
                label_weight: 400,
                label_italic: true,
                outline: Square,
                shading: Open,
                weight: Narrow,
                size: (WIDTH, 48),
                tint: Tint::Crimson,
            },
            // Wide, double-outlined and in the sand tint, with a plain face
            // at the largest label size: a square outline because it is
            // hardware, and nothing else here is sand and double at once.
            NodeType::Hdlc => Appearance {
                face: Face::Plain,
                label_size: 14,
                label_weight: 700,
                label_italic: false,
                outline: Square,
                shading: Double,
                weight: Wide,
                size: (WIDTH, 56),
                tint: Tint::Magenta,
            },
            NodeType::Can => Appearance {
                face: Face::Mono,
                label_size: 14,
                label_weight: 700,
                label_italic: false,
                outline: Square,
                shading: Double,
                weight: Narrow,
                size: (WIDTH, 64),
                tint: Tint::Mulberry,
            },
            NodeType::Iio => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 400,
                label_italic: true,
                outline: Square,
                shading: Hatched,
                weight: Wide,
                size: (WIDTH, 56),
                tint: Tint::Rose,
            },
            NodeType::Rpmsg => Appearance {
                face: Face::Mono,
                label_size: 10,
                label_weight: 700,
                label_italic: true,
                outline: Square,
                shading: Solid,
                weight: Wide,
                size: (WIDTH, 48),
                tint: Tint::Plum,
            },
            NodeType::Network => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 700,
                label_italic: false,
                outline: Square,
                shading: Open,
                weight: Wide,
                size: (WIDTH, 56),
                tint: Tint::Orchid,
            },
            NodeType::DeviceFile => Appearance {
                face: Face::Mono,
                label_size: 10,
                label_weight: 400,
                label_italic: false,
                outline: Square,
                shading: Double,
                weight: Wide,
                size: (WIDTH, 64),
                tint: Tint::Heather,
            },
            // Driven rather than read, and shaped to say so.
            NodeType::Gpio => Appearance {
                face: Face::Plain,
                label_size: 14,
                label_weight: 700,
                label_italic: false,
                outline: Keyed,
                shading: Solid,
                weight: Wide,
                size: (WIDTH, 72),
                tint: Tint::Clay,
            },
            // Software aboard.
            // The one node that does nothing to what passes: rounded, because
            // it is software aboard, and then open, narrow and the shortest
            // thing on the canvas. A shape with the least in it for the node
            // with the least in it -- and a height nothing else uses, which is
            // what tells it from the other two rounded shapes without colour.
            NodeType::Null => Appearance {
                face: Face::Plain,
                label_size: 10,
                label_weight: 400,
                label_italic: false,
                outline: Rounded,
                shading: Open,
                weight: Narrow,
                size: (WIDTH, 40),
                tint: Tint::Olive,
            },
            NodeType::Compute => Appearance {
                face: Face::Plain,
                label_size: 14,
                label_weight: 400,
                label_italic: false,
                outline: Rounded,
                shading: Solid,
                weight: Wide,
                size: (WIDTH, 64),
                tint: Tint::Pine,
            },
            // A diagram of its own. Doubly outlined and the tallest thing on
            // the canvas bar the two ends of the graph, because it stands for
            // a page rather than a device: the two outlines read as a box
            // holding a box, which is what it is.
            NodeType::Subdiagram => Appearance {
                face: Face::Plain,
                label_size: 14,
                label_weight: 700,
                label_italic: false,
                outline: Rounded,
                shading: Double,
                weight: Wide,
                size: (WIDTH, 88),
                tint: Tint::Moss,
            },
            // Ends of the graph.
            NodeType::Log => Appearance {
                face: Face::Plain,
                label_size: 12,
                label_weight: 400,
                label_italic: false,
                outline: Circle,
                shading: Hatched,
                weight: Narrow,
                size: (WIDTH, 96),
                tint: Tint::Gold,
            },
            NodeType::Moc => Appearance {
                face: Face::Plain,
                label_size: 16,
                label_weight: 700,
                label_italic: true,
                outline: Circle,
                shading: Solid,
                weight: Wide,
                size: (WIDTH, 128),
                tint: Tint::Azure,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No two node types are told apart by colour alone.
    ///
    /// The rule this module exists for, and the reason it is a test rather
    /// than a sentence in a comment: a tenth node type added later with a new
    /// colour and a borrowed shape would satisfy every reviewer and nobody
    /// with deuteranopia. Two channels must differ, not one, so that losing a
    /// channel to a projector or a printer still leaves a distinction.
    #[test]
    fn every_pair_of_types_differs_in_two_things_that_are_not_colour() {
        for (a, b) in NodeType::ALL.iter().enumerate().flat_map(|(i, a)| {
            NodeType::ALL[i + 1..].iter().map(move |b| (*a, *b))
        }) {
            let one = a.appearance();
            let two = b.appearance();
            let differences = [
                one.outline != two.outline,
                one.shading != two.shading,
                one.weight != two.weight,
                one.size != two.size,
                // How the label is set, which is one channel however many
                // properties carry it: a face, a size, a weight and an italic
                // are all type, and counting them separately would let four
                // typographic tweaks stand in for a shape.
                (one.face, one.label_size, one.label_weight, one.label_italic)
                    != (two.face, two.label_size, two.label_weight, two.label_italic),
            ]
            .iter()
            .filter(|differs| **differs)
            .count();

            assert!(
                differences >= 2,
                "{} and {} differ in {differences} ways that are not colour: \
                 {one:?} against {two:?}",
                a.spelling(),
                b.spelling()
            );
        }
    }

    /// Every type is drawn differently from every other, colour included.
    ///
    /// Weaker than the rule above and worth stating separately: this one would
    /// pass for two types that differ only in colour, which is why it is not
    /// the only test here.
    #[test]
    fn no_two_types_look_the_same() {
        for (i, a) in NodeType::ALL.iter().enumerate() {
            for b in &NodeType::ALL[i + 1..] {
                assert_ne!(
                    a.appearance(),
                    b.appearance(),
                    "{} and {} are drawn identically",
                    a.spelling(),
                    b.spelling()
                );
            }
        }
    }

    /// Every type has its own tint.
    ///
    /// Asked for, and worth a test rather than a glance down the table:
    /// fifteen entries written one at a time is fifteen chances to reach for
    /// the one above. Colour remains the redundant channel -- the rule at the
    /// top of this file is what tells types apart -- so this is not what makes
    /// the palette usable, only what makes it complete.
    #[test]
    fn no_two_types_share_a_tint() {
        for (i, a) in NodeType::ALL.iter().enumerate() {
            for b in &NodeType::ALL[i + 1..] {
                assert_ne!(
                    a.appearance().tint,
                    b.appearance().tint,
                    "{} and {} are the same colour",
                    a.spelling(),
                    b.spelling()
                );
            }
        }
    }

    /// Every terminal a type has is drawn somewhere, and in the right corner.
    ///
    /// The arrangement is the argument: the vehicleward stream along the top
    /// going right, the groundward stream along the bottom going left, so
    /// every arrow points the way the data travels across a canvas whose
    /// leftmost pane is the ground and whose rightmost is the payloads. A
    /// terminal in the wrong corner would make the drawing say the opposite of
    /// the file.
    #[test]
    fn every_terminal_is_drawn_in_the_corner_its_stream_runs_through() {
        for t in NodeType::ALL {
            for terminal in t.terminals().each() {
                let corner = corner_of(terminal)
                    .unwrap_or_else(|| panic!("{}: {terminal} has no corner", t.spelling()));
                // The top row is what the ground sends and the bottom row is
                // what comes back, and each row runs one way.
                let vehicleward = *terminal == "from_moc" || *terminal == "to_payload";
                assert_eq!(corner.top, vehicleward, "{terminal}");
                assert_eq!(corner.rightward, vehicleward, "{terminal}");
                // Arriving on the side the data comes from: the ground is to
                // the left of the vehicle and the payloads to its right.
                let from_the_left = *terminal == "from_moc" || *terminal == "to_moc";
                assert_eq!(corner.left, from_the_left, "{terminal}");
            }
        }
        // And a computation's own input is not a terminal: it is drawn where
        // the link arrives, with its name on it.
        assert_eq!(corner_of("in0"), None);
    }

    /// Every terminal a type has fits inside it, clear of the others.
    ///
    /// What bounds [`TERMINAL`]. The `null` node is 40 pixels tall and carries
    /// all four, so growing a terminal is growing it against that: two rows
    /// have to fit inside the height with daylight between them, and two
    /// columns inside the width. A box that overran would be a box hanging
    /// outside its node -- pressable where nothing is drawn -- or two boxes
    /// lying on top of each other, where a press means whichever happens to be
    /// on top.
    #[test]
    fn the_terminals_of_every_type_fit_inside_it() {
        let inset = TERMINAL_INSET as i32;
        let (box_width, box_height) = (TERMINAL.0 as i32, TERMINAL.1 as i32);

        for t in NodeType::ALL {
            let (width, height) = t.appearance().size;
            let mut boxes: Vec<(i32, i32)> = Vec::new();
            for terminal in t.terminals().each() {
                let corner = corner_of(terminal).expect("a corner");
                let at = terminal_at(corner, (width, height));
                // Inside the node, with its inset on every side.
                assert!(at.0 >= inset && at.1 >= inset, "{terminal}: {at:?}");
                assert!(
                    at.0 + box_width <= width as i32 - inset
                        && at.1 + box_height <= height as i32 - inset,
                    "{}: {terminal} at {at:?} overruns {width}x{height}",
                    t.spelling()
                );
                boxes.push(at);
            }

            // And clear of each other, which is the rule the smallest node is
            // measured against.
            for (one, at) in boxes.iter().enumerate() {
                for theirs in boxes.iter().skip(one + 1) {
                    let over = at.0 < theirs.0 + box_width
                        && theirs.0 < at.0 + box_width
                        && at.1 < theirs.1 + box_height
                        && theirs.1 < at.1 + box_height;
                    assert!(
                        !over,
                        "{}: terminals at {at:?} and {theirs:?} overlap",
                        t.spelling()
                    );
                }
            }
        }
    }

    /// A box says `in` or `out`, and the upper-left one says nothing.
    ///
    /// Words rather than arrows, because an arrow has to be read against the
    /// canvas before it says anything, where the word says what the terminal
    /// is for the node it is on. `out` where the data goes right, `in` where
    /// it goes left -- so the two always agree with `rightward`, which is what
    /// this asserts rather than repeating the table.
    ///
    /// Nothing in the upper left. That one is `from_moc`, and its arrow was
    /// the one a reader took for "a line starts here" -- the mistake that had
    /// somebody dragging from the corner of a moc node, where there is no
    /// terminal at all.
    #[test]
    fn a_terminal_says_in_or_out_and_the_upper_left_says_nothing() {
        assert_eq!(corner_of("from_moc").expect("a corner").says, "");
        for terminal in ["to_payload", "from_payload", "to_moc"] {
            let corner = corner_of(terminal).expect("a corner");
            assert_eq!(
                corner.says,
                if corner.rightward { "out" } else { "in" },
                "{terminal}"
            );
        }
    }

    /// The editor draws the terminals the schema says a type has.
    ///
    /// Read from `tcslibgs` rather than copied, so this is a test that the
    /// reading happens: a terminal added to the schema has to appear on the
    /// canvas without anybody remembering to come here.
    #[test]
    fn the_canvas_shows_the_terminals_the_schema_gives_a_type() {
        for t in NodeType::ALL {
            let Some(schema) = tcslibgs::graph::terminals::terminals_of(t.spelling()) else {
                continue;
            };
            assert_eq!(t.terminals().sinks, schema.sinks, "{}", t.spelling());
            assert_eq!(t.terminals().sources, schema.sources, "{}", t.spelling());
        }

        // A payload is the right-hand end of both streams, so both of its
        // terminals are drawn on its left; the ground is the left-hand end,
        // and both of its are on its right. Spelled out here because it is
        // the thing a reader of the canvas sees first.
        for t in NodeType::ALL.iter().filter(|t| t.is_payload()) {
            for terminal in t.terminals().each() {
                assert!(corner_of(terminal).unwrap().left, "{terminal}");
            }
        }
        for terminal in NodeType::Moc.terminals().each() {
            assert!(!corner_of(terminal).unwrap().left, "{terminal}");
        }
    }

    /// The types the schema leaves open are drawn as what they are.
    ///
    /// Every one of them has something to drag a link from or to, because a
    /// terminal is now the only thing a link can be dragged from or to: a node
    /// drawn without one is a node nothing can be linked to. A log is the
    /// exception that proves it -- nothing is read from a log, so it has no
    /// data source and nothing can be dragged out of it, which is the rule the
    /// flight software enforces anyway.
    #[test]
    fn every_type_has_something_to_link_to() {
        for t in NodeType::ALL {
            assert!(
                !t.terminals().sinks.is_empty() || !t.terminals().sources.is_empty(),
                "{} has no terminal, so nothing can be linked to it",
                t.spelling()
            );
        }

        assert!(NodeType::Log.terminals().has("from_payload"));
        assert!(
            NodeType::Log.terminals().sources.is_empty(),
            "nothing is read from a log"
        );
        // A computation may be on either stream in either direction, and so
        // may a page: what is inside a page may have any of the four.
        assert_eq!(NodeType::Compute.terminals().each().count(), 4);
        assert_eq!(NodeType::Subdiagram.terminals().each().count(), 4);
    }

    /// A terminal is written into a file only where it has to be.
    ///
    /// Which is where the type has two of that kind, so that the file says
    /// which of them a link means. A payload and the ground link have one of
    /// each and their links name neither; a `null` node, a computation and a
    /// page have two of each, and theirs do.
    ///
    /// Asked of the table the canvas draws from, because the question is about
    /// a gesture: what somebody dragged a line to is one of the boxes on the
    /// screen, and a type whose terminals the schema leaves open still has
    /// them drawn.
    #[test]
    fn a_terminal_is_written_only_where_it_says_which() {
        for t in [NodeType::Null, NodeType::Compute, NodeType::Subdiagram] {
            assert!(t.must_name(true), "{}", t.spelling());
            assert!(t.must_name(false), "{}", t.spelling());
        }
        // One data sink and one data source: nothing to say which.
        for t in NodeType::ALL.iter().filter(|t| t.is_payload()) {
            assert!(!t.must_name(true), "{}", t.spelling());
            assert!(!t.must_name(false), "{}", t.spelling());
        }
        assert!(!NodeType::Moc.must_name(true));
        assert!(!NodeType::Moc.must_name(false));
        // A log receives on one and sends on none.
        assert!(!NodeType::Log.must_name(true));
        assert!(!NodeType::Log.must_name(false));
    }

    /// Which terminal a line is drawn to, where the link does not say.
    #[test]
    fn a_line_is_drawn_to_the_terminal_the_link_means() {
        // Named, and the type has it.
        assert_eq!(
            terminal_for("null", Some("from_moc"), true),
            Some("from_moc")
        );
        // Named, and the type has not: a computation's own input, which is
        // not a terminal. The groundward sink is where readings arrive.
        assert_eq!(
            terminal_for("compute", Some("in0"), true),
            Some("from_payload")
        );
        // Nothing named, and one of that kind: the one.
        assert_eq!(terminal_for("i2c", None, false), Some("to_moc"));
        assert_eq!(terminal_for("moc", None, true), Some("from_payload"));
        // Nothing named and two of that kind: the groundward one, which is
        // what a node does unless the file says otherwise. The checking
        // refuses such a file; the canvas still has to draw it.
        assert_eq!(terminal_for("null", None, false), Some("to_moc"));
        assert_eq!(terminal_for("null", None, true), Some("from_payload"));
        // Nothing to read from a log, and nothing known about a type this
        // build does not have.
        assert_eq!(terminal_for("log", None, false), None);
        assert_eq!(terminal_for("contraption", None, true), None);
    }

    /// A line joins a node at the outer edge of the terminal's own box.
    #[test]
    fn a_line_joins_a_node_at_its_terminals_edge() {
        let (width, height) = NodeType::I2c.appearance().size;
        let at = (400, 200);

        // The groundward pair of a payload, both on its left: the readings
        // leave at the bottom going left, what asks for them arrives at the
        // top going right.
        let ((x, y), rightward) = attachment("i2c", at, "to_moc").expect("a payload");
        assert_eq!(x, at.0 as f32, "a terminal on the left joins the left edge");
        assert!(!rightward, "the readings leave towards the ground");
        assert!(y > at.1 as f32 + height as f32 / 2.0, "the lower corner");

        let ((_, top), rightward) = attachment("i2c", at, "from_moc").expect("a payload");
        assert!(rightward && top < y, "what asks arrives at the upper corner");

        // And the ground link's pair, both on its right.
        let ((x, _), _) = attachment("moc", at, "from_payload").expect("the ground");
        let (moc_width, _) = NodeType::Moc.appearance().size;
        assert_eq!(x, (at.0 + moc_width as i32) as f32);

        // A terminal a node does not have is not a place to join it, and
        // neither is a name that is not a terminal at all.
        assert_eq!(attachment("i2c", at, "in0"), None);
        assert_eq!(attachment("contraption", at, "to_moc"), None);
        let _ = width;
    }

    /// Red is not in the palette at all.    /// Red is not in the palette at all.
    ///
    /// Checked against the names rather than against values, because the
    /// palette holds meanings and the window holds the colours: a `Tint` that
    /// meant red would be the failure, whatever hex the window gave it.
    #[test]
    fn no_node_type_is_red() {
        for t in NodeType::ALL {
            let tint = t.appearance().tint;
            assert_ne!(
                format!("{tint:?}").to_lowercase(),
                "red",
                "{} is tinted red, which is reserved for errors",
                t.spelling()
            );
        }
    }

    /// Every label is set legibly.
    ///
    /// Type is a distinguishing channel, which makes it tempting to reach for
    /// a six-pixel italic to tell two things apart. A distinction nobody can
    /// read is not one, so there is a floor.
    #[test]
    fn no_label_is_too_small_to_read() {
        for t in NodeType::ALL {
            let a = t.appearance();
            assert!(
                a.label_size >= 10,
                "{} sets its label at {}px",
                t.spelling(),
                a.label_size
            );
            assert!(
                a.label_weight == 400 || a.label_weight == 700,
                "{} asks for a weight no system face is sure to have: {}",
                t.spelling(),
                a.label_weight
            );
        }
    }

    /// Every node is drawn the same width.
    ///
    /// Alignment is the point: nodes of four widths in a column have no edge
    /// to line up on, and a diagram of them looks approximate however
    /// carefully it was laid out. A test rather than a comment because the
    /// next type added will be written by copying one of these, and the one it
    /// gets copied from is whichever was most convenient.
    #[test]
    fn every_type_is_drawn_the_same_width() {
        for t in NodeType::ALL {
            assert_eq!(
                t.appearance().size.0,
                WIDTH,
                "{} is {}px wide and everything else is {WIDTH}px",
                t.spelling(),
                t.appearance().size.0
            );
        }
    }

    /// A width that holds the longest name it has to hold.
    ///
    /// The label is a channel only while it can be read: the type name is
    /// shown under every node, and one that elides to "device-fi..." is a
    /// distinction that has stopped working. Measured generously -- a little
    /// over half the label size per character, which no system face exceeds
    /// for lower case -- because the point is a floor and not a layout.
    #[test]
    fn the_width_holds_the_longest_type_name() {
        for t in NodeType::ALL {
            let wanted = t.spelling().len() as f32 * 7.0 + 8.0;
            assert!(
                WIDTH as f32 >= wanted,
                "{} wants about {wanted:.0}px and a node is {WIDTH}px",
                t.spelling()
            );
        }
    }

    /// What is a payload and what is not, said twice and agreeing.
    ///
    /// [`NodeType::is_payload`] is written out type by type so that a new type
    /// has to declare which side of the canvas it belongs on. That leaves two
    /// statements of the same fact, and two statements of one fact drift: this
    /// is what stops them. The families are the shape grammar, and the grammar
    /// already divides what the vehicle carries from what the system is made
    /// of.
    #[test]
    fn a_payload_is_a_device_or_a_driven_line() {
        for t in NodeType::ALL {
            let by_family = matches!(t.family(), "Devices" | "Driven");
            assert_eq!(
                t.is_payload(),
                by_family,
                "{} is in the {} family and says is_payload() is {}",
                t.spelling(),
                t.family(),
                t.is_payload()
            );
        }

        // And both panes have something to offer: an empty selection pane is
        // a pane nobody can tell the purpose of.
        assert!(NodeType::ALL.iter().any(|t| t.is_payload()));
        assert!(NodeType::ALL.iter().any(|t| !t.is_payload()));
    }

    /// Every type says what it needs, and needs something.
    ///
    /// A type with no required attribute is a node that can be placed and
    /// left meaning nothing -- which is a configuration the flight software
    /// would refuse for reasons the editor could have given on the spot.
    #[test]
    fn every_type_states_what_it_requires() {
        for t in NodeType::ALL {
            let attributes = t.attributes();
            // A null node is the one type with nothing to say: it carries
            // what arrives through, so its name and its type are the whole of
            // it, and the configuration check refuses any attribute written
            // on one. Every other type that said nothing would be a node that
            // means nothing.
            if t == NodeType::Null {
                assert!(attributes.is_empty(), "a null node says nothing");
                continue;
            }
            assert!(!attributes.is_empty(), "{} says nothing", t.spelling());
            assert!(
                attributes.iter().any(|(_, required)| *required),
                "{} requires nothing, so an empty one would be a node that \
                 means nothing",
                t.spelling()
            );

            // Required ones first, so the panel asks for what it must have
            // before what it may.
            let first_optional = attributes.iter().position(|(_, r)| !*r);
            if let Some(at) = first_optional {
                assert!(
                    attributes[at..].iter().all(|(_, r)| !*r),
                    "{}'s required attributes are not all first",
                    t.spelling()
                );
            }

            // And no attribute twice, which would give the panel two fields
            // writing to one value.
            let mut names: Vec<&str> = attributes.iter().map(|(n, _)| *n).collect();
            names.sort_unstable();
            let before = names.len();
            names.dedup();
            assert_eq!(before, names.len(), "{} lists one twice", t.spelling());
        }
    }

    /// Every label size is a step on the scale.
    ///
    /// The sizes were 9, 10, 11, 12, 13 and 15 pixels, each chosen when the
    /// type it belonged to was added. Four steps is enough to tell thirteen
    /// types apart -- the accessibility test above proves it, since it still
    /// passes -- and a window built from a scale looks composed where one
    /// built from whatever was wanted at the time looks assembled.
    #[test]
    fn every_label_size_is_on_the_scale() {
        const SCALE: [u32; 4] = [10, 12, 14, 16];
        for t in NodeType::ALL {
            let size = t.appearance().label_size;
            assert!(
                SCALE.contains(&size),
                "{} is set at {size}px, which is not a step on {SCALE:?}",
                t.spelling()
            );
        }
    }

    /// A label fits the shape it is in.
    ///
    /// A type is only a channel while the text is inside the box: a fifteen
    /// pixel label in a forty-eight pixel node leaves no room for the type
    /// name under it, and a label that overflows is a label that gets elided
    /// to nothing.
    #[test]
    fn every_label_fits_its_node() {
        for t in NodeType::ALL {
            let a = t.appearance();
            // The name, the type beneath it, the padding, and a little room:
            // whatever the face, a line takes about one and a third times its
            // size.
            let wanted = (a.label_size as f32 * 1.35) + 12.0 + 12.0;
            assert!(
                a.size.1 as f32 >= wanted,
                "{} is {}px tall and its label wants {wanted:.0}px",
                t.spelling(),
                a.size.1
            );
        }
    }

    /// Every family's members share the outline that defines it.
    ///
    /// The families are only worth showing if they mean something, and what
    /// they mean is the shape: a reader who learns that square is hardware has
    /// learned it for all eight of them or for none.
    #[test]
    fn a_family_shares_one_outline() {
        use std::collections::BTreeMap;

        let mut outlines: BTreeMap<&str, Vec<Outline>> = BTreeMap::new();
        for t in NodeType::ALL {
            outlines
                .entry(t.family())
                .or_default()
                .push(t.appearance().outline);
        }
        for (family, mut shapes) in outlines {
            shapes.sort_by_key(|o| format!("{o:?}"));
            shapes.dedup();
            assert_eq!(
                shapes.len(),
                1,
                "{family} is drawn with {} different outlines",
                shapes.len()
            );
        }
    }

    /// Every type is in a family, and every family has a name worth reading.
    #[test]
    fn every_type_belongs_somewhere() {
        for t in NodeType::ALL {
            let family = t.family();
            assert!(!family.is_empty(), "{} is in no family", t.spelling());
            assert!(
                family.chars().next().unwrap().is_uppercase(),
                "{family} is a heading and should read as one"
            );
        }
    }

    /// Every type has a spelling, and it is the configuration's word for it.
    #[test]
    fn every_type_spells_itself_as_the_configuration_does() {
        for t in NodeType::ALL {
            let spelling = t.spelling();
            assert!(!spelling.is_empty());
            assert_eq!(
                spelling,
                spelling.to_lowercase(),
                "a type's spelling is the configuration's, which is lower case"
            );
        }
        // And no two share one, which would make a palette entry ambiguous.
        let mut spellings: Vec<_> = NodeType::ALL.iter().map(|t| t.spelling()).collect();
        spellings.sort_unstable();
        let before = spellings.len();
        spellings.dedup();
        assert_eq!(before, spellings.len(), "two types share a spelling");
    }
}
