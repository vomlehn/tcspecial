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

/// The palette: one tint per node type, and no two alike.
///
/// Fourteen rather than nine, so that no two types share a colour. Colour is
/// still the redundant channel -- the outline, the shading, the border, the
/// height and the way the label is set are what tell two types apart, and the
/// test above says so -- but a reader who *can* use colour now gets it for
/// every type instead of for most of them.
///
/// The fourteen were chosen for one thing before anything else: that a reader
/// with any of the three dichromacies can tell them apart. They are the output
/// of a search over sRGB that maximised the smallest perceived difference
/// between any two of them, taken under normal vision and under simulated
/// protanopia, deuteranopia and tritanopia at once, and constrained to a
/// moderate chroma and a contrast against white of at least three to one so
/// that each is legible as a border. The worst pair of the fourteen is 17 CIE
/// units apart under every one of the four -- and `main.rs` has the test that
/// says so, checked against the colours the window is actually given.
///
/// Spread in lightness on purpose: lightness is the one channel every kind of
/// colour blindness keeps, including the total ones. Five levels from about
/// 28 to about 60.
///
/// Named by colour here, where the earlier palette was named by meaning, for a
/// plain reason: with one per type the meaning is the type, and a name like
/// `I2cTint` would be the enum repeating [`NodeType`]. What the separation
/// still buys is that this file says *which* tint and `main.rs` says what it is
/// painted, so the accessibility tests can check the rules without this file
/// holding a single hex value.
///
/// No `Red`. Error colouring does not come from this table at all -- see
/// [`ERROR_TINT`] -- which is what keeps red out of the ordinary drawing by
/// construction instead of by discipline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tint {
    Indigo,
    Forest,
    Bronze,
    Plum,
    Purple,
    Magenta,
    Blue,
    Rose,
    Olive,
    Taupe,
    Periwinkle,
    Gold,
    Blush,
    Lavender,
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
    Log,
    Moc,
    /// A diagram of its own, kept in its own file and shown here as one box.
    Subdiagram,
}

impl NodeType {
    /// Every type, for the palette and for the tests.
    pub const ALL: [NodeType; 14] = [
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
            // An interface, not a path: see tcslibgs::node::hdlc. The
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
            NodeType::Log => &[("directory", true), ("format", true)],
            NodeType::Moc => &[("address", true), ("port", true)],
            NodeType::Subdiagram => &[("file", true)],
        }
    }

    /// Which family this type belongs to.
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
                tint: Tint::Purple,
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
                tint: Tint::Magenta,
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
                tint: Tint::Rose,
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
                tint: Tint::Bronze,
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
                tint: Tint::Gold,
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
                tint: Tint::Taupe,
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
                tint: Tint::Periwinkle,
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
                tint: Tint::Lavender,
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
                tint: Tint::Blush,
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
                tint: Tint::Olive,
            },
            // Software aboard.
            NodeType::Compute => Appearance {
                face: Face::Plain,
                label_size: 14,
                label_weight: 400,
                label_italic: false,
                outline: Rounded,
                shading: Solid,
                weight: Wide,
                size: (WIDTH, 64),
                tint: Tint::Blue,
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
                tint: Tint::Plum,
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
                tint: Tint::Forest,
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
                tint: Tint::Indigo,
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
    /// fourteen entries written one at a time is fourteen chances to reach for
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

    /// Red is not in the palette at all.
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
