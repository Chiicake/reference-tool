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

    format!("[{start}]-[{end}]")
}

#[cfg(test)]
mod tests {
    use super::compress_citation_indexes;

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
