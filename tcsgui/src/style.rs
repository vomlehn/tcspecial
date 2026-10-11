//! What each node type looks like, and the rules the looks obey.
//!
//! Two rules, and both are checked by the tests at the bottom rather than
//! trusted to whoever edits the table next.
//!
//! **Nothing is told apart by colour alone.** Every node type differs from
//! every other in at least two channels that are not colour -- its outline
//! shape, how wide its border is, how its inside is shaded, and how large it
//! is. A reader who sees no colour at all still sees nine distinguishable
//! shapes, and a reader who sees colour gets the same information twice. Two
//! channels rather than one on purpose: a single distinguishing channel is one
//! print, one projector or one pair of eyes away from being no channel.
//!
//! **Red means something is wrong.** No node type, no selection, no hover and
//! no link is red. It is reserved so that red anywhere on the canvas means an
//! error and can be looked for, which is worth more than a tenth palette
//! entry.

/// The outline a node is drawn with.
///
/// Slint draws all of these from its own primitives -- a `Rectangle` with and
/// without `border-radius`, a `Rectangle` of equal sides with a radius of
/// half its width for a circle -- so none of this needs an image or a font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outline {
    /// Square corners. Hardware: what is physically there.
    Square,
    /// Rounded corners. Software aboard: a computation.
    Rounded,
    /// A circle. An end of the graph: the ground, or a log.
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

/// What a node type looks like.
///
/// `tint` is a hint and never the whole of the distinction. It is named rather
/// than given as a value so the window can hold the palette and this table can
/// hold the meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Appearance {
    pub outline: Outline,
    pub shading: Shading,
    pub weight: Weight,
    /// The nominal width and height, in pixels, before a label widens it.
    /// Size is the fourth channel: a MOC link is visibly larger than a sensor.
    pub size: (u32, u32),
    pub tint: Tint,
}

/// The palette, by meaning rather than by colour.
///
/// No `Red`. Error colouring does not come from this table at all -- see
/// [`ERROR_TINT`] -- which is what keeps red out of the ordinary drawing by
/// construction instead of by discipline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tint {
    Slate,
    Teal,
    Amber,
    Violet,
    Olive,
    Indigo,
    Sand,
    Cyan,
    Grey,
}

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
    Can,
    Iio,
    Rpmsg,
    Network,
    Pseudofile,
    Gpio,
    Compute,
    Log,
    Moc,
}

impl NodeType {
    /// Every type, for the palette and for the tests.
    pub const ALL: [NodeType; 12] = [
        NodeType::I2c,
        NodeType::Spi,
        NodeType::Serial,
        NodeType::Can,
        NodeType::Iio,
        NodeType::Rpmsg,
        NodeType::Network,
        NodeType::Pseudofile,
        NodeType::Gpio,
        NodeType::Compute,
        NodeType::Log,
        NodeType::Moc,
    ];

    /// The word the configuration file uses, which is the word the editor
    /// shows: an operator reading the file and an operator reading the canvas
    /// should not have to translate.
    pub fn spelling(self) -> &'static str {
        match self {
            NodeType::I2c => "i2c",
            NodeType::Spi => "spi",
            NodeType::Serial => "serial",
            NodeType::Can => "can",
            NodeType::Iio => "iio",
            NodeType::Rpmsg => "rpmsg",
            NodeType::Network => "network",
            NodeType::Pseudofile => "pseudofile",
            NodeType::Gpio => "gpio",
            NodeType::Compute => "compute",
            NodeType::Log => "log",
            NodeType::Moc => "moc",
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
                outline: Square,
                shading: Solid,
                weight: Narrow,
                size: (120, 48),
                tint: Tint::Slate,
            },
            NodeType::Spi => Appearance {
                outline: Square,
                shading: Hatched,
                weight: Narrow,
                size: (120, 56),
                tint: Tint::Teal,
            },
            NodeType::Serial => Appearance {
                outline: Square,
                shading: Open,
                weight: Narrow,
                size: (132, 48),
                tint: Tint::Amber,
            },
            NodeType::Can => Appearance {
                outline: Square,
                shading: Double,
                weight: Narrow,
                size: (120, 64),
                tint: Tint::Violet,
            },
            NodeType::Iio => Appearance {
                outline: Square,
                shading: Hatched,
                weight: Wide,
                size: (132, 56),
                tint: Tint::Olive,
            },
            NodeType::Rpmsg => Appearance {
                outline: Square,
                shading: Solid,
                weight: Wide,
                size: (144, 48),
                tint: Tint::Indigo,
            },
            NodeType::Network => Appearance {
                outline: Square,
                shading: Open,
                weight: Wide,
                size: (144, 56),
                tint: Tint::Cyan,
            },
            NodeType::Pseudofile => Appearance {
                outline: Square,
                shading: Double,
                weight: Wide,
                size: (132, 64),
                tint: Tint::Sand,
            },
            // Driven rather than read, and shaped to say so.
            NodeType::Gpio => Appearance {
                outline: Keyed,
                shading: Solid,
                weight: Wide,
                size: (120, 72),
                tint: Tint::Grey,
            },
            // Software aboard.
            NodeType::Compute => Appearance {
                outline: Rounded,
                shading: Solid,
                weight: Wide,
                size: (152, 64),
                tint: Tint::Teal,
            },
            // Ends of the graph.
            NodeType::Log => Appearance {
                outline: Circle,
                shading: Hatched,
                weight: Narrow,
                size: (96, 96),
                tint: Tint::Sand,
            },
            NodeType::Moc => Appearance {
                outline: Circle,
                shading: Solid,
                weight: Wide,
                size: (128, 128),
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
