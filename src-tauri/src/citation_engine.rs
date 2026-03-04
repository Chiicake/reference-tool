#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiteCommand {
    pub start: usize,
    pub end: usize,
    pub keys: Vec<String>,
}

pub fn parse_citation_keys(raw_input: &str) -> Vec<String> {
    raw_input
        .split(|ch: char| ch == ',' || ch == '，' || ch.is_whitespace())
        .map(|value| value.trim_matches(|ch: char| ch == '{' || ch == '}').trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn extract_latex_cite_commands(input: &str) -> Vec<CiteCommand> {
    let bytes = input.as_bytes();
    let mut commands = Vec::new();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        if bytes[cursor] != b'\\' || !input[cursor..].starts_with("\\cite") {
            cursor += 1;
            continue;
        }

        let command_start = cursor;
        let mut index = command_start + "\\cite".len();

        while let Some(ch) = char_at(input, index) {
            if ch.is_ascii_alphabetic() || ch == '*' {
                index += ch.len_utf8();
                continue;
            }

            break;
        }

        index = skip_whitespace(input, index);

        while char_at(input, index) == Some('[') {
            let Some(closing) = find_matching_delimiter(input, index, '[', ']') else {
                break;
            };
            index = skip_whitespace(input, closing + 1);
        }

        if char_at(input, index) != Some('{') {
            cursor = command_start + 1;
            continue;
        }

        let Some(closing) = find_matching_delimiter(input, index, '{', '}') else {
            cursor = command_start + 1;
            continue;
        };

        let keys = parse_citation_keys(&input[index + 1..closing]);
        commands.push(CiteCommand {
            start: command_start,
            end: closing + 1,
            keys,
        });

        cursor = closing + 1;
    }

    commands
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

fn find_matching_delimiter(input: &str, start: usize, open: char, close: char) -> Option<usize> {
    if char_at(input, start) != Some(open) {
        return None;
    }

    let mut depth = 0usize;
    let mut index = start;

    while index < input.len() {
        let ch = char_at(input, index)?;
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }

        index += ch.len_utf8();
    }

    None
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

#[cfg(test)]
mod tests {
    use super::{compress_citation_indexes, extract_latex_cite_commands, parse_citation_keys};

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
    fn extracts_latex_cite_commands_from_paragraph() {
        let commands =
            extract_latex_cite_commands("text \\cite{8016573} more \\cite{9221208,6425066} end");

        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].keys, vec!["8016573"]);
        assert_eq!(commands[1].keys, vec!["9221208", "6425066"]);
    }

    #[test]
    fn extracts_cite_with_optional_arguments() {
        let commands = extract_latex_cite_commands("\\citep[see][p.2]{a1,b2}");
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].keys, vec!["a1", "b2"]);
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
