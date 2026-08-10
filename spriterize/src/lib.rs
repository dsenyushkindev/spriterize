//! Reusable document and generator facilities shared by the GUI and build tools.

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod collection;
pub mod project;
pub mod wrapped_image;

pub mod gui {
    pub mod graph;
}
