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
//! Phase 1 implements `tikray view <png|jpeg>` and nothing else.

pub mod error;
pub mod load;

pub use error::TikrayError;
pub use load::load;
