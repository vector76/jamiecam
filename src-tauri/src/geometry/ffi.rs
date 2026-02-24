//! Raw bindgen-generated FFI bindings for the cam_geometry C API.
//!
//! Generated from `src-tauri/cpp/cam_geometry.h` by `bindgen` during
//! `cargo build`.  The output file `ffi_generated.rs` lives in `$OUT_DIR`
//! and is included here at compile time.
//!
//! **Do not call these functions directly outside this module.**
//! Use the safe wrappers in `geometry::safe` instead.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(clippy::all)]

include!(concat!(env!("OUT_DIR"), "/ffi_generated.rs"));
