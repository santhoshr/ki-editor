use ropey::Rope;
use crate::{
    char_index_range::CharIndexRange,
    edit::{Action, ActionGroup, Edit},
    selection::{CharIndex, Selection},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberSpan {
    pub start: usize, // character offset in line
    pub end: usize,   // character offset in line (exclusive)
    pub raw: String,
    pub kind: NumberKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumberKind {
    Hex {
        prefix: String,
        uppercase: bool,
        min_width: usize,
        value: i64,
    },
    Bin {
        prefix: String,
        min_width: usize,
        value: i64,
    },
    Dec {
        min_width: usize,
        value: i64,
        had_minus: bool,
    },
}

/// Scans a slice of characters representing a single line for number tokens.
pub fn find_numbers_on_line(chars: &[char]) -> Vec<NumberSpan> {
    let n = chars.len();
    let mut spans = Vec::new();
    let mut i = 0;

    while i < n {
        let preceded_by_ident = i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');

        // Hexadecimal check: 0x... or 0X...
        if chars[i] == '0'
            && i + 1 < n
            && (chars[i + 1] == 'x' || chars[i + 1] == 'X')
            && !preceded_by_ident
            && i + 2 < n
            && chars[i + 2].is_ascii_hexdigit()
        {
            let prefix: String = chars[i..i + 2].iter().collect();
            let mut j = i + 2;
            while j < n && chars[j].is_ascii_hexdigit() {
                j += 1;
            }
            let raw: String = chars[i..j].iter().collect();
            let digits: String = chars[i + 2..j].iter().collect();
            if let Ok(value) = i64::from_str_radix(&digits, 16) {
                let uppercase = digits.chars().any(|c| c.is_ascii_uppercase());
                spans.push(NumberSpan {
                    start: i,
                    end: j,
                    raw,
                    kind: NumberKind::Hex {
                        prefix,
                        uppercase,
                        min_width: digits.len(),
                        value,
                    },
                });
                i = j;
                continue;
            }
        }

        // Binary check: 0b... or 0B...
        if chars[i] == '0'
            && i + 1 < n
            && (chars[i + 1] == 'b' || chars[i + 1] == 'B')
            && !preceded_by_ident
            && i + 2 < n
            && (chars[i + 2] == '0' || chars[i + 2] == '1')
        {
            let prefix: String = chars[i..i + 2].iter().collect();
            let mut j = i + 2;
            while j < n && (chars[j] == '0' || chars[j] == '1') {
                j += 1;
            }
            let raw: String = chars[i..j].iter().collect();
            let digits: String = chars[i + 2..j].iter().collect();
            if let Ok(value) = i64::from_str_radix(&digits, 2) {
                spans.push(NumberSpan {
                    start: i,
                    end: j,
                    raw,
                    kind: NumberKind::Bin {
                        prefix,
                        min_width: digits.len(),
                        value,
                    },
                });
                i = j;
                continue;
            }
        }

        // Negative decimal check: -[0-9]+
        if chars[i] == '-' && !preceded_by_ident && i + 1 < n && chars[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < n && chars[j].is_ascii_digit() {
                j += 1;
            }
            let raw: String = chars[i..j].iter().collect();
            let digits: String = chars[i + 1..j].iter().collect();
            if let Ok(abs_val) = digits.parse::<i64>() {
                let value = -abs_val;
                let has_leading_zero = digits.len() > 1 && digits.starts_with('0');
                let min_width = if has_leading_zero { digits.len() } else { 0 };
                spans.push(NumberSpan {
                    start: i,
                    end: j,
                    raw,
                    kind: NumberKind::Dec {
                        min_width,
                        value,
                        had_minus: true,
                    },
                });
                i = j;
                continue;
            }
        }

        // Positive decimal check: [0-9]+
        if chars[i].is_ascii_digit() {
            let start = i;
            let mut j = i;
            while j < n && chars[j].is_ascii_digit() {
                j += 1;
            }
            let raw: String = chars[start..j].iter().collect();
            if let Ok(value) = raw.parse::<i64>() {
                let has_leading_zero = raw.len() > 1 && raw.starts_with('0');
                let min_width = if has_leading_zero { raw.len() } else { 0 };
                spans.push(NumberSpan {
                    start,
                    end: j,
                    raw,
                    kind: NumberKind::Dec {
                        min_width,
                        value,
                        had_minus: false,
                    },
                });
                i = j;
                continue;
            }
        }

        i += 1;
    }

    spans
}

pub fn apply_delta(span: &NumberSpan, delta: i64) -> String {
    match &span.kind {
        NumberKind::Hex {
            prefix,
            uppercase,
            min_width,
            value,
        } => {
            let new_val = value.wrapping_add(delta) as u64;
            if *uppercase {
                format!("{prefix}{:0width$X}", new_val, width = *min_width)
            } else {
                format!("{prefix}{:0width$x}", new_val, width = *min_width)
            }
        }
        NumberKind::Bin {
            prefix,
            min_width,
            value,
        } => {
            let new_val = value.wrapping_add(delta) as u64;
            format!("{prefix}{:0width$b}", new_val, width = *min_width)
        }
        NumberKind::Dec {
            min_width,
            value,
            had_minus: _,
        } => {
            let new_val = value.saturating_add(delta);
            if *min_width > 1 {
                if new_val < 0 {
                    format!("-{:0width$}", new_val.unsigned_abs(), width = *min_width)
                } else {
                    format!("{:0width$}", new_val, width = *min_width)
                }
            } else {
                format!("{new_val}")
            }
        }
    }
}

pub fn change_numbers<'a>(
    rope: &Rope,
    selections: impl IntoIterator<Item = &'a Selection>,
    cursor_direction: &super::Direction,
    delta: i64,
) -> (bool, Vec<ActionGroup>) {
    let mut any_changed = false;
    let mut action_groups = Vec::new();

    for selection in selections {
        if rope.len_chars() == 0 {
            action_groups.push(ActionGroup::new(vec![Action::Select(selection.clone())]));
            continue;
        }

        let char_index = selection.to_char_index(cursor_direction).0;
        let char_index = char_index.min(rope.len_chars().saturating_sub(1));
        let line_index = rope.char_to_line(char_index);
        let line_start_char = rope.line_to_char(line_index);
        let line_chars: Vec<char> = rope.line(line_index).chars().collect();
        let cursor_col = char_index.saturating_sub(line_start_char);

        let spans = find_numbers_on_line(&line_chars);

        // Check if an extended selection overlaps with a number
        let sel_range = selection.extended_range();
        let chosen_span = if selection.initial_range.is_some() && sel_range.len() > 1 {
            let start_col = sel_range.start.0.saturating_sub(line_start_char);
            let end_col = sel_range.end.0.saturating_sub(line_start_char);
            spans
                .iter()
                .find(|s| (s.start <= cursor_col && cursor_col < s.end) || (s.start < end_col && s.end > start_col))
                .or_else(|| spans.iter().find(|s| s.start >= cursor_col))
        } else {
            spans
                .iter()
                .find(|s| s.start <= cursor_col && cursor_col < s.end)
                .or_else(|| spans.iter().find(|s| s.start >= cursor_col))
        };

        if let Some(span) = chosen_span {
            any_changed = true;
            let new_str = apply_delta(span, delta);
            let new_char_count = new_str.chars().count();
            let global_start = line_start_char + span.start;
            let global_end = line_start_char + span.end;
            let new_cursor_char = global_start + new_char_count - 1;

            let new_range = if selection.initial_range.is_some() {
                CharIndexRange::from(CharIndex(global_start)..CharIndex(global_start + new_char_count))
            } else {
                CharIndexRange::from(CharIndex(new_cursor_char)..CharIndex(new_cursor_char + 1))
            };

            let new_selection = selection
                .clone()
                .set_range(new_range)
                .set_initial_range(if selection.initial_range.is_some() {
                    Some(new_range)
                } else {
                    None
                });

            let edit_range = CharIndexRange::from(CharIndex(global_start)..CharIndex(global_end));

            action_groups.push(ActionGroup::new(vec![
                Action::Edit(Edit::new(
                    rope,
                    edit_range,
                    Rope::from_str(&new_str),
                )),
                Action::Select(new_selection),
            ]));
        } else {
            action_groups.push(ActionGroup::new(vec![Action::Select(selection.clone())]));
        }
    }

    (any_changed, action_groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_numbers_decimal() {
        let line: Vec<char> = "let x = 42;".chars().collect();
        let spans = find_numbers_on_line(&line);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].raw, "42");
        assert_eq!(spans[0].start, 8);
        assert_eq!(spans[0].end, 10);
        assert_eq!(apply_delta(&spans[0], 1), "43");
        assert_eq!(apply_delta(&spans[0], -1), "41");
    }

    #[test]
    fn test_find_numbers_negative() {
        let line: Vec<char> = "let x = -10;".chars().collect();
        let spans = find_numbers_on_line(&line);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].raw, "-10");
        assert_eq!(spans[0].start, 8);
        assert_eq!(spans[0].end, 11);
        assert_eq!(apply_delta(&spans[0], 1), "-9");
        assert_eq!(apply_delta(&spans[0], -1), "-11");
    }

    #[test]
    fn test_subtraction_not_negative() {
        let line: Vec<char> = "x-5".chars().collect();
        let spans = find_numbers_on_line(&line);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].raw, "5");
        assert_eq!(spans[0].start, 2);
    }

    #[test]
    fn test_zero_transition() {
        let line: Vec<char> = "0".chars().collect();
        let spans = find_numbers_on_line(&line);
        assert_eq!(spans.len(), 1);
        assert_eq!(apply_delta(&spans[0], 1), "1");
        assert_eq!(apply_delta(&spans[0], -1), "-1");

        let line_neg: Vec<char> = "-1".chars().collect();
        let spans_neg = find_numbers_on_line(&line_neg);
        assert_eq!(apply_delta(&spans_neg[0], 1), "0");
        assert_eq!(apply_delta(&spans_neg[0], -1), "-2");
    }

    #[test]
    fn test_leading_zeros() {
        let line: Vec<char> = "007".chars().collect();
        let spans = find_numbers_on_line(&line);
        assert_eq!(apply_delta(&spans[0], 1), "008");
        assert_eq!(apply_delta(&spans[0], -1), "006");

        let line9: Vec<char> = "009".chars().collect();
        let spans9 = find_numbers_on_line(&line9);
        assert_eq!(apply_delta(&spans9[0], 1), "010");
    }

    #[test]
    fn test_hex_and_bin() {
        let line_hex: Vec<char> = "0x0f".chars().collect();
        let spans_hex = find_numbers_on_line(&line_hex);
        assert_eq!(spans_hex.len(), 1);
        assert_eq!(apply_delta(&spans_hex[0], 1), "0x10");
        assert_eq!(apply_delta(&spans_hex[0], -1), "0x0e");

        let line_bin: Vec<char> = "0b101".chars().collect();
        let spans_bin = find_numbers_on_line(&line_bin);
        assert_eq!(spans_bin.len(), 1);
        assert_eq!(apply_delta(&spans_bin[0], 1), "0b110");
        assert_eq!(apply_delta(&spans_bin[0], -1), "0b100");
    }
}
