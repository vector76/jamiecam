//! .jcam project file I/O.
//!
//! A `.jcam` file is a standard ZIP archive containing at minimum a
//! `project.json` manifest. This module provides:
//!
//! - [`types`] — serializable types that mirror the `project.json` schema
//! - [`serialization`] — atomic save and validated load functions

pub mod serialization;
pub mod types;
