//! artlib — procedural art as composable mathematics.
//!
//! A Rust port of the Python `artlib`. Two ideas carry the whole thing:
//!
//! 1. A **shape is a function**, not a loop: `(x, y) -> signed distance`, so
//!    shapes compose as arithmetic on that distance (see [`fields`]).
//! 2. The **field decides coverage**, so edges antialias (see [`raster`]).
//!
//! Surfaces — "what is it made of" — are grids of floats with noise and
//! arithmetic ([`texture`]); a grid is also a field, so the two halves meet.
//!
//! Fidelity to the Python original is a hard requirement for the *deterministic*
//! parts (shapes, algebra, transforms, shaders, compositing), checked
//! pixel-for-pixel against golden images in `tests/`. The noise sources
//! reproduce the Python *algorithms* but not its exact random stream — see
//! [`prng`] for why.

mod prng;

pub mod fields;
pub mod raster;
pub mod texture;

pub use prng::Prng;
