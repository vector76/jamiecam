//! Machining operation data model.
//!
//! [`Operation`] is the in-memory and on-disk representation of a single
//! machining step in the project. Each operation has common fields (id, name,
//! enabled, tool_id) and a type-discriminated [`OperationParams`] payload.
//!
//! The JSON representation uses an adjacently-tagged enum flattened into the
//! parent object so the `type` discriminant appears at the operation level
//! alongside the other common fields, and `params` is a separate nested object.
//! See `docs/project-file-format.md` for the full JSON schema.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Tool compensation side for profile operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompensationSide {
    Left,
    Right,
    Center,
}

/// Parameters for a Profile (contour) operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileParams {
    /// Cut depth in project units.
    pub depth: f64,
    /// Maximum depth per pass in project units.
    pub stepdown: f64,
    /// Which side of the path the tool compensates to.
    pub compensation_side: CompensationSide,
}

/// Parameters for a Pocket operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PocketParams {
    /// Cut depth in project units.
    pub depth: f64,
    /// Maximum depth per pass in project units.
    pub stepdown: f64,
    /// Radial stepover as a percentage of tool diameter (0–100).
    pub stepover_percent: f64,
}

/// Parameters for a Drill operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrillParams {
    /// Drill depth in project units.
    pub depth: f64,
    /// Peck increment in project units; `null` for full-depth (non-peck) drilling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peck_depth: Option<f64>,
}

/// Type-discriminated operation parameters.
///
/// Uses adjacently-tagged serde so the JSON representation places the `"type"`
/// discriminant and `"params"` object at the same level as the other operation
/// fields when flattened into [`Operation`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "params", rename_all = "snake_case")]
pub enum OperationParams {
    Profile(ProfileParams),
    Pocket(PocketParams),
    Drill(DrillParams),
}

/// A machining operation in the project operation list.
///
/// Common fields (id, name, enabled, tool_id) are kept at the top level.
/// The operation-specific params are stored in a type-discriminated
/// [`OperationParams`] payload, flattened so that `"type"` and `"params"`
/// appear alongside the common fields in JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    /// Unique identifier for this operation.
    pub id: Uuid,
    /// Human-readable operation name.
    pub name: String,
    /// Whether the operation is active in the toolpath; defaults to `true`.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// The tool assigned to this operation.
    pub tool_id: Uuid,
    /// Type and parameters specific to this operation kind.
    #[serde(flatten)]
    pub params: OperationParams,
}

fn default_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_id() -> Uuid {
        Uuid::parse_str("7f3c1a00-0000-0000-0000-000000000001").unwrap()
    }

    fn make_profile_op() -> Operation {
        Operation {
            id: Uuid::parse_str("aaaa0000-0000-0000-0000-000000000001").unwrap(),
            name: "Outer Profile".to_string(),
            enabled: true,
            tool_id: tool_id(),
            params: OperationParams::Profile(ProfileParams {
                depth: 10.0,
                stepdown: 2.5,
                compensation_side: CompensationSide::Left,
            }),
        }
    }

    fn make_pocket_op() -> Operation {
        Operation {
            id: Uuid::parse_str("bbbb0000-0000-0000-0000-000000000002").unwrap(),
            name: "Rough Pocket".to_string(),
            enabled: true,
            tool_id: tool_id(),
            params: OperationParams::Pocket(PocketParams {
                depth: 15.0,
                stepdown: 3.0,
                stepover_percent: 45.0,
            }),
        }
    }

    fn make_drill_op() -> Operation {
        Operation {
            id: Uuid::parse_str("cccc0000-0000-0000-0000-000000000003").unwrap(),
            name: "Drill Holes".to_string(),
            enabled: true,
            tool_id: tool_id(),
            params: OperationParams::Drill(DrillParams {
                depth: 20.0,
                peck_depth: Some(5.0),
            }),
        }
    }

    #[test]
    fn profile_operation_serde_round_trip() {
        let original = make_profile_op();
        let json = serde_json::to_string(&original).expect("serialize");
        let recovered: Operation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, recovered);
    }

    #[test]
    fn pocket_operation_serde_round_trip() {
        let original = make_pocket_op();
        let json = serde_json::to_string(&original).expect("serialize");
        let recovered: Operation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, recovered);
    }

    #[test]
    fn drill_operation_serde_round_trip() {
        let original = make_drill_op();
        let json = serde_json::to_string(&original).expect("serialize");
        let recovered: Operation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, recovered);
    }

    #[test]
    fn drill_peck_depth_absent_when_none() {
        let op = Operation {
            id: Uuid::new_v4(),
            name: "Full-Depth Drill".to_string(),
            enabled: true,
            tool_id: tool_id(),
            params: OperationParams::Drill(DrillParams {
                depth: 20.0,
                peck_depth: None,
            }),
        };
        let value = serde_json::to_value(&op).expect("to_value");
        let params = &value["params"];
        assert!(
            params.get("peckDepth").is_none(),
            "peckDepth must be absent when None"
        );
    }

    #[test]
    fn operation_enabled_defaults_to_true_when_absent() {
        let json = r#"{
            "id": "aaaa0000-0000-0000-0000-000000000001",
            "name": "Test",
            "toolId": "7f3c1a00-0000-0000-0000-000000000001",
            "type": "pocket",
            "params": { "depth": 5.0, "stepdown": 1.0, "stepoverPercent": 50.0 }
        }"#;
        let op: Operation = serde_json::from_str(json).expect("deserialize");
        assert!(op.enabled, "enabled should default to true");
    }

    #[test]
    fn operation_type_field_at_top_level() {
        let op = make_pocket_op();
        let value = serde_json::to_value(&op).expect("to_value");
        assert_eq!(value["type"], "pocket", "type must be at top level");
        assert!(
            value.get("params").is_some(),
            "params must be a nested object"
        );
        assert!(
            value["params"].get("type").is_none(),
            "type must NOT be inside params"
        );
    }

    #[test]
    fn operation_fields_are_camel_case() {
        let op = make_profile_op();
        let value = serde_json::to_value(&op).expect("to_value");
        assert!(value.get("toolId").is_some(), "toolId must be camelCase");
        assert!(
            value.get("tool_id").is_none(),
            "tool_id snake_case must not appear"
        );
        let params = &value["params"];
        assert!(
            params.get("compensationSide").is_some(),
            "compensationSide must be camelCase"
        );
    }
}
