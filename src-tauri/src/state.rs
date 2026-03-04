use std::collections::HashMap;

use tauri::{AppHandle, Manager};

use crate::citation_engine::{compress_citation_indexes, parse_citation_keys};
use crate::formatter::{format_entry, OutputFormat};
use crate::models::{AppSnapshot, CiteResult, ImportResult, LibraryEntry, PersistedState};
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
        let persisted = storage
            .load_or_default()
            .map_err(|error| error.to_string())?;

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

    pub fn cite_keys(&mut self, raw_input: &str) -> Result<CiteResult, String> {
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

        let mut index_by_key = self
            .persisted
            .citation_order
            .iter()
            .enumerate()
            .map(|(position, key)| (key.clone(), position + 1))
            .collect::<HashMap<_, _>>();

        let mut resolved_indexes = Vec::with_capacity(keys.len());
        let mut newly_added_count = 0;

        for key in keys {
            if let Some(index) = index_by_key.get(&key).copied() {
                resolved_indexes.push(index);
                continue;
            }

            self.persisted.citation_order.push(key.clone());
            let assigned_index = self.persisted.citation_order.len();
            index_by_key.insert(key, assigned_index);
            resolved_indexes.push(assigned_index);
            newly_added_count += 1;
        }

        let citation_text = compress_citation_indexes(&resolved_indexes);
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

        for key in keys {
            if self.persisted.entries.contains_key(key) {
                continue;
            }

            if missing_keys.iter().any(|existing| existing == key) {
                continue;
            }

            missing_keys.push(key.clone());
        }

        missing_keys
    }

    fn build_cited_references_text(&self, output_format: OutputFormat) -> String {
        self.persisted
            .citation_order
            .iter()
            .enumerate()
            .map(|(index, key)| {
                let formatted = self
                    .persisted
                    .entries
                    .get(key)
                    .map(|entry| format_entry(entry, output_format))
                    .unwrap_or_else(|| format!("[Missing entry for key: {key}]"));

                format!("[{}]  {}", index + 1, formatted)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn storage_path(&self) -> String {
        self.storage.path().display().to_string()
    }
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
}
