use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::models::LibraryEntry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BibParseError {
    message: String,
    position: Option<usize>,
}

impl BibParseError {
    fn new(message: impl Into<String>, position: Option<usize>) -> Self {
        Self {
            message: message.into(),
            position,
        }
    }
}

impl Display for BibParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(position) = self.position {
            return write!(f, "{} at byte {}", self.message, position);
        }

        write!(f, "{}", self.message)
    }
}

impl Error for BibParseError {}

pub fn parse_bib_entries(input: &str) -> Result<Vec<LibraryEntry>, BibParseError> {
    let mut entries = Vec::new();
    let mut cursor = 0;

    while let Some(relative_at) = input[cursor..].find('@') {
        let at_index = cursor + relative_at;
        let mut index = skip_whitespace(input, at_index + 1);

        let type_start = index;
        while let Some(ch) = char_at(input, index) {
            if !is_entry_type_char(ch) {
                break;
            }
            index += ch.len_utf8();
        }

        if type_start == index {
            return Err(BibParseError::new("Missing entry type", Some(index)));
        }

        let entry_type = input[type_start..index].trim();
        index = skip_whitespace(input, index);

        let open = char_at(input, index).ok_or_else(|| {
            BibParseError::new("Missing opening delimiter after entry type", Some(index))
        })?;

        let close = match open {
            '{' => '}',
            '(' => ')',
            _ => {
                return Err(BibParseError::new(
                    "Entry body must start with '{' or '('",
                    Some(index),
                ));
            }
        };

        let close_index = find_matching_delimiter(input, index, open, close)?;
        let body = &input[index + open.len_utf8()..close_index];

        if !is_special_entry_type(entry_type) {
            let entry = parse_standard_entry(entry_type, body, index + open.len_utf8())?;
            entries.push(entry);
        }

        cursor = close_index + close.len_utf8();
    }

    Ok(entries)
}

fn parse_standard_entry(
    entry_type: &str,
    body: &str,
    base_position: usize,
) -> Result<LibraryEntry, BibParseError> {
    let mut cursor = skip_whitespace(body, 0);
    let key_start = cursor;

    while let Some(ch) = char_at(body, cursor) {
        if ch == ',' {
            break;
        }

        cursor += ch.len_utf8();
    }

    let key = body[key_start..cursor].trim();
    if key.is_empty() {
        return Err(BibParseError::new(
            "Missing citation key in entry",
            Some(base_position + key_start),
        ));
    }

    if char_at(body, cursor) == Some(',') {
        cursor += 1;
    }

    let fields = parse_fields(body, cursor, base_position)?;

    Ok(LibraryEntry {
        key: key.to_string(),
        entry_type: entry_type.to_ascii_uppercase(),
        fields,
        raw: Some(body.trim().to_string()),
    })
}

fn parse_fields(
    body: &str,
    mut cursor: usize,
    base_position: usize,
) -> Result<BTreeMap<String, String>, BibParseError> {
    let mut fields = BTreeMap::new();

    while cursor < body.len() {
        cursor = skip_whitespace_and_commas(body, cursor);
        if cursor >= body.len() {
            break;
        }

        let field_name_start = cursor;
        while let Some(ch) = char_at(body, cursor) {
            if ch == '=' || ch.is_whitespace() {
                break;
            }
            if ch == ',' {
                return Err(BibParseError::new(
                    "Unexpected ',' while parsing field name",
                    Some(base_position + cursor),
                ));
            }

            cursor += ch.len_utf8();
        }

        let field_name = body[field_name_start..cursor].trim();
        if field_name.is_empty() {
            return Err(BibParseError::new(
                "Empty field name in entry",
                Some(base_position + field_name_start),
            ));
        }

        cursor = skip_whitespace(body, cursor);
        if char_at(body, cursor) != Some('=') {
            return Err(BibParseError::new(
                "Expected '=' after field name",
                Some(base_position + cursor),
            ));
        }
        cursor += 1;

        cursor = skip_whitespace(body, cursor);
        if cursor >= body.len() {
            return Err(BibParseError::new(
                "Missing field value after '='",
                Some(base_position + cursor),
            ));
        }

        let (value, next_cursor) = parse_field_value(body, cursor, base_position)?;
        fields.insert(field_name.to_ascii_lowercase(), value);
        cursor = next_cursor;

        cursor = skip_whitespace(body, cursor);
        if char_at(body, cursor) == Some(',') {
            cursor += 1;
        }
    }

    Ok(fields)
}

fn parse_field_value(
    body: &str,
    cursor: usize,
    base_position: usize,
) -> Result<(String, usize), BibParseError> {
    match char_at(body, cursor) {
        Some('{') => parse_braced_value(body, cursor, base_position),
        Some('"') => parse_quoted_value(body, cursor, base_position),
        Some(_) => parse_bare_value(body, cursor, base_position),
        None => Err(BibParseError::new(
            "Missing field value",
            Some(base_position + cursor),
        )),
    }
}

fn parse_braced_value(
    body: &str,
    start: usize,
    base_position: usize,
) -> Result<(String, usize), BibParseError> {
    let mut depth = 0;
    let mut cursor = start;

    while cursor < body.len() {
        let ch = char_at(body, cursor).ok_or_else(|| {
            BibParseError::new(
                "Failed to read braced field value",
                Some(base_position + cursor),
            )
        })?;

        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                let raw = &body[start + 1..cursor];
                let normalized = normalize_field_value(raw);
                return Ok((normalized, cursor + ch.len_utf8()));
            }
        }

        cursor += ch.len_utf8();
    }

    Err(BibParseError::new(
        "Unclosed braced field value",
        Some(base_position + start),
    ))
}

fn parse_quoted_value(
    body: &str,
    start: usize,
    base_position: usize,
) -> Result<(String, usize), BibParseError> {
    let mut cursor = start + 1;
    let mut escaped = false;

    while cursor < body.len() {
        let ch = char_at(body, cursor).ok_or_else(|| {
            BibParseError::new(
                "Failed to read quoted field value",
                Some(base_position + cursor),
            )
        })?;

        if ch == '"' && !escaped {
            let raw = &body[start + 1..cursor];
            let normalized = normalize_field_value(raw);
            return Ok((normalized, cursor + ch.len_utf8()));
        }

        escaped = ch == '\\' && !escaped;
        if ch != '\\' {
            escaped = false;
        }

        cursor += ch.len_utf8();
    }

    Err(BibParseError::new(
        "Unclosed quoted field value",
        Some(base_position + start),
    ))
}

fn parse_bare_value(
    body: &str,
    start: usize,
    base_position: usize,
) -> Result<(String, usize), BibParseError> {
    let mut cursor = start;

    while let Some(ch) = char_at(body, cursor) {
        if ch == ',' {
            break;
        }
        cursor += ch.len_utf8();
    }

    let raw = body[start..cursor].trim();
    if raw.is_empty() {
        return Err(BibParseError::new(
            "Bare field value is empty",
            Some(base_position + start),
        ));
    }

    Ok((normalize_field_value(raw), cursor))
}

fn normalize_field_value(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn find_matching_delimiter(
    input: &str,
    open_index: usize,
    open: char,
    close: char,
) -> Result<usize, BibParseError> {
    let mut depth = 0;
    let mut cursor = open_index;
    let mut in_quotes = false;
    let mut escaped = false;

    while cursor < input.len() {
        let ch = char_at(input, cursor).ok_or_else(|| {
            BibParseError::new("Failed while scanning entry delimiters", Some(cursor))
        })?;

        if in_quotes {
            if ch == '"' && !escaped {
                in_quotes = false;
            }

            escaped = ch == '\\' && !escaped;
            if ch != '\\' {
                escaped = false;
            }
        } else {
            if ch == '"' {
                in_quotes = true;
            } else if ch == open {
                depth += 1;
            } else if ch == close {
                depth -= 1;
                if depth == 0 {
                    return Ok(cursor);
                }
            }
        }

        cursor += ch.len_utf8();
    }

    Err(BibParseError::new(
        "Unclosed entry delimiter",
        Some(open_index),
    ))
}

fn skip_whitespace(input: &str, mut index: usize) -> usize {
    while let Some(ch) = char_at(input, index) {
        if !ch.is_whitespace() {
            break;
        }

        index += ch.len_utf8();
    }

    index
}

fn skip_whitespace_and_commas(input: &str, mut index: usize) -> usize {
    while let Some(ch) = char_at(input, index) {
        if ch != ',' && !ch.is_whitespace() {
            break;
        }

        index += ch.len_utf8();
    }

    index
}

fn char_at(input: &str, index: usize) -> Option<char> {
    input.get(index..)?.chars().next()
}

fn is_entry_type_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-')
}

fn is_special_entry_type(entry_type: &str) -> bool {
    entry_type.eq_ignore_ascii_case("comment")
        || entry_type.eq_ignore_ascii_case("preamble")
        || entry_type.eq_ignore_ascii_case("string")
}

#[cfg(test)]
mod tests {
    use super::parse_bib_entries;

    #[test]
    fn parses_single_article_entry() {
        let input = r#"@ARTICLE{9750059,
  author={Liu, Xin and Yu, Yingfeng and Li, Feng and Durrani, Tariq S.},
  journal={IEEE Transactions on Intelligent Transportation Systems},
  title={{Throughput Maximization for RIS-UAV Relaying Communications}},
  year={2022},
  volume={23},
  number={10},
  pages={19569-19574},
  doi={10.1109/TITS.2022.3161698}
}"#;

        let entries = parse_bib_entries(input).expect("single entry should parse");
        assert_eq!(entries.len(), 1);

        let entry = &entries[0];
        assert_eq!(entry.key, "9750059");
        assert_eq!(entry.entry_type, "ARTICLE");
        assert_eq!(entry.fields.get("year").map(String::as_str), Some("2022"));
        assert_eq!(
            entry.fields.get("doi").map(String::as_str),
            Some("10.1109/TITS.2022.3161698")
        );
    }

    #[test]
    fn parses_multiple_entries_and_skips_special_types() {
        let input = r#"
@comment{ignored entry}
@ARTICLE{a1, title={A Title}, year={2024}}
@STRING{abbr = "IEEE"}
@INPROCEEDINGS{b2, title={B Title}, year={2023}}
"#;

        let entries = parse_bib_entries(input).expect("entries should parse");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].key, "a1");
        assert_eq!(entries[1].key, "b2");
        assert_eq!(entries[1].entry_type, "INPROCEEDINGS");
    }

    #[test]
    fn handles_nested_braces_and_commas_in_values() {
        let input = r#"@ARTICLE{n1,
  title={A {Very, Very} Specific Title},
  note={Line one,
  line two}
}"#;

        let entries = parse_bib_entries(input).expect("entry should parse");
        let entry = &entries[0];

        assert_eq!(
            entry.fields.get("title").map(String::as_str),
            Some("A {Very, Very} Specific Title")
        );
        assert_eq!(
            entry.fields.get("note").map(String::as_str),
            Some("Line one, line two")
        );
    }

    #[test]
    fn returns_error_for_unclosed_entry() {
        let input = "@ARTICLE{missing_end, title={oops}";

        let error = parse_bib_entries(input).expect_err("parser should return an error");
        assert!(error.to_string().contains("Unclosed entry delimiter"));
    }
}
