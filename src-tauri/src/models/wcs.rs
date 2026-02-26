//! Work Coordinate System (WCS) data model.
//!
//! [`WorkCoordinateSystem`] represents a named coordinate frame used to
//! position machining operations relative to the workpiece.
//! See `docs/project-file-format.md` for the full JSON schema.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A 3-component f64 vector, used for origin positions and axis directions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

fn default_origin() -> Vec3 {
    Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    }
}

fn default_x_axis() -> Vec3 {
    Vec3 {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    }
}

fn default_z_axis() -> Vec3 {
    Vec3 {
        x: 0.0,
        y: 0.0,
        z: 1.0,
    }
}

/// A named coordinate frame for positioning machining operations.
///
/// Orientation is defined by two orthogonal unit vectors: `x_axis` and
/// `z_axis`. The Y axis is derived as `z_axis × x_axis`. Defaults represent
/// the identity frame (world coordinates, Z-up).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkCoordinateSystem {
    /// Unique identifier for this WCS entry.
    pub id: Uuid,
    /// Human-readable name (e.g. `"G54 — Top Setup"`).
    pub name: String,
    /// WCS origin in world coordinates.
    #[serde(default = "default_origin")]
    pub origin: Vec3,
    /// X-axis unit vector defining the WCS orientation.
    #[serde(default = "default_x_axis")]
    pub x_axis: Vec3,
    /// Z-axis unit vector defining the WCS orientation.
    #[serde(default = "default_z_axis")]
    pub z_axis: Vec3,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_wcs() -> WorkCoordinateSystem {
        WorkCoordinateSystem {
            id: Uuid::parse_str("3f8a2b00-0000-0000-0000-000000000001").unwrap(),
            name: "G54 — Top Setup".to_string(),
            origin: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            x_axis: Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            z_axis: Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        }
    }

    #[test]
    fn wcs_serde_round_trip() {
        let original = make_wcs();
        let json = serde_json::to_string(&original).expect("serialize WorkCoordinateSystem");
        let recovered: WorkCoordinateSystem =
            serde_json::from_str(&json).expect("deserialize WorkCoordinateSystem");
        assert_eq!(original, recovered);
    }

    #[test]
    fn wcs_fields_are_camel_case() {
        let wcs = make_wcs();
        let value = serde_json::to_value(&wcs).expect("to_value");
        assert!(value.get("xAxis").is_some());
        assert!(value.get("zAxis").is_some());
        assert!(value.get("x_axis").is_none());
        assert!(value.get("z_axis").is_none());
    }

    #[test]
    fn wcs_axes_default_to_identity_when_absent() {
        let json = r#"{"id":"3f8a2b00-0000-0000-0000-000000000001","name":"Test"}"#;
        let wcs: WorkCoordinateSystem = serde_json::from_str(json).expect("deserialize");
        assert_eq!(
            wcs.x_axis,
            Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0
            }
        );
        assert_eq!(
            wcs.z_axis,
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 1.0
            }
        );
        assert_eq!(
            wcs.origin,
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0
            }
        );
    }
}
