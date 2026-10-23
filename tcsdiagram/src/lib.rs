//! How a configuration is drawn.
//!
//! The working panes, the shape of every node type, the routes the links take
//! between them and the colours all of it is drawn in -- the drawing tcsgui
//! edits, here so that something else can draw the same thing. Tcspecial's
//! visual window is the other: it shows a running vehicle's graph as the
//! editor shows the configuration, and lights a link as data crosses it. Two
//! drawings of one configuration that differed would be two answers to where a
//! node is, so there is one.
//!
//! Everything here is arithmetic and text: paths as SVG path data, colours as
//! hex strings, sizes in sheet pixels. Nothing here uses Slint, because each
//! program compiles the markup that paints these, `ui/diagram.slint`, into
//! its own build, and a Slint type of one program is not a type of the other.
//! Each turns a [`view::NodeLook`] and a [`view::LinkLook`] into its own.
//!
//! Nothing here is an editor's either. What is selected, what is being
//! dragged and what the history holds are tcsgui's; what is drawn is a list
//! of nodes and the lines between them, [`view::OnSheet`] and
//! [`view::LineOnSheet`].

pub mod look;
pub mod pane;
pub mod route;
pub mod settings;
pub mod style;
pub mod view;
