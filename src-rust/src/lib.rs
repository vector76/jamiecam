//! JamieCam Mode 1 (G-code Viewer) core, compiled to WebAssembly.
//!
//! The crate exposes pure-Rust G-code parsing and dexel material-removal
//! simulation, plus a small wasm-bindgen surface that the browser frontend
//! calls into.

pub mod dexel;
pub mod error;
pub mod gcode_parser;
pub mod geometry2d;
pub mod grbl;
pub mod parse_warning;
pub mod profile;
pub mod types;
pub mod wasm;
pub mod working_env;
