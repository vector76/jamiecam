use super::PostProcessorError;

/// Output units for the generated G-code program.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Units {
    Metric,
    Imperial,
}

/// Kinematic family for 5-axis machines (`machine.five_axis_type`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FiveAxisType {
    HeadHead,
    HeadTable,
    TableTable,
}

/// Arc representation format (`motion.arc_format`).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcFormat {
    /// Center offsets from arc start (I, J, K words). Handles arcs of any angle.
    Ijk,
    /// Signed radius word. Cannot represent exactly 180° arcs.
    R,
}

/// Fully describes one CNC controller. Loaded from a TOML file.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PostProcessorConfig {
    pub meta: MetaConfig,
    pub machine: MachineConfig,
    pub format: FormatConfig,
    pub axes: AxesConfig,
    pub program: ProgramConfig,
    pub tool_change: ToolChangeConfig,
    pub motion: MotionConfig,
    pub words: WordsConfig,
    pub spindle: SpindleConfig,
    pub coolant: CoolantConfig,
    pub cycles: CyclesConfig,
    pub misc: MiscConfig,
}

/// `[meta]` — identity and display information.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MetaConfig {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
}

/// `[machine]` — machine capability flags.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MachineConfig {
    pub units: Units,
    pub max_axes: u32,
    pub five_axis_type: Option<FiveAxisType>,
    /// When true, the controller compensates for the pivot distance automatically
    /// (RTCP / TCPM). Requires `tool_change.rtcp_on` to be set.
    #[serde(default)]
    pub rtcp_supported: bool,
}

/// `[format]` — output formatting options.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FormatConfig {
    pub line_numbers: bool,
    pub line_number_start: u32,
    pub line_number_increment: u32,
    pub line_number_max: u32,
    pub decimal_places: u32,
    pub trailing_zeros: bool,
    pub leading_zero_suppression: bool,
    pub word_separator: String,
    pub eol: String,
    pub percent_delimiters: bool,
    pub block_delete_char: String,
}

/// `[axes.limits]` — software limits for rotary axes.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AxisLimits {
    pub a_min: f64,
    pub a_max: f64,
    pub b_min: f64,
    pub b_max: f64,
    pub c_min: f64,
    pub c_max: f64,
}

/// `[axes]` — axis letter assignments and optional limits.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AxesConfig {
    pub x: String,
    pub y: String,
    pub z: String,
    pub a: Option<String>,
    pub b: Option<String>,
    pub c: Option<String>,
    pub limits: Option<AxisLimits>,
}

/// `[program]` — program structure: numbering, comments, header/footer.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProgramConfig {
    pub number_prefix: String,
    pub number: u32,
    pub number_format: String,
    pub comment_open: String,
    pub comment_close: String,
    pub header: Vec<String>,
    pub footer: Vec<String>,
}

/// `[tool_change]` — tool-change sequence templates.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ToolChangeConfig {
    pub pre: Vec<String>,
    /// Template string for the tool-change block. Must contain `{tool_number}`.
    pub command: String,
    pub post: Vec<String>,
    pub suppress_first_if_t1: bool,
    /// RTCP activation code emitted after a tool change on RTCP-capable machines.
    /// Required when `machine.rtcp_supported = true`.
    pub rtcp_on: Option<String>,
}

/// `[motion]` — motion command words and arc configuration.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MotionConfig {
    pub rapid: String,
    pub linear: String,
    pub arc_cw: String,
    pub arc_ccw: String,
    pub arc_format: ArcFormat,
    pub plane_xy: String,
    pub plane_xz: String,
    pub plane_yz: String,
}

/// `[words]` — feed/speed/mode word letters and codes.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct WordsConfig {
    pub feed: String,
    pub spindle: String,
    pub tool: String,
    pub tool_offset: String,
    pub dwell: String,
    pub feed_per_min: String,
    pub feed_per_rev: String,
    pub inverse_time: String,
    pub absolute: String,
    pub incremental: String,
}

/// `[spindle]` — spindle control codes.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SpindleConfig {
    pub on_cw: String,
    pub on_ccw: String,
    pub off: String,
    pub orient: Option<String>,
    pub max_rpm: u32,
}

/// `[coolant]` — coolant control codes.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CoolantConfig {
    pub flood: String,
    pub mist: String,
    pub air: String,
    pub off: String,
    pub through_tool: Option<String>,
}

/// `[cycles]` — canned drilling cycle support.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CyclesConfig {
    /// When `false`, all cycles are expanded to explicit linear moves.
    pub supported: bool,
    /// Spot / through-drill cycle code (e.g. `"G81"`). Required when `supported = true`.
    pub drill: Option<String>,
    pub peck: Option<String>,
    pub chip_break: Option<String>,
    pub boring_feed: Option<String>,
    pub boring_dwell: Option<String>,
    pub reaming: Option<String>,
    pub tapping: Option<String>,
    pub tapping_ccw: Option<String>,
    pub cycle_cancel: Option<String>,
    pub r_plane_abs: Option<String>,
    pub r_plane_r: Option<String>,
}

/// `[misc]` — miscellaneous stop/pause codes.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MiscConfig {
    pub optional_stop: String,
    pub program_stop: String,
}

/// Parse a TOML string into a [`PostProcessorConfig`], running validation.
pub fn parse(toml_str: &str) -> Result<PostProcessorConfig, PostProcessorError> {
    let cfg: PostProcessorConfig =
        toml::from_str(toml_str).map_err(|e| PostProcessorError::Config(e.to_string()))?;
    validate(&cfg)?;
    Ok(cfg)
}

fn validate(cfg: &PostProcessorConfig) -> Result<(), PostProcessorError> {
    // `{tool_number}` must appear in tool_change.command.
    if !cfg.tool_change.command.contains("{tool_number}") {
        return Err(PostProcessorError::Config(
            "tool_change.command must contain {tool_number}".to_string(),
        ));
    }

    // When cycles are supported, a drill code must be provided.
    if cfg.cycles.supported {
        let drill_present = cfg.cycles.drill.as_deref().is_some_and(|s| !s.is_empty());
        if !drill_present {
            return Err(PostProcessorError::Config(
                "cycles.drill must be defined when cycles.supported = true".to_string(),
            ));
        }
    }

    // When RTCP is supported, rtcp_on must be non-empty.
    if cfg.machine.rtcp_supported {
        let rtcp_on_present = cfg
            .tool_change
            .rtcp_on
            .as_deref()
            .is_some_and(|s| !s.is_empty());
        if !rtcp_on_present {
            return Err(PostProcessorError::Config(
                "tool_change.rtcp_on must be defined when machine.rtcp_supported = true"
                    .to_string(),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid TOML that passes all validation rules.
    fn minimal_valid_toml() -> String {
        r#"
[meta]
id = "test"
name = "Test Controller"
description = "Test"
version = "1.0"
author = "Test"

[machine]
units = "metric"
max_axes = 3

[format]
line_numbers = true
line_number_start = 10
line_number_increment = 10
line_number_max = 9999
decimal_places = 3
trailing_zeros = false
leading_zero_suppression = false
word_separator = " "
eol = "\r\n"
percent_delimiters = true
block_delete_char = ""

[axes]
x = "X"
y = "Y"
z = "Z"

[program]
number_prefix = "O"
number = 1000
number_format = "%04d"
comment_open = "("
comment_close = ")"
header = ["G90 G94 G17"]
footer = ["M30"]

[tool_change]
pre = []
command = "T{tool_number} M06"
post = []
suppress_first_if_t1 = false

[motion]
rapid = "G00"
linear = "G01"
arc_cw = "G02"
arc_ccw = "G03"
arc_format = "ijk"
plane_xy = "G17"
plane_xz = "G18"
plane_yz = "G19"

[words]
feed = "F"
spindle = "S"
tool = "T"
tool_offset = "H"
dwell = "P"
feed_per_min = "G94"
feed_per_rev = "G95"
inverse_time = "G93"
absolute = "G90"
incremental = "G91"

[spindle]
on_cw = "M03"
on_ccw = "M04"
off = "M05"
max_rpm = 15000

[coolant]
flood = "M08"
mist = "M07"
air = "M07"
off = "M09"

[cycles]
supported = false

[misc]
optional_stop = "M01"
program_stop = "M00"
"#
        .to_string()
    }

    #[test]
    fn valid_config_parses_successfully() {
        assert!(parse(&minimal_valid_toml()).is_ok());
    }

    #[test]
    fn invalid_toml_returns_config_error() {
        let result = parse("this is not valid toml ::::");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PostProcessorError::Config(_)));
    }

    #[test]
    fn missing_tool_number_in_command_returns_error() {
        let toml = minimal_valid_toml().replace(
            r#"command = "T{tool_number} M06""#,
            r#"command = "T01 M06""#,
        );
        let result = parse(&toml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, PostProcessorError::Config(_)));
        assert!(err.to_string().contains("{tool_number}"));
    }

    #[test]
    fn cycles_supported_without_drill_code_returns_error() {
        let toml = minimal_valid_toml()
            .replace("[cycles]\nsupported = false", "[cycles]\nsupported = true");
        let result = parse(&toml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, PostProcessorError::Config(_)));
        assert!(err.to_string().contains("drill"));
    }

    #[test]
    fn cycles_supported_with_drill_code_passes_validation() {
        let toml = minimal_valid_toml().replace(
            "[cycles]\nsupported = false",
            "[cycles]\nsupported = true\ndrill = \"G81\"",
        );
        assert!(parse(&toml).is_ok());
    }

    #[test]
    fn rtcp_supported_without_rtcp_on_returns_error() {
        let toml = minimal_valid_toml().replace(
            "[machine]\nunits = \"metric\"\nmax_axes = 3",
            "[machine]\nunits = \"metric\"\nmax_axes = 3\nrtcp_supported = true",
        );
        let result = parse(&toml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, PostProcessorError::Config(_)));
        assert!(err.to_string().contains("rtcp_on"));
    }

    #[test]
    fn rtcp_supported_with_rtcp_on_passes_validation() {
        let toml = minimal_valid_toml()
            .replace(
                "[machine]\nunits = \"metric\"\nmax_axes = 3",
                "[machine]\nunits = \"metric\"\nmax_axes = 3\nrtcp_supported = true",
            )
            .replace(
                "suppress_first_if_t1 = false",
                "suppress_first_if_t1 = false\nrtcp_on = \"G43.4 H{tool_number}\"",
            );
        assert!(parse(&toml).is_ok());
    }

    #[test]
    fn rtcp_supported_false_does_not_require_rtcp_on() {
        let toml = minimal_valid_toml().replace(
            "[machine]\nunits = \"metric\"\nmax_axes = 3",
            "[machine]\nunits = \"metric\"\nmax_axes = 3\nrtcp_supported = false",
        );
        assert!(parse(&toml).is_ok());
    }

    #[test]
    fn cycles_not_supported_does_not_require_drill_code() {
        // minimal_valid_toml already has supported = false with no drill code
        assert!(parse(&minimal_valid_toml()).is_ok());
    }
}
