//! Internal modal state tracked during G-code parsing.

#![allow(dead_code)]

use crate::models::Vec3;

use super::types::{FeedMode, Plane, SpindleDir, Units};

/// The current modal state of the parser, updated as G-code lines are processed.
pub(crate) struct ModalState {
    pub motion_mode: Option<MotionMode>,
    pub plane: Plane,
    pub distance_mode: DistanceMode,
    pub feed_mode: FeedMode,
    pub units: Units,
    pub feed_rate: f64,
    pub spindle_speed: f64,
    pub spindle_dir: SpindleDir,
    pub tool_number: u32,
    pub staged_tool: u32,
    pub position: Vec3,
    pub cycle_active: Option<CycleMode>,
    pub cycle_r: f64,
    pub cycle_z: f64,
    pub cycle_q: f64,
    pub cycle_p: f64,
    pub retract_mode: RetractMode,
    pub initial_z: f64,
}

impl Default for ModalState {
    fn default() -> Self {
        Self {
            motion_mode: None,
            plane: Plane::Xy,                      // G17
            distance_mode: DistanceMode::Absolute, // G90
            feed_mode: FeedMode::PerMinute,        // G94
            units: Units::Metric,                  // G21
            feed_rate: 0.0,
            spindle_speed: 0.0,
            spindle_dir: SpindleDir::Off,
            tool_number: 0,
            staged_tool: 0,
            position: Vec3::zero(),
            cycle_active: None,
            cycle_r: 0.0,
            cycle_z: 0.0,
            cycle_q: 0.0,
            cycle_p: 0.0,
            retract_mode: RetractMode::InitialPoint, // G98
            initial_z: 0.0,
        }
    }
}

/// Active motion mode (G0/G1/G2/G3).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MotionMode {
    Rapid,
    Linear,
    CwArc,
    CcwArc,
}

/// Distance interpretation mode (G90/G91).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DistanceMode {
    Absolute,
    Incremental,
}

/// Active canned cycle type.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CycleMode {
    G81,
    G82,
    G83,
    G73,
}

/// Canned cycle retract mode (G98/G99).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RetractMode {
    InitialPoint,
    RPlane,
}
