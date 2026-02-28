/// Formats a coordinate value for G-code output.
///
/// * `decimal_places` — number of digits after the decimal point.
/// * `strip_trailing_zeros` — remove trailing zeros in the fractional part
///   (and the decimal point itself if no fractional digits remain).
/// * `suppress_leading_zero` — for values whose absolute value is less than 1,
///   omit the leading `0` (e.g. `0.5` → `.5`, `-0.5` → `-.5`).
pub fn format_coord(
    value: f64,
    decimal_places: u32,
    strip_trailing_zeros: bool,
    suppress_leading_zero: bool,
) -> String {
    let mut s = format!("{:.prec$}", value, prec = decimal_places as usize);

    if strip_trailing_zeros && s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }

    if suppress_leading_zero {
        if s.starts_with("0.") {
            s = s[1..].to_string();
        } else if s.starts_with("-0.") {
            s = format!("-{}", &s[2..]);
        }
    }

    s
}

/// Context values available for substitution in G-code template strings.
pub struct TemplateContext {
    pub tool_number: u32,
    pub tool_diameter: f64,
    pub tool_description: String,
    pub spindle_speed: f64,
    pub feed_rate: f64,
    pub program_number: u32,
}

/// Replaces template variables in `template` with values from `ctx`.
///
/// Supported variables: `{tool_number}`, `{tool_diameter}`, `{tool_description}`,
/// `{spindle_speed}`, `{feed_rate}`, `{program_number}`.
///
/// An optional width specifier can follow the variable name with a colon
/// (`{tool_number:4}`) to right-justify the substituted value in a field of
/// that many characters (space-padded on the left).
///
/// Unknown variable names are left as-is (including the surrounding braces).
pub fn render_template(template: &str, ctx: &TemplateContext) -> String {
    let mut result = String::with_capacity(template.len());
    let mut chars = template.chars();

    while let Some(ch) = chars.next() {
        if ch != '{' {
            result.push(ch);
            continue;
        }

        // Collect everything up to the matching '}'
        let mut token = String::new();
        let mut closed = false;
        for inner in chars.by_ref() {
            if inner == '}' {
                closed = true;
                break;
            }
            token.push(inner);
        }

        if !closed {
            // Unclosed brace — emit literally
            result.push('{');
            result.push_str(&token);
            continue;
        }

        result.push_str(&expand_token(&token, ctx));
    }

    result
}

/// Resolves a single `name` or `name:width` token to its substituted string.
fn expand_token(token: &str, ctx: &TemplateContext) -> String {
    let (name, width): (&str, Option<usize>) = match token.find(':') {
        Some(pos) => (&token[..pos], token[pos + 1..].parse().ok()),
        None => (token, None),
    };

    let value = match name {
        "tool_number" => ctx.tool_number.to_string(),
        "tool_diameter" => ctx.tool_diameter.to_string(),
        "tool_description" => ctx.tool_description.clone(),
        "spindle_speed" => ctx.spindle_speed.to_string(),
        "feed_rate" => ctx.feed_rate.to_string(),
        "program_number" => ctx.program_number.to_string(),
        _ => return format!("{{{}}}", token), // unknown — re-emit verbatim
    };

    match width {
        Some(w) => format!("{:>width$}", value, width = w),
        None => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // format_coord — basic formatting
    // -------------------------------------------------------------------------

    #[test]
    fn format_coord_positive_integer_value() {
        assert_eq!(format_coord(5.0, 3, false, false), "5.000");
    }

    #[test]
    fn format_coord_negative_value() {
        assert_eq!(format_coord(-12.5, 3, false, false), "-12.500");
    }

    #[test]
    fn format_coord_zero() {
        assert_eq!(format_coord(0.0, 3, false, false), "0.000");
    }

    #[test]
    fn format_coord_value_less_than_one() {
        assert_eq!(format_coord(0.5, 3, false, false), "0.500");
    }

    #[test]
    fn format_coord_negative_value_less_than_one() {
        assert_eq!(format_coord(-0.5, 3, false, false), "-0.500");
    }

    #[test]
    fn format_coord_zero_decimal_places() {
        assert_eq!(format_coord(3.7, 0, false, false), "4");
    }

    // -------------------------------------------------------------------------
    // format_coord — strip_trailing_zeros
    // -------------------------------------------------------------------------

    #[test]
    fn strip_trailing_zeros_removes_zeros() {
        assert_eq!(format_coord(1.5, 3, true, false), "1.5");
    }

    #[test]
    fn strip_trailing_zeros_removes_decimal_point_when_all_zeros() {
        assert_eq!(format_coord(3.0, 3, true, false), "3");
    }

    #[test]
    fn strip_trailing_zeros_off_keeps_zeros() {
        assert_eq!(format_coord(1.5, 3, false, false), "1.500");
    }

    #[test]
    fn strip_trailing_zeros_negative_value() {
        assert_eq!(format_coord(-0.5, 3, true, false), "-0.5");
    }

    #[test]
    fn strip_trailing_zeros_zero_value() {
        // 0.000 → all zeros stripped → "0" (no decimal point)
        assert_eq!(format_coord(0.0, 3, true, false), "0");
    }

    #[test]
    fn strip_trailing_zeros_no_decimal_places() {
        // No decimal point produced, so stripping is a no-op
        assert_eq!(format_coord(5.0, 0, true, false), "5");
    }

    // -------------------------------------------------------------------------
    // format_coord — suppress_leading_zero
    // -------------------------------------------------------------------------

    #[test]
    fn suppress_leading_zero_positive_fraction() {
        assert_eq!(format_coord(0.5, 3, false, true), ".500");
    }

    #[test]
    fn suppress_leading_zero_negative_fraction() {
        assert_eq!(format_coord(-0.5, 3, false, true), "-.500");
    }

    #[test]
    fn suppress_leading_zero_off_positive_fraction() {
        assert_eq!(format_coord(0.5, 3, false, false), "0.500");
    }

    #[test]
    fn suppress_leading_zero_off_negative_fraction() {
        assert_eq!(format_coord(-0.5, 3, false, false), "-0.500");
    }

    #[test]
    fn suppress_leading_zero_does_not_affect_values_gte_one() {
        assert_eq!(format_coord(1.5, 3, false, true), "1.500");
    }

    #[test]
    fn suppress_leading_zero_does_not_affect_values_lte_minus_one() {
        assert_eq!(format_coord(-1.5, 3, false, true), "-1.500");
    }

    #[test]
    fn suppress_leading_zero_zero_value_no_strip() {
        // 0.000 → suppress leading zero → .000
        assert_eq!(format_coord(0.0, 3, false, true), ".000");
    }

    // -------------------------------------------------------------------------
    // format_coord — combined flags
    // -------------------------------------------------------------------------

    #[test]
    fn strip_and_suppress_combined_fractional() {
        // 0.500 → strip → 0.5 → suppress → .5
        assert_eq!(format_coord(0.5, 3, true, true), ".5");
    }

    #[test]
    fn strip_and_suppress_combined_negative_fractional() {
        // -0.500 → strip → -0.5 → suppress → -.5
        assert_eq!(format_coord(-0.5, 3, true, true), "-.5");
    }

    #[test]
    fn strip_and_suppress_zero_all_stripped() {
        // 0.000 → strip → "0" (no dot) → suppress: "0" unchanged (no "0." prefix)
        assert_eq!(format_coord(0.0, 3, true, true), "0");
    }

    // -------------------------------------------------------------------------
    // render_template — individual variables
    // -------------------------------------------------------------------------

    fn ctx() -> TemplateContext {
        TemplateContext {
            tool_number: 7,
            tool_diameter: 6.35,
            tool_description: "1/4\" End Mill".to_string(),
            spindle_speed: 12000.0,
            feed_rate: 500.0,
            program_number: 42,
        }
    }

    #[test]
    fn render_tool_number() {
        assert_eq!(render_template("{tool_number}", &ctx()), "7");
    }

    #[test]
    fn render_tool_diameter() {
        assert_eq!(render_template("{tool_diameter}", &ctx()), "6.35");
    }

    #[test]
    fn render_tool_description() {
        assert_eq!(
            render_template("{tool_description}", &ctx()),
            "1/4\" End Mill"
        );
    }

    #[test]
    fn render_spindle_speed() {
        assert_eq!(render_template("{spindle_speed}", &ctx()), "12000");
    }

    #[test]
    fn render_feed_rate() {
        assert_eq!(render_template("{feed_rate}", &ctx()), "500");
    }

    #[test]
    fn render_program_number() {
        assert_eq!(render_template("{program_number}", &ctx()), "42");
    }

    // -------------------------------------------------------------------------
    // render_template — composite templates
    // -------------------------------------------------------------------------

    #[test]
    fn render_composite_tool_change() {
        let result = render_template("T{tool_number} M06 (tool: {tool_description})", &ctx());
        assert_eq!(result, "T7 M06 (tool: 1/4\" End Mill)");
    }

    #[test]
    fn render_template_no_variables() {
        assert_eq!(render_template("G90 G94 G17", &ctx()), "G90 G94 G17");
    }

    #[test]
    fn render_template_empty_string() {
        assert_eq!(render_template("", &ctx()), "");
    }

    #[test]
    fn render_unknown_variable_preserved() {
        assert_eq!(render_template("{unknown_var}", &ctx()), "{unknown_var}");
    }

    #[test]
    fn render_unclosed_brace_preserved() {
        assert_eq!(render_template("T{tool_number", &ctx()), "T{tool_number");
    }

    // -------------------------------------------------------------------------
    // render_template — width specifier
    // -------------------------------------------------------------------------

    #[test]
    fn width_specifier_pads_short_value() {
        // tool_number = 7 → right-justified in 4 chars → "   7"
        assert_eq!(render_template("{tool_number:4}", &ctx()), "   7");
    }

    #[test]
    fn width_specifier_exact_fit() {
        // program_number = 42 → width 2 → "42" (no padding needed)
        assert_eq!(render_template("{program_number:2}", &ctx()), "42");
    }

    #[test]
    fn width_specifier_value_wider_than_field() {
        // spindle_speed = 12000 → width 3 → "12000" (not truncated)
        assert_eq!(render_template("{spindle_speed:3}", &ctx()), "12000");
    }

    #[test]
    fn width_specifier_zero_width() {
        // Width 0 means no minimum — value emitted as-is
        assert_eq!(render_template("{tool_number:0}", &ctx()), "7");
    }

    #[test]
    fn width_specifier_on_description() {
        // tool_description = "1/4\" End Mill" (13 chars) → width 15 → 2 leading spaces
        assert_eq!(
            render_template("{tool_description:15}", &ctx()),
            "  1/4\" End Mill"
        );
    }

    #[test]
    fn width_specifier_invalid_falls_back_to_plain_value() {
        // Non-numeric width specifier — width is ignored and the value is substituted normally
        assert_eq!(render_template("{tool_number:abc}", &ctx()), "7");
    }
}
