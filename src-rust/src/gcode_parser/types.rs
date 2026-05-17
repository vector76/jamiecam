//! Public output types for the G-code parser.

use serde::{Deserialize, Serialize};

use crate::types::Vec3;

/// The complete result of parsing a G-code program.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedProgram {
    pub metadata: ProgramMetadata,
    pub segments: Vec<MotionSegment>,
    pub tool_changes: Vec<ToolChange>,
    pub warnings: Vec<ParseWarning>,
}

/// Header-level metadata extracted from the program.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramMetadata {
    pub program_number: Option<u32>,
    pub source_units: Units,
    pub header_comments: Vec<String>,
}

/// A resolved motion segment produced by the parser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MotionSegment {
    Rapid {
        start: Vec3,
        end: Vec3,
        metadata: SegmentMetadata,
    },
    Linear {
        start: Vec3,
        end: Vec3,
        feed_rate: f64,
        metadata: SegmentMetadata,
    },
    Arc {
        start: Vec3,
        end: Vec3,
        center: Vec3,
        clockwise: bool,
        plane: Plane,
        feed_rate: f64,
        metadata: SegmentMetadata,
    },
    Dwell {
        position: Vec3,
        seconds: f64,
        metadata: SegmentMetadata,
    },
}

/// Per-segment metadata capturing the modal state at the time of emission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentMetadata {
    pub source_line: usize,
    pub tool_number: u32,
    pub spindle_speed: f64,
    pub spindle_dir: SpindleDir,
    pub feed_mode: FeedMode,
}

/// A tool change event referencing the segment index where it occurs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolChange {
    pub segment_index: usize,
    pub tool_number: u32,
}

/// A non-fatal warning generated during parsing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseWarning {
    pub line: usize,
    pub message: String,
}

/// Active working plane for arc interpolation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Plane {
    Xy,
    Xz,
    Yz,
}

/// Spindle rotation direction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpindleDir {
    Cw,
    Ccw,
    Off,
}

/// Feed rate interpretation mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeedMode {
    PerMinute,
    InverseTime,
    PerRevolution,
}

/// Program units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Units {
    Metric,
    Imperial,
}
