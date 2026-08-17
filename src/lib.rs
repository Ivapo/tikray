//! Tikray — view and convert images from the terminal.
//!
//! The whole program is one short pipeline with a different edge enabled:
//! **decode-or-rasterize → one in-memory image buffer → display-or-encode**
//! (tkr-001 §2.1). That buffer is [`image::DynamicImage`], chosen over a fixed
//! RGBA8 raster so the source's channel layout and bit depth survive the waist.
//!
//! This is a library with the CLI as one caller, not a binary with helpers
//! (§2.2): Phase 4 adds a second caller, and a core living inside `main` would
//! have to be rewritten at that point rather than reused.
//!
//! `tikray view <png|jpeg|svg>` and `tikray convert <in> <out>` are what exist
//! so far — both output edges over one waist. Vector input is not a second
//! program: [`svg::rasterize`] fills the same buffer, so everything downstream
//! of the waist is blind to which edge filled it, and every input the load path
//! supports is convertible on arrival rather than one at a time.

pub mod cli;
pub mod convert;
pub mod display;
pub mod error;
pub mod load;
pub mod svg;
pub mod term;
pub mod tui;

pub use error::TikrayError;
pub use load::load;
