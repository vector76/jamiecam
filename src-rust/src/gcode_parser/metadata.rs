//! Parser for structured metadata comments embedded in G-code file headers.
//!
//! Recognises `; @STOCK` and `; @TOOL` comment lines that appear before the
//! first motion block and converts them into typed structs.  By the time this
//! parser receives its input, the G-code tokenizer has already stripped the
//! leading semicolon and surrounding whitespace, so a `; @STOCK …` line
//! arrives as `@STOCK …`.  Non-directive header comments (e.g. a filename or
//! description line) are passed through unchanged and silently ignored.
//!
//! ## Comment format
//!
//! ```text
//! @STOCK type=box width=<W> depth=<D> height=<H> [origin=<X>,<Y>,<Z>]
//! @TOOL  number=<N> type=<TYPE> diameter=<D> [flutes=<F>] [material=<M>]
//! ```
//!
//! All key comparisons are case-insensitive.  Enumerated values (e.g. `type`)
//! are matched case-insensitively.  Unknown keys are silently ignored for
//! forward compatibility.  A comment that is missing any required field is
//! dropped entirely and produces a [`ParseWarning`].

use serde::{Deserialize, Serialize};

use crate::types::Vec3;

use super::types::ParseWarning;

// ── Output types ─────────────────────────────────────────────────────────────

/// Parsed stock metadata from a `; @STOCK` comment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GcodeStockMetadata {
    /// Stock shape identifier. Currently always `"box"`.
    pub stock_type: String,
    /// X dimension (mm).
    pub width: f64,
    /// Y dimension (mm).
    pub depth: f64,
    /// Z dimension (mm).
    pub height: f64,
    /// Minimum-XYZ corner in work coordinates. Defaults to `(0, 0, 0)` when
    /// the `origin` key is absent from the comment.
    pub origin: Vec3,
}

/// Parsed tool metadata from a `; @TOOL` comment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GcodeToolMetadata {
    /// Tool number matching the T-word in the G-code body.
    pub number: u32,
    /// Tool type string (e.g. `"flat_endmill"`, `"ball_nose"`).
    pub tool_type: String,
    /// Cutting diameter (mm).
    pub diameter: f64,
    /// Number of flutes (optional).
    pub flutes: Option<u32>,
    /// Tool body material string (optional, e.g. `"carbide"`, `"hss"`).
    pub material: Option<String>,
}

/// Combined result of parsing all metadata comments in a program header.
#[derive(Debug, Clone, PartialEq)]
pub struct GcodeMetadata {
    /// Stock definition, if at least one valid `; @STOCK` comment was found.
    /// When multiple valid `; @STOCK` comments are present, the first one is
    /// used and the rest produce warnings.
    pub stock: Option<GcodeStockMetadata>,
    /// Tool definitions.  When two `; @TOOL` comments share the same tool
    /// number, the last definition for that number is used and a warning is
    /// emitted.
    pub tools: Vec<GcodeToolMetadata>,
    /// Non-fatal warnings produced during metadata parsing.
    pub warnings: Vec<ParseWarning>,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Parse structured metadata from the pre-motion header comments of a G-code
/// program.
///
/// `header_comments` is the slice already stored in
/// [`crate::gcode_parser::ProgramMetadata::header_comments`].  Each string
/// has already had its leading semicolon and surrounding whitespace stripped by
/// the tokenizer.
///
/// Lines that do not begin with `@STOCK` or `@TOOL` (case-insensitive) are
/// silently ignored — they are ordinary human-readable comments.
pub fn parse_metadata(header_comments: &[String]) -> GcodeMetadata {
    let mut stock: Option<GcodeStockMetadata> = None;
    // Tool map: tool_number → GcodeToolMetadata; preserves last-wins semantics.
    let mut tool_map: std::collections::HashMap<u32, GcodeToolMetadata> =
        std::collections::HashMap::new();
    // Ordered list of tool numbers as they appear (for stable output ordering).
    let mut tool_order: Vec<u32> = Vec::new();
    let mut warnings: Vec<ParseWarning> = Vec::new();

    for (idx, comment) in header_comments.iter().enumerate() {
        // Use a 1-based index into header_comments as a pseudo line number.
        // Actual source file line numbers are not tracked here; callers can
        // remap these if needed.
        let line = idx + 1;

        // Trim first so that both the starts_with check and the subsequent
        // slice into the same string are consistent.  Without this, a comment
        // with leading whitespace would pass the starts_with test (because
        // `upper` is trimmed) but be sliced at the wrong byte offset.
        let trimmed = comment.trim();
        let upper = trimmed.to_uppercase();

        // Use strip_prefix + a word-boundary check so that "@TOOLCHANGER" or
        // "@STOCKROOM" don't accidentally match as directives.
        if upper
            .strip_prefix("@STOCK")
            .is_some_and(|after| after.is_empty() || after.starts_with(char::is_whitespace))
        {
            let rest = &trimmed[6..]; // skip "@STOCK"
            match parse_stock_comment(rest) {
                Ok(s) => {
                    if stock.is_some() {
                        warnings.push(ParseWarning {
                            line: Some(line as u32),
                            message: format!(
                                "duplicate @STOCK comment ignored; only the first definition is used (comment: \"{comment}\")"
                            ),
                        });
                    } else {
                        stock = Some(s);
                    }
                }
                Err(reason) => {
                    warnings.push(ParseWarning {
                        line: Some(line as u32),
                        message: format!(
                            "malformed @STOCK comment ignored — {reason} (comment: \"{comment}\")"
                        ),
                    });
                }
            }
        } else if upper
            .strip_prefix("@TOOL")
            .is_some_and(|after| after.is_empty() || after.starts_with(char::is_whitespace))
        {
            let rest = &trimmed[5..]; // skip "@TOOL"
            match parse_tool_comment(rest) {
                Ok(t) => {
                    let num = t.number;
                    if tool_map.contains_key(&num) {
                        warnings.push(ParseWarning {
                            line: Some(line as u32),
                            message: format!(
                                "duplicate @TOOL T{num} comment; later definition replaces earlier one (comment: \"{comment}\")"
                            ),
                        });
                    } else {
                        tool_order.push(num);
                    }
                    tool_map.insert(num, t);
                }
                Err(reason) => {
                    warnings.push(ParseWarning {
                        line: Some(line as u32),
                        message: format!(
                            "malformed @TOOL comment ignored — {reason} (comment: \"{comment}\")"
                        ),
                    });
                }
            }
        }
        // All other comments are ignored.
    }

    let tools = tool_order
        .into_iter()
        .filter_map(|n| tool_map.remove(&n))
        .collect();

    GcodeMetadata {
        stock,
        tools,
        warnings,
    }
}

// ── Internal parsers ──────────────────────────────────────────────────────────

/// Parse key=value pairs from the part of a comment that follows the directive
/// keyword (e.g. after `@STOCK`).  Returns a `HashMap<lowercase_key, value>`.
fn parse_kv(s: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for token in s.split_whitespace() {
        if let Some(eq) = token.find('=') {
            let key = token[..eq].to_lowercase();
            let val = token[eq + 1..].to_string();
            if !key.is_empty() && !val.is_empty() {
                map.insert(key, val);
            }
        }
    }
    map
}

/// Parse a `; @STOCK` comment body (the text after `@STOCK`).
///
/// Returns `Err(reason)` when any required field is missing or malformed.
fn parse_stock_comment(rest: &str) -> Result<GcodeStockMetadata, String> {
    let kv = parse_kv(rest);

    // Required: type must be present and equal "box" (case-insensitive).
    let stock_type = kv
        .get("type")
        .ok_or_else(|| "missing required key 'type'".to_string())?
        .to_lowercase();
    if stock_type != "box" {
        return Err(format!(
            "unsupported stock type '{stock_type}'; only 'box' is supported"
        ));
    }

    let width = parse_required_f64(&kv, "width")?;
    let depth = parse_required_f64(&kv, "depth")?;
    let height = parse_required_f64(&kv, "height")?;

    // Optional: origin defaults to (0, 0, 0).
    let origin = if let Some(origin_str) = kv.get("origin") {
        parse_origin(origin_str)?
    } else {
        Vec3::zero()
    };

    Ok(GcodeStockMetadata {
        stock_type: "box".to_string(),
        width,
        depth,
        height,
        origin,
    })
}

/// Parse a `; @TOOL` comment body (the text after `@TOOL`).
///
/// Returns `Err(reason)` when any required field is missing or malformed.
fn parse_tool_comment(rest: &str) -> Result<GcodeToolMetadata, String> {
    let kv = parse_kv(rest);

    let number_str = kv
        .get("number")
        .ok_or_else(|| "missing required key 'number'".to_string())?;
    let number: u32 = number_str
        .parse()
        .map_err(|_| format!("invalid tool number '{number_str}'"))?;

    let tool_type = kv
        .get("type")
        .ok_or_else(|| "missing required key 'type'".to_string())?
        .to_lowercase();

    let diameter = parse_required_f64(&kv, "diameter")?;

    let flutes: Option<u32> = kv
        .get("flutes")
        .map(|v| {
            v.parse::<u32>()
                .map_err(|_| format!("invalid flutes value '{v}'"))
        })
        .transpose()?;

    let material = kv.get("material").cloned();

    Ok(GcodeToolMetadata {
        number,
        tool_type,
        diameter,
        flutes,
        material,
    })
}

/// Parse a required f64 value from the kv map.
///
/// Returns `Err` if the key is absent, the value is not a valid number, or the
/// parsed value is non-finite (`NaN`, `±Inf`).
fn parse_required_f64(
    kv: &std::collections::HashMap<String, String>,
    key: &str,
) -> Result<f64, String> {
    let s = kv
        .get(key)
        .ok_or_else(|| format!("missing required key '{key}'"))?;
    let v = s
        .parse::<f64>()
        .map_err(|_| format!("invalid value '{s}' for key '{key}'"))?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(format!("value for key '{key}' must be finite, got '{s}'"))
    }
}

/// Parse an origin string of the form `"X,Y,Z"` into a [`Vec3`].
///
/// Returns `Err` if the format is wrong, any coordinate is not a valid number,
/// or any coordinate is non-finite.
fn parse_origin(s: &str) -> Result<Vec3, String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 3 {
        return Err(format!("origin must be 'X,Y,Z' but got '{s}'"));
    }
    let parse_coord = |part: &str, axis: char| -> Result<f64, String> {
        let v = part
            .parse::<f64>()
            .map_err(|_| format!("invalid origin {axis} '{part}'"))?;
        if v.is_finite() {
            Ok(v)
        } else {
            Err(format!("origin {axis} must be finite, got '{part}'"))
        }
    };
    Ok(Vec3 {
        x: parse_coord(parts[0], 'X')?,
        y: parse_coord(parts[1], 'Y')?,
        z: parse_coord(parts[2], 'Z')?,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn comments(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|s| s.to_string()).collect()
    }

    // ── Stock parsing ─────────────────────────────────────────────────────

    #[test]
    fn stock_full_comment() {
        let result = parse_metadata(&comments(&[
            "@STOCK type=box width=100 depth=80 height=20 origin=0,0,0",
        ]));
        assert!(result.warnings.is_empty());
        let stock = result.stock.unwrap();
        assert_eq!(stock.stock_type, "box");
        assert_eq!(stock.width, 100.0);
        assert_eq!(stock.depth, 80.0);
        assert_eq!(stock.height, 20.0);
        assert_eq!(
            stock.origin,
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 0.0
            }
        );
    }

    #[test]
    fn stock_origin_defaults_to_zero_when_absent() {
        let result = parse_metadata(&comments(&["@STOCK type=box width=50 depth=50 height=10"]));
        assert!(result.warnings.is_empty());
        let stock = result.stock.unwrap();
        assert_eq!(stock.origin, Vec3::zero());
    }

    #[test]
    fn stock_negative_origin_is_valid() {
        let result = parse_metadata(&comments(&[
            "@STOCK type=box width=50 depth=50 height=10 origin=-5,-10,0",
        ]));
        assert!(result.warnings.is_empty());
        let stock = result.stock.unwrap();
        assert_eq!(
            stock.origin,
            Vec3 {
                x: -5.0,
                y: -10.0,
                z: 0.0
            }
        );
    }

    #[test]
    fn stock_case_insensitive_directive() {
        let result = parse_metadata(&comments(&["@stock type=box width=30 depth=30 height=5"]));
        assert!(result.warnings.is_empty());
        assert!(result.stock.is_some());
    }

    #[test]
    fn stock_leading_whitespace_in_comment() {
        // Header comments are normally trimmed by the tokenizer, but
        // parse_metadata should handle leading whitespace correctly regardless.
        let result = parse_metadata(&comments(&["  @STOCK type=box width=30 depth=30 height=5"]));
        assert!(result.warnings.is_empty());
        assert!(result.stock.is_some());
    }

    #[test]
    fn tool_leading_whitespace_in_comment() {
        let result = parse_metadata(&comments(&[
            "  @TOOL number=1 type=flat_endmill diameter=10",
        ]));
        assert!(result.warnings.is_empty());
        assert_eq!(result.tools.len(), 1);
    }

    #[test]
    fn stock_case_insensitive_keys() {
        let result = parse_metadata(&comments(&["@STOCK TYPE=box WIDTH=30 DEPTH=30 HEIGHT=5"]));
        assert!(result.warnings.is_empty());
        assert!(result.stock.is_some());
    }

    #[test]
    fn stock_missing_required_width_produces_warning() {
        let result = parse_metadata(&comments(&["@STOCK type=box depth=30 height=5"]));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].message.contains("width"));
        assert!(result.stock.is_none());
    }

    #[test]
    fn stock_missing_required_depth_produces_warning() {
        let result = parse_metadata(&comments(&["@STOCK type=box width=30 height=5"]));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.stock.is_none());
    }

    #[test]
    fn stock_missing_required_height_produces_warning() {
        let result = parse_metadata(&comments(&["@STOCK type=box width=30 depth=30"]));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.stock.is_none());
    }

    #[test]
    fn stock_missing_type_produces_warning() {
        let result = parse_metadata(&comments(&["@STOCK width=30 depth=30 height=5"]));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.stock.is_none());
    }

    #[test]
    fn stock_unsupported_type_produces_warning() {
        let result = parse_metadata(&comments(&["@STOCK type=cylinder radius=30 height=5"]));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].message.contains("cylinder"));
        assert!(result.stock.is_none());
    }

    #[test]
    fn stock_invalid_origin_format_produces_warning() {
        let result = parse_metadata(&comments(&[
            "@STOCK type=box width=30 depth=30 height=5 origin=bad",
        ]));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.stock.is_none());
    }

    #[test]
    fn stock_duplicate_uses_first_and_warns() {
        let result = parse_metadata(&comments(&[
            "@STOCK type=box width=100 depth=100 height=20",
            "@STOCK type=box width=50 depth=50 height=10",
        ]));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].message.contains("duplicate"));
        let stock = result.stock.unwrap();
        assert_eq!(stock.width, 100.0); // first wins
    }

    #[test]
    fn stock_unknown_keys_silently_ignored() {
        let result = parse_metadata(&comments(&[
            "@STOCK type=box width=30 depth=30 height=5 future_key=ignored",
        ]));
        assert!(result.warnings.is_empty());
        assert!(result.stock.is_some());
    }

    // ── Tool parsing ──────────────────────────────────────────────────────

    #[test]
    fn tool_full_comment() {
        let result = parse_metadata(&comments(&[
            "@TOOL number=1 type=flat_endmill diameter=10 flutes=4 material=carbide",
        ]));
        assert!(result.warnings.is_empty());
        assert_eq!(result.tools.len(), 1);
        let tool = &result.tools[0];
        assert_eq!(tool.number, 1);
        assert_eq!(tool.tool_type, "flat_endmill");
        assert_eq!(tool.diameter, 10.0);
        assert_eq!(tool.flutes, Some(4));
        assert_eq!(tool.material.as_deref(), Some("carbide"));
    }

    #[test]
    fn tool_optional_fields_absent() {
        let result = parse_metadata(&comments(&["@TOOL number=2 type=ball_nose diameter=6"]));
        assert!(result.warnings.is_empty());
        let tool = &result.tools[0];
        assert_eq!(tool.number, 2);
        assert_eq!(tool.tool_type, "ball_nose");
        assert_eq!(tool.diameter, 6.0);
        assert_eq!(tool.flutes, None);
        assert_eq!(tool.material, None);
    }

    #[test]
    fn tool_case_insensitive_directive() {
        let result = parse_metadata(&comments(&["@tool number=1 type=flat_endmill diameter=10"]));
        assert!(result.warnings.is_empty());
        assert_eq!(result.tools.len(), 1);
    }

    #[test]
    fn tool_case_insensitive_keys() {
        let result = parse_metadata(&comments(&["@TOOL NUMBER=1 TYPE=flat_endmill DIAMETER=10"]));
        assert!(result.warnings.is_empty());
        assert_eq!(result.tools.len(), 1);
    }

    #[test]
    fn tool_type_stored_lowercase() {
        let result = parse_metadata(&comments(&["@TOOL number=1 type=FLAT_ENDMILL diameter=10"]));
        assert!(result.warnings.is_empty());
        assert_eq!(result.tools[0].tool_type, "flat_endmill");
    }

    #[test]
    fn tool_missing_number_produces_warning() {
        let result = parse_metadata(&comments(&["@TOOL type=flat_endmill diameter=10"]));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.tools.is_empty());
    }

    #[test]
    fn tool_missing_type_produces_warning() {
        let result = parse_metadata(&comments(&["@TOOL number=1 diameter=10"]));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.tools.is_empty());
    }

    #[test]
    fn tool_missing_diameter_produces_warning() {
        let result = parse_metadata(&comments(&["@TOOL number=1 type=flat_endmill"]));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.tools.is_empty());
    }

    #[test]
    fn tool_duplicate_number_last_wins_and_warns() {
        let result = parse_metadata(&comments(&[
            "@TOOL number=1 type=flat_endmill diameter=10",
            "@TOOL number=1 type=ball_nose diameter=6",
        ]));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].message.contains("duplicate"));
        assert_eq!(result.tools.len(), 1);
        // Last definition wins.
        assert_eq!(result.tools[0].tool_type, "ball_nose");
        assert_eq!(result.tools[0].diameter, 6.0);
    }

    #[test]
    fn tool_multiple_different_numbers() {
        let result = parse_metadata(&comments(&[
            "@TOOL number=1 type=flat_endmill diameter=10",
            "@TOOL number=2 type=ball_nose diameter=6",
        ]));
        assert!(result.warnings.is_empty());
        assert_eq!(result.tools.len(), 2);
        assert_eq!(result.tools[0].number, 1);
        assert_eq!(result.tools[1].number, 2);
    }

    #[test]
    fn tool_unknown_keys_silently_ignored() {
        let result = parse_metadata(&comments(&[
            "@TOOL number=1 type=flat_endmill diameter=10 future_key=ignored",
        ]));
        assert!(result.warnings.is_empty());
        assert_eq!(result.tools.len(), 1);
    }

    // ── Directive word-boundary ───────────────────────────────────────────

    #[test]
    fn stock_prefix_only_not_matched() {
        // "@STOCKROOM" starts with "@STOCK" but is not a @STOCK directive.
        // It should be silently ignored, not produce a spurious warning.
        let result = parse_metadata(&comments(&[
            "@STOCKROOM type=box width=30 depth=30 height=5",
        ]));
        assert!(result.warnings.is_empty());
        assert!(result.stock.is_none());
    }

    #[test]
    fn tool_prefix_only_not_matched() {
        // "@TOOLCHANGER" starts with "@TOOL" but is not a @TOOL directive.
        let result = parse_metadata(&comments(&["@TOOLCHANGER number=1 diameter=10"]));
        assert!(result.warnings.is_empty());
        assert!(result.tools.is_empty());
    }

    // ── Non-finite f64 values ─────────────────────────────────────────────

    #[test]
    fn stock_nan_width_produces_warning() {
        let result = parse_metadata(&comments(&["@STOCK type=box width=NaN depth=30 height=5"]));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.stock.is_none());
    }

    #[test]
    fn stock_inf_height_produces_warning() {
        let result = parse_metadata(&comments(&["@STOCK type=box width=30 depth=30 height=inf"]));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.stock.is_none());
    }

    #[test]
    fn stock_nan_origin_coord_produces_warning() {
        let result = parse_metadata(&comments(&[
            "@STOCK type=box width=30 depth=30 height=5 origin=NaN,0,0",
        ]));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.stock.is_none());
    }

    #[test]
    fn tool_nan_diameter_produces_warning() {
        let result = parse_metadata(&comments(&[
            "@TOOL number=1 type=flat_endmill diameter=NaN",
        ]));
        assert_eq!(result.warnings.len(), 1);
        assert!(result.tools.is_empty());
    }

    // ── Mixed / edge cases ────────────────────────────────────────────────

    #[test]
    fn non_directive_comments_ignored() {
        let result = parse_metadata(&comments(&[
            "demo-pocket.nc",
            "Two-level pocket",
            "@STOCK type=box width=100 depth=100 height=20",
            "@TOOL number=1 type=flat_endmill diameter=10",
        ]));
        assert!(result.warnings.is_empty());
        assert!(result.stock.is_some());
        assert_eq!(result.tools.len(), 1);
    }

    #[test]
    fn empty_header_produces_empty_result() {
        let result = parse_metadata(&[]);
        assert!(result.stock.is_none());
        assert!(result.tools.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn all_ordinary_comments_produces_empty_result() {
        let result = parse_metadata(&comments(&["This is a header", "No directives here"]));
        assert!(result.stock.is_none());
        assert!(result.tools.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn demo_pocket_nc_header_parses_correctly() {
        // Simulate the header_comments that parse_gcode produces from demo-pocket.nc.
        // The tokenizer strips the leading '; ' so we get the text that follows.
        let result = parse_metadata(&comments(&[
            "demo-pocket.nc",
            "Two-level stepped pocket: 100x100x20 mm stock, 10 mm flat endmill.",
            "@STOCK type=box width=100 depth=100 height=20 origin=0,0,0",
            "@TOOL number=1 type=flat_endmill diameter=10 flutes=4 material=carbide",
        ]));
        assert!(result.warnings.is_empty());
        let stock = result.stock.unwrap();
        assert_eq!(stock.width, 100.0);
        assert_eq!(stock.depth, 100.0);
        assert_eq!(stock.height, 20.0);
        assert_eq!(stock.origin, Vec3::zero());
        assert_eq!(result.tools.len(), 1);
        assert_eq!(result.tools[0].number, 1);
        assert_eq!(result.tools[0].tool_type, "flat_endmill");
        assert_eq!(result.tools[0].diameter, 10.0);
    }
}
