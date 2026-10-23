//! The colours a configuration is drawn in.
//!
//! Written down rather than computed: the palette was searched for offline and
//! the arithmetic that holds it to its rules is tcsgui's, in its test-only
//! `sight.rs`, whose tests read these. Here so that the editor and tcspecial's
//! visual window paint a node and a line alike.
//!
//! Every colour is a hex string, `#rrggbb`. Each program turns one into its
//! own toolkit's colour with [`rgb`].

use crate::pane::{pane_of, Pane};
use crate::style::{self, NodeType};

/// What a line is drawn in, by the stream it carries.
///
/// A line used to be grey, whichever way its data went, so a reader tracing a
/// control loop had to tell the two streams apart by which corner each line
/// left -- which is reading a diagram by its arithmetic. Now the colour says
/// it: a line carrying the payload's own data is of the payload family, and
/// one carrying what the ground sends is of the ground's.
///
/// The middle of the three tiers the palette is built as, and a shade of the
/// pane its data comes from: the payload stream leaves the payload pane and
/// is drawn in a darker teal, the ground's stream leaves the moc pane and is
/// drawn in a darker blue. A drawing then reads as one picture -- a pale
/// sheet, lines on the sheet, solid blocks on the lines -- a line can never be
/// mistaken for a node's own edge, and the colour of a line says which end of
/// the canvas its data started at.
///
/// Written down rather than computed, because the palette is written down --
/// the maths lives in `sight.rs`, which is test-only. What holds these to the
/// tiers is `the_palette_runs_from_the_pane_through_the_line_to_the_bodies`,
/// and what holds them apart from everything else is
/// `no_two_colours_of_the_scheme_look_alike`.
///
/// Grey for a line whose stream cannot be told, which is a line this build
/// does not understand: it is drawn, because a connection missing from a
/// drawing is worse than one drawn in the wrong colour.
pub fn line_ink(leaves_by: Option<&str>) -> String {
    match leaves_by {
        Some("payload-out") => PAYLOAD_LINE,
        Some("moc-out") => MOC_LINE,
        _ => LINE_GREY,
    }
    .to_string()
}

/// What is written on a node: its outline, its name, its terminals.
///
/// White. A body is painted in its own tint now, and the tints are the darkest
/// of the three tiers the palette is built as -- a working pane is the
/// lightest shade of its family, a link line is darker, and the bodies are
/// darker still -- so what is written on a body has to be legible on the
/// lightest body and the darkest alike. White clears four and a half to one
/// against every one of them, which is the threshold for text, and a test says
/// so.
///
/// Not the pane's own ground, which was the first thing tried: the payload
/// pane is the darkest of the three at 84 of lightness, and its own shade on
/// its own lightest body is 3.4 to one -- under the floor for a name somebody
/// has to read.
pub const MARK: &str = "#ffffff";

/// What a line is drawn in when the stream it carries cannot be told: see
/// [`line_ink`].
pub const LINE_GREY: &str = "#6b7280";

/// The payload stream's line: a darker shade of the payload pane, at 58.2 of
/// lightness.
///
/// A line is drawn in the colour of the pane its data comes *from*, darker.
/// The payload's own readings start in the payload pane and generally end at
/// the ground, so the line is a teal taken from the teal pane they leave --
/// which means a reader can see where a line's data came from without
/// following it back. What it is darker *than* is also what it crosses, so the
/// shade is where it is for the reason below.
///
/// Twenty-four points darker, not "a bit". That is what the two other rules
/// leave: a line has to stay eight points above the lightest node body, which
/// puts the floor at 57.4, and it has to be seen on a pane at 90 of lightness,
/// which puts the lightest usable shade at 58.2 -- the two meet, and there is
/// nothing in between. Anything lighter reaches 1.3 to 1 against its own pane
/// and disappears; nothing in the gamut reaches 2.5 to one at all while
/// staying clear of the bodies. See
/// `tests::the_palette_runs_from_the_pane_through_the_line_to_the_bodies`.
pub const PAYLOAD_LINE: &str = "#289aa2";

/// And what the ground sends: a darker shade of the moc pane, at 58.2.
///
/// Thirty-two points darker, for the same reasons, and still a blue -- the
/// ground's own hue, which is the one thing about the lines that did not have
/// to change.
pub const MOC_LINE: &str = "#0292e4";

/// What a node of this type is painted.
///
/// A rung of its own panel's ramp. Each panel starts twelve CIE units below
/// the line that carries its data -- the moc panel below the moc line, the
/// payload panel below the payload line, and the vehicle panel below both,
/// having no line of its own -- and steps three and a half units darker for
/// each node, taking the panel's nodes in alphabetical order. A reader can
/// then find a node in a panel by how dark it is, and the panel reads as one
/// family rather than as a set of unrelated colours.
///
/// Twelve for the first step and not the eight the tiers ask, because the moc
/// family's line and its bodies are the same hue: at the same hue a step in
/// lightness *is* the difference, and twelve is the floor two colours of this
/// scheme are held apart by.
///
/// The ramp stops at thirteen of lightness, which is where the three families
/// stop being told apart: below it a dark blue, a dark green and a dark teal
/// converge under deuteranopia. A panel
/// with more nodes than its ramp has rungs starts again at the top, so two
/// nodes of one panel may share a colour -- which is allowed here because
/// colour is the redundant channel: every pair of types differs in two of
/// outline, edge, border width, size and label, and
/// `style::NodeType::appearance` is held to that by its own test.
///
/// What this gave up is the twelve units between *neighbours* in one panel.
/// A ramp of ten in one hue family cannot keep it -- see `design.rst` -- so
/// within a panel the steps are three to five units, which is "a shade
/// darker", and across panels the closest pair is 30.9. The ramp is the
/// information: a node's place in its panel is its colour's place in the
/// ramp.
///
/// Searched offline and written down, as the palette before it was: the
/// maths lives in `sight.rs`, which is test-only, and
/// `tests::every_panel_is_a_ramp_from_its_own_line` holds these to the rule.
pub fn tint_of(node_type: &str) -> String {
    let pane = pane_of(node_type);
    let rungs = match pane {
        Pane::Moc => MOC_RUNGS,
        Pane::Vehicle => VEHICLE_RUNGS,
        Pane::Payloads => PAYLOAD_RUNGS,
    };
    let order = ramp_order(pane);
    // Where this type comes in its own panel, alphabetically. A type this
    // build does not know is not in any panel and is drawn as a fault, so the
    // first rung is as good an answer as any.
    let at = order
        .iter()
        .position(|name| *name == node_type)
        .unwrap_or(0);
    rungs[at % rungs.len()].to_string()
}

/// The ground link's panel: one node, so one rung.
pub const MOC_RUNGS: &[&str] = &["#0073b5"];

/// The vehicle's own graph, in the order the panel lists it: compute, log,
/// null-moc (which took the null node's), ping, subdiagram.
pub const VEHICLE_RUNGS: &[&str] = &["#547800", "#4b7000", "#446600", "#3d5d00"];

/// And what it carries, which is the longest ramp: ten rungs from 46.2 of
/// lightness down to 15.0, three and a half units apart, which is as far as
/// the floor allows.
pub const PAYLOAD_RUNGS: &[&str] = &[
    "#007a7d", "#007173", "#00676a", "#005e61", "#005557", "#004c4e", "#004446", "#003b3d",
    "#003335", "#002b2c",
];

/// Each panel's own node types in the order the ramp is walked: the types the
/// rungs were chosen for first, alphabetically, and then the apps,
/// alphabetically.
///
/// The apps came later than the rungs, and a type that comes later takes the
/// colours from the top of the ramp again rather than its place in the
/// alphabet: sorted in with the rest, app_all_pl and app_pl took the first two
/// payload rungs and moved every device a rung or two down, so that a can
/// node was no longer the colour a can node had been. Wrapping is allowed --
/// see `tint_of` -- and moving a colour that is already in use is not.
pub fn ramp_order(pane: Pane) -> Vec<&'static str> {
    let mut types: Vec<NodeType> = NodeType::ALL
        .iter()
        .copied()
        .filter(|t| pane_of(t.spelling()) == pane)
        .collect();
    types.sort_by_key(|t| (came_later(*t), t.spelling()));
    types.iter().map(|t| t.spelling()).collect()
}

/// When a type came, for `ramp_order`: the types the rungs were chosen for,
/// then the five apps, then the types that came after them. `null-moc` took
/// the place and the colour of the `null` node it replaced, and the other
/// three that replaced it come last, so that no colour already in use moved.
pub fn came_later(node_type: NodeType) -> u8 {
    match node_type {
        NodeType::AppPayloadVeh
        | NodeType::AppMocVeh
        | NodeType::AppAllVeh
        | NodeType::AppPl
        | NodeType::AppAllPl => 1,
        NodeType::NullPayload | NodeType::TeeMoc | NodeType::TeePayload => 2,
        _ => 0,
    }
}

/// What a working pane is painted, at the head of its column.
pub fn ground_of(ground: style::Ground) -> String {
    match ground {
        style::Ground::Moc => "#dee2fc",
        style::Ground::Vehicle => "#e8e4ca",
        style::Ground::Payloads => "#bad2d0",
    }
    .to_string()
}

/// The three parts of a hex colour, `#rrggbb`.
///
/// A dark grey for a string that is not one, which is what the editor drew
/// before this was shared: a colour that could not be read is drawn, as a
/// line of the wrong colour is, rather than left out.
pub fn rgb(hex: &str) -> (u8, u8, u8) {
    let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0x33_33_33);
    (
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    )
}

/// How far towards white a lit line is from its own line's colour.
///
/// Tcspecial's visual window draws a link that has just carried data as a
/// wide line under the ordinary one, a shade lighter than it: lighter so the
/// ordinary line stays the one a reader follows, and the same hue so the two
/// still say which stream it is. Half way to white, which is lighter enough
/// to be seen as a halo on the white sheet and dark enough to be seen at all.
pub const LIGHTER_BY: f64 = 0.5;

/// A shade lighter than `hex`: [`LIGHTER_BY`] of the way from it to white.
pub fn lighter(hex: &str) -> String {
    let (r, g, b) = rgb(hex);
    let towards = |part: u8| -> u8 {
        let part = f64::from(part);
        (part + (255.0 - part) * LIGHTER_BY).round() as u8
    };
    format!("#{:02x}{:02x}{:02x}", towards(r), towards(g), towards(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_is_read_from_hex_and_anything_else_is_dark_grey() {
        assert_eq!(rgb("#289aa2"), (0x28, 0x9a, 0xa2));
        assert_eq!(rgb("not a colour"), (0x33, 0x33, 0x33));
    }

    /// Lighter, and of the same family: each part moves half way to white,
    /// so the order of the three parts -- which is the hue -- is kept.
    #[test]
    fn a_lighter_shade_is_half_way_to_white() {
        assert_eq!(lighter("#000000"), "#808080");
        assert_eq!(lighter("#ffffff"), "#ffffff");
        let (r, g, b) = rgb(&lighter(PAYLOAD_LINE));
        let (r0, g0, b0) = rgb(PAYLOAD_LINE);
        assert!(r > r0 && g > g0 && b > b0);
        assert!(
            r < g && r < b,
            "still a teal: little red beside its green and blue"
        );
    }
}
