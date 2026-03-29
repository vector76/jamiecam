//! G-code parser: reads ISO 6983 G-code text and produces structured motion data.

pub(crate) mod cycles;
pub(crate) mod interpreter;
pub(crate) mod modal;
pub(crate) mod state;
pub(crate) mod tokenizer;
pub mod types;

pub use types::{
    FeedMode, MotionSegment, ParseWarning, ParsedProgram, Plane, ProgramMetadata, SegmentMetadata,
    SpindleDir, ToolChange, Units,
};

use interpreter::interpret_line;
use state::ModalState;
use tokenizer::tokenize_line;

/// Percent-delimiter state machine.
#[derive(PartialEq)]
enum PercentState {
    BeforeFirst,
    Inside,
    AfterSecond,
}

/// Parse a complete G-code program and return structured motion data.
///
/// This is the primary public entry point for the parser. It never fails —
/// problems are reported as warnings inside the returned `ParsedProgram`.
pub fn parse_gcode(input: &str) -> ParsedProgram {
    let mut state = ModalState::default();
    let mut segments: Vec<MotionSegment> = Vec::new();
    let mut tool_changes: Vec<ToolChange> = Vec::new();
    let mut warnings: Vec<ParseWarning> = Vec::new();
    let mut header_comments: Vec<String> = Vec::new();
    let mut program_number: Option<u32> = None;
    let mut first_motion_seen = false;

    // Determine whether percent delimiters are present anywhere in the input.
    let has_percent = split_lines(input).any(|l| l.trim() == "%");
    let mut pct_state = if has_percent {
        PercentState::BeforeFirst
    } else {
        PercentState::Inside // no delimiters → parse everything
    };

    for (idx, line) in split_lines(input).enumerate() {
        let line_number = idx + 1; // 1-based

        // Check for percent marker before tokenizing.
        if line.trim() == "%" {
            match pct_state {
                PercentState::BeforeFirst => pct_state = PercentState::Inside,
                PercentState::Inside => pct_state = PercentState::AfterSecond,
                PercentState::AfterSecond => {} // extra % after second — ignore
            }
            continue;
        }

        // Skip lines outside the percent-delimited region.
        if pct_state != PercentState::Inside {
            continue;
        }

        let tokens = tokenize_line(line);

        if tokens.has_expression {
            warnings.push(ParseWarning {
                line: line_number,
                message: "Macro expressions not supported, line skipped".to_string(),
            });
            continue;
        }

        if tokens.is_blank {
            // Still collect header comments from blank (comment-only) lines.
            if !first_motion_seen {
                if let Some(ref text) = tokens.comment {
                    header_comments.push(text.clone());
                }
            }
            continue;
        }

        if tokens.program_number.is_some() && program_number.is_none() {
            program_number = tokens.program_number;
        }

        if !first_motion_seen {
            if let Some(ref text) = tokens.comment {
                header_comments.push(text.clone());
            }
        }

        let result = interpret_line(&tokens.words, &mut state, line_number);

        if !result.segments.is_empty() {
            first_motion_seen = true;
        }

        if let Some(mut tc) = result.tool_change {
            // Adjust segment_index to be relative to the full program.
            tc.segment_index += segments.len();
            tool_changes.push(tc);
        }

        segments.extend(result.segments);
        warnings.extend(result.warnings);

        if result.program_end {
            break;
        }
    }

    ParsedProgram {
        metadata: ProgramMetadata {
            program_number,
            source_units: state.units,
            header_comments,
        },
        segments,
        tool_changes,
        warnings,
    }
}

/// Split input on `\n`, `\r\n`, or standalone `\r`.
///
/// First splits on `\n`, then strips a trailing `\r` from each chunk (handles
/// `\r\n`), then splits any remaining `\r` characters (handles old-Mac `\r`).
fn split_lines(input: &str) -> impl Iterator<Item = &str> {
    input
        .split('\n')
        .flat_map(|chunk| chunk.strip_suffix('\r').unwrap_or(chunk).split('\r'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Vec3;

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    // --- Empty input ---

    #[test]
    fn empty_string() {
        let result = parse_gcode("");
        assert!(result.segments.is_empty());
        assert!(result.warnings.is_empty());
        assert!(result.tool_changes.is_empty());
        assert_eq!(result.metadata.program_number, None);
        assert_eq!(result.metadata.source_units, Units::Metric);
        assert!(result.metadata.header_comments.is_empty());
    }

    // --- Comments-only program ---

    #[test]
    fn comments_only_program() {
        let input = "(header comment 1)\n(header comment 2)\n";
        let result = parse_gcode(input);
        assert!(result.segments.is_empty());
        assert_eq!(result.metadata.header_comments.len(), 2);
        assert_eq!(result.metadata.header_comments[0], "header comment 1");
        assert_eq!(result.metadata.header_comments[1], "header comment 2");
    }

    // --- Windows line endings ---

    #[test]
    fn windows_line_endings() {
        let input = "G21\r\nG90\r\nG0 X10\r\n";
        let result = parse_gcode(input);
        assert_eq!(result.segments.len(), 1);
        match &result.segments[0] {
            MotionSegment::Rapid { end, .. } => {
                assert_eq!(*end, v(10.0, 0.0, 0.0));
            }
            other => panic!("expected Rapid, got {:?}", other),
        }
    }

    // --- Old Mac line endings ---

    #[test]
    fn old_mac_line_endings() {
        let input = "G21\rG90\rG0 X10\r";
        let result = parse_gcode(input);
        assert_eq!(result.segments.len(), 1);
        match &result.segments[0] {
            MotionSegment::Rapid { end, .. } => {
                assert_eq!(*end, v(10.0, 0.0, 0.0));
            }
            other => panic!("expected Rapid, got {:?}", other),
        }
    }

    // --- Percent delimiters ---

    #[test]
    fn percent_delimiters_filter() {
        let input = "G1 X999 F100\n%\nG21\nG0 X10\n%\nG1 X999 F100\n";
        let result = parse_gcode(input);
        // Only the content between % markers should be parsed.
        assert_eq!(result.segments.len(), 1);
        match &result.segments[0] {
            MotionSegment::Rapid { end, .. } => {
                assert_eq!(*end, v(10.0, 0.0, 0.0));
            }
            other => panic!("expected Rapid, got {:?}", other),
        }
    }

    // --- No percent markers → all content parsed ---

    #[test]
    fn no_percent_markers() {
        let input = "G21\nG0 X10\nG1 X20 F500\n";
        let result = parse_gcode(input);
        assert_eq!(result.segments.len(), 2);
    }

    // --- Simple multi-line program ---

    #[test]
    fn simple_multiline_program() {
        let input = "G21\nG90\nG0 X0 Y0 Z10\nG1 X10 F500\nM2\n";
        let result = parse_gcode(input);

        // Should have 2 segments: one rapid, one linear
        assert_eq!(result.segments.len(), 2);

        match &result.segments[0] {
            MotionSegment::Rapid { start, end, .. } => {
                assert_eq!(*start, v(0.0, 0.0, 0.0));
                assert_eq!(*end, v(0.0, 0.0, 10.0));
            }
            other => panic!("expected Rapid, got {:?}", other),
        }

        match &result.segments[1] {
            MotionSegment::Linear {
                start,
                end,
                feed_rate,
                ..
            } => {
                assert_eq!(*start, v(0.0, 0.0, 10.0));
                assert_eq!(*end, v(10.0, 0.0, 10.0));
                assert_eq!(*feed_rate, 500.0);
            }
            other => panic!("expected Linear, got {:?}", other),
        }

        assert_eq!(result.metadata.source_units, Units::Metric);
    }

    // --- M2 terminates parsing ---

    #[test]
    fn m2_terminates() {
        let input = "G0 X10\nM2\nG0 X999\n";
        let result = parse_gcode(input);
        // Only one segment — the line after M2 should not be parsed.
        assert_eq!(result.segments.len(), 1);
        match &result.segments[0] {
            MotionSegment::Rapid { end, .. } => {
                assert_eq!(*end, v(10.0, 0.0, 0.0));
            }
            other => panic!("expected Rapid, got {:?}", other),
        }
    }

    // --- M30 terminates parsing ---

    #[test]
    fn m30_terminates() {
        let input = "G0 X10\nM30\nG0 X999\n";
        let result = parse_gcode(input);
        assert_eq!(result.segments.len(), 1);
    }

    // --- Source line numbers ---

    #[test]
    fn source_line_numbers() {
        let input = "G21\nG90\n\nG0 X10\nG1 X20 F500\n";
        let result = parse_gcode(input);
        assert_eq!(result.segments.len(), 2);

        match &result.segments[0] {
            MotionSegment::Rapid { metadata, .. } => {
                assert_eq!(metadata.source_line, 4);
            }
            other => panic!("expected Rapid, got {:?}", other),
        }

        match &result.segments[1] {
            MotionSegment::Linear { metadata, .. } => {
                assert_eq!(metadata.source_line, 5);
            }
            other => panic!("expected Linear, got {:?}", other),
        }
    }

    // --- Program number extraction ---

    #[test]
    fn program_number_extraction() {
        let input = "O1234\nG21\nG0 X10\n";
        let result = parse_gcode(input);
        assert_eq!(result.metadata.program_number, Some(1234));
    }

    // --- Source units ---

    #[test]
    fn source_units_default_metric() {
        let result = parse_gcode("G0 X10\n");
        assert_eq!(result.metadata.source_units, Units::Metric);
    }

    #[test]
    fn source_units_imperial() {
        let input = "G20\nG0 X1\n";
        let result = parse_gcode(input);
        assert_eq!(result.metadata.source_units, Units::Imperial);
    }

    // --- Tool change record ---

    #[test]
    fn tool_change_record() {
        let input = "T1 M6\nG0 X10\n";
        let result = parse_gcode(input);
        assert_eq!(result.tool_changes.len(), 1);
        assert_eq!(result.tool_changes[0].tool_number, 1);
        assert_eq!(result.tool_changes[0].segment_index, 0);
    }

    // --- Tool change segment_index offset ---

    #[test]
    fn tool_change_segment_index_offset() {
        let input = "G0 X10\nG1 X20 F500\nT2 M6\nG1 X30 F500\n";
        let result = parse_gcode(input);
        assert_eq!(result.tool_changes.len(), 1);
        // T2 M6 is on line 3; segments from lines 1 and 2 already accumulated (2 segments).
        assert_eq!(result.tool_changes[0].segment_index, 2);
        assert_eq!(result.tool_changes[0].tool_number, 2);
    }

    // --- Warning accumulation ---

    #[test]
    fn warning_accumulation() {
        let input = "G999\nG998\n";
        let result = parse_gcode(input);
        assert_eq!(result.warnings.len(), 2);
        assert_eq!(result.warnings[0].line, 1);
        assert_eq!(result.warnings[1].line, 2);
    }

    // --- Macro expression warning ---

    #[test]
    fn macro_expression_warning() {
        let input = "G0 X10\n#100 = 5.0\nG1 X20 F500\n";
        let result = parse_gcode(input);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("Macro expressions") && w.line == 2));
        // The other two lines should still parse.
        assert_eq!(result.segments.len(), 2);
    }

    // --- Header comments stop after first motion ---

    #[test]
    fn header_comments_stop_after_motion() {
        let input = "(header)\nG0 X10\n(not header)\nG1 X20 F500\n";
        let result = parse_gcode(input);
        assert_eq!(result.metadata.header_comments.len(), 1);
        assert_eq!(result.metadata.header_comments[0], "header");
    }

    // --- Line numbers correct with Windows endings ---

    #[test]
    fn source_line_numbers_crlf() {
        let input = "G21\r\nG90\r\n\r\nG0 X10\r\nG1 X20 F500\r\n";
        let result = parse_gcode(input);
        assert_eq!(result.segments.len(), 2);

        match &result.segments[0] {
            MotionSegment::Rapid { metadata, .. } => {
                assert_eq!(metadata.source_line, 4);
            }
            other => panic!("expected Rapid, got {:?}", other),
        }

        match &result.segments[1] {
            MotionSegment::Linear { metadata, .. } => {
                assert_eq!(metadata.source_line, 5);
            }
            other => panic!("expected Linear, got {:?}", other),
        }
    }

    // --- Line numbers correct with old Mac endings ---

    #[test]
    fn source_line_numbers_cr() {
        let input = "G21\rG90\r\rG0 X10\rG1 X20 F500\r";
        let result = parse_gcode(input);
        assert_eq!(result.segments.len(), 2);

        match &result.segments[0] {
            MotionSegment::Rapid { metadata, .. } => {
                assert_eq!(metadata.source_line, 4);
            }
            other => panic!("expected Rapid, got {:?}", other),
        }

        match &result.segments[1] {
            MotionSegment::Linear { metadata, .. } => {
                assert_eq!(metadata.source_line, 5);
            }
            other => panic!("expected Linear, got {:?}", other),
        }
    }

    // --- Percent delimiters with \r line endings ---

    #[test]
    fn percent_delimiters_cr_endings() {
        let input = "G1 X999 F100\r%\rG0 X10\r%\rG1 X999 F100\r";
        let result = parse_gcode(input);
        assert_eq!(result.segments.len(), 1);
        match &result.segments[0] {
            MotionSegment::Rapid { end, .. } => {
                assert_eq!(*end, v(10.0, 0.0, 0.0));
            }
            other => panic!("expected Rapid, got {:?}", other),
        }
    }
}
