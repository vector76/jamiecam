//! GRBL G-code emitter.
//!
//! Produces a self-contained GRBL-flavoured G-code program from a planner-
//! generated [`ToolpathOutput`], the [`Tool`] that produced it, and the
//! [`BoxDimensions`] of the stock. The output is intentionally minimal:
//!
//! - `; @STOCK` / `; @TOOL` header comments matching the format
//!   [`crate::gcode_parser::metadata`] already parses, so a round-trip
//!   recovers stock and tool metadata.
//! - `G21` (mm) + `G90` (absolute) units/modal setup.
//! - `S<rpm> M3` spindle on.
//! - `G0` rapids, `G1 ... F<feed>` cutting moves. Everything is linear —
//!   no arcs (`G2`/`G3`).
//! - `M5` spindle off + `M2` program end.
//!
//! Per `docs/phase-4-design.md` §7 the emitter is hardcoded to GRBL: no
//! config files, no dialect switching.

use std::fmt::Write;

use crate::error::AppError;
use crate::profile::{ToolpathMotion, ToolpathOutput};
use crate::types::BoxDimensions;
use crate::working_env::Tool;

/// Tool number stamped into the `; @TOOL` header and any future `Tn M6`
/// emission. The Mode 2 MVP runs one operation per project (see
/// `docs/phase-4-design.md` §5), so a single hardcoded tool slot is enough.
const TOOL_NUMBER: u32 = 1;

/// Emit a GRBL-flavoured G-code program for `toolpath`.
///
/// Returns [`AppError::InvalidInput`] if the stock, tool, or any motion
/// contains a non-finite value (or a non-positive diameter / spindle
/// speed / feed).
pub fn emit_grbl(
    toolpath: &ToolpathOutput,
    tool: &Tool,
    stock: &BoxDimensions,
) -> Result<String, AppError> {
    validate_stock(stock)?;
    validate_tool(tool)?;

    let mut out = String::new();
    write_header(&mut out, tool, stock);
    write_setup(&mut out, tool);
    write_motions(&mut out, toolpath)?;
    write_footer(&mut out);
    Ok(out)
}

fn validate_stock(stock: &BoxDimensions) -> Result<(), AppError> {
    let all_finite = stock.width.is_finite()
        && stock.depth.is_finite()
        && stock.height.is_finite()
        && stock.origin.x.is_finite()
        && stock.origin.y.is_finite()
        && stock.origin.z.is_finite();
    if !all_finite {
        return Err(AppError::InvalidInput(
            "stock dimensions and origin must be finite".into(),
        ));
    }
    Ok(())
}

fn validate_tool(tool: &Tool) -> Result<(), AppError> {
    if !tool.diameter.is_finite() || tool.diameter <= 0.0 {
        return Err(AppError::InvalidInput(format!(
            "tool diameter must be positive and finite, got {}",
            tool.diameter
        )));
    }
    let rpm = tool.recommended.spindle_rpm;
    if !rpm.is_finite() || rpm <= 0.0 {
        return Err(AppError::InvalidInput(format!(
            "spindle RPM must be positive and finite, got {rpm}"
        )));
    }
    Ok(())
}

fn write_header(out: &mut String, tool: &Tool, stock: &BoxDimensions) {
    writeln!(
        out,
        "; @STOCK type=box width={} depth={} height={} origin={},{},{}",
        fmt_num(stock.width),
        fmt_num(stock.depth),
        fmt_num(stock.height),
        fmt_num(stock.origin.x),
        fmt_num(stock.origin.y),
        fmt_num(stock.origin.z),
    )
    .unwrap();

    let material = sanitize_token(&tool.material);
    write!(
        out,
        "; @TOOL number={} type=endmill diameter={} flutes={}",
        TOOL_NUMBER,
        fmt_num(tool.diameter),
        tool.flute_count,
    )
    .unwrap();
    if let Some(material) = material {
        write!(out, " material={material}").unwrap();
    }
    out.push('\n');
}

fn write_setup(out: &mut String, tool: &Tool) {
    writeln!(out, "G21").unwrap();
    writeln!(out, "G90").unwrap();
    writeln!(out, "S{} M3", fmt_num(tool.recommended.spindle_rpm)).unwrap();
}

fn write_motions(out: &mut String, toolpath: &ToolpathOutput) -> Result<(), AppError> {
    for (idx, motion) in toolpath.iter().enumerate() {
        match motion {
            ToolpathMotion::Rapid { to } => {
                check_coords(to, idx)?;
                writeln!(
                    out,
                    "G0 X{} Y{} Z{}",
                    fmt_num(to[0]),
                    fmt_num(to[1]),
                    fmt_num(to[2]),
                )
                .unwrap();
            }
            ToolpathMotion::Linear { to, feed } => {
                check_coords(to, idx)?;
                if !feed.is_finite() || *feed <= 0.0 {
                    return Err(AppError::InvalidInput(format!(
                        "linear motion {idx} has invalid feed {feed}"
                    )));
                }
                writeln!(
                    out,
                    "G1 X{} Y{} Z{} F{}",
                    fmt_num(to[0]),
                    fmt_num(to[1]),
                    fmt_num(to[2]),
                    fmt_num(*feed),
                )
                .unwrap();
            }
        }
    }
    Ok(())
}

fn check_coords(to: &[f64; 3], idx: usize) -> Result<(), AppError> {
    if !to[0].is_finite() || !to[1].is_finite() || !to[2].is_finite() {
        return Err(AppError::InvalidInput(format!(
            "motion {idx} has non-finite coordinate [{}, {}, {}]",
            to[0], to[1], to[2]
        )));
    }
    Ok(())
}

fn write_footer(out: &mut String) {
    writeln!(out, "M5").unwrap();
    writeln!(out, "M2").unwrap();
}

/// Format a finite f64 with up to 4 decimal places, trimming trailing
/// zeros so integer-valued numbers come out as e.g. `100` rather than
/// `100.0000`. Non-finite inputs are rejected upstream.
fn fmt_num(v: f64) -> String {
    let s = format!("{v:.4}");
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Sanitize a free-form token (e.g. `tool.material`) for embedding in a
/// `key=value` header comment: trim, drop if empty, and replace internal
/// whitespace with underscores so the kv tokenizer (whitespace-split)
/// keeps it as a single token.
fn sanitize_token(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        trimmed
            .chars()
            .map(|c| if c.is_whitespace() { '_' } else { c })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcode_parser::{parse_gcode, parse_metadata, MotionSegment};
    use crate::types::Vec3;
    use crate::working_env::{FeedsAndSpeeds, ToolId};

    fn sample_tool() -> Tool {
        Tool {
            id: ToolId::new("t1"),
            name: "1/8\" flat".into(),
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

    fn sample_stock() -> BoxDimensions {
        BoxDimensions {
            origin: Vec3::zero(),
            width: 100.0,
            depth: 80.0,
            height: 20.0,
        }
    }

    // ── Header emission ──────────────────────────────────────────────

    #[test]
    fn emits_stock_header_line_first() {
        let out = emit_grbl(&Vec::new(), &sample_tool(), &sample_stock()).unwrap();
        let first_line = out.lines().next().unwrap();
        assert_eq!(
            first_line,
            "; @STOCK type=box width=100 depth=80 height=20 origin=0,0,0"
        );
    }

    #[test]
    fn stock_header_includes_non_zero_origin() {
        let stock = BoxDimensions {
            origin: Vec3 {
                x: -5.0,
                y: -10.0,
                z: 0.0,
            },
            width: 50.0,
            depth: 50.0,
            height: 10.0,
        };
        let out = emit_grbl(&Vec::new(), &sample_tool(), &stock).unwrap();
        assert!(out.contains("origin=-5,-10,0"), "got: {out}");
    }

    #[test]
    fn emits_tool_header_line_second() {
        let out = emit_grbl(&Vec::new(), &sample_tool(), &sample_stock()).unwrap();
        let second_line = out.lines().nth(1).unwrap();
        assert_eq!(
            second_line,
            "; @TOOL number=1 type=endmill diameter=3.175 flutes=2 material=carbide"
        );
    }

    #[test]
    fn tool_header_omits_material_when_blank() {
        let mut tool = sample_tool();
        tool.material = "   ".into();
        let out = emit_grbl(&Vec::new(), &tool, &sample_stock()).unwrap();
        let second_line = out.lines().nth(1).unwrap();
        assert!(!second_line.contains("material="), "got: {second_line}");
        assert!(second_line.contains("flutes=2"));
    }

    #[test]
    fn tool_header_replaces_whitespace_in_material() {
        let mut tool = sample_tool();
        tool.material = "high speed steel".into();
        let out = emit_grbl(&Vec::new(), &tool, &sample_stock()).unwrap();
        assert!(out.contains("material=high_speed_steel"), "got: {out}");
    }

    // ── Units / spindle setup ────────────────────────────────────────

    #[test]
    fn emits_units_and_motion_mode() {
        let out = emit_grbl(&Vec::new(), &sample_tool(), &sample_stock()).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[2], "G21");
        assert_eq!(lines[3], "G90");
    }

    #[test]
    fn emits_spindle_on_with_rpm() {
        let out = emit_grbl(&Vec::new(), &sample_tool(), &sample_stock()).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[4], "S18000 M3");
    }

    // ── Motion serialisation ─────────────────────────────────────────

    #[test]
    fn rapid_motion_emits_g0_xyz() {
        let toolpath = vec![ToolpathMotion::Rapid {
            to: [12.5, -3.0, 5.0],
        }];
        let out = emit_grbl(&toolpath, &sample_tool(), &sample_stock()).unwrap();
        assert!(out.contains("\nG0 X12.5 Y-3 Z5\n"), "got: {out}");
    }

    #[test]
    fn linear_motion_emits_g1_xyz_with_feed() {
        let toolpath = vec![ToolpathMotion::Linear {
            to: [10.0, 0.0, -1.5],
            feed: 800.0,
        }];
        let out = emit_grbl(&toolpath, &sample_tool(), &sample_stock()).unwrap();
        assert!(out.contains("\nG1 X10 Y0 Z-1.5 F800\n"), "got: {out}");
    }

    #[test]
    fn motions_emitted_in_order() {
        let toolpath = vec![
            ToolpathMotion::Rapid {
                to: [0.0, 0.0, 5.0],
            },
            ToolpathMotion::Linear {
                to: [0.0, 0.0, -1.5],
                feed: 200.0,
            },
            ToolpathMotion::Linear {
                to: [10.0, 0.0, -1.5],
                feed: 800.0,
            },
            ToolpathMotion::Rapid {
                to: [10.0, 0.0, 5.0],
            },
        ];
        let out = emit_grbl(&toolpath, &sample_tool(), &sample_stock()).unwrap();
        let motion_lines: Vec<&str> = out
            .lines()
            .filter(|l| l.starts_with("G0") || l.starts_with("G1"))
            .collect();
        assert_eq!(
            motion_lines,
            vec![
                "G0 X0 Y0 Z5",
                "G1 X0 Y0 Z-1.5 F200",
                "G1 X10 Y0 Z-1.5 F800",
                "G0 X10 Y0 Z5",
            ]
        );
    }

    // ── Footer ───────────────────────────────────────────────────────

    #[test]
    fn footer_is_m5_then_m2() {
        let out = emit_grbl(&Vec::new(), &sample_tool(), &sample_stock()).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        let n = lines.len();
        assert_eq!(lines[n - 2], "M5");
        assert_eq!(lines[n - 1], "M2");
    }

    #[test]
    fn empty_toolpath_still_produces_header_setup_footer() {
        let out = emit_grbl(&Vec::new(), &sample_tool(), &sample_stock()).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        // 2 header + G21 + G90 + spindle + M5 + M2 = 7
        assert_eq!(lines.len(), 7, "got: {out:?}");
    }

    // ── Error paths ──────────────────────────────────────────────────

    #[test]
    fn nan_coord_in_rapid_returns_invalid_input() {
        let toolpath = vec![ToolpathMotion::Rapid {
            to: [f64::NAN, 0.0, 0.0],
        }];
        let err = emit_grbl(&toolpath, &sample_tool(), &sample_stock()).unwrap_err();
        match err {
            AppError::InvalidInput(msg) => assert!(msg.contains("non-finite"), "msg: {msg}"),
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn inf_coord_in_linear_returns_invalid_input() {
        let toolpath = vec![ToolpathMotion::Linear {
            to: [0.0, f64::INFINITY, 0.0],
            feed: 500.0,
        }];
        let err = emit_grbl(&toolpath, &sample_tool(), &sample_stock()).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn non_finite_feed_returns_invalid_input() {
        let toolpath = vec![ToolpathMotion::Linear {
            to: [0.0, 0.0, 0.0],
            feed: f64::NAN,
        }];
        let err = emit_grbl(&toolpath, &sample_tool(), &sample_stock()).unwrap_err();
        match err {
            AppError::InvalidInput(msg) => assert!(msg.contains("feed"), "msg: {msg}"),
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn non_positive_feed_returns_invalid_input() {
        let toolpath = vec![ToolpathMotion::Linear {
            to: [0.0, 0.0, 0.0],
            feed: 0.0,
        }];
        let err = emit_grbl(&toolpath, &sample_tool(), &sample_stock()).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    #[test]
    fn non_positive_diameter_returns_invalid_input() {
        let mut tool = sample_tool();
        tool.diameter = 0.0;
        let err = emit_grbl(&Vec::new(), &tool, &sample_stock()).unwrap_err();
        match err {
            AppError::InvalidInput(msg) => assert!(msg.contains("diameter"), "msg: {msg}"),
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn non_finite_spindle_rpm_returns_invalid_input() {
        let mut tool = sample_tool();
        tool.recommended.spindle_rpm = f64::INFINITY;
        let err = emit_grbl(&Vec::new(), &tool, &sample_stock()).unwrap_err();
        match err {
            AppError::InvalidInput(msg) => assert!(msg.contains("spindle"), "msg: {msg}"),
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn non_finite_stock_dimension_returns_invalid_input() {
        let mut stock = sample_stock();
        stock.width = f64::NAN;
        let err = emit_grbl(&Vec::new(), &sample_tool(), &stock).unwrap_err();
        match err {
            AppError::InvalidInput(msg) => assert!(msg.contains("stock"), "msg: {msg}"),
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn non_finite_stock_origin_returns_invalid_input() {
        let mut stock = sample_stock();
        stock.origin.z = f64::NAN;
        let err = emit_grbl(&Vec::new(), &sample_tool(), &stock).unwrap_err();
        assert!(matches!(err, AppError::InvalidInput(_)));
    }

    // ── Round-trip through the parser ────────────────────────────────
    //
    // Per docs/phase-4-design.md §7 the cheap correctness gate for the
    // emitter is that what it writes parses back to the same toolpath
    // (motion-wise) and the same stock/tool metadata Mode 1 expects.

    /// Tolerance for round-trip comparisons: the emitter formats numbers
    /// with up to 4 decimal places, so worst-case rounding error is ~5e-5.
    const ROUND_TRIP_TOL: f64 = 1e-4;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() <= ROUND_TRIP_TOL
    }

    #[test]
    fn round_trip_motions_match_within_tolerance() {
        // A profile-flavoured sequence: rapid to clearance, rapid to XY,
        // plunge, two cutting moves, rapid retract.
        let toolpath = vec![
            ToolpathMotion::Rapid {
                to: [0.0, 0.0, 5.0],
            },
            ToolpathMotion::Rapid {
                to: [12.5, -3.0, 5.0],
            },
            ToolpathMotion::Linear {
                to: [12.5, -3.0, -1.5],
                feed: 200.0,
            },
            ToolpathMotion::Linear {
                to: [22.5, -3.0, -1.5],
                feed: 800.0,
            },
            ToolpathMotion::Linear {
                to: [22.5, 7.0, -1.5],
                feed: 800.0,
            },
            ToolpathMotion::Rapid {
                to: [22.5, 7.0, 5.0],
            },
        ];
        let gcode = emit_grbl(&toolpath, &sample_tool(), &sample_stock()).unwrap();
        let parsed = parse_gcode(&gcode);
        assert!(
            parsed.warnings.is_empty(),
            "unexpected parser warnings: {:?}",
            parsed.warnings
        );
        assert_eq!(
            parsed.segments.len(),
            toolpath.len(),
            "segment count mismatch; gcode:\n{gcode}"
        );

        for (i, (motion, seg)) in toolpath.iter().zip(parsed.segments.iter()).enumerate() {
            match (motion, seg) {
                (ToolpathMotion::Rapid { to }, MotionSegment::Rapid { end, .. }) => {
                    assert!(approx(end.x, to[0]), "seg {i} rapid X: {end:?} vs {to:?}");
                    assert!(approx(end.y, to[1]), "seg {i} rapid Y: {end:?} vs {to:?}");
                    assert!(approx(end.z, to[2]), "seg {i} rapid Z: {end:?} vs {to:?}");
                }
                (
                    ToolpathMotion::Linear { to, feed },
                    MotionSegment::Linear { end, feed_rate, .. },
                ) => {
                    assert!(approx(end.x, to[0]), "seg {i} linear X: {end:?} vs {to:?}");
                    assert!(approx(end.y, to[1]), "seg {i} linear Y: {end:?} vs {to:?}");
                    assert!(approx(end.z, to[2]), "seg {i} linear Z: {end:?} vs {to:?}");
                    assert!(
                        approx(*feed_rate, *feed),
                        "seg {i} linear feed: {feed_rate} vs {feed}"
                    );
                }
                (m, s) => panic!("seg {i} kind mismatch: emitted {m:?}, parsed {s:?}"),
            }
        }
    }

    #[test]
    fn round_trip_motions_with_sub_thousandth_precision() {
        // Coordinates and feed needing the full 4-decimal precision the
        // emitter offers — exercise the upper bound of rounding error.
        let toolpath = vec![ToolpathMotion::Linear {
            to: [1.23456, -2.7891, 0.1234],
            feed: 567.8912,
        }];
        let gcode = emit_grbl(&toolpath, &sample_tool(), &sample_stock()).unwrap();
        let parsed = parse_gcode(&gcode);
        assert!(parsed.warnings.is_empty());
        assert_eq!(parsed.segments.len(), 1);
        match &parsed.segments[0] {
            MotionSegment::Linear { end, feed_rate, .. } => {
                assert!(approx(end.x, 1.23456));
                assert!(approx(end.y, -2.7891));
                assert!(approx(end.z, 0.1234));
                assert!(approx(*feed_rate, 567.8912));
            }
            other => panic!("expected Linear, got {other:?}"),
        }
    }

    #[test]
    fn round_trip_stock_header_parses_to_matching_metadata() {
        let stock = BoxDimensions {
            origin: Vec3 {
                x: -5.0,
                y: -10.0,
                z: 0.0,
            },
            width: 100.0,
            depth: 80.0,
            height: 20.0,
        };
        let gcode = emit_grbl(&Vec::new(), &sample_tool(), &stock).unwrap();
        let parsed = parse_gcode(&gcode);
        let meta = parse_metadata(&parsed.metadata.header_comments);
        assert!(
            meta.warnings.is_empty(),
            "metadata warnings: {:?}",
            meta.warnings
        );

        let parsed_stock = meta.stock.expect("@STOCK header should parse");
        assert_eq!(parsed_stock.stock_type, "box");
        assert!(approx(parsed_stock.width, stock.width));
        assert!(approx(parsed_stock.depth, stock.depth));
        assert!(approx(parsed_stock.height, stock.height));
        assert!(approx(parsed_stock.origin.x, stock.origin.x));
        assert!(approx(parsed_stock.origin.y, stock.origin.y));
        assert!(approx(parsed_stock.origin.z, stock.origin.z));
    }

    #[test]
    fn round_trip_tool_header_parses_to_matching_metadata() {
        let tool = sample_tool();
        let gcode = emit_grbl(&Vec::new(), &tool, &sample_stock()).unwrap();
        let parsed = parse_gcode(&gcode);
        let meta = parse_metadata(&parsed.metadata.header_comments);
        assert!(
            meta.warnings.is_empty(),
            "metadata warnings: {:?}",
            meta.warnings
        );

        assert_eq!(meta.tools.len(), 1);
        let parsed_tool = &meta.tools[0];
        assert_eq!(parsed_tool.number, TOOL_NUMBER);
        assert_eq!(parsed_tool.tool_type, "endmill");
        assert!(approx(parsed_tool.diameter, tool.diameter));
        assert_eq!(parsed_tool.flutes, Some(tool.flute_count));
        assert_eq!(parsed_tool.material.as_deref(), Some("carbide"));
    }

    #[test]
    fn round_trip_tool_header_omits_material_when_blank() {
        let mut tool = sample_tool();
        tool.material = "   ".into();
        let gcode = emit_grbl(&Vec::new(), &tool, &sample_stock()).unwrap();
        let parsed = parse_gcode(&gcode);
        let meta = parse_metadata(&parsed.metadata.header_comments);
        assert!(meta.warnings.is_empty());
        assert_eq!(meta.tools.len(), 1);
        assert_eq!(meta.tools[0].material, None);
    }
}
