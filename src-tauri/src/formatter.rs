use crate::models::LibraryEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    DefaultV1,
}

pub trait ReferenceFormatter {
    fn format_entry(&self, entry: &LibraryEntry) -> String;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultFormatterV1;

impl ReferenceFormatter for DefaultFormatterV1 {
    fn format_entry(&self, entry: &LibraryEntry) -> String {
        format_default_entry(entry)
    }
}

pub fn format_entry(entry: &LibraryEntry, output_format: OutputFormat) -> String {
    match output_format {
        OutputFormat::DefaultV1 => DefaultFormatterV1.format_entry(entry),
    }
}

fn format_default_entry(entry: &LibraryEntry) -> String {
    let authors = format_authors(entry);
    let title = first_non_empty(entry, &["title", "booktitle"])
        .map(clean_text)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| entry.key.clone());
    let marker = doc_marker(entry);

    let mut output = String::new();

    if !authors.is_empty() {
        output.push_str(&authors);
        output.push_str(". ");
    }

    output.push_str(&title);
    output.push('[');
    output.push_str(marker);
    output.push_str("].");

    let publication = publication_segment(entry);
    if !publication.is_empty() {
        output.push(' ');
        output.push_str(&publication);
        if !output.ends_with('.') {
            output.push('.');
        }
    }

    if let Some(doi) = first_non_empty(entry, &["doi"]).map(clean_text) {
        if !doi.is_empty() {
            output.push_str(" DOI: ");
            output.push_str(&doi);
            if !output.ends_with('.') {
                output.push('.');
            }
        }
    }

    if let Some(url) = first_non_empty(entry, &["url"]).map(clean_text) {
        if !url.is_empty() {
            output.push(' ');
            output.push_str(&url);
            if !output.ends_with('.') {
                output.push('.');
            }
        }
    }

    output
}

fn publication_segment(entry: &LibraryEntry) -> String {
    let venue = first_non_empty(
        entry,
        &["journal", "booktitle", "publisher", "institution", "school"],
    )
    .map(clean_text)
    .unwrap_or_default();
    let year = first_non_empty(entry, &["year"])
        .map(clean_text)
        .unwrap_or_default();
    let volume = first_non_empty(entry, &["volume"])
        .map(clean_text)
        .unwrap_or_default();
    let number = first_non_empty(entry, &["number", "issue"])
        .map(clean_text)
        .unwrap_or_default();
    let pages = first_non_empty(entry, &["pages"])
        .map(clean_text)
        .unwrap_or_default();

    let mut segment = String::new();

    if !venue.is_empty() {
        segment.push_str(&venue);
    }

    if !year.is_empty() {
        if !segment.is_empty() {
            segment.push_str(", ");
        }
        segment.push_str(&year);
    }

    let volume_issue = match (volume.is_empty(), number.is_empty()) {
        (false, false) => format!("{}({})", volume, number),
        (false, true) => volume,
        (true, false) => format!("({})", number),
        (true, true) => String::new(),
    };

    if !volume_issue.is_empty() {
        if !segment.is_empty() {
            segment.push_str(", ");
        }
        segment.push_str(&volume_issue);
    }

    if !pages.is_empty() {
        if !segment.is_empty() {
            segment.push_str(": ");
        }
        segment.push_str(&pages);
    }

    segment
}

fn doc_marker(entry: &LibraryEntry) -> &'static str {
    match entry.entry_type.to_ascii_uppercase().as_str() {
        "ARTICLE" => "J",
        "INPROCEEDINGS" | "CONFERENCE" | "PROCEEDINGS" => "C",
        "BOOK" => "M",
        "STANDARD" => "S",
        "PATENT" => "P",
        "THESIS" | "PHDTHESIS" | "MASTERSTHESIS" => "D",
        "REPORT" | "TECHREPORT" => "R",
        _ => {
            if first_non_empty(entry, &["url"]).is_some() {
                "EB/OL"
            } else {
                "Z"
            }
        }
    }
}

fn format_authors(entry: &LibraryEntry) -> String {
    let Some(raw_authors) = first_non_empty(entry, &["author", "editor"]) else {
        return String::new();
    };

    let normalized = clean_text(raw_authors);
    if normalized.is_empty() {
        return String::new();
    }

    normalized
        .split(" and ")
        .map(str::trim)
        .filter(|author| !author.is_empty())
        .map(format_single_author)
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_single_author(author: &str) -> String {
    if looks_cjk(author) {
        return author.to_string();
    }

    if let Some((family, given)) = author.split_once(',') {
        let family = clean_text(family);
        let initials = initials_from_segment(given);
        if initials.is_empty() {
            return family;
        }

        return format!("{} {}", family, initials);
    }

    let parts = author
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    if parts.is_empty() {
        return String::new();
    }

    if parts.len() == 1 {
        return clean_text(parts[0]);
    }

    let family = clean_text(parts[parts.len() - 1]);
    let given = parts[..parts.len() - 1].join(" ");
    let initials = initials_from_segment(&given);
    if initials.is_empty() {
        family
    } else {
        format!("{} {}", family, initials)
    }
}

fn initials_from_segment(segment: &str) -> String {
    segment
        .split(|ch: char| ch.is_whitespace() || ch == '-' || ch == '.')
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.chars().find(|ch| ch.is_alphanumeric()))
        .map(|ch| ch.to_ascii_uppercase().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn looks_cjk(value: &str) -> bool {
    value.chars().any(is_cjk_char)
}

fn is_cjk_char(ch: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&ch)
        || ('\u{3400}'..='\u{4DBF}').contains(&ch)
        || ('\u{F900}'..='\u{FAFF}').contains(&ch)
}

fn first_non_empty<'a>(entry: &'a LibraryEntry, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        entry
            .fields
            .get(*key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

fn clean_text(input: &str) -> String {
    input
        .replace(['{', '}'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::models::LibraryEntry;

    use super::{format_entry, DefaultFormatterV1, OutputFormat, ReferenceFormatter};

    fn build_entry(entry_type: &str, fields: &[(&str, &str)]) -> LibraryEntry {
        let mut field_map = BTreeMap::new();
        for (key, value) in fields {
            field_map.insert((*key).to_string(), (*value).to_string());
        }

        LibraryEntry {
            key: "test-key".to_string(),
            entry_type: entry_type.to_string(),
            fields: field_map,
            raw: None,
        }
    }

    #[test]
    fn formats_article_in_default_style() {
        let entry = build_entry(
            "ARTICLE",
            &[
                (
                    "author",
                    "Liu, Xin and Yu, Yingfeng and Li, Feng and Durrani, Tariq S.",
                ),
                (
                    "title",
                    "{Throughput Maximization for RIS-UAV Relaying Communications}",
                ),
                (
                    "journal",
                    "IEEE Transactions on Intelligent Transportation Systems",
                ),
                ("year", "2022"),
                ("volume", "23"),
                ("number", "10"),
                ("pages", "19569-19574"),
                ("doi", "10.1109/TITS.2022.3161698"),
            ],
        );

        let formatted = format_entry(&entry, OutputFormat::DefaultV1);
        assert_eq!(
            formatted,
            "Liu X, Yu Y, Li F, Durrani T S. Throughput Maximization for RIS-UAV Relaying Communications[J]. IEEE Transactions on Intelligent Transportation Systems, 2022, 23(10): 19569-19574. DOI: 10.1109/TITS.2022.3161698."
        );
    }

    #[test]
    fn falls_back_to_eb_ol_marker_for_url_entries() {
        let entry = build_entry(
            "MISC",
            &[
                ("title", "城市轨道交通运营数据速报"),
                ("year", "2024"),
                ("url", "http://example.test/report"),
            ],
        );

        let formatted = DefaultFormatterV1.format_entry(&entry);
        assert!(formatted.contains("城市轨道交通运营数据速报[EB/OL]."));
        assert!(formatted.contains("2024."));
        assert!(formatted.contains("http://example.test/report."));
    }

    #[test]
    fn keeps_chinese_authors_without_initial_reordering() {
        let entry = build_entry(
            "STANDARD",
            &[
                ("author", "交通运输部"),
                ("title", "城市轨道交通运营数据速报"),
                ("year", "2024"),
            ],
        );

        let formatted = format_entry(&entry, OutputFormat::DefaultV1);
        assert!(formatted.starts_with("交通运输部. 城市轨道交通运营数据速报[S]."));
    }

    #[test]
    fn falls_back_to_key_when_title_is_missing() {
        let entry = LibraryEntry {
            key: "9750059".to_string(),
            entry_type: "ARTICLE".to_string(),
            fields: BTreeMap::new(),
            raw: None,
        };

        let formatted = format_entry(&entry, OutputFormat::DefaultV1);
        assert!(formatted.starts_with("9750059[J]."));
    }
}
