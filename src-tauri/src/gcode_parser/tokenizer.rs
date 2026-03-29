//! Lexical layer: tokenizes raw G-code lines into words and metadata.

#![allow(dead_code)] // Consumed by later beads; no callers yet.

/// A single G-code word: a letter plus a numeric value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GcodeWord {
    pub letter: char,
    pub value: f64,
}

/// Result of tokenizing one line of G-code.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TokenizedLine {
    pub words: Vec<GcodeWord>,
    pub comment: Option<String>,
    pub is_percent_marker: bool,
    pub program_number: Option<u32>,
    pub is_blank: bool,
    pub has_expression: bool,
}

/// Tokenize a single raw G-code line into words and classification flags.
pub(crate) fn tokenize_line(raw: &str) -> TokenizedLine {
    let mut comment: Option<String> = None;
    let mut stripped = String::with_capacity(raw.len());

    // --- Pass 1: strip comments, extract comment text ---
    let mut chars = raw.chars().peekable();
    while let Some(&ch) = chars.peek() {
        if ch == '(' {
            // Parenthesised comment — handle nesting / unmatched parens gracefully.
            chars.next();
            let mut depth: u32 = 1;
            let mut text = String::new();
            for c in chars.by_ref() {
                if c == '(' {
                    depth += 1;
                    text.push(c);
                } else if c == ')' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    text.push(c);
                } else {
                    text.push(c);
                }
            }
            // depth > 0 means unmatched open paren — we just consumed the rest of line.
            let trimmed = text.trim().to_string();
            if !trimmed.is_empty() {
                comment = Some(trimmed);
            }
        } else if ch == ';' {
            // Semicolon comment — rest of line.
            chars.next(); // consume ';'
            let rest: String = chars.collect();
            let trimmed = rest.trim().to_string();
            if !trimmed.is_empty() {
                comment = Some(trimmed);
            }
            break;
        } else {
            stripped.push(ch);
            chars.next();
        }
    }

    let trimmed = stripped.trim();

    // --- Blank line ---
    if trimmed.is_empty() {
        return TokenizedLine {
            words: Vec::new(),
            comment,
            is_percent_marker: false,
            program_number: None,
            is_blank: true,
            has_expression: false,
        };
    }

    // --- Percent marker ---
    if trimmed == "%" {
        return TokenizedLine {
            words: Vec::new(),
            comment,
            is_percent_marker: true,
            program_number: None,
            is_blank: false,
            has_expression: false,
        };
    }

    // --- Expression / variable detection ---
    if trimmed.contains('#') || trimmed.contains('[') {
        return TokenizedLine {
            words: Vec::new(),
            comment,
            is_percent_marker: false,
            program_number: None,
            is_blank: false,
            has_expression: true,
        };
    }

    // --- Parse words ---
    let words = parse_words(trimmed);

    // --- N-word-only → blank ---
    let all_n = !words.is_empty() && words.iter().all(|w| w.letter == 'N');
    if all_n {
        return TokenizedLine {
            words: Vec::new(),
            comment,
            is_percent_marker: false,
            program_number: None,
            is_blank: true,
            has_expression: false,
        };
    }

    // --- O-word (program number) ---
    let program_number = if words.len() == 1 && words[0].letter == 'O' {
        Some(words[0].value as u32)
    } else {
        None
    };

    TokenizedLine {
        words,
        comment,
        is_percent_marker: false,
        program_number,
        is_blank: false,
        has_expression: false,
    }
}

/// Parse G-code words from a comment-stripped, trimmed line.
fn parse_words(s: &str) -> Vec<GcodeWord> {
    let mut words = Vec::new();
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let b = bytes[i];
        // Skip whitespace.
        if b == b' ' || b == b'\t' {
            i += 1;
            continue;
        }
        // A word starts with a letter.
        if b.is_ascii_alphabetic() {
            let letter = (b as char).to_ascii_uppercase();
            i += 1;
            // Collect the numeric value.
            let num_start = i;
            // Optional sign.
            if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
                i += 1;
            }
            // Digits and decimal point.
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let num_str = &s[num_start..i];
            let value: f64 = if num_str.is_empty() {
                0.0
            } else {
                num_str.parse().unwrap_or(0.0)
            };
            words.push(GcodeWord { letter, value });
        } else {
            // Skip unexpected characters.
            i += 1;
        }
    }

    words
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Individual word parsing ---

    #[test]
    fn word_integer() {
        let line = tokenize_line("G01");
        assert_eq!(line.words.len(), 1);
        assert_eq!(line.words[0].letter, 'G');
        assert_eq!(line.words[0].value, 1.0);
    }

    #[test]
    fn word_negative_decimal() {
        let line = tokenize_line("X-5.25");
        assert_eq!(line.words.len(), 1);
        assert_eq!(line.words[0].letter, 'X');
        assert_eq!(line.words[0].value, -5.25);
    }

    #[test]
    fn word_decimal_with_trailing_zero() {
        let line = tokenize_line("F1500.0");
        assert_eq!(line.words.len(), 1);
        assert_eq!(line.words[0].letter, 'F');
        assert_eq!(line.words[0].value, 1500.0);
    }

    #[test]
    fn word_suppressed_leading_zero() {
        let line = tokenize_line("X.5");
        assert_eq!(line.words.len(), 1);
        assert_eq!(line.words[0].letter, 'X');
        assert_eq!(line.words[0].value, 0.5);
    }

    #[test]
    fn word_trailing_decimal() {
        let line = tokenize_line("X5.");
        assert_eq!(line.words.len(), 1);
        assert_eq!(line.words[0].letter, 'X');
        assert_eq!(line.words[0].value, 5.0);
    }

    // --- Comment stripping ---

    #[test]
    fn paren_comment_with_words() {
        let line = tokenize_line("(comment) G01 X5");
        assert_eq!(line.comment.as_deref(), Some("comment"));
        assert_eq!(line.words.len(), 2);
        assert_eq!(
            line.words[0],
            GcodeWord {
                letter: 'G',
                value: 1.0
            }
        );
        assert_eq!(
            line.words[1],
            GcodeWord {
                letter: 'X',
                value: 5.0
            }
        );
    }

    #[test]
    fn semicolon_comment() {
        let line = tokenize_line("G01 X5 ;inline");
        assert_eq!(line.comment.as_deref(), Some("inline"));
        assert_eq!(line.words.len(), 2);
        assert_eq!(
            line.words[0],
            GcodeWord {
                letter: 'G',
                value: 1.0
            }
        );
        assert_eq!(
            line.words[1],
            GcodeWord {
                letter: 'X',
                value: 5.0
            }
        );
    }

    // --- No-space concatenation ---

    #[test]
    fn no_space_concat() {
        let line = tokenize_line("G01X5Y3Z-1F200");
        assert_eq!(line.words.len(), 5);
        assert_eq!(
            line.words[0],
            GcodeWord {
                letter: 'G',
                value: 1.0
            }
        );
        assert_eq!(
            line.words[1],
            GcodeWord {
                letter: 'X',
                value: 5.0
            }
        );
        assert_eq!(
            line.words[2],
            GcodeWord {
                letter: 'Y',
                value: 3.0
            }
        );
        assert_eq!(
            line.words[3],
            GcodeWord {
                letter: 'Z',
                value: -1.0
            }
        );
        assert_eq!(
            line.words[4],
            GcodeWord {
                letter: 'F',
                value: 200.0
            }
        );
    }

    // --- Mixed case ---

    #[test]
    fn mixed_case() {
        let line = tokenize_line("g01x5");
        assert_eq!(line.words.len(), 2);
        assert_eq!(
            line.words[0],
            GcodeWord {
                letter: 'G',
                value: 1.0
            }
        );
        assert_eq!(
            line.words[1],
            GcodeWord {
                letter: 'X',
                value: 5.0
            }
        );
    }

    // --- Percent marker ---

    #[test]
    fn percent_marker() {
        let line = tokenize_line("%");
        assert!(line.is_percent_marker);
        assert!(line.words.is_empty());
    }

    #[test]
    fn percent_marker_with_whitespace() {
        let line = tokenize_line("  %  ");
        assert!(line.is_percent_marker);
    }

    // --- N-word-only → blank ---

    #[test]
    fn n_word_only_is_blank() {
        let line = tokenize_line("N10");
        assert!(line.is_blank);
        assert!(line.words.is_empty());
    }

    // --- Expression detection ---

    #[test]
    fn expression_hash() {
        let line = tokenize_line("#100 = 5.0");
        assert!(line.has_expression);
    }

    #[test]
    fn expression_bracket() {
        let line = tokenize_line("X[#1 + 2]");
        assert!(line.has_expression);
    }

    // --- Blank / whitespace lines ---

    #[test]
    fn blank_line_empty() {
        let line = tokenize_line("");
        assert!(line.is_blank);
    }

    #[test]
    fn blank_line_whitespace() {
        let line = tokenize_line("   ");
        assert!(line.is_blank);
    }

    // --- Program number ---

    #[test]
    fn program_number() {
        let line = tokenize_line("O1234");
        assert_eq!(line.program_number, Some(1234));
    }

    #[test]
    fn o_word_with_other_words_not_program_number() {
        let line = tokenize_line("O1234 G01");
        assert_eq!(line.program_number, None);
    }

    // --- Edge: nested paren comment ---

    #[test]
    fn nested_paren_comment() {
        let line = tokenize_line("(outer (inner)) G0 X1");
        assert_eq!(line.comment.as_deref(), Some("outer (inner)"));
        assert_eq!(line.words.len(), 2);
    }

    // --- Edge: unmatched paren ---

    #[test]
    fn unmatched_paren_does_not_crash() {
        let line = tokenize_line("(no closing paren G0 X1");
        // Everything after '(' is consumed as comment; no words remain.
        assert!(line.is_blank);
        assert_eq!(line.comment.as_deref(), Some("no closing paren G0 X1"));
    }

    // --- Edge: comment-only line ---

    #[test]
    fn comment_only_line_is_blank() {
        let line = tokenize_line("(just a comment)");
        assert!(line.is_blank);
        assert_eq!(line.comment.as_deref(), Some("just a comment"));
    }

    // --- Edge: leading zeros ---

    #[test]
    fn leading_zeros() {
        let line = tokenize_line("G005.250");
        assert_eq!(
            line.words[0],
            GcodeWord {
                letter: 'G',
                value: 5.25
            }
        );
    }

    // --- Edge: multiple paren comments — last one wins ---

    #[test]
    fn multiple_paren_comments() {
        let line = tokenize_line("(first) G01 (second) X5");
        assert_eq!(line.comment.as_deref(), Some("second"));
        assert_eq!(line.words.len(), 2);
    }

    // --- Edge: N-word prefix on a real line is not blank ---

    #[test]
    fn n_word_prefix_not_blank() {
        let line = tokenize_line("N10 G01 X5");
        assert!(!line.is_blank);
        assert_eq!(line.words.len(), 3);
        assert_eq!(line.words[0].letter, 'N');
    }

    // --- Edge: positive sign prefix ---

    #[test]
    fn positive_sign_prefix() {
        let line = tokenize_line("X+5.25");
        assert_eq!(
            line.words[0],
            GcodeWord {
                letter: 'X',
                value: 5.25
            }
        );
    }

    // --- Edge: paren comment inside code, semicolon at end ---

    #[test]
    fn paren_then_semicolon() {
        let line = tokenize_line("(info) G01 X5 ;eol");
        // Semicolon comment overwrites paren comment.
        assert_eq!(line.comment.as_deref(), Some("eol"));
        assert_eq!(line.words.len(), 2);
    }
}
