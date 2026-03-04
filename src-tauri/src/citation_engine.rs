pub fn parse_citation_keys(raw_input: &str) -> Vec<String> {
    let normalized_input = normalize_latex_cite_input(raw_input);

    normalized_input
        .split(|ch: char| ch == ',' || ch == '，' || ch.is_whitespace())
        .map(|value| value.trim_matches(|ch: char| ch == '{' || ch == '}').trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn normalize_latex_cite_input(raw_input: &str) -> String {
    let mut normalized = String::with_capacity(raw_input.len());
    let mut cursor = 0;

    while let Some(relative_index) = raw_input[cursor..].find("\\cite") {
        let command_start = cursor + relative_index;
        normalized.push_str(&raw_input[cursor..command_start]);

        let mut index = command_start + "\\cite".len();

        while let Some(ch) = char_at(raw_input, index) {
            if ch.is_ascii_alphabetic() || ch == '*' {
                index += ch.len_utf8();
                continue;
            }

            break;
        }

        index = skip_whitespace(raw_input, index);

        while char_at(raw_input, index) == Some('[') {
            let Some(closing) = find_matching_delimiter(raw_input, index, b'[', b']') else {
                break;
            };

            index = skip_whitespace(raw_input, closing + 1);
        }

        if char_at(raw_input, index) == Some('{') {
            if let Some(closing) = find_matching_delimiter(raw_input, index, b'{', b'}') {
                if !ends_with_separator(&normalized) {
                    normalized.push(' ');
                }

                normalized.push_str(raw_input[index + 1..closing].trim());
                normalized.push(' ');
                cursor = closing + 1;
                continue;
            }
        }

        normalized.push_str("\\cite");
        cursor = command_start + "\\cite".len();
    }

    normalized.push_str(&raw_input[cursor..]);
    normalized
}

fn find_matching_delimiter(input: &str, start: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = input.as_bytes();
    if bytes.get(start).copied() != Some(open) {
        return None;
    }

    let mut depth = 0usize;
    let mut index = start;

    while index < bytes.len() {
        let value = bytes[index];
        if value == open {
            depth += 1;
        } else if value == close {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }

        index += 1;
    }

    None
}

fn ends_with_separator(text: &str) -> bool {
    text.chars()
        .last()
        .map(|ch| ch.is_whitespace() || ch == ',' || ch == '，')
        .unwrap_or(true)
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

fn char_at(input: &str, index: usize) -> Option<char> {
    input.get(index..)?.chars().next()
}

pub fn compress_citation_indexes(indexes: &[usize]) -> String {
    if indexes.is_empty() {
        return String::new();
    }

    let mut sorted = indexes.to_vec();
    sorted.sort_unstable();
    sorted.dedup();

    let mut ranges = Vec::new();
    let mut start = sorted[0];
    let mut previous = sorted[0];

    for current in sorted.iter().copied().skip(1) {
        if current == previous + 1 {
            previous = current;
            continue;
        }

        ranges.push(format_range(start, previous));
        start = current;
        previous = current;
    }

    ranges.push(format_range(start, previous));
    ranges.join(", ")
}

fn format_range(start: usize, end: usize) -> String {
    if start == end {
        return format!("[{start}]");
    }

    if end == start + 1 {
        return format!("[{start}],[{end}]");
    }

    format!("[{start}]-[{end}]")
}

#[cfg(test)]
mod tests {
    use super::{compress_citation_indexes, parse_citation_keys};

    #[test]
    fn parses_keys_with_commas_whitespace_and_newlines() {
        let parsed = parse_citation_keys("10495806,10648348\n10980318 10807485，9750059");
        assert_eq!(
            parsed,
            vec!["10495806", "10648348", "10980318", "10807485", "9750059"]
        );
    }

    #[test]
    fn parsing_drops_empty_tokens() {
        let parsed = parse_citation_keys(" ,\n，\t ");
        assert!(parsed.is_empty());
    }

    #[test]
    fn parses_latex_cite_wrapper() {
        let parsed = parse_citation_keys("\\cite{9837375,6giotacs}");
        assert_eq!(parsed, vec!["9837375", "6giotacs"]);
    }

    #[test]
    fn parses_latex_cite_with_optional_text() {
        let parsed = parse_citation_keys("prefix \\citep[see]{a1,b2} suffix");
        assert_eq!(parsed, vec!["prefix", "a1", "b2", "suffix"]);
    }

    #[test]
    fn compresses_empty_input_to_empty_string() {
        assert_eq!(compress_citation_indexes(&[]), "");
    }

    #[test]
    fn compresses_single_range() {
        assert_eq!(compress_citation_indexes(&[1, 2, 3]), "[1]-[3]");
    }

    #[test]
    fn compresses_multiple_ranges() {
        assert_eq!(compress_citation_indexes(&[1, 2, 3, 5]), "[1]-[3], [5]");
    }

    #[test]
    fn does_not_compress_two_consecutive_indexes() {
        assert_eq!(compress_citation_indexes(&[1, 2]), "[1],[2]");
    }

    #[test]
    fn sorts_and_deduplicates_before_compression() {
        assert_eq!(
            compress_citation_indexes(&[5, 3, 2, 2, 1, 8, 7, 6]),
            "[1]-[3], [5]-[8]"
        );
    }

    #[test]
    fn keeps_separated_indexes() {
        assert_eq!(compress_citation_indexes(&[1, 3, 6]), "[1], [3], [6]");
    }
}
