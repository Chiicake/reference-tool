use std::collections::{HashMap, HashSet};

use tauri::{AppHandle, Manager};

use crate::citation_engine::{
    compress_citation_indexes, extract_latex_cite_commands, parse_citation_keys, CiteCommand,
};
use crate::formatter::{format_entry, OutputFormat};
use crate::models::{
    AppSnapshot, CiteResult, EntryLookupResult, ImportResult, LibraryEntry, PersistedState,
};
use crate::storage::Storage;

const STATE_FILE_NAME: &str = "library_state.json";

#[derive(Debug)]
pub struct AppState {
    storage: Storage,
    persisted: PersistedState,
}

impl AppState {
    pub fn initialize(app_handle: &AppHandle) -> Result<Self, String> {
        let storage_path = app_handle
            .path()
            .app_data_dir()
            .map_err(|error| format!("Failed to resolve app data directory: {error}"))?
            .join(STATE_FILE_NAME);

        let storage = Storage::new(storage_path);
        Self::from_storage(storage)
    }

    pub fn from_storage(storage: Storage) -> Result<Self, String> {
        let mut persisted = storage
            .load_or_default()
            .map_err(|error| error.to_string())?;

        normalize_persisted_state(&mut persisted);

        if !storage.path().exists() {
            storage
                .save(&persisted)
                .map_err(|error| error.to_string())?;
        }

        Ok(Self { storage, persisted })
    }

    pub fn snapshot(&self) -> AppSnapshot {
        AppSnapshot::from_persisted(&self.persisted)
    }

    pub fn cited_references_text(&self) -> String {
        self.build_cited_references_text(OutputFormat::DefaultV1)
    }

    pub fn next_citation_index(&self) -> usize {
        self.persisted.next_citation_index
    }

    pub fn find_entry_by_key(&self, key: &str) -> Option<EntryLookupResult> {
        let normalized_key = key.trim();
        if normalized_key.is_empty() {
            return None;
        }

        let entry = self.persisted.entries.get(normalized_key)?;

        let title = entry
            .fields
            .get("title")
            .map(|value| normalize_inline_text(value))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| entry.key.clone());

        let authors = entry
            .fields
            .get("author")
            .or_else(|| entry.fields.get("editor"))
            .map(|value| normalize_authors(value))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "未知作者".to_string());

        Some(EntryLookupResult {
            key: entry.key.clone(),
            title,
            authors,
        })
    }

    pub fn import_entries(&mut self, entries: Vec<LibraryEntry>) -> Result<ImportResult, String> {
        let total = entries.len();
        let mut new_count = 0;
        let mut overwritten_count = 0;

        for entry in entries {
            let key = entry.key.clone();
            if self.persisted.entries.contains_key(&key) {
                overwritten_count += 1;
            } else {
                new_count += 1;
            }

            self.persisted.entries.insert(key, entry);
        }

        self.storage
            .save(&self.persisted)
            .map_err(|error| format!("Failed to persist imported entries: {error}"))?;

        let imported = new_count + overwritten_count;
        let failed = total.saturating_sub(imported);

        Ok(ImportResult {
            total,
            imported,
            new_count,
            overwritten_count,
            failed,
            message: format!(
                "Import finished: {imported} processed ({new_count} new, {overwritten_count} overwritten)."
            ),
        })
    }

    pub fn clear_library(&mut self) -> Result<(), String> {
        self.persisted.entries.clear();
        self.persisted.citation_order.clear();
        self.persisted.citation_index_by_key.clear();
        self.persisted.next_citation_index = 1;
        self.persisted.citation_start_index = 1;

        self.storage
            .save(&self.persisted)
            .map_err(|error| format!("Failed to clear library: {error}"))
    }

    pub fn clear_citations(&mut self) -> Result<(), String> {
        self.persisted.citation_order.clear();
        self.persisted.citation_index_by_key.clear();
        self.persisted.next_citation_index = 1;
        self.persisted.citation_start_index = 1;

        self.storage
            .save(&self.persisted)
            .map_err(|error| format!("Failed to clear citations: {error}"))
    }

    pub fn set_next_citation_index(&mut self, next_index: Option<usize>) -> Result<(), String> {
        self.ensure_citation_index_state();

        let max_assigned = self.max_assigned_index().unwrap_or(0);

        let resolved_next = match next_index {
            Some(value) => value,
            None => max_assigned.saturating_add(1).max(1),
        };

        if resolved_next == 0 {
            return Err("Next citation index must be >= 1".to_string());
        }

        if resolved_next <= max_assigned {
            return Err(format!(
                "Next citation index must be greater than current maximum index [{max_assigned}]"
            ));
        }

        self.persisted.next_citation_index = resolved_next;
        if self.persisted.citation_order.is_empty() {
            self.persisted.citation_start_index = resolved_next;
        }

        self.storage
            .save(&self.persisted)
            .map_err(|error| format!("Failed to update citation start index: {error}"))
    }

    pub fn cite_keys(&mut self, raw_input: &str) -> Result<CiteResult, String> {
        self.ensure_citation_index_state();

        let cite_commands = extract_latex_cite_commands(raw_input);
        if !cite_commands.is_empty() {
            return self.cite_paragraph(raw_input, &cite_commands);
        }

        let keys = parse_citation_keys(raw_input);
        if keys.is_empty() {
            return Err("Citation input is empty. Please provide at least one key.".to_string());
        }

        let missing_keys = self.collect_missing_keys(&keys);
        if !missing_keys.is_empty() {
            return Err(format!(
                "Missing citation key(s): {}",
                missing_keys.join(", ")
            ));
        }

        let (mut index_by_key, mut seen_in_order) = self.build_lookup_state();
        let (resolved_indexes, newly_added_count) =
            self.resolve_indexes_for_keys(&keys, &mut index_by_key, &mut seen_in_order);

        let citation_text = compress_citation_indexes(&resolved_indexes);
        self.finalize_citation(citation_text, newly_added_count)
    }

    fn cite_paragraph(
        &mut self,
        raw_input: &str,
        cite_commands: &[CiteCommand],
    ) -> Result<CiteResult, String> {
        if cite_commands.iter().any(|command| command.keys.is_empty()) {
            return Err(
                "Found empty \\cite{} command. Please provide at least one key.".to_string(),
            );
        }

        let requested_keys = cite_commands
            .iter()
            .flat_map(|command| command.keys.iter().cloned())
            .collect::<Vec<_>>();

        let missing_keys = self.collect_missing_keys(&requested_keys);
        if !missing_keys.is_empty() {
            return Err(format!(
                "Missing citation key(s): {}",
                missing_keys.join(", ")
            ));
        }

        let (mut index_by_key, mut seen_in_order) = self.build_lookup_state();
        let mut rendered = String::with_capacity(raw_input.len() + 32);
        let mut cursor = 0usize;
        let mut newly_added_count = 0usize;

        for command in cite_commands {
            rendered.push_str(&raw_input[cursor..command.start]);

            let (indexes, newly_added) =
                self.resolve_indexes_for_keys(&command.keys, &mut index_by_key, &mut seen_in_order);
            newly_added_count += newly_added;

            rendered.push_str(&compress_citation_indexes(&indexes));
            cursor = command.end;
        }

        rendered.push_str(&raw_input[cursor..]);

        self.finalize_citation(rendered, newly_added_count)
    }

    fn build_lookup_state(&self) -> (HashMap<String, usize>, HashSet<String>) {
        let index_by_key = self
            .persisted
            .citation_index_by_key
            .iter()
            .map(|(key, index)| (key.clone(), *index))
            .collect::<HashMap<_, _>>();
        let seen_in_order = self
            .persisted
            .citation_order
            .iter()
            .cloned()
            .collect::<HashSet<_>>();

        (index_by_key, seen_in_order)
    }

    fn resolve_indexes_for_keys(
        &mut self,
        keys: &[String],
        index_by_key: &mut HashMap<String, usize>,
        seen_in_order: &mut HashSet<String>,
    ) -> (Vec<usize>, usize) {
        let mut resolved_indexes = Vec::with_capacity(keys.len());
        let mut newly_added_count = 0usize;

        for key in keys {
            if let Some(index) = index_by_key.get(key).copied() {
                if seen_in_order.insert(key.clone()) {
                    self.persisted.citation_order.push(key.clone());
                }

                resolved_indexes.push(index);
                continue;
            }

            let assigned_index = self.reserve_next_index();
            self.persisted.citation_order.push(key.clone());
            seen_in_order.insert(key.clone());
            self.persisted
                .citation_index_by_key
                .insert(key.clone(), assigned_index);
            index_by_key.insert(key.clone(), assigned_index);
            resolved_indexes.push(assigned_index);
            newly_added_count += 1;
        }

        (resolved_indexes, newly_added_count)
    }

    fn finalize_citation(
        &mut self,
        citation_text: String,
        newly_added_count: usize,
    ) -> Result<CiteResult, String> {
        let cited_references_text = self.cited_references_text();

        self.storage
            .save(&self.persisted)
            .map_err(|error| format!("Failed to persist citation state: {error}"))?;

        Ok(CiteResult {
            citation_text,
            cited_references_text,
            newly_added_count,
        })
    }

    fn collect_missing_keys(&self, keys: &[String]) -> Vec<String> {
        let mut missing_keys = Vec::new();
        let mut seen_missing = HashSet::new();

        for key in keys {
            if self.persisted.entries.contains_key(key) {
                continue;
            }

            if !seen_missing.insert(key.clone()) {
                continue;
            }

            missing_keys.push(key.clone());
        }

        missing_keys
    }

    fn build_cited_references_text(&self, output_format: OutputFormat) -> String {
        let mut cited_keys = self
            .persisted
            .citation_order
            .iter()
            .filter_map(|key| {
                self.persisted
                    .citation_index_by_key
                    .get(key)
                    .copied()
                    .map(|index| (key.clone(), index))
            })
            .collect::<Vec<_>>();

        cited_keys.sort_by_key(|(_, index)| *index);
        cited_keys.dedup_by(|(left_key, _), (right_key, _)| left_key == right_key);

        cited_keys
            .iter()
            .map(|(key, index)| {
                let formatted = self
                    .persisted
                    .entries
                    .get(key)
                    .map(|entry| format_entry(entry, output_format))
                    .unwrap_or_else(|| format!("[Missing entry for key: {key}]"));

                format!("[{index}]  {formatted}")
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn max_assigned_index(&self) -> Option<usize> {
        self.persisted.citation_index_by_key.values().copied().max()
    }

    fn ensure_citation_index_state(&mut self) {
        normalize_persisted_state(&mut self.persisted);
    }

    fn reserve_next_index(&mut self) -> usize {
        let minimum_next = self.max_assigned_index().unwrap_or(0).saturating_add(1);
        if self.persisted.next_citation_index < minimum_next {
            self.persisted.next_citation_index = minimum_next;
        }

        let assigned = self.persisted.next_citation_index;
        self.persisted.next_citation_index = assigned.saturating_add(1);
        assigned
    }

    pub fn storage_path(&self) -> String {
        self.storage.path().display().to_string()
    }
}

fn normalize_persisted_state(persisted: &mut PersistedState) {
    if persisted.citation_start_index == 0 {
        persisted.citation_start_index = 1;
    }

    if persisted.next_citation_index == 0 {
        persisted.next_citation_index = 1;
    }

    dedup_citation_order(&mut persisted.citation_order);

    if persisted.citation_index_by_key.is_empty() && !persisted.citation_order.is_empty() {
        for (position, key) in persisted.citation_order.iter().enumerate() {
            persisted
                .citation_index_by_key
                .insert(key.clone(), persisted.citation_start_index + position);
        }
    }

    let order_keys = persisted
        .citation_order
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    persisted
        .citation_index_by_key
        .retain(|key, _| order_keys.contains(key));

    let mut next_assign = persisted
        .citation_index_by_key
        .values()
        .copied()
        .max()
        .unwrap_or(0)
        .saturating_add(1)
        .max(persisted.citation_start_index);

    for key in persisted.citation_order.iter() {
        if persisted.citation_index_by_key.contains_key(key) {
            continue;
        }

        persisted
            .citation_index_by_key
            .insert(key.clone(), next_assign);
        next_assign = next_assign.saturating_add(1);
    }

    let max_assigned = persisted
        .citation_index_by_key
        .values()
        .copied()
        .max()
        .unwrap_or(0);

    if max_assigned == 0 {
        if persisted.citation_start_index > 1 && persisted.next_citation_index == 1 {
            persisted.next_citation_index = persisted.citation_start_index;
        } else {
            persisted.next_citation_index = persisted.next_citation_index.max(1);
        }
        return;
    }

    if persisted.next_citation_index <= max_assigned {
        persisted.next_citation_index = max_assigned.saturating_add(1);
    }
}

fn dedup_citation_order(order: &mut Vec<String>) {
    let mut seen = HashSet::new();
    order.retain(|key| seen.insert(key.clone()));
}

fn normalize_inline_text(raw: &str) -> String {
    raw.replace(['{', '}'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_authors(raw: &str) -> String {
    normalize_inline_text(raw).replace(" and ", ", ")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::env;
    use std::path::PathBuf;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::models::{LibraryEntry, PersistedState};
    use crate::storage::Storage;

    use super::AppState;

    fn unique_state_path(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after UNIX_EPOCH")
            .as_nanos();

        env::temp_dir().join(format!(
            "reference-tool-state-{}-{}-{}.json",
            test_name,
            process::id(),
            nanos
        ))
    }

    fn build_entry(key: &str, title: &str) -> LibraryEntry {
        let mut fields = BTreeMap::new();
        fields.insert("title".to_string(), title.to_string());

        LibraryEntry {
            key: key.to_string(),
            entry_type: "ARTICLE".to_string(),
            fields,
            raw: None,
        }
    }

    #[test]
    fn import_entries_tracks_new_and_overwritten_counts() {
        let path = unique_state_path("import-count");
        let storage = Storage::new(path.clone());

        let mut persisted = PersistedState::default();
        persisted
            .entries
            .insert("k1".to_string(), build_entry("k1", "old"));

        let mut app_state = AppState { storage, persisted };

        let result = app_state
            .import_entries(vec![build_entry("k1", "new"), build_entry("k2", "another")])
            .expect("import should succeed");

        assert_eq!(result.total, 2);
        assert_eq!(result.imported, 2);
        assert_eq!(result.new_count, 1);
        assert_eq!(result.overwritten_count, 1);
        assert_eq!(result.failed, 0);

        let k1_title = app_state
            .persisted
            .entries
            .get("k1")
            .and_then(|entry| entry.fields.get("title"))
            .expect("k1 title should exist");
        assert_eq!(k1_title, "new");

        std::fs::remove_file(path).expect("cleanup state file");
    }

    #[test]
    fn cite_keys_reuses_existing_and_assigns_new_indexes() {
        let path = unique_state_path("cite-indexes");
        let storage = Storage::new(path.clone());

        let mut persisted = PersistedState::default();
        persisted
            .entries
            .insert("k1".to_string(), build_entry("k1", "Reference A"));
        persisted
            .entries
            .insert("k2".to_string(), build_entry("k2", "Reference B"));
        persisted
            .entries
            .insert("k3".to_string(), build_entry("k3", "Reference C"));
        persisted.citation_order = vec!["k1".to_string(), "k2".to_string()];

        let mut app_state = AppState { storage, persisted };

        let result = app_state
            .cite_keys("k2,k3,k1")
            .expect("cite should resolve indexes");

        assert_eq!(result.citation_text, "[1]-[3]");
        assert_eq!(result.newly_added_count, 1);
        assert_eq!(
            app_state.persisted.citation_order,
            vec!["k1".to_string(), "k2".to_string(), "k3".to_string()]
        );
        assert!(result
            .cited_references_text
            .contains("[1]  Reference A[J]."));
        assert!(result
            .cited_references_text
            .contains("[3]  Reference C[J]."));

        if path.exists() {
            std::fs::remove_file(path).expect("cleanup state file");
        }
    }

    #[test]
    fn cite_keys_replaces_latex_cite_commands_in_paragraph() {
        let path = unique_state_path("cite-paragraph");
        let storage = Storage::new(path.clone());

        let mut persisted = PersistedState::default();
        persisted
            .entries
            .insert("8016573".to_string(), build_entry("8016573", "Reference A"));
        persisted
            .entries
            .insert("9221208".to_string(), build_entry("9221208", "Reference B"));
        persisted
            .entries
            .insert("6425066".to_string(), build_entry("6425066", "Reference C"));

        let mut app_state = AppState { storage, persisted };

        let result = app_state
            .cite_keys("规模增长\\cite{8016573}，场景普及\\cite{9221208,6425066}。")
            .expect("paragraph cite should be replaced");

        assert_eq!(result.citation_text, "规模增长[1]，场景普及[2],[3]。");
        assert_eq!(result.newly_added_count, 3);

        if path.exists() {
            std::fs::remove_file(path).expect("cleanup state file");
        }
    }

    #[test]
    fn cite_keys_is_transactional_on_missing_key() {
        let path = unique_state_path("cite-missing");
        let storage = Storage::new(path.clone());

        let mut persisted = PersistedState::default();
        persisted
            .entries
            .insert("k1".to_string(), build_entry("k1", "Reference A"));
        persisted.citation_order = vec!["k1".to_string()];

        let mut app_state = AppState { storage, persisted };
        let before_order = app_state.persisted.citation_order.clone();

        let error = app_state
            .cite_keys("k1,missing-key")
            .expect_err("missing key should fail transaction");

        assert!(error.contains("Missing citation key(s): missing-key"));
        assert_eq!(app_state.persisted.citation_order, before_order);

        if path.exists() {
            std::fs::remove_file(path).expect("cleanup state file");
        }
    }

    #[test]
    fn cite_keys_rejects_empty_input() {
        let path = unique_state_path("cite-empty");
        let storage = Storage::new(path.clone());
        let mut app_state = AppState {
            storage,
            persisted: PersistedState::default(),
        };

        let error = app_state
            .cite_keys("  , \n，\t")
            .expect_err("empty input should fail");
        assert_eq!(
            error,
            "Citation input is empty. Please provide at least one key."
        );

        if path.exists() {
            std::fs::remove_file(path).expect("cleanup state file");
        }
    }

    #[test]
    fn cite_keys_keeps_non_consecutive_indexes_separated() {
        let path = unique_state_path("cite-range");
        let storage = Storage::new(path.clone());

        let mut persisted = PersistedState::default();
        persisted
            .entries
            .insert("k1".to_string(), build_entry("k1", "Reference A"));
        persisted
            .entries
            .insert("k2".to_string(), build_entry("k2", "Reference B"));
        persisted
            .entries
            .insert("k3".to_string(), build_entry("k3", "Reference C"));
        persisted.citation_order = vec!["k1".to_string(), "k2".to_string(), "k3".to_string()];

        let mut app_state = AppState { storage, persisted };

        let result = app_state
            .cite_keys("k1, k3")
            .expect("cite should return non-consecutive indexes");

        assert_eq!(result.citation_text, "[1], [3]");
        assert_eq!(result.newly_added_count, 0);

        if path.exists() {
            std::fs::remove_file(path).expect("cleanup state file");
        }
    }

    #[test]
    fn import_then_cite_persists_to_disk() {
        let path = unique_state_path("persist-workflow");
        let storage = Storage::new(path.clone());

        let mut app_state = AppState {
            storage: storage.clone(),
            persisted: PersistedState::default(),
        };

        app_state
            .import_entries(vec![
                build_entry("k1", "Reference A"),
                build_entry("k2", "Reference B"),
            ])
            .expect("import should succeed");

        let cite_result = app_state
            .cite_keys("k2,k1")
            .expect("cite should succeed after import");
        assert_eq!(cite_result.citation_text, "[1],[2]");

        let persisted = storage
            .load_or_default()
            .expect("stored state should be readable");

        assert_eq!(persisted.entries.len(), 2);
        assert_eq!(
            persisted.citation_order,
            vec!["k2".to_string(), "k1".to_string()]
        );

        if path.exists() {
            std::fs::remove_file(path).expect("cleanup state file");
        }
    }

    #[test]
    fn clear_citations_resets_numbering_to_one() {
        let path = unique_state_path("clear-citations");
        let storage = Storage::new(path.clone());

        let mut persisted = PersistedState::default();
        persisted.citation_start_index = 10;
        persisted
            .entries
            .insert("k1".to_string(), build_entry("k1", "Reference A"));
        persisted.citation_order = vec!["k1".to_string()];
        persisted.citation_index_by_key.insert("k1".to_string(), 10);
        persisted.next_citation_index = 11;

        let mut app_state = AppState { storage, persisted };
        app_state
            .clear_citations()
            .expect("clearing citations should succeed");

        let result = app_state
            .cite_keys("k1")
            .expect("citation after clear should use reset index");

        assert_eq!(result.citation_text, "[1]");
        assert_eq!(result.newly_added_count, 1);

        if path.exists() {
            std::fs::remove_file(path).expect("cleanup state file");
        }
    }

    #[test]
    fn set_next_citation_index_allows_non_empty_when_greater_than_max() {
        let path = unique_state_path("set-next-guard");
        let storage = Storage::new(path.clone());

        let mut persisted = PersistedState::default();
        persisted
            .entries
            .insert("k1".to_string(), build_entry("k1", "Reference A"));
        persisted
            .entries
            .insert("k2".to_string(), build_entry("k2", "Reference B"));
        persisted.citation_order = vec!["k1".to_string()];
        persisted.citation_index_by_key.insert("k1".to_string(), 10);
        persisted.next_citation_index = 11;

        let mut app_state = AppState { storage, persisted };

        app_state
            .set_next_citation_index(Some(16))
            .expect("setting next index should work with existing citations");

        let cite_result = app_state
            .cite_keys("k2")
            .expect("new citation should follow configured next index");
        assert_eq!(cite_result.citation_text, "[16]");

        if path.exists() {
            std::fs::remove_file(path).expect("cleanup state file");
        }
    }

    #[test]
    fn set_next_citation_index_rejects_value_not_greater_than_max() {
        let path = unique_state_path("set-next-reject");
        let storage = Storage::new(path.clone());

        let mut persisted = PersistedState::default();
        persisted
            .entries
            .insert("k1".to_string(), build_entry("k1", "Reference A"));
        persisted.citation_order = vec!["k1".to_string()];
        persisted.citation_index_by_key.insert("k1".to_string(), 10);
        persisted.next_citation_index = 11;

        let mut app_state = AppState { storage, persisted };

        let error = app_state
            .set_next_citation_index(Some(10))
            .expect_err("should reject when next index is not greater than current max");
        assert!(error.contains("must be greater than current maximum index"));

        if path.exists() {
            std::fs::remove_file(path).expect("cleanup state file");
        }
    }

    #[test]
    fn set_next_citation_index_empty_value_uses_max_plus_one() {
        let path = unique_state_path("set-next-auto");
        let storage = Storage::new(path.clone());

        let mut persisted = PersistedState::default();
        persisted
            .entries
            .insert("k1".to_string(), build_entry("k1", "Reference A"));
        persisted.citation_order = vec!["k1".to_string()];
        persisted.citation_index_by_key.insert("k1".to_string(), 10);
        persisted.next_citation_index = 30;

        let mut app_state = AppState { storage, persisted };
        app_state
            .set_next_citation_index(None)
            .expect("auto next should be applied");

        assert_eq!(app_state.next_citation_index(), 11);

        if path.exists() {
            std::fs::remove_file(path).expect("cleanup state file");
        }
    }

    #[test]
    fn set_next_citation_index_applies_to_first_new_citation() {
        let path = unique_state_path("set-next-apply");
        let storage = Storage::new(path.clone());

        let mut persisted = PersistedState::default();
        persisted
            .entries
            .insert("k1".to_string(), build_entry("k1", "Reference A"));
        let mut app_state = AppState { storage, persisted };

        app_state
            .set_next_citation_index(Some(25))
            .expect("set next citation index should succeed");

        let result = app_state
            .cite_keys("k1")
            .expect("citation should use configured next index");

        assert_eq!(result.citation_text, "[25]");

        if path.exists() {
            std::fs::remove_file(path).expect("cleanup state file");
        }
    }

    #[test]
    fn clear_library_removes_entries_and_citations() {
        let path = unique_state_path("clear-library");
        let storage = Storage::new(path.clone());

        let mut persisted = PersistedState::default();
        persisted
            .entries
            .insert("k1".to_string(), build_entry("k1", "Reference A"));
        persisted.citation_order = vec!["k1".to_string()];
        persisted.citation_index_by_key.insert("k1".to_string(), 3);
        persisted.next_citation_index = 4;

        let mut app_state = AppState { storage, persisted };
        app_state
            .clear_library()
            .expect("clear library should succeed");

        assert!(app_state.persisted.entries.is_empty());
        assert!(app_state.persisted.citation_order.is_empty());
        assert!(app_state.persisted.citation_index_by_key.is_empty());
        assert_eq!(app_state.persisted.next_citation_index, 1);

        if path.exists() {
            std::fs::remove_file(path).expect("cleanup state file");
        }
    }

    #[test]
    fn find_entry_by_key_returns_title_and_authors() {
        let path = unique_state_path("lookup-entry");
        let storage = Storage::new(path.clone());

        let mut persisted = PersistedState::default();
        let mut fields = BTreeMap::new();
        fields.insert(
            "title".to_string(),
            "{Throughput Maximization for RIS-UAV Relaying Communications}".to_string(),
        );
        fields.insert(
            "author".to_string(),
            "Liu, Xin and Yu, Yingfeng and Li, Feng".to_string(),
        );

        persisted.entries.insert(
            "9750059".to_string(),
            LibraryEntry {
                key: "9750059".to_string(),
                entry_type: "ARTICLE".to_string(),
                fields,
                raw: None,
            },
        );

        let app_state = AppState { storage, persisted };
        let result = app_state
            .find_entry_by_key("9750059")
            .expect("entry should be found");

        assert_eq!(result.key, "9750059");
        assert_eq!(
            result.title,
            "Throughput Maximization for RIS-UAV Relaying Communications"
        );
        assert_eq!(result.authors, "Liu, Xin, Yu, Yingfeng, Li, Feng");

        if path.exists() {
            std::fs::remove_file(path).expect("cleanup state file");
        }
    }

    #[test]
    fn find_entry_by_key_returns_none_for_missing_key() {
        let path = unique_state_path("lookup-missing");
        let storage = Storage::new(path.clone());

        let app_state = AppState {
            storage,
            persisted: PersistedState::default(),
        };

        assert!(app_state.find_entry_by_key("missing").is_none());

        if path.exists() {
            std::fs::remove_file(path).expect("cleanup state file");
        }
    }
}
