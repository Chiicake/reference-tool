use tauri::{AppHandle, Manager};

use crate::models::{AppSnapshot, PersistedState};
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

    pub fn storage_path(&self) -> String {
        self.storage.path().display().to_string()
    }
}
