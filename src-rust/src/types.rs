//! Core data types shared across the crate.
//!
//! These types previously lived in `models/`, `geometry/safe.rs`, and
//! `toolpath/types.rs`. After the Tauri-to-web port stripped the project
//! state, FFI geometry kernel, and toolpath planner, only the small set
//! of value types used by Mode 1 (G-code Viewer) remains.

use serde::{Deserialize, Serialize};

// ── Vec3 ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub fn zero() -> Self {
        Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

impl Default for Vec3 {
    fn default() -> Self {
        Self::zero()
    }
}

// ── Stock ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxDimensions {
    #[serde(default)]
    pub origin: Vec3,
    pub width: f64,
    pub depth: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StockDefinition {
    Box(BoxDimensions),
}

// ── Mesh ──────────────────────────────────────────────────────────────────────

/// Per-face triangle group within a [`MeshData`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceGroup {
    pub start_triangle: u32,
    pub triangle_count: u32,
}

/// Tessellated triangle mesh ready for transfer to the frontend.
///
/// Buffers use `f32` vertices/normals and `u32` indices to match Three.js's
/// preferred layout. All compute happens in `f64`; the downcast happens at
/// the wasm boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshData {
    pub vertices: Vec<f32>,
    pub normals: Vec<f32>,
    pub indices: Vec<u32>,
    pub face_groups: Vec<FaceGroup>,
}

// ── Line geometry ─────────────────────────────────────────────────────────────

/// Flat-array line geometry for Three.js viewport rendering.
///
/// Per line segment: 6 floats in `positions` (start XYZ + end XYZ),
/// 6 floats in `colours` (RGB × 2), 1 byte in `types`.
///
/// Type byte values: `0` = linking/rapid, `1` = cutting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineGeometryData {
    pub positions: Vec<f32>,
    pub colours: Vec<f32>,
    pub types: Vec<u8>,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec3_zero_is_default() {
        assert_eq!(Vec3::default(), Vec3::zero());
    }

    #[test]
    fn box_stock_serializes_with_type_tag() {
        let stock = StockDefinition::Box(BoxDimensions {
            origin: Vec3::zero(),
            width: 10.0,
            depth: 20.0,
            height: 5.0,
        });
        let value = serde_json::to_value(&stock).unwrap();
        assert_eq!(value["type"], "box");
        assert_eq!(value["width"], 10.0);
    }

    #[test]
    fn box_stock_origin_defaults_to_zero() {
        let json = r#"{"type":"box","width":10.0,"depth":10.0,"height":10.0}"#;
        let stock: StockDefinition = serde_json::from_str(json).unwrap();
        let StockDefinition::Box(b) = stock;
        assert_eq!(b.origin, Vec3::zero());
    }
}
