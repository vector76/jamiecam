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
    /// Maximum depth per pass in project units; `None` or `Some(v <= 0)` → single pass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stepdown: Option<f64>,
    /// Which side of the path the tool compensates to.
    pub compensation_side: CompensationSide,
    /// Face fingerprints that define the machining boundary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_lead_in_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_lead_out_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helical_entry_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helical_entry_pitch: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ramp_entry_angle_deg: Option<f64>,
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
    /// Face fingerprints that define the machining boundary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_lead_in_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_lead_out_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helical_entry_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helical_entry_pitch: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ramp_entry_angle_deg: Option<f64>,
}

/// A single drill point location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrillPoint {
    pub x: f64,
    pub y: f64,
}

/// Parameters for a Drill operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrillParams {
    /// Drill depth in project units.
    pub depth: f64,
    /// Points to drill.
    #[serde(default)]
    pub points: Vec<DrillPoint>,
    /// Peck increment in project units; `null` for full-depth (non-peck) drilling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peck_depth: Option<f64>,
}

/// Parameters for a Z-Level Roughing operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZLevelRoughingParams {
    /// Cut depth in project units.
    pub depth: f64,
    /// Maximum depth per pass in project units.
    pub stepdown: f64,
    /// Radial stepover as a fraction of tool diameter (0–1).
    pub stepover: f64,
    /// Face fingerprints that define the machining boundary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_lead_in_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_lead_out_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helical_entry_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helical_entry_pitch: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ramp_entry_angle_deg: Option<f64>,
}

/// Parameters for a Z-Level Finishing operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZLevelFinishingParams {
    /// Cut depth in project units.
    pub depth: f64,
    /// Maximum depth per pass in project units.
    pub stepdown: f64,
    /// Material left on walls/floors before the finishing pass.
    pub finishing_allowance: f64,
    /// Whether to run an extra spring pass with zero offset.
    #[serde(default)]
    pub spring_pass: bool,
    /// Face fingerprints that define the machining boundary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_lead_in_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_lead_out_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helical_entry_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helical_entry_pitch: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ramp_entry_angle_deg: Option<f64>,
    /// Whether to enable rest machining (only cut material left by a prior op).
    #[serde(default)]
    pub rest_machining: bool,
    /// ID of the reference operation for rest machining.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest_machining_reference_id: Option<String>,
}

/// Parameters for an Adaptive Clearing operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdaptiveClearingParams {
    /// Cut depth in project units.
    pub depth: f64,
    /// Maximum depth per pass in project units.
    pub stepdown: f64,
    /// Optimal tool load as a fraction of tool diameter (e.g. 0.25).
    pub optimal_load: f64,
    /// Radial stepover as a percentage of tool diameter (0–100).
    pub stepover_percent: f64,
    /// Face fingerprints that define the machining boundary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_lead_in_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_lead_out_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helical_entry_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helical_entry_pitch: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ramp_entry_angle_deg: Option<f64>,
}

/// Parameters for a Parallel Finishing operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParallelFinishingParams {
    /// Raster stepover distance in mm.
    #[serde(default = "default_pf_stepover")]
    pub stepover: f64,
    /// Raster direction angle in degrees (0 = X-axis).
    #[serde(default)]
    pub direction_angle_deg: f64,
    /// Stock allowance in mm (offset along surface normal).
    #[serde(default)]
    pub allowance: f64,
    /// Face fingerprints that define the surface selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_lead_in_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arc_lead_out_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helical_entry_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helical_entry_pitch: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ramp_entry_angle_deg: Option<f64>,
}

fn default_pf_stepover() -> f64 {
    0.5
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
    ZLevelRoughing(ZLevelRoughingParams),
    ZLevelFinishing(ZLevelFinishingParams),
    AdaptiveClearing(AdaptiveClearingParams),
    #[serde(rename = "parallelFinishing")]
    ParallelFinishing(ParallelFinishingParams),
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
    /// Optional spindle speed override in RPM; `null` to use the tool default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_speed_override: Option<u32>,
    /// Optional feed rate override in mm/min; `null` to use the tool default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_rate_override: Option<f64>,
    /// Type and parameters specific to this operation kind.
    #[serde(flatten)]
    pub params: OperationParams,
    #[serde(default)]
    pub cache: CacheState,
}

fn default_enabled() -> bool {
    true
}

/// Stats snapshot stored alongside the cache key.
/// Uses u32 for counts (not usize) for cross-platform serialisation stability.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CachedStats {
    pub total_pass_count: u32,
    pub total_point_count: u32,
    pub total_path_length_mm: f64,
}

/// Records the cache key and validity state for an operation's toolpath.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CacheState {
    /// SHA-256 key computed at last successful calculate, e.g. "sha256:abc...".
    pub key: Option<String>,
    /// True when the stored toolpath is considered current.
    pub valid: bool,
    /// UTC ISO-8601 timestamp of last successful calculate.
    pub computed_at: Option<String>,
    /// Summary stats from the last successful calculate.
    pub stats: Option<CachedStats>,
    /// ZIP-internal path where the toolpath is persisted, e.g. "toolpaths/<uuid>.json".
    pub binary_file: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_id() -> Uuid {
        Uuid::parse_str("7f3c1a00-0000-0000-0000-000000000001").unwrap()
    }

    fn make_zlevel_op() -> Operation {
        Operation {
            id: Uuid::parse_str("dddd0000-0000-0000-0000-000000000004").unwrap(),
            name: "Z-Level Rough".to_string(),
            enabled: true,
            tool_id: tool_id(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::ZLevelRoughing(ZLevelRoughingParams {
                depth: 8.0,
                stepdown: 2.0,
                stepover: 0.5,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        }
    }

    fn make_profile_op() -> Operation {
        Operation {
            id: Uuid::parse_str("aaaa0000-0000-0000-0000-000000000001").unwrap(),
            name: "Outer Profile".to_string(),
            enabled: true,
            tool_id: tool_id(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Profile(ProfileParams {
                depth: 10.0,
                stepdown: Some(2.5),
                compensation_side: CompensationSide::Left,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        }
    }

    fn make_pocket_op() -> Operation {
        Operation {
            id: Uuid::parse_str("bbbb0000-0000-0000-0000-000000000002").unwrap(),
            name: "Rough Pocket".to_string(),
            enabled: true,
            tool_id: tool_id(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Pocket(PocketParams {
                depth: 15.0,
                stepdown: 3.0,
                stepover_percent: 45.0,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        }
    }

    fn make_drill_op() -> Operation {
        Operation {
            id: Uuid::parse_str("cccc0000-0000-0000-0000-000000000003").unwrap(),
            name: "Drill Holes".to_string(),
            enabled: true,
            tool_id: tool_id(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Drill(DrillParams {
                depth: 20.0,
                points: vec![],
                peck_depth: Some(5.0),
            }),
            cache: CacheState::default(),
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
    fn zlevel_roughing_round_trips() {
        let params = ZLevelRoughingParams {
            depth: 5.0,
            stepdown: 1.0,
            stepover: 0.5,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let op = OperationParams::ZLevelRoughing(params);
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("\"type\":\"z_level_roughing\""));
        let back: OperationParams = serde_json::from_str(&json).unwrap();
        assert_eq!(op, back);
    }

    #[test]
    fn zlevel_roughing_operation_serde_round_trip() {
        let original = make_zlevel_op();
        let json = serde_json::to_string(&original).expect("serialize");
        let recovered: Operation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, recovered);
    }

    #[test]
    fn zlevel_roughing_type_field_is_z_level_roughing() {
        let op = make_zlevel_op();
        let value = serde_json::to_value(&op).expect("to_value");
        assert_eq!(value["type"], "z_level_roughing");
        assert!(value.get("params").is_some());
    }

    #[test]
    fn drill_peck_depth_absent_when_none() {
        let op = Operation {
            id: Uuid::new_v4(),
            name: "Full-Depth Drill".to_string(),
            enabled: true,
            tool_id: tool_id(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Drill(DrillParams {
                depth: 20.0,
                points: vec![],
                peck_depth: None,
            }),
            cache: CacheState::default(),
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

    #[test]
    fn drill_point_serde_round_trip() {
        let point = DrillPoint { x: 1.5, y: -2.75 };
        let json = serde_json::to_string(&point).expect("serialize");
        let recovered: DrillPoint = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(point, recovered);
    }

    #[test]
    fn drill_params_with_points_round_trip() {
        let params = DrillParams {
            depth: 10.0,
            points: vec![DrillPoint { x: 0.0, y: 0.0 }, DrillPoint { x: 5.0, y: 5.0 }],
            peck_depth: None,
        };
        let json = serde_json::to_string(&params).expect("serialize");
        let recovered: DrillParams = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(params, recovered);
    }

    #[test]
    fn drill_params_points_defaults_to_empty_vec() {
        let json = r#"{"depth": 10.0}"#;
        let params: DrillParams = serde_json::from_str(json).expect("deserialize");
        assert_eq!(params.points, vec![], "points must default to empty vec");
    }

    #[test]
    fn operation_override_fields_absent_when_none() {
        let op = Operation {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            enabled: true,
            tool_id: tool_id(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Pocket(PocketParams {
                depth: 5.0,
                stepdown: 1.0,
                stepover_percent: 50.0,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };
        let value = serde_json::to_value(&op).expect("to_value");
        assert!(
            value.get("spindleSpeedOverride").is_none(),
            "spindleSpeedOverride must be absent when None"
        );
        assert!(
            value.get("feedRateOverride").is_none(),
            "feedRateOverride must be absent when None"
        );
    }

    #[test]
    fn operation_override_fields_present_when_set() {
        let op = Operation {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            enabled: true,
            tool_id: tool_id(),
            spindle_speed_override: Some(12000),
            feed_rate_override: Some(800.0),
            params: OperationParams::Pocket(PocketParams {
                depth: 5.0,
                stepdown: 1.0,
                stepover_percent: 50.0,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };
        let value = serde_json::to_value(&op).expect("to_value");
        assert_eq!(
            value["spindleSpeedOverride"], 12000,
            "spindleSpeedOverride must be 12000"
        );
        assert_eq!(
            value["feedRateOverride"], 800.0,
            "feedRateOverride must be 800.0"
        );
    }

    #[test]
    fn operation_override_fields_default_to_none_when_absent() {
        let json = r#"{
            "id": "aaaa0000-0000-0000-0000-000000000001",
            "name": "Test",
            "toolId": "7f3c1a00-0000-0000-0000-000000000001",
            "type": "pocket",
            "params": { "depth": 5.0, "stepdown": 1.0, "stepoverPercent": 50.0 }
        }"#;
        let op: Operation = serde_json::from_str(json).expect("deserialize");
        assert!(
            op.spindle_speed_override.is_none(),
            "spindle_speed_override must default to None"
        );
        assert!(
            op.feed_rate_override.is_none(),
            "feed_rate_override must default to None"
        );
    }

    #[test]
    fn cache_field_defaults_when_absent() {
        let json = r#"{
            "id": "aaaa0000-0000-0000-0000-000000000001",
            "name": "Test",
            "toolId": "7f3c1a00-0000-0000-0000-000000000001",
            "type": "pocket",
            "params": { "depth": 5.0, "stepdown": 1.0, "stepoverPercent": 50.0 }
        }"#;
        let op: Operation = serde_json::from_str(json).expect("deserialize");
        assert_eq!(op.cache, CacheState::default());
    }

    #[test]
    fn pocket_geometry_absent_when_none() {
        let params = PocketParams {
            depth: 5.0,
            stepdown: 1.0,
            stepover_percent: 50.0,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let value = serde_json::to_value(&params).expect("to_value");
        assert!(
            value.get("geometry").is_none(),
            "geometry must be absent when None"
        );
    }

    #[test]
    fn pocket_geometry_present_when_set() {
        let params = PocketParams {
            depth: 5.0,
            stepdown: 1.0,
            stepover_percent: 50.0,
            geometry: Some(vec!["fp1".into()]),
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let value = serde_json::to_value(&params).expect("to_value");
        assert_eq!(value["geometry"][0], "fp1");
    }

    #[test]
    fn pocket_geometry_round_trip_with_fingerprints() {
        let params = PocketParams {
            depth: 5.0,
            stepdown: 1.0,
            stepover_percent: 50.0,
            geometry: Some(vec!["abc".into()]),
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let json = serde_json::to_string(&params).expect("serialize");
        let recovered: PocketParams = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(params, recovered);
    }

    #[test]
    fn pocket_geometry_defaults_absent_in_old_json() {
        let json = r#"{"depth": 5.0, "stepdown": 1.0, "stepoverPercent": 50.0}"#;
        let params: PocketParams = serde_json::from_str(json).expect("deserialize");
        assert!(
            params.geometry.is_none(),
            "geometry must default to None when absent"
        );
    }

    #[test]
    fn profile_geometry_absent_when_none() {
        let params = ProfileParams {
            depth: 10.0,
            stepdown: Some(2.5),
            compensation_side: CompensationSide::Left,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let value = serde_json::to_value(&params).expect("to_value");
        assert!(
            value.get("geometry").is_none(),
            "geometry must be absent when None"
        );
    }

    #[test]
    fn profile_geometry_present_when_set() {
        let params = ProfileParams {
            depth: 10.0,
            stepdown: Some(2.5),
            compensation_side: CompensationSide::Left,
            geometry: Some(vec!["fp1".into()]),
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let value = serde_json::to_value(&params).expect("to_value");
        assert_eq!(value["geometry"][0], "fp1");
    }

    #[test]
    fn profile_geometry_round_trip_with_fingerprints() {
        let params = ProfileParams {
            depth: 10.0,
            stepdown: Some(2.5),
            compensation_side: CompensationSide::Left,
            geometry: Some(vec!["abc".into()]),
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let json = serde_json::to_string(&params).expect("serialize");
        let recovered: ProfileParams = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(params, recovered);
    }

    #[test]
    fn profile_geometry_defaults_absent_in_old_json() {
        let json = r#"{"depth": 10.0, "stepdown": 2.5, "compensationSide": "left"}"#;
        let params: ProfileParams = serde_json::from_str(json).expect("deserialize");
        assert!(
            params.geometry.is_none(),
            "geometry must default to None when absent"
        );
    }

    #[test]
    fn zlevel_roughing_geometry_absent_when_none() {
        let params = ZLevelRoughingParams {
            depth: 8.0,
            stepdown: 2.0,
            stepover: 0.5,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let value = serde_json::to_value(&params).expect("to_value");
        assert!(
            value.get("geometry").is_none(),
            "geometry must be absent when None"
        );
    }

    #[test]
    fn zlevel_roughing_geometry_present_when_set() {
        let params = ZLevelRoughingParams {
            depth: 8.0,
            stepdown: 2.0,
            stepover: 0.5,
            geometry: Some(vec!["fp1".into()]),
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let value = serde_json::to_value(&params).expect("to_value");
        assert_eq!(value["geometry"][0], "fp1");
    }

    #[test]
    fn zlevel_roughing_geometry_round_trip_with_fingerprints() {
        let params = ZLevelRoughingParams {
            depth: 8.0,
            stepdown: 2.0,
            stepover: 0.5,
            geometry: Some(vec!["abc".into()]),
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        };
        let json = serde_json::to_string(&params).expect("serialize");
        let recovered: ZLevelRoughingParams = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(params, recovered);
    }

    #[test]
    fn zlevel_roughing_geometry_defaults_absent_in_old_json() {
        let json = r#"{"depth": 8.0, "stepdown": 2.0, "stepover": 0.5}"#;
        let params: ZLevelRoughingParams = serde_json::from_str(json).expect("deserialize");
        assert!(
            params.geometry.is_none(),
            "geometry must default to None when absent"
        );
    }

    #[test]
    fn adaptive_clearing_serde_round_trip() {
        let params = AdaptiveClearingParams {
            depth: 10.0,
            stepdown: 2.0,
            optimal_load: 0.25,
            stepover_percent: 40.0,
            geometry: Some(vec!["face1".into(), "face2".into()]),
            arc_lead_in_radius: Some(3.0),
            arc_lead_out_radius: Some(2.5),
            helical_entry_radius: Some(4.0),
            helical_entry_pitch: Some(1.5),
            ramp_entry_angle_deg: Some(5.0),
        };
        let op = OperationParams::AdaptiveClearing(params);
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("\"type\":\"adaptive_clearing\""));
        let back: OperationParams = serde_json::from_str(&json).unwrap();
        assert_eq!(op, back);
    }

    #[test]
    fn adaptive_clearing_operation_round_trip() {
        let original = Operation {
            id: Uuid::parse_str("eeee0000-0000-0000-0000-000000000005").unwrap(),
            name: "Adaptive Clear".to_string(),
            enabled: true,
            tool_id: tool_id(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::AdaptiveClearing(AdaptiveClearingParams {
                depth: 12.0,
                stepdown: 3.0,
                optimal_load: 0.25,
                stepover_percent: 40.0,
                geometry: Some(vec!["fp1".into()]),
                arc_lead_in_radius: Some(2.0),
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState::default(),
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let recovered: Operation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, recovered);
    }

    /// Existing variants must still deserialize after adding AdaptiveClearing.
    #[test]
    fn existing_variants_still_deserialize_after_adaptive_clearing() {
        // Profile
        let profile = make_profile_op();
        let json = serde_json::to_string(&profile).unwrap();
        let back: Operation = serde_json::from_str(&json).unwrap();
        assert_eq!(profile, back);

        // Pocket
        let pocket = make_pocket_op();
        let json = serde_json::to_string(&pocket).unwrap();
        let back: Operation = serde_json::from_str(&json).unwrap();
        assert_eq!(pocket, back);

        // Drill
        let drill = make_drill_op();
        let json = serde_json::to_string(&drill).unwrap();
        let back: Operation = serde_json::from_str(&json).unwrap();
        assert_eq!(drill, back);

        // ZLevelRoughing
        let zlr = make_zlevel_op();
        let json = serde_json::to_string(&zlr).unwrap();
        let back: Operation = serde_json::from_str(&json).unwrap();
        assert_eq!(zlr, back);
    }

    #[test]
    fn cache_state_round_trip() {
        let op = Operation {
            id: Uuid::parse_str("aaaa0000-0000-0000-0000-000000000001").unwrap(),
            name: "Cached Op".to_string(),
            enabled: true,
            tool_id: tool_id(),
            spindle_speed_override: None,
            feed_rate_override: None,
            params: OperationParams::Pocket(PocketParams {
                depth: 5.0,
                stepdown: 1.0,
                stepover_percent: 50.0,
                geometry: None,
                arc_lead_in_radius: None,
                arc_lead_out_radius: None,
                helical_entry_radius: None,
                helical_entry_pitch: None,
                ramp_entry_angle_deg: None,
            }),
            cache: CacheState {
                key: Some("sha256:abcdef1234567890".to_string()),
                valid: true,
                computed_at: Some("2026-03-01T00:00:00Z".to_string()),
                stats: Some(CachedStats {
                    total_pass_count: 3,
                    total_point_count: 150,
                    total_path_length_mm: 1234.5,
                }),
                binary_file: Some("toolpaths/some-uuid.json".to_string()),
            },
        };
        let json = serde_json::to_string(&op).expect("serialize");
        let recovered: Operation = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(op, recovered);
    }

    #[test]
    fn parallel_finishing_params_serialize_as_camel_case_type() {
        let op = OperationParams::ParallelFinishing(ParallelFinishingParams {
            stepover: 0.5,
            direction_angle_deg: 0.0,
            allowance: 0.0,
            geometry: None,
            arc_lead_in_radius: None,
            arc_lead_out_radius: None,
            helical_entry_radius: None,
            helical_entry_pitch: None,
            ramp_entry_angle_deg: None,
        });
        let val = serde_json::to_value(&op).unwrap();
        assert_eq!(val["type"], "parallelFinishing");
        assert!(val["params"].get("directionAngleDeg").is_some());
    }
}
