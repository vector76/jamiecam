use super::config::{PeckRetractMode, PostProcessorConfig};
use super::formatter::format_coord;
use super::PostProcessorError;
use crate::toolpath::types::{MoveKind, Pass, PassKind, DEFAULT_CLEARANCE_OFFSET};

/// Returns true if the config enables canned cycles.
pub fn cycles_supported(config: &PostProcessorConfig) -> bool {
    config.cycles.supported
}

/// Selects G83 vs G73 based on peck_retract_mode (defaults to Full/G83 when absent).
/// Returns None if config does not have a peck code defined.
pub fn peck_cycle_code(config: &PostProcessorConfig) -> Option<&str> {
    match &config.cycles.peck_retract_mode {
        Some(PeckRetractMode::ChipBreak) => config.cycles.chip_break.as_deref(),
        _ => config.cycles.peck.as_deref(),
    }
}

/// Returns the drill (non-peck) cycle code (e.g. "G81"), or None.
pub fn drill_cycle_code(config: &PostProcessorConfig) -> Option<&str> {
    config.cycles.drill.as_deref()
}

/// Returns the cycle cancel code (e.g. "G80"), or None.
pub fn cycle_cancel_code(config: &PostProcessorConfig) -> Option<&str> {
    config.cycles.cycle_cancel.as_deref()
}

/// Returns the R-plane-abs mode code (e.g. "G98"), or None.
pub fn r_plane_abs_code(config: &PostProcessorConfig) -> Option<&str> {
    config.cycles.r_plane_abs.as_deref()
}

/// Formats the cycle activation line for a drill cutting pass.
/// Example Simple output:  "G98 G81 Z-12.5 R2 F150"
/// Example Peck output:    "G98 G83 Z-12.5 R2 Q5 F150"
/// (Exact number formatting depends on the config's decimal/trailing-zero settings.)
/// Returns Err if config doesn't have required cycle codes.
pub fn format_cycle_header(
    params: &DrillCycleParams,
    feed_rate: f64,
    config: &PostProcessorConfig,
) -> Result<String, PostProcessorError> {
    let cycle_code = match &params.kind {
        DrillCycleKind::Simple => drill_cycle_code(config).ok_or_else(|| {
            PostProcessorError::NotSupported("drill cycle code not configured".to_string())
        })?,
        DrillCycleKind::Peck { .. } => peck_cycle_code(config).ok_or_else(|| {
            PostProcessorError::NotSupported("peck cycle code not configured".to_string())
        })?,
    };

    let dp = config.format.decimal_places;
    let strip = !config.format.trailing_zeros;
    let suppress_lz = config.format.leading_zero_suppression;
    let sep = &config.format.word_separator;

    let z_str = format_coord(params.drill_depth_z, dp, strip, suppress_lz);
    let r_str = format_coord(params.r_plane_z, dp, strip, suppress_lz);
    let f_str = format_coord(feed_rate, dp, strip, suppress_lz);

    let mut parts: Vec<String> = Vec::new();

    if let Some(rpa) = r_plane_abs_code(config) {
        parts.push(rpa.to_string());
    }
    parts.push(cycle_code.to_string());
    parts.push(format!("{}{z_str}", config.axes.z));
    parts.push(format!("R{r_str}"));

    if let DrillCycleKind::Peck { increment } = &params.kind {
        let q_str = format_coord(*increment, dp, strip, suppress_lz);
        parts.push(format!("Q{q_str}"));
    }

    parts.push(format!("{}{f_str}", config.words.feed));

    Ok(parts.join(sep))
}

/// Formats the cycle cancel line (e.g. "G80").
pub fn format_cycle_cancel(config: &PostProcessorConfig) -> Result<String, PostProcessorError> {
    cycle_cancel_code(config)
        .map(|s| s.to_string())
        .ok_or_else(|| {
            PostProcessorError::NotSupported("cycle cancel code not configured".to_string())
        })
}

/// Detect whether a `Pass` carries a drill cycle signature.
///
/// A `Cutting` pass is considered a drill pass when:
/// - `cuts` has an odd count ≥ 3 (1 + 2N for N ≥ 1)
/// - `cuts[0]` is `Rapid` (R-plane approach)
/// - Subsequent points alternate Feed (odd indices) and Rapid (even indices)
/// - All cut-points share the same XY position (within 1e-9 mm tolerance)
pub fn is_drill_cutting_pass(pass: &Pass) -> bool {
    if pass.kind != PassKind::Cutting {
        return false;
    }
    let cuts = &pass.cuts;
    let n = cuts.len();
    if n < 3 || n % 2 == 0 {
        return false;
    }
    if cuts[0].move_kind != MoveKind::Rapid {
        return false;
    }
    let x0 = cuts[0].position.x;
    let y0 = cuts[0].position.y;
    for (i, cut) in cuts.iter().enumerate().skip(1) {
        // Check XY uniformity
        if (cut.position.x - x0).abs() > 1e-9 || (cut.position.y - y0).abs() > 1e-9 {
            return false;
        }
        // Check alternating move kinds: odd indices are Feed, even indices are Rapid
        let expected_feed = i % 2 == 1;
        let is_feed = cut.move_kind == MoveKind::Feed;
        let is_rapid = cut.move_kind == MoveKind::Rapid;
        if expected_feed && !is_feed {
            return false;
        }
        if !expected_feed && !is_rapid {
            return false;
        }
    }
    true
}

/// Classification of a drill cycle.
#[derive(Debug, Clone, PartialEq)]
pub enum DrillCycleKind {
    /// Simple drill: 3 cut-points `[Rapid, Feed, Rapid]`.
    Simple,
    /// Peck drill: 1 + 2N cut-points (N ≥ 2); `increment` is depth per peck.
    Peck { increment: f64 },
}

/// Extracted parameters for a drill cycle pass.
#[derive(Debug, Clone, PartialEq)]
pub struct DrillCycleParams {
    /// Whether this is a simple drill or a peck cycle.
    pub kind: DrillCycleKind,
    /// Z height of `cuts[0]` — the R-plane (clearance approach height).
    pub r_plane_z: f64,
    /// Z height of the deepest Feed point (`cuts[cuts.len() - 2]`).
    pub drill_depth_z: f64,
}

/// Classify a `Pass` as a drill cycle and extract its parameters.
///
/// Returns `None` if the pass does not satisfy `is_drill_cutting_pass`.
pub fn classify_drill_pass(pass: &Pass) -> Option<DrillCycleParams> {
    if !is_drill_cutting_pass(pass) {
        return None;
    }
    let cuts = &pass.cuts;
    let r_plane_z = cuts[0].position.z;
    let drill_depth_z = cuts[cuts.len() - 2].position.z;
    let kind = if cuts.len() == 3 {
        DrillCycleKind::Simple
    } else {
        // Peck increment: stock_top_z - first_peck_z
        // stock_top_z is reconstructed as (r_plane_z - DEFAULT_CLEARANCE_OFFSET)
        let stock_top_z = r_plane_z - DEFAULT_CLEARANCE_OFFSET;
        let first_peck_z = cuts[1].position.z;
        let increment = stock_top_z - first_peck_z;
        DrillCycleKind::Peck { increment }
    };
    Some(DrillCycleParams {
        kind,
        r_plane_z,
        drill_depth_z,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Vec3;
    use crate::postprocessor::PostProcessor;
    use crate::toolpath::types::{CutPoint, Pass, PassKind};

    fn fanuc_config() -> crate::postprocessor::config::PostProcessorConfig {
        PostProcessor::builtin("fanuc-0i").unwrap().config
    }

    fn grbl_config() -> crate::postprocessor::config::PostProcessorConfig {
        PostProcessor::builtin("grbl").unwrap().config
    }

    fn make_cut(x: f64, y: f64, z: f64, move_kind: MoveKind) -> CutPoint {
        CutPoint {
            position: Vec3 { x, y, z },
            move_kind,
            tool_orientation: None,
        }
    }

    #[test]
    fn test_is_drill_pass_simple() {
        let pass = Pass {
            kind: PassKind::Cutting,
            cuts: vec![
                make_cut(10.0, 20.0, 15.0, MoveKind::Rapid),
                make_cut(10.0, 20.0, 2.0, MoveKind::Feed),
                make_cut(10.0, 20.0, 15.0, MoveKind::Rapid),
            ],
        };
        assert!(is_drill_cutting_pass(&pass));
        let params = classify_drill_pass(&pass).unwrap();
        assert_eq!(params.kind, DrillCycleKind::Simple);
        assert_eq!(params.r_plane_z, 15.0);
        assert_eq!(params.drill_depth_z, 2.0);
    }

    #[test]
    fn test_is_drill_pass_peck() {
        let pass = Pass {
            kind: PassKind::Cutting,
            cuts: vec![
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
                make_cut(0.0, 0.0, 7.0, MoveKind::Feed),
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
                make_cut(0.0, 0.0, 4.0, MoveKind::Feed),
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
                make_cut(0.0, 0.0, 1.0, MoveKind::Feed),
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
            ],
        };
        assert!(is_drill_cutting_pass(&pass));
        let params = classify_drill_pass(&pass).unwrap();
        assert_eq!(params.r_plane_z, 15.0);
        assert_eq!(params.drill_depth_z, 1.0);
        match params.kind {
            DrillCycleKind::Peck { increment } => {
                assert!(
                    (increment - 3.0).abs() < 1e-9,
                    "expected increment ~3.0, got {increment}"
                );
            }
            _ => panic!("expected Peck variant"),
        }
    }

    #[test]
    fn test_is_not_drill_pass_mixed_xy() {
        let pass = Pass {
            kind: PassKind::Cutting,
            cuts: vec![
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
                make_cut(1.0, 0.0, 2.0, MoveKind::Feed),
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
            ],
        };
        assert!(!is_drill_cutting_pass(&pass));
    }

    #[test]
    fn test_is_not_drill_pass_wrong_count() {
        let pass = Pass {
            kind: PassKind::Cutting,
            cuts: vec![
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
                make_cut(0.0, 0.0, 7.0, MoveKind::Feed),
                make_cut(0.0, 0.0, 15.0, MoveKind::Rapid),
                make_cut(0.0, 0.0, 4.0, MoveKind::Feed),
            ],
        };
        assert!(!is_drill_cutting_pass(&pass));
    }

    // fanuc-0i: decimal_places=3, trailing_zeros=false → strip trailing zeros
    // format_coord(2.0, 3, true, false) = "2", format_coord(150.0, ...) = "150"

    #[test]
    fn test_format_header_simple_g81() {
        let config = fanuc_config();
        let params = DrillCycleParams {
            kind: DrillCycleKind::Simple,
            r_plane_z: 2.0,
            drill_depth_z: -12.5,
        };
        let result = format_cycle_header(&params, 150.0, &config).unwrap();
        assert!(result.contains("G81"), "expected G81 in: {result}");
        assert!(result.contains("Z-12.5"), "expected Z-12.5 in: {result}");
        assert!(result.contains("R2"), "expected R2 in: {result}");
        assert!(result.contains("F150"), "expected F150 in: {result}");
        assert!(!result.contains('Q'), "did not expect Q in: {result}");
    }

    #[test]
    fn test_format_header_peck_g83() {
        let config = fanuc_config();
        let params = DrillCycleParams {
            kind: DrillCycleKind::Peck { increment: 5.0 },
            r_plane_z: 2.0,
            drill_depth_z: -12.5,
        };
        let result = format_cycle_header(&params, 150.0, &config).unwrap();
        assert!(result.contains("G83"), "expected G83 in: {result}");
        assert!(result.contains("Q5"), "expected Q5 in: {result}");
    }

    #[test]
    fn test_cycles_not_supported_returns_false() {
        let config = grbl_config();
        assert!(!cycles_supported(&config));
    }

    #[test]
    fn test_peck_retract_mode_selects_g83() {
        let config = fanuc_config();
        assert_eq!(peck_cycle_code(&config), Some("G83"));
    }

    #[test]
    fn test_format_cycle_cancel_returns_g80() {
        let config = fanuc_config();
        let result = format_cycle_cancel(&config).unwrap();
        assert_eq!(result, "G80");
    }

    #[test]
    fn test_format_cycle_cancel_err_when_not_configured() {
        let config = grbl_config();
        assert!(format_cycle_cancel(&config).is_err());
    }
}
