use std::sync::RwLock;

use tauri::State;

use crate::models::AppSnapshot;
use crate::state::AppState;

pub type SharedAppState = RwLock<AppState>;

#[tauri::command]
pub fn get_app_snapshot(state: State<'_, SharedAppState>) -> Result<AppSnapshot, String> {
    let app_state = state
        .read()
        .map_err(|_| "Failed to read app state: lock poisoned".to_string())?;

    Ok(app_state.snapshot())
}

#[tauri::command]
pub fn get_storage_path(state: State<'_, SharedAppState>) -> Result<String, String> {
    let app_state = state
        .read()
        .map_err(|_| "Failed to read app state: lock poisoned".to_string())?;

    Ok(app_state.storage_path())
}
