//! Modal group classification and state update logic for G-code parsing.

#![allow(dead_code)]

use crate::models::Vec3;

use super::state::{DistanceMode, ModalState, MotionMode};
use super::types::{FeedMode, Plane, SegmentMetadata, SpindleDir, Units};

/// Modal group classification for G-codes per ISO 6983.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ModalGroup {
    Motion,
    PlaneSelection,
    DistanceMode,
    FeedMode,
    Units,
    CannedCycle,
    CycleRetract,
    NonModal,
    ToolLengthComp,
    WorkOffset,
    CutterComp,
}

/// M-code action classification.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MCodeAction {
    /// M0, M1 — program stop (no parser state effect).
    Stop,
    /// M2, M30 — program end.
    ProgramEnd,
    /// M3, M4, M5 — spindle control.
    Spindle,
    /// M6 — tool change.
    ToolChange,
    /// M8, M9 — coolant (no parser state effect).
    Coolant,
    /// M98, M99 — subprogram call/return (generates warning).
    Subprogram,
}

/// Classify a G-code number into its modal group.
pub(crate) fn classify_gcode(value: f64) -> Option<ModalGroup> {
    let int_val = value.round() as i32;
    if (value - int_val as f64).abs() > 0.001 {
        return None;
    }
    match int_val {
        0..=3 => Some(ModalGroup::Motion),
        17..=19 => Some(ModalGroup::PlaneSelection),
        90 | 91 => Some(ModalGroup::DistanceMode),
        93..=95 => Some(ModalGroup::FeedMode),
        20 | 21 => Some(ModalGroup::Units),
        73 | 80..=83 => Some(ModalGroup::CannedCycle),
        98 | 99 => Some(ModalGroup::CycleRetract),
        4 | 28 => Some(ModalGroup::NonModal),
        43 | 49 => Some(ModalGroup::ToolLengthComp),
        54..=59 => Some(ModalGroup::WorkOffset),
        40..=42 => Some(ModalGroup::CutterComp),
        _ => None,
    }
}

/// Classify an M-code number into its action category.
pub(crate) fn classify_mcode(value: f64) -> Option<MCodeAction> {
    let int_val = value.round() as i32;
    if (value - int_val as f64).abs() > 0.001 {
        return None;
    }
    match int_val {
        0 | 1 => Some(MCodeAction::Stop),
        2 | 30 => Some(MCodeAction::ProgramEnd),
        3..=5 => Some(MCodeAction::Spindle),
        6 => Some(MCodeAction::ToolChange),
        8 | 9 => Some(MCodeAction::Coolant),
        98 | 99 => Some(MCodeAction::Subprogram),
        _ => None,
    }
}

// --- State mutation methods on ModalState ---

impl ModalState {
    /// Resolve an endpoint from optional axis values, applying distance mode and unit conversion.
    pub fn resolve_position(&self, x: Option<f64>, y: Option<f64>, z: Option<f64>) -> Vec3 {
        let scale = match self.units {
            Units::Imperial => 25.4,
            Units::Metric => 1.0,
        };

        match self.distance_mode {
            DistanceMode::Absolute => Vec3 {
                x: x.map(|v| v * scale).unwrap_or(self.position.x),
                y: y.map(|v| v * scale).unwrap_or(self.position.y),
                z: z.map(|v| v * scale).unwrap_or(self.position.z),
            },
            DistanceMode::Incremental => Vec3 {
                x: self.position.x + x.map(|v| v * scale).unwrap_or(0.0),
                y: self.position.y + y.map(|v| v * scale).unwrap_or(0.0),
                z: self.position.z + z.map(|v| v * scale).unwrap_or(0.0),
            },
        }
    }

    /// Normalize a feed rate value to mm-based units.
    /// G93 (inverse time): stored raw. G94/G95 with G20: multiply by 25.4.
    pub fn normalize_feed_rate(&self, f_value: f64) -> f64 {
        match self.feed_mode {
            FeedMode::InverseTime => f_value,
            FeedMode::PerMinute | FeedMode::PerRevolution => match self.units {
                Units::Imperial => f_value * 25.4,
                Units::Metric => f_value,
            },
        }
    }

    pub fn set_motion_mode(&mut self, mode: MotionMode) {
        self.motion_mode = Some(mode);
    }

    pub fn set_plane(&mut self, plane: Plane) {
        self.plane = plane;
    }

    pub fn set_distance_mode(&mut self, mode: DistanceMode) {
        self.distance_mode = mode;
    }

    pub fn set_feed_mode(&mut self, mode: FeedMode) {
        self.feed_mode = mode;
    }

    /// Set units. Changing units does NOT retroactively convert position or feed rate.
    pub fn set_units(&mut self, units: Units) {
        self.units = units;
    }

    pub fn set_spindle(&mut self, dir: SpindleDir) {
        self.spindle_dir = dir;
    }

    pub fn set_spindle_speed(&mut self, rpm: f64) {
        self.spindle_speed = rpm;
    }

    pub fn stage_tool(&mut self, number: u32) {
        self.staged_tool = number;
    }

    /// Move staged tool to active. Returns the new active tool number.
    pub fn activate_tool(&mut self) -> u32 {
        self.tool_number = self.staged_tool;
        self.tool_number
    }

    /// Snapshot current state into segment metadata.
    pub fn build_segment_metadata(&self, source_line: usize) -> SegmentMetadata {
        SegmentMetadata {
            source_line,
            tool_number: self.tool_number,
            spindle_speed: self.spindle_speed,
            spindle_dir: self.spindle_dir.clone(),
            feed_mode: self.feed_mode.clone(),
        }
    }
}

#[cfg(test)]
// Test fixtures deliberately use `let mut x = T::default(); x.field = ...;`
// for readability when only a couple of fields differ from defaults.
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::super::state::RetractMode;
    use super::*;

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    // --- Modal group classification ---

    #[test]
    fn classify_motion_codes() {
        assert_eq!(classify_gcode(0.0), Some(ModalGroup::Motion));
        assert_eq!(classify_gcode(1.0), Some(ModalGroup::Motion));
        assert_eq!(classify_gcode(2.0), Some(ModalGroup::Motion));
        assert_eq!(classify_gcode(3.0), Some(ModalGroup::Motion));
    }

    #[test]
    fn classify_plane_selection() {
        assert_eq!(classify_gcode(17.0), Some(ModalGroup::PlaneSelection));
        assert_eq!(classify_gcode(18.0), Some(ModalGroup::PlaneSelection));
        assert_eq!(classify_gcode(19.0), Some(ModalGroup::PlaneSelection));
    }

    #[test]
    fn classify_distance_and_feed_and_units() {
        assert_eq!(classify_gcode(90.0), Some(ModalGroup::DistanceMode));
        assert_eq!(classify_gcode(91.0), Some(ModalGroup::DistanceMode));
        assert_eq!(classify_gcode(93.0), Some(ModalGroup::FeedMode));
        assert_eq!(classify_gcode(94.0), Some(ModalGroup::FeedMode));
        assert_eq!(classify_gcode(95.0), Some(ModalGroup::FeedMode));
        assert_eq!(classify_gcode(20.0), Some(ModalGroup::Units));
        assert_eq!(classify_gcode(21.0), Some(ModalGroup::Units));
    }

    #[test]
    fn classify_canned_cycles_and_retract() {
        assert_eq!(classify_gcode(73.0), Some(ModalGroup::CannedCycle));
        assert_eq!(classify_gcode(80.0), Some(ModalGroup::CannedCycle));
        assert_eq!(classify_gcode(81.0), Some(ModalGroup::CannedCycle));
        assert_eq!(classify_gcode(82.0), Some(ModalGroup::CannedCycle));
        assert_eq!(classify_gcode(83.0), Some(ModalGroup::CannedCycle));
        assert_eq!(classify_gcode(98.0), Some(ModalGroup::CycleRetract));
        assert_eq!(classify_gcode(99.0), Some(ModalGroup::CycleRetract));
    }

    #[test]
    fn classify_non_modal_and_tool_comp() {
        assert_eq!(classify_gcode(4.0), Some(ModalGroup::NonModal));
        assert_eq!(classify_gcode(28.0), Some(ModalGroup::NonModal));
        assert_eq!(classify_gcode(43.0), Some(ModalGroup::ToolLengthComp));
        assert_eq!(classify_gcode(49.0), Some(ModalGroup::ToolLengthComp));
    }

    #[test]
    fn classify_work_offsets() {
        for g in 54..=59 {
            assert_eq!(classify_gcode(g as f64), Some(ModalGroup::WorkOffset));
        }
    }

    #[test]
    fn classify_cutter_comp() {
        assert_eq!(classify_gcode(40.0), Some(ModalGroup::CutterComp));
        assert_eq!(classify_gcode(41.0), Some(ModalGroup::CutterComp));
        assert_eq!(classify_gcode(42.0), Some(ModalGroup::CutterComp));
    }

    #[test]
    fn classify_unknown_gcode_returns_none() {
        assert_eq!(classify_gcode(999.0), None);
        assert_eq!(classify_gcode(50.0), None);
    }

    // --- M-code classification ---

    #[test]
    fn classify_mcodes() {
        assert_eq!(classify_mcode(0.0), Some(MCodeAction::Stop));
        assert_eq!(classify_mcode(1.0), Some(MCodeAction::Stop));
        assert_eq!(classify_mcode(2.0), Some(MCodeAction::ProgramEnd));
        assert_eq!(classify_mcode(30.0), Some(MCodeAction::ProgramEnd));
        assert_eq!(classify_mcode(3.0), Some(MCodeAction::Spindle));
        assert_eq!(classify_mcode(4.0), Some(MCodeAction::Spindle));
        assert_eq!(classify_mcode(5.0), Some(MCodeAction::Spindle));
        assert_eq!(classify_mcode(6.0), Some(MCodeAction::ToolChange));
        assert_eq!(classify_mcode(8.0), Some(MCodeAction::Coolant));
        assert_eq!(classify_mcode(9.0), Some(MCodeAction::Coolant));
        assert_eq!(classify_mcode(98.0), Some(MCodeAction::Subprogram));
        assert_eq!(classify_mcode(99.0), Some(MCodeAction::Subprogram));
        assert_eq!(classify_mcode(50.0), None);
    }

    // --- Default state ---

    #[test]
    fn default_state_matches_spec() {
        let s = ModalState::default();
        assert_eq!(s.plane, Plane::Xy);
        assert_eq!(s.distance_mode, DistanceMode::Absolute);
        assert_eq!(s.feed_mode, FeedMode::PerMinute);
        assert_eq!(s.units, Units::Metric);
        assert_eq!(s.spindle_dir, SpindleDir::Off);
        assert_eq!(s.retract_mode, RetractMode::InitialPoint); // G98
        assert_eq!(s.tool_number, 0);
        assert_eq!(s.position, v(0.0, 0.0, 0.0));
        assert_eq!(s.motion_mode, None);
    }

    // --- Coordinate resolution ---

    #[test]
    fn resolve_absolute_partial_axes_retains_current() {
        let mut s = ModalState::default();
        s.position = v(10.0, 20.0, 30.0);
        let pos = s.resolve_position(Some(5.0), None, Some(15.0));
        assert_eq!(pos, v(5.0, 20.0, 15.0));
    }

    #[test]
    fn resolve_incremental_accumulation() {
        let mut s = ModalState::default();
        s.distance_mode = DistanceMode::Incremental;

        let pos1 = s.resolve_position(Some(1.0), Some(2.0), Some(3.0));
        assert_eq!(pos1, v(1.0, 2.0, 3.0));

        s.position = pos1;
        let pos2 = s.resolve_position(Some(1.0), None, Some(-1.0));
        assert_eq!(pos2, v(2.0, 2.0, 2.0));

        s.position = pos2;
        let pos3 = s.resolve_position(None, Some(5.0), None);
        assert_eq!(pos3, v(2.0, 7.0, 2.0));
    }

    #[test]
    fn resolve_incremental_missing_axes_zero_increment() {
        let mut s = ModalState::default();
        s.distance_mode = DistanceMode::Incremental;
        s.position = v(10.0, 20.0, 30.0);
        let pos = s.resolve_position(None, None, None);
        assert_eq!(pos, v(10.0, 20.0, 30.0));
    }

    // --- Unit conversion ---

    #[test]
    fn resolve_imperial_coordinates_converted_to_mm() {
        let mut s = ModalState::default();
        s.units = Units::Imperial;
        let pos = s.resolve_position(Some(1.0), Some(2.0), Some(3.0));
        assert!((pos.x - 25.4).abs() < 1e-10);
        assert!((pos.y - 50.8).abs() < 1e-10);
        assert!((pos.z - 76.2).abs() < 1e-10);
    }

    #[test]
    fn resolve_imperial_incremental() {
        let mut s = ModalState::default();
        s.units = Units::Imperial;
        s.distance_mode = DistanceMode::Incremental;
        s.position = v(25.4, 0.0, 0.0); // position stored in mm
        let pos = s.resolve_position(Some(1.0), None, None);
        assert_eq!(pos, v(50.8, 0.0, 0.0));
    }

    // --- Feed rate normalization ---

    #[test]
    fn feed_rate_per_minute_metric_unchanged() {
        let s = ModalState::default();
        assert_eq!(s.normalize_feed_rate(500.0), 500.0);
    }

    #[test]
    fn feed_rate_per_minute_imperial_scaled() {
        let mut s = ModalState::default();
        s.units = Units::Imperial;
        assert_eq!(s.normalize_feed_rate(10.0), 254.0);
    }

    #[test]
    fn feed_rate_inverse_time_raw() {
        let mut s = ModalState::default();
        s.feed_mode = FeedMode::InverseTime;
        assert_eq!(s.normalize_feed_rate(42.0), 42.0);
    }

    #[test]
    fn feed_rate_inverse_time_imperial_still_raw() {
        let mut s = ModalState::default();
        s.feed_mode = FeedMode::InverseTime;
        s.units = Units::Imperial;
        assert_eq!(s.normalize_feed_rate(42.0), 42.0);
    }

    #[test]
    fn feed_rate_per_revolution_imperial_scaled() {
        let mut s = ModalState::default();
        s.feed_mode = FeedMode::PerRevolution;
        s.units = Units::Imperial;
        assert_eq!(s.normalize_feed_rate(0.1), 0.1 * 25.4);
    }

    // --- Tool staging and activation ---

    #[test]
    fn tool_staging_does_not_activate() {
        let mut s = ModalState::default();
        s.stage_tool(2);
        assert_eq!(s.tool_number, 0);
        assert_eq!(s.staged_tool, 2);
    }

    #[test]
    fn tool_activate_moves_staged_to_active() {
        let mut s = ModalState::default();
        s.stage_tool(2);
        let active = s.activate_tool();
        assert_eq!(active, 2);
        assert_eq!(s.tool_number, 2);
    }

    // --- Segment metadata ---

    #[test]
    fn build_segment_metadata_captures_state() {
        let mut s = ModalState::default();
        s.tool_number = 5;
        s.spindle_speed = 12000.0;
        s.spindle_dir = SpindleDir::Cw;
        s.feed_mode = FeedMode::PerMinute;

        let meta = s.build_segment_metadata(42);
        assert_eq!(meta.source_line, 42);
        assert_eq!(meta.tool_number, 5);
        assert_eq!(meta.spindle_speed, 12000.0);
        assert_eq!(meta.spindle_dir, SpindleDir::Cw);
        assert_eq!(meta.feed_mode, FeedMode::PerMinute);
    }
}
