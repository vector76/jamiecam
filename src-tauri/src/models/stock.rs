//! Stock solid definition data model.
//!
//! [`StockDefinition`] is an enum so that future variants (cylinder, mesh)
//! can be added without breaking the existing `Box` variant on disk.
//! See `docs/project-file-format.md` for the full JSON schema.

use serde::{Deserialize, Serialize};

/// A 3-component f64 vector, used for origin positions and dimensions.
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

/// Dimensions and position of a box-shaped stock solid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxDimensions {
    /// Minimum-XYZ corner of the stock in WCS coordinates.
    #[serde(default)]
    pub origin: Vec3,
    /// Stock width along the X axis.
    pub width: f64,
    /// Stock depth along the Y axis.
    pub depth: f64,
    /// Stock height along the Z axis.
    pub height: f64,
}

/// The stock material block for this project.
///
/// Modelled as an internally-tagged enum so future variants (`Cylinder`,
/// `Mesh`) can be added without a breaking format change. Serializes as
/// `{ "type": "box", ... }` for the `Box` variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StockDefinition {
    Box(BoxDimensions),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_box_stock() -> StockDefinition {
        StockDefinition::Box(BoxDimensions {
            origin: Vec3 {
                x: -5.0,
                y: -5.0,
                z: -2.0,
            },
            width: 120.0,
            depth: 80.0,
            height: 30.0,
        })
    }

    #[test]
    fn stock_serde_round_trip() {
        let original = make_box_stock();
        let json = serde_json::to_string(&original).expect("serialize StockDefinition");
        let recovered: StockDefinition =
            serde_json::from_str(&json).expect("deserialize StockDefinition");
        assert_eq!(original, recovered);
    }

    #[test]
    fn box_stock_serializes_with_type_tag() {
        let stock = make_box_stock();
        let value = serde_json::to_value(&stock).expect("to_value");
        assert_eq!(value["type"], "box");
        assert_eq!(value["width"], 120.0);
        assert_eq!(value["depth"], 80.0);
        assert_eq!(value["height"], 30.0);
        assert_eq!(value["origin"]["x"], -5.0);
    }

    #[test]
    fn box_stock_origin_defaults_to_zero() {
        let json = r#"{"type":"box","width":10.0,"depth":10.0,"height":10.0}"#;
        let stock: StockDefinition = serde_json::from_str(json).expect("deserialize");
        let StockDefinition::Box(b) = stock;
        assert_eq!(b.origin, Vec3::zero());
    }
}
