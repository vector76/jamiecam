/// Tracks the currently active G-code modal state for word suppression.
///
/// Each modal group holds the last-emitted value. `should_emit_*` returns `true`
/// (and updates state) when the new value differs from the cached one, or `false`
/// when it is identical and the word can be omitted.
#[derive(Default)]
pub struct ModalState {
    motion_code: Option<String>,
    feed: Option<f64>,
    spindle: Option<f64>,
    tool: Option<u32>,
    coord_x: Option<f64>,
    coord_y: Option<f64>,
    coord_z: Option<f64>,
    coord_a: Option<f64>,
    coord_b: Option<f64>,
    coord_c: Option<f64>,
    plane: Option<String>,
    distance_mode: Option<String>,
    feed_mode: Option<String>,
}

/// Tolerance for floating-point modal comparisons (coordinates, feed rate, spindle speed).
/// Suppresses redundant words when values differ only by floating-point rounding error.
const NUMERIC_TOLERANCE: f64 = 1e-6;

/// Updates `slot` with `code` if it differs; returns `true` when the caller should emit.
fn update_string_modal(slot: &mut Option<String>, code: &str) -> bool {
    if slot.as_deref() == Some(code) {
        return false;
    }
    *slot = Some(code.to_string());
    true
}

/// Updates `slot` with `value` if it differs by more than `NUMERIC_TOLERANCE`; returns `true` when the caller should emit.
fn update_float_modal(slot: &mut Option<f64>, value: f64) -> bool {
    if let Some(last) = *slot {
        if (last - value).abs() < NUMERIC_TOLERANCE {
            return false;
        }
    }
    *slot = Some(value);
    true
}

impl ModalState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` and caches `code` if it differs from the last emitted motion code.
    pub fn should_emit_motion(&mut self, code: &str) -> bool {
        update_string_modal(&mut self.motion_code, code)
    }

    /// Returns `true` and caches `feed` if it differs from the last emitted feed rate.
    pub fn should_emit_feed(&mut self, feed: f64) -> bool {
        update_float_modal(&mut self.feed, feed)
    }

    /// Returns `true` and caches `speed` if it differs from the last emitted spindle speed.
    pub fn should_emit_spindle(&mut self, speed: f64) -> bool {
        update_float_modal(&mut self.spindle, speed)
    }

    /// Returns `true` and caches `number` if it differs from the last emitted tool number.
    pub fn should_emit_tool(&mut self, number: u32) -> bool {
        if self.tool == Some(number) {
            return false;
        }
        self.tool = Some(number);
        true
    }

    /// Returns `true` and caches the coordinate if it differs by more than 1e-6 mm.
    pub fn should_emit_coord(&mut self, axis: char, value: f64) -> bool {
        let slot = match axis {
            'X' | 'x' => &mut self.coord_x,
            'Y' | 'y' => &mut self.coord_y,
            'Z' | 'z' => &mut self.coord_z,
            'A' | 'a' => &mut self.coord_a,
            'B' | 'b' => &mut self.coord_b,
            'C' | 'c' => &mut self.coord_c,
            _ => return true, // unknown axis — always emit
        };
        update_float_modal(slot, value)
    }

    /// Returns `true` and caches `code` if it differs from the last emitted plane-select code.
    pub fn should_emit_plane(&mut self, code: &str) -> bool {
        update_string_modal(&mut self.plane, code)
    }

    /// Returns `true` and caches `code` if it differs from the last emitted distance-mode code.
    pub fn should_emit_distance_mode(&mut self, code: &str) -> bool {
        update_string_modal(&mut self.distance_mode, code)
    }

    /// Returns `true` and caches `code` if it differs from the last emitted feed-mode code.
    pub fn should_emit_feed_mode(&mut self, code: &str) -> bool {
        update_string_modal(&mut self.feed_mode, code)
    }

    /// Clears all modal state (call on tool change or program reset).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── motion code ──────────────────────────────────────────────────────────

    #[test]
    fn motion_emits_first_time() {
        let mut ms = ModalState::new();
        assert!(ms.should_emit_motion("G01"));
    }

    #[test]
    fn motion_suppressed_on_repeat() {
        let mut ms = ModalState::new();
        ms.should_emit_motion("G01");
        assert!(!ms.should_emit_motion("G01"));
    }

    #[test]
    fn motion_re_emits_after_change() {
        let mut ms = ModalState::new();
        ms.should_emit_motion("G01");
        assert!(ms.should_emit_motion("G00"));
    }

    // ── feed rate ────────────────────────────────────────────────────────────

    #[test]
    fn feed_emits_first_time() {
        let mut ms = ModalState::new();
        assert!(ms.should_emit_feed(500.0));
    }

    #[test]
    fn feed_suppressed_on_repeat() {
        let mut ms = ModalState::new();
        ms.should_emit_feed(500.0);
        assert!(!ms.should_emit_feed(500.0));
    }

    #[test]
    fn feed_re_emits_after_change() {
        let mut ms = ModalState::new();
        ms.should_emit_feed(500.0);
        assert!(ms.should_emit_feed(1000.0));
    }

    // ── spindle speed ────────────────────────────────────────────────────────

    #[test]
    fn spindle_emits_first_time() {
        let mut ms = ModalState::new();
        assert!(ms.should_emit_spindle(3000.0));
    }

    #[test]
    fn spindle_suppressed_on_repeat() {
        let mut ms = ModalState::new();
        ms.should_emit_spindle(3000.0);
        assert!(!ms.should_emit_spindle(3000.0));
    }

    #[test]
    fn spindle_re_emits_after_change() {
        let mut ms = ModalState::new();
        ms.should_emit_spindle(3000.0);
        assert!(ms.should_emit_spindle(6000.0));
    }

    // ── tool number ──────────────────────────────────────────────────────────

    #[test]
    fn tool_emits_first_time() {
        let mut ms = ModalState::new();
        assert!(ms.should_emit_tool(1));
    }

    #[test]
    fn tool_suppressed_on_repeat() {
        let mut ms = ModalState::new();
        ms.should_emit_tool(1);
        assert!(!ms.should_emit_tool(1));
    }

    #[test]
    fn tool_re_emits_after_change() {
        let mut ms = ModalState::new();
        ms.should_emit_tool(1);
        assert!(ms.should_emit_tool(2));
    }

    // ── coordinate words ─────────────────────────────────────────────────────

    #[test]
    fn coord_emits_first_time() {
        let mut ms = ModalState::new();
        assert!(ms.should_emit_coord('X', 10.0));
    }

    #[test]
    fn coord_suppressed_on_repeat() {
        let mut ms = ModalState::new();
        ms.should_emit_coord('X', 10.0);
        assert!(!ms.should_emit_coord('X', 10.0));
    }

    #[test]
    fn coord_re_emits_after_change() {
        let mut ms = ModalState::new();
        ms.should_emit_coord('X', 10.0);
        assert!(ms.should_emit_coord('X', 20.0));
    }

    #[test]
    fn coord_suppressed_within_tolerance() {
        let mut ms = ModalState::new();
        ms.should_emit_coord('Y', 5.0);
        // Difference of 5e-7 is below 1e-6 threshold
        assert!(!ms.should_emit_coord('Y', 5.0 + 5e-7));
    }

    #[test]
    fn coord_emits_just_above_tolerance() {
        let mut ms = ModalState::new();
        ms.should_emit_coord('Z', 5.0);
        // Difference of 2e-6 is above 1e-6 threshold
        assert!(ms.should_emit_coord('Z', 5.0 + 2e-6));
    }

    #[test]
    fn coord_axes_are_independent() {
        let mut ms = ModalState::new();
        ms.should_emit_coord('X', 1.0);
        // Y is still unset, so it should emit
        assert!(ms.should_emit_coord('Y', 1.0));
    }

    #[test]
    fn coord_all_axes_tracked() {
        let mut ms = ModalState::new();
        for axis in ['X', 'Y', 'Z', 'A', 'B', 'C'] {
            assert!(ms.should_emit_coord(axis, 0.0), "first emit for {axis}");
            assert!(
                !ms.should_emit_coord(axis, 0.0),
                "repeat suppressed for {axis}"
            );
            assert!(ms.should_emit_coord(axis, 1.0), "change emits for {axis}");
        }
    }

    // ── plane select ─────────────────────────────────────────────────────────

    #[test]
    fn plane_emits_first_time() {
        let mut ms = ModalState::new();
        assert!(ms.should_emit_plane("G17"));
    }

    #[test]
    fn plane_suppressed_on_repeat() {
        let mut ms = ModalState::new();
        ms.should_emit_plane("G17");
        assert!(!ms.should_emit_plane("G17"));
    }

    #[test]
    fn plane_re_emits_after_change() {
        let mut ms = ModalState::new();
        ms.should_emit_plane("G17");
        assert!(ms.should_emit_plane("G18"));
    }

    // ── distance mode ────────────────────────────────────────────────────────

    #[test]
    fn distance_mode_emits_first_time() {
        let mut ms = ModalState::new();
        assert!(ms.should_emit_distance_mode("G90"));
    }

    #[test]
    fn distance_mode_suppressed_on_repeat() {
        let mut ms = ModalState::new();
        ms.should_emit_distance_mode("G90");
        assert!(!ms.should_emit_distance_mode("G90"));
    }

    #[test]
    fn distance_mode_re_emits_after_change() {
        let mut ms = ModalState::new();
        ms.should_emit_distance_mode("G90");
        assert!(ms.should_emit_distance_mode("G91"));
    }

    // ── feed mode ────────────────────────────────────────────────────────────

    #[test]
    fn feed_mode_emits_first_time() {
        let mut ms = ModalState::new();
        assert!(ms.should_emit_feed_mode("G94"));
    }

    #[test]
    fn feed_mode_suppressed_on_repeat() {
        let mut ms = ModalState::new();
        ms.should_emit_feed_mode("G94");
        assert!(!ms.should_emit_feed_mode("G94"));
    }

    #[test]
    fn feed_mode_re_emits_after_change() {
        let mut ms = ModalState::new();
        ms.should_emit_feed_mode("G94");
        assert!(ms.should_emit_feed_mode("G95"));
    }

    // ── reset ────────────────────────────────────────────────────────────────

    #[test]
    fn reset_clears_all_state() {
        let mut ms = ModalState::new();
        ms.should_emit_motion("G01");
        ms.should_emit_feed(500.0);
        ms.should_emit_spindle(3000.0);
        ms.should_emit_tool(1);
        ms.should_emit_coord('X', 10.0);
        ms.should_emit_plane("G17");
        ms.should_emit_distance_mode("G90");
        ms.should_emit_feed_mode("G94");

        ms.reset();

        // Everything should emit again after reset
        assert!(ms.should_emit_motion("G01"));
        assert!(ms.should_emit_feed(500.0));
        assert!(ms.should_emit_spindle(3000.0));
        assert!(ms.should_emit_tool(1));
        assert!(ms.should_emit_coord('X', 10.0));
        assert!(ms.should_emit_plane("G17"));
        assert!(ms.should_emit_distance_mode("G90"));
        assert!(ms.should_emit_feed_mode("G94"));
    }
}
