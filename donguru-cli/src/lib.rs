//! Library face of the Donguru CLI.
//!
//! The binary (`src/main.rs`) is the primary entry point, this exists so
//! integration tests and examples (e.g. `examples/theme_preview.rs`) can reuse
//! the theme and command tree.

pub mod cli;
pub mod commands;
pub mod context;
pub mod exit;
pub mod theme;
