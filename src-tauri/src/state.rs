use tauri::{AppHandle, Manager};

use crate::models::{AppSnapshot, ImportResult, LibraryEntry, PersistedState};
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
}
