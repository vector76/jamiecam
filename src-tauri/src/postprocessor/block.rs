use super::config::PostProcessorConfig;
use super::formatter::format_coord;

/// The value carried by a single G-code word.
#[derive(Debug, Clone, PartialEq)]
pub enum WordValue {
    Coord(f64),
    Int(i32),
    Str(String),
}

/// A single G-code word: a letter paired with a value.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub letter: char,
    pub value: WordValue,
}

impl Word {
    pub fn coord(letter: char, value: f64) -> Self {
        Word {
            letter,
            value: WordValue::Coord(value),
        }
    }

    pub fn int(letter: char, value: i32) -> Self {
        Word {
            letter,
            value: WordValue::Int(value),
        }
    }
}

/// A single line of G-code, holding words in canonical order and an optional comment.
pub struct Block {
    words: Vec<Word>,
    comment: Option<String>,
}

impl Block {
    /// Renders the block to a G-code string.
    ///
    /// If `line_number` is `Some`, an N-word is prepended before all other words.
    /// Coordinate values are formatted using `fmt.format` settings.
    /// Comments are wrapped using `fmt.program.comment_open` / `comment_close`.
    pub fn render(&self, line_number: Option<u32>, fmt: &PostProcessorConfig) -> String {
        let sep = &fmt.format.word_separator;
        let mut line = String::new();
        let mut needs_sep = false;

        if let Some(n) = line_number {
            line.push_str(&format!("N{}", n));
            needs_sep = true;
        }

        for word in &self.words {
            if needs_sep {
                line.push_str(sep);
            }
            line.push_str(&render_word(word, fmt));
            needs_sep = true;
        }

        if let Some(text) = &self.comment {
            if needs_sep {
                line.push_str(sep);
            }
            line.push_str(&fmt.program.comment_open);
            line.push_str(text);
            line.push_str(&fmt.program.comment_close);
        }

        line.push_str(&fmt.format.eol);
        line
    }
}

fn render_word(word: &Word, fmt: &PostProcessorConfig) -> String {
    match &word.value {
        WordValue::Coord(v) => format!(
            "{}{}",
            word.letter,
            format_coord(
                *v,
                fmt.format.decimal_places,
                !fmt.format.trailing_zeros,
                fmt.format.leading_zero_suppression,
            )
        ),
        WordValue::Int(i) => format!("{}{}", word.letter, i),
        WordValue::Str(s) => s.clone(),
    }
}

/// Builds a [`Block`] by accumulating words in named slots, then emitting them
/// in canonical G-code word order on [`build`](BlockBuilder::build):
///
/// motion G → other G-codes → X Y Z A B C → I J K R → F → S → T → coolant M → spindle M
#[derive(Default)]
pub struct BlockBuilder {
    motion: Option<String>,
    g_codes: Vec<String>,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    a: Option<f64>,
    b: Option<f64>,
    c: Option<f64>,
    i: Option<f64>,
    j: Option<f64>,
    k: Option<f64>,
    r: Option<f64>,
    feed_val: Option<f64>,
    spindle_speed: Option<f64>,
    tool_num: Option<u32>,
    coolant: Option<String>,
    spindle_m_code: Option<String>,
    comment_text: Option<String>,
}

impl BlockBuilder {
    pub fn new() -> Self {
        BlockBuilder::default()
    }

    /// Sets the motion G-code (e.g., `"G00"`, `"G01"`, `"G02"`).
    pub fn motion(mut self, code: &str) -> Self {
        self.motion = Some(code.to_string());
        self
    }

    /// Adds an additional G-code word (e.g., `"G90"`, `"G17"`).
    pub fn g(mut self, code: &str) -> Self {
        self.g_codes.push(code.to_string());
        self
    }

    /// Adds an axis word. `letter` must be one of X, Y, Z, A, B, C (case-insensitive).
    pub fn axis(mut self, letter: char, value: f64) -> Self {
        match letter.to_ascii_uppercase() {
            'X' => self.x = Some(value),
            'Y' => self.y = Some(value),
            'Z' => self.z = Some(value),
            'A' => self.a = Some(value),
            'B' => self.b = Some(value),
            'C' => self.c = Some(value),
            _ => {}
        }
        self
    }

    /// Adds an arc parameter word. `letter` must be one of I, J, K, R (case-insensitive).
    pub fn arc_param(mut self, letter: char, value: f64) -> Self {
        match letter.to_ascii_uppercase() {
            'I' => self.i = Some(value),
            'J' => self.j = Some(value),
            'K' => self.k = Some(value),
            'R' => self.r = Some(value),
            _ => {}
        }
        self
    }

    /// Sets the feed rate F word.
    pub fn feed(mut self, value: f64) -> Self {
        self.feed_val = Some(value);
        self
    }

    /// Sets the spindle speed S word.
    pub fn spindle(mut self, value: f64) -> Self {
        self.spindle_speed = Some(value);
        self
    }

    /// Sets the tool number T word.
    pub fn tool(mut self, number: u32) -> Self {
        self.tool_num = Some(number);
        self
    }

    /// Sets the coolant M-code (e.g., `"M08"`).
    pub fn coolant_m(mut self, code: &str) -> Self {
        self.coolant = Some(code.to_string());
        self
    }

    /// Sets the spindle M-code (e.g., `"M03"`, `"M04"`).
    pub fn spindle_m(mut self, code: &str) -> Self {
        self.spindle_m_code = Some(code.to_string());
        self
    }

    /// Sets the block comment text (without delimiters).
    pub fn comment(mut self, text: &str) -> Self {
        self.comment_text = Some(text.to_string());
        self
    }

    /// Consumes the builder and produces a [`Block`] with words in canonical order.
    pub fn build(self) -> Block {
        let mut words: Vec<Word> = Vec::with_capacity(16 + self.g_codes.len());

        if let Some(code) = self.motion {
            words.push(Word {
                letter: 'G',
                value: WordValue::Str(code),
            });
        }

        for code in self.g_codes {
            words.push(Word {
                letter: 'G',
                value: WordValue::Str(code),
            });
        }

        for (letter, opt_val) in [
            ('X', self.x),
            ('Y', self.y),
            ('Z', self.z),
            ('A', self.a),
            ('B', self.b),
            ('C', self.c),
        ] {
            if let Some(v) = opt_val {
                words.push(Word::coord(letter, v));
            }
        }

        for (letter, opt_val) in [('I', self.i), ('J', self.j), ('K', self.k), ('R', self.r)] {
            if let Some(v) = opt_val {
                words.push(Word::coord(letter, v));
            }
        }

        if let Some(v) = self.feed_val {
            words.push(Word::coord('F', v));
        }

        if let Some(v) = self.spindle_speed {
            words.push(Word::coord('S', v));
        }

        if let Some(n) = self.tool_num {
            words.push(Word::int('T', n as i32));
        }

        if let Some(code) = self.coolant {
            words.push(Word {
                letter: 'M',
                value: WordValue::Str(code),
            });
        }

        if let Some(code) = self.spindle_m_code {
            words.push(Word {
                letter: 'M',
                value: WordValue::Str(code),
            });
        }

        Block {
            words,
            comment: self.comment_text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postprocessor::config;

    fn base_toml() -> String {
        r#"
[meta]
id = "test"
name = "Test"
description = "Test controller"
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
eol = "\n"
percent_delimiters = false
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
header = []
footer = []

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

    fn default_fmt() -> config::PostProcessorConfig {
        config::parse(&base_toml()).expect("minimal test TOML must be valid")
    }

    // -------------------------------------------------------------------------
    // Word order
    // -------------------------------------------------------------------------

    #[test]
    fn canonical_word_order_full_block() {
        let fmt = default_fmt();
        let block = BlockBuilder::new()
            .spindle_m("M03")
            .coolant_m("M08")
            .tool(5)
            .spindle(12000.0)
            .feed(500.0)
            .arc_param('J', 10.0)
            .arc_param('I', 5.0)
            .axis('Z', -1.0)
            .axis('X', 10.0)
            .axis('Y', 20.0)
            .g("G90")
            .motion("G01")
            .build();

        let rendered = block.render(None, &fmt);
        // Strip EOL for easier comparison
        let line = rendered.trim_end();

        // Check canonical order: G01 G90 X Y Z I J F S T M08 M03
        let parts: Vec<&str> = line.split(' ').collect();
        let pos = |s: &str| parts.iter().position(|&p| p == s).expect(s);

        assert!(pos("G01") < pos("G90"), "motion before other G");
        assert!(pos("G90") < pos("X10"), "G codes before axes");
        assert!(pos("X10") < pos("Y20"), "X before Y");
        assert!(pos("Y20") < pos("Z-1"), "Y before Z");
        assert!(pos("Z-1") < pos("I5"), "axes before arc params");
        assert!(pos("I5") < pos("J10"), "I before J");
        assert!(pos("J10") < pos("F500"), "arc params before F");
        assert!(pos("F500") < pos("S12000"), "F before S");
        assert!(pos("S12000") < pos("T5"), "S before T");
        assert!(pos("T5") < pos("M08"), "T before coolant M");
        assert!(pos("M08") < pos("M03"), "coolant M before spindle M");
    }

    #[test]
    fn axis_order_xyz_abc() {
        let fmt = default_fmt();
        let block = BlockBuilder::new()
            .axis('C', 90.0)
            .axis('A', 45.0)
            .axis('B', 0.0)
            .axis('Z', -5.0)
            .axis('Y', 10.0)
            .axis('X', 5.0)
            .build();

        let line = block.render(None, &fmt);
        let line = line.trim_end();
        let parts: Vec<&str> = line.split(' ').collect();
        let pos = |s: &str| parts.iter().position(|&p| p == s).expect(s);

        assert!(pos("X5") < pos("Y10"));
        assert!(pos("Y10") < pos("Z-5"));
        assert!(pos("Z-5") < pos("A45"));
        assert!(pos("A45") < pos("B0"));
        assert!(pos("B0") < pos("C90"));
    }

    #[test]
    fn arc_param_order_ijkr() {
        let fmt = default_fmt();
        let block = BlockBuilder::new()
            .arc_param('R', 5.0)
            .arc_param('K', 3.0)
            .arc_param('J', 2.0)
            .arc_param('I', 1.0)
            .build();

        let line = block.render(None, &fmt);
        let line = line.trim_end();
        let parts: Vec<&str> = line.split(' ').collect();
        let pos = |s: &str| parts.iter().position(|&p| p == s).expect(s);

        assert!(pos("I1") < pos("J2"));
        assert!(pos("J2") < pos("K3"));
        assert!(pos("K3") < pos("R5"));
    }

    // -------------------------------------------------------------------------
    // Line numbers
    // -------------------------------------------------------------------------

    #[test]
    fn line_number_included_when_some() {
        let fmt = default_fmt();
        let block = BlockBuilder::new().motion("G00").axis('X', 1.0).build();
        let line = block.render(Some(10), &fmt);
        assert!(
            line.starts_with("N10 "),
            "expected N10 prefix, got: {:?}",
            line
        );
    }

    #[test]
    fn line_number_omitted_when_none() {
        let fmt = default_fmt();
        let block = BlockBuilder::new().motion("G00").axis('X', 1.0).build();
        let line = block.render(None, &fmt);
        assert!(
            !line.starts_with('N'),
            "expected no N prefix, got: {:?}",
            line
        );
    }

    #[test]
    fn line_number_is_first_word() {
        let fmt = default_fmt();
        let block = BlockBuilder::new()
            .g("G90")
            .motion("G00")
            .axis('X', 5.0)
            .build();
        let line = block.render(Some(100), &fmt);
        assert!(line.starts_with("N100 G00"));
    }

    // -------------------------------------------------------------------------
    // Comments
    // -------------------------------------------------------------------------

    #[test]
    fn comment_appended_with_paren_style() {
        let fmt = default_fmt(); // comment_open="(" comment_close=")"
        let block = BlockBuilder::new()
            .motion("G00")
            .axis('X', 0.0)
            .comment("rapid to origin")
            .build();
        let line = block.render(None, &fmt);
        assert!(
            line.trim_end().ends_with("(rapid to origin)"),
            "got: {:?}",
            line
        );
    }

    #[test]
    fn comment_appended_with_semicolon_style() {
        let toml = base_toml()
            .replace(r#"comment_open = "(""#, r#"comment_open = "; ""#)
            .replace(r#"comment_close = ")""#, r#"comment_close = """#);
        let fmt = config::parse(&toml).unwrap();
        let block = BlockBuilder::new().motion("G00").comment("my note").build();
        let line = block.render(None, &fmt);
        assert!(line.trim_end().ends_with("; my note"), "got: {:?}", line);
    }

    #[test]
    fn comment_only_block_no_leading_separator() {
        let fmt = default_fmt();
        let block = BlockBuilder::new().comment("setup complete").build();
        let line = block.render(None, &fmt);
        assert_eq!(line.trim_end(), "(setup complete)");
    }

    #[test]
    fn comment_separated_from_words_by_separator() {
        let fmt = default_fmt();
        let block = BlockBuilder::new()
            .motion("G01")
            .axis('X', 5.0)
            .comment("cut")
            .build();
        let line = block.render(None, &fmt);
        // The comment must be separated from the last word by the word_separator (" ")
        assert!(line.contains("X5 (cut)"), "got: {:?}", line);
    }

    // -------------------------------------------------------------------------
    // EOL
    // -------------------------------------------------------------------------

    #[test]
    fn eol_appended() {
        let fmt = default_fmt(); // eol = "\n"
        let block = BlockBuilder::new().motion("G00").build();
        let line = block.render(None, &fmt);
        assert!(line.ends_with('\n'), "got: {:?}", line);
    }

    #[test]
    fn crlf_eol() {
        let toml = base_toml().replace(r#"eol = "\n""#, r#"eol = "\r\n""#);
        let fmt = config::parse(&toml).unwrap();
        let block = BlockBuilder::new().motion("G00").build();
        let line = block.render(None, &fmt);
        assert!(line.ends_with("\r\n"), "got: {:?}", line);
    }

    // -------------------------------------------------------------------------
    // Coordinate formatting
    // -------------------------------------------------------------------------

    #[test]
    fn trailing_zeros_stripped_when_trailing_zeros_false() {
        let fmt = default_fmt(); // trailing_zeros = false → strip
        let block = BlockBuilder::new().axis('X', 1.5).build();
        let line = block.render(None, &fmt);
        assert!(line.contains("X1.5"), "got: {:?}", line);
    }

    #[test]
    fn trailing_zeros_kept_when_trailing_zeros_true() {
        let toml = base_toml().replace("trailing_zeros = false", "trailing_zeros = true");
        let fmt = config::parse(&toml).unwrap();
        let block = BlockBuilder::new().axis('X', 1.5).build();
        let line = block.render(None, &fmt);
        assert!(line.contains("X1.500"), "got: {:?}", line);
    }

    #[test]
    fn leading_zero_suppression() {
        let toml = base_toml().replace(
            "leading_zero_suppression = false",
            "leading_zero_suppression = true",
        );
        let fmt = config::parse(&toml).unwrap();
        let block = BlockBuilder::new().axis('X', 0.5).build();
        let line = block.render(None, &fmt);
        assert!(line.contains("X.5"), "got: {:?}", line);
    }

    // -------------------------------------------------------------------------
    // Int word
    // -------------------------------------------------------------------------

    #[test]
    fn tool_word_renders_as_integer() {
        let fmt = default_fmt();
        let block = BlockBuilder::new().tool(7).build();
        let line = block.render(None, &fmt);
        assert!(line.trim_end().contains("T7"), "got: {:?}", line);
    }

    // -------------------------------------------------------------------------
    // Empty block
    // -------------------------------------------------------------------------

    #[test]
    fn empty_block_renders_only_eol() {
        let fmt = default_fmt();
        let block = BlockBuilder::new().build();
        let line = block.render(None, &fmt);
        assert_eq!(line, "\n");
    }
}
