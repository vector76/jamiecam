use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitSystem {
    Mm,
    Inches,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Curve2d {
    pub id: Uuid,
    pub is_closed: bool,
    /// Points in mm, Y-up coordinate system
    pub points: Vec<[f64; 2]>,
    pub layer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundingBox2d {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl BoundingBox2d {
    pub fn from_points(points: &[[f64; 2]]) -> Self {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for &[x, y] in points {
            if x < min_x {
                min_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if x > max_x {
                max_x = x;
            }
            if y > max_y {
                max_y = y;
            }
        }

        BoundingBox2d {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurveSummary {
    pub id: Uuid,
    pub is_closed: bool,
    pub bbox: BoundingBox2d,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedArtwork {
    pub file_path: String,
    pub unit_system: UnitSystem,
    pub curves: Vec<Curve2d>,
    pub import_warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounding_box_from_points_correct_extents() {
        let points: &[[f64; 2]] = &[[1.0, 2.0], [5.0, -3.0], [0.0, 7.0], [4.0, 4.0]];
        let bbox = BoundingBox2d::from_points(points);
        assert_eq!(bbox.min_x, 0.0);
        assert_eq!(bbox.min_y, -3.0);
        assert_eq!(bbox.max_x, 5.0);
        assert_eq!(bbox.max_y, 7.0);
    }

    #[test]
    fn bounding_box_single_point() {
        let points: &[[f64; 2]] = &[[3.0, 4.0]];
        let bbox = BoundingBox2d::from_points(points);
        assert_eq!(bbox.min_x, 3.0);
        assert_eq!(bbox.min_y, 4.0);
        assert_eq!(bbox.max_x, 3.0);
        assert_eq!(bbox.max_y, 4.0);
    }

    #[test]
    fn unit_system_serializes_to_snake_case() {
        let mm = serde_json::to_string(&UnitSystem::Mm).unwrap();
        assert_eq!(mm, "\"mm\"");

        let inches = serde_json::to_string(&UnitSystem::Inches).unwrap();
        assert_eq!(inches, "\"inches\"");
    }

    #[test]
    fn curve2d_round_trips_through_serde_json() {
        let id = Uuid::new_v4();
        let curve = Curve2d {
            id,
            is_closed: true,
            points: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
            layer: Some("outline".to_string()),
        };

        let json = serde_json::to_string(&curve).unwrap();
        let deserialized: Curve2d = serde_json::from_str(&json).unwrap();

        assert_eq!(curve, deserialized);
    }

    #[test]
    fn curve2d_round_trips_with_no_layer() {
        let id = Uuid::new_v4();
        let curve = Curve2d {
            id,
            is_closed: false,
            points: vec![[1.0, 2.0], [3.0, 4.0]],
            layer: None,
        };

        let json = serde_json::to_string(&curve).unwrap();
        let deserialized: Curve2d = serde_json::from_str(&json).unwrap();

        assert_eq!(curve, deserialized);
    }
}
