//! Serializable types that mirror the `project.json` schema inside a `.jcam` archive.
//!
//! These types are the on-disk representation. The in-memory representation
//! lives in [`crate::state`]; conversion between the two is done in
//! [`super::serialization`].

use serde::{Deserialize, Serialize};

use crate::models::Tool;

/// Core project metadata stored under the `"project"` key in `project.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    pub description: String,
    /// Unit system: `"mm"` (metric) or `"inch"` (imperial).
    pub units: String,
}

/// Reference to the source geometry model file.
///
/// Stored under `"source_model"` in `project.json`. The in-memory counterpart
/// with tessellated mesh data is [`crate::state::LoadedModel`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceModelRef {
    /// Absolute path to the model file at last save.
    pub path: String,
    /// SHA-256 hex digest of the model file at last load (Phase 1+ cache key).
    pub checksum: String,
    /// `true` when the model file is embedded in the ZIP as `model/source.*`.
    pub embedded: bool,
}

/// Top-level structure of `project.json` inside a `.jcam` archive.
///
/// This is the **on-disk** representation. The **in-memory** representation is
/// [`crate::state::Project`]; conversion between the two is performed by
/// [`super::serialization::save`] and [`super::serialization::load`].
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectFile {
    /// Format version; only version `1` is supported in Phase 0.
    pub schema_version: u32,
    /// JamieCam version string that last saved this file (`CARGO_PKG_VERSION`).
    pub app_version: String,
    /// ISO-8601 creation timestamp (UTC).
    pub created_at: String,
    /// ISO-8601 last-modified timestamp (UTC).
    pub modified_at: String,
    /// Core project metadata (name, description, units).
    pub project: ProjectMeta,
    /// Source geometry model reference, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_model: Option<SourceModelRef>,
    // ── Scaffolding — remaining types replaced in later phases ───────────────
    /// Stock solid definition (populated in a future phase).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stock: Option<serde_json::Value>,
    /// Work coordinate systems (populated in a future phase).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wcs: Vec<serde_json::Value>,
    /// Tool library.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool>,
    /// Machining operations (populated in a future phase).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operations: Vec<serde_json::Value>,
}
