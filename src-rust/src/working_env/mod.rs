//! Working environment: machine setups, tools, and their availability matrix.
//!
//! Per `docs/phase-4-design.md` §6 the working environment is saved separately
//! from `.jcam` project files because it describes the user's CNC hardware
//! rather than any particular project. Setups and tools are modeled as
//! *separate* collections (the same tool often fits more than one setup); an
//! [`AvailabilityMatrix`] records which `(setup, tool)` pairs are compatible.
//!
//! Resolve-by-id helpers and the typed errors they produce land in a follow-up.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::types::BoxDimensions;

// ── Identifiers ───────────────────────────────────────────────────────────────

/// Stable identifier for a [`MachineSetup`]. UUID strings are the intended
/// source; validation lands with the resolve-by-id helpers in a follow-up.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SetupId(String);

impl SetupId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable identifier for a [`Tool`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolId(String);

impl ToolId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ── Machine setup ─────────────────────────────────────────────────────────────

/// Bundle of safety-related machine parameters carried on every setup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetyParams {
    /// Clearance height above stock for rapid moves, in mm.
    pub safe_z: f64,
    /// Maximum rapid-traverse feed rate, in mm/min.
    pub rapid_feed_rate: f64,
}

/// A per-mode hardware bundle: workspace bounds, kinematics, post-processor,
/// safety parameters. The user may have several — e.g. one per physical CNC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineSetup {
    pub id: SetupId,
    pub name: String,
    pub workspace: BoxDimensions,
    /// Kinematics identifier token (e.g. `"3-axis-router"`).
    pub kinematics: String,
    /// Post-processor identifier token (e.g. `"grbl-1.1"`).
    pub post_processor: String,
    pub safety: SafetyParams,
}

// ── Tool ──────────────────────────────────────────────────────────────────────

/// Recommended cutting parameters for a [`Tool`]. Optional starting point —
/// the planner may override on a per-operation basis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedsAndSpeeds {
    /// Spindle speed, in RPM.
    pub spindle_rpm: f64,
    /// Lateral feed rate, in mm/min.
    pub feed_rate: f64,
    /// Z-axis plunge feed rate, in mm/min.
    pub plunge_rate: f64,
}

/// Cutter geometry plus material and recommended feeds/speeds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub id: ToolId,
    pub name: String,
    /// Cutter diameter, in mm.
    pub diameter: f64,
    pub flute_count: u32,
    /// Overall length, in mm.
    pub length: f64,
    pub material: String,
    pub recommended: FeedsAndSpeeds,
}

// ── Availability matrix ───────────────────────────────────────────────────────

/// One `(setup, tool)` compatibility entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilityPair {
    pub setup_id: SetupId,
    pub tool_id: ToolId,
}

impl AvailabilityPair {
    pub fn new(setup_id: SetupId, tool_id: ToolId) -> Self {
        Self { setup_id, tool_id }
    }
}

/// Records which tools are usable on which setups. Stored as a sorted set so
/// serialization is deterministic and `contains` is `O(log n)`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AvailabilityMatrix {
    pairs: BTreeSet<AvailabilityPair>,
}

impl AvailabilityMatrix {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if the pair was newly inserted.
    pub fn insert(&mut self, setup_id: SetupId, tool_id: ToolId) -> bool {
        self.pairs.insert(AvailabilityPair::new(setup_id, tool_id))
    }

    /// Returns `true` if the pair was present and removed.
    pub fn remove(&mut self, setup_id: &SetupId, tool_id: &ToolId) -> bool {
        self.pairs
            .remove(&AvailabilityPair::new(setup_id.clone(), tool_id.clone()))
    }

    pub fn contains(&self, setup_id: &SetupId, tool_id: &ToolId) -> bool {
        self.pairs
            .contains(&AvailabilityPair::new(setup_id.clone(), tool_id.clone()))
    }

    pub fn iter(&self) -> impl Iterator<Item = &AvailabilityPair> {
        self.pairs.iter()
    }

    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

// ── Aggregate ─────────────────────────────────────────────────────────────────

/// The persisted working environment — all setups, all tools, and the
/// compatibility matrix between them. Saved separately from project files.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkingEnvironment {
    pub setups: Vec<MachineSetup>,
    pub tools: Vec<Tool>,
    pub availability: AvailabilityMatrix,
}

impl WorkingEnvironment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_setup(&mut self, setup: MachineSetup) {
        self.setups.push(setup);
    }

    pub fn add_tool(&mut self, tool: Tool) {
        self.tools.push(tool);
    }

    pub fn setups(&self) -> &[MachineSetup] {
        &self.setups
    }

    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }

    pub fn availability(&self) -> &AvailabilityMatrix {
        &self.availability
    }

    pub fn availability_mut(&mut self) -> &mut AvailabilityMatrix {
        &mut self.availability
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Vec3;

    fn sample_setup(id: &str) -> MachineSetup {
        MachineSetup {
            id: SetupId::new(id),
            name: format!("Setup {id}"),
            workspace: BoxDimensions {
                origin: Vec3::zero(),
                width: 300.0,
                depth: 200.0,
                height: 80.0,
            },
            kinematics: "3-axis-router".into(),
            post_processor: "grbl-1.1".into(),
            safety: SafetyParams {
                safe_z: 5.0,
                rapid_feed_rate: 3000.0,
            },
        }
    }

    fn sample_tool(id: &str) -> Tool {
        Tool {
            id: ToolId::new(id),
            name: format!("Tool {id}"),
            diameter: 3.175,
            flute_count: 2,
            length: 38.0,
            material: "carbide".into(),
            recommended: FeedsAndSpeeds {
                spindle_rpm: 18000.0,
                feed_rate: 800.0,
                plunge_rate: 200.0,
            },
        }
    }

    #[test]
    fn id_constructors_round_trip_string() {
        let s = SetupId::new("setup-uuid-1");
        let t = ToolId::new("tool-uuid-1");
        assert_eq!(s.as_str(), "setup-uuid-1");
        assert_eq!(t.as_str(), "tool-uuid-1");
    }

    #[test]
    fn ids_serialize_transparently_as_strings() {
        let s = SetupId::new("abc");
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v, serde_json::json!("abc"));

        let back: SetupId = serde_json::from_value(serde_json::json!("xyz")).unwrap();
        assert_eq!(back, SetupId::new("xyz"));
    }

    #[test]
    fn working_environment_new_is_empty() {
        let env = WorkingEnvironment::new();
        assert!(env.setups().is_empty());
        assert!(env.tools().is_empty());
        assert!(env.availability().is_empty());
    }

    #[test]
    fn add_setup_and_tool_appends_and_iterates() {
        let mut env = WorkingEnvironment::new();
        env.add_setup(sample_setup("s1"));
        env.add_setup(sample_setup("s2"));
        env.add_tool(sample_tool("t1"));

        let setup_ids: Vec<&str> = env.setups().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(setup_ids, vec!["s1", "s2"]);

        let tool_ids: Vec<&str> = env.tools().iter().map(|t| t.id.as_str()).collect();
        assert_eq!(tool_ids, vec!["t1"]);
    }

    #[test]
    fn availability_matrix_insert_contains_remove() {
        let mut matrix = AvailabilityMatrix::new();
        let s = SetupId::new("s1");
        let t = ToolId::new("t1");

        assert!(!matrix.contains(&s, &t));
        assert!(matrix.insert(s.clone(), t.clone()));
        assert!(matrix.contains(&s, &t));
        assert_eq!(matrix.len(), 1);

        // Re-inserting the same pair is a no-op.
        assert!(!matrix.insert(s.clone(), t.clone()));
        assert_eq!(matrix.len(), 1);

        assert!(matrix.remove(&s, &t));
        assert!(!matrix.contains(&s, &t));
        assert!(matrix.is_empty());
    }

    #[test]
    fn availability_matrix_iterates_in_sorted_order() {
        let mut matrix = AvailabilityMatrix::new();
        matrix.insert(SetupId::new("s2"), ToolId::new("t1"));
        matrix.insert(SetupId::new("s1"), ToolId::new("t2"));
        matrix.insert(SetupId::new("s1"), ToolId::new("t1"));

        let pairs: Vec<(&str, &str)> = matrix
            .iter()
            .map(|p| (p.setup_id.as_str(), p.tool_id.as_str()))
            .collect();
        assert_eq!(pairs, vec![("s1", "t1"), ("s1", "t2"), ("s2", "t1")]);
    }

    #[test]
    fn machine_setup_serializes_camel_case() {
        let setup = sample_setup("s1");
        let v = serde_json::to_value(&setup).unwrap();
        assert_eq!(v["id"], "s1");
        assert_eq!(v["postProcessor"], "grbl-1.1");
        assert_eq!(v["safety"]["safeZ"], 5.0);
        assert_eq!(v["safety"]["rapidFeedRate"], 3000.0);
        assert_eq!(v["workspace"]["width"], 300.0);
    }

    #[test]
    fn tool_serializes_camel_case() {
        let tool = sample_tool("t1");
        let v = serde_json::to_value(&tool).unwrap();
        assert_eq!(v["id"], "t1");
        assert_eq!(v["fluteCount"], 2);
        assert_eq!(v["recommended"]["spindleRpm"], 18000.0);
        assert_eq!(v["recommended"]["plungeRate"], 200.0);
    }

    #[test]
    fn availability_matrix_serializes_as_pair_array() {
        let mut matrix = AvailabilityMatrix::new();
        matrix.insert(SetupId::new("s1"), ToolId::new("t1"));
        matrix.insert(SetupId::new("s2"), ToolId::new("t1"));

        let v = serde_json::to_value(&matrix).unwrap();
        let arr = v.as_array().expect("matrix serializes as array");
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["setupId"], "s1");
        assert_eq!(arr[0]["toolId"], "t1");
        assert_eq!(arr[1]["setupId"], "s2");
    }

    #[test]
    fn working_environment_round_trips_via_json() {
        let mut env = WorkingEnvironment::new();
        env.add_setup(sample_setup("s1"));
        env.add_setup(sample_setup("s2"));
        env.add_tool(sample_tool("t1"));
        env.add_tool(sample_tool("t2"));
        env.availability_mut()
            .insert(SetupId::new("s1"), ToolId::new("t1"));
        env.availability_mut()
            .insert(SetupId::new("s1"), ToolId::new("t2"));
        env.availability_mut()
            .insert(SetupId::new("s2"), ToolId::new("t2"));

        let json = serde_json::to_string(&env).unwrap();
        let back: WorkingEnvironment = serde_json::from_str(&json).unwrap();
        assert_eq!(back, env);
    }

    #[test]
    fn empty_working_environment_round_trips() {
        let env = WorkingEnvironment::new();
        let json = serde_json::to_string(&env).unwrap();
        let back: WorkingEnvironment = serde_json::from_str(&json).unwrap();
        assert_eq!(back, env);
    }
}
