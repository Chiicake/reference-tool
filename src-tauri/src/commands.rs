use std::fs;
use std::path::Path;
use std::sync::RwLock;

use tauri::State;

use crate::bib_parser;
use crate::models::{AppSnapshot, CiteResult, ImportResult};
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

#[tauri::command]
pub fn get_cited_references_text(state: State<'_, SharedAppState>) -> Result<String, String> {
    let app_state = state
        .read()
        .map_err(|_| "Failed to read app state: lock poisoned".to_string())?;

    Ok(app_state.cited_references_text())
}

#[tauri::command]
pub fn import_bib_file(
    path: String,
    state: State<'_, SharedAppState>,
) -> Result<ImportResult, String> {
    ensure_bib_extension(&path)?;

    let content = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read bib file '{}': {error}", path))?;

    let parsed_entries = bib_parser::parse_bib_entries(&content)
        .map_err(|error| format!("Failed to parse bib file '{}': {error}", path))?;

    ensure_non_empty_entries(parsed_entries.len(), &path)?;

    let mut app_state = state
        .write()
        .map_err(|_| "Failed to write app state: lock poisoned".to_string())?;

    app_state.import_entries(parsed_entries)
}

#[tauri::command]
pub fn cite_keys(input: String, state: State<'_, SharedAppState>) -> Result<CiteResult, String> {
    let mut app_state = state
        .write()
        .map_err(|_| "Failed to write app state: lock poisoned".to_string())?;

    app_state.cite_keys(&input)
}

#[tauri::command]
pub fn clear_library(state: State<'_, SharedAppState>) -> Result<AppSnapshot, String> {
    let mut app_state = state
        .write()
        .map_err(|_| "Failed to write app state: lock poisoned".to_string())?;

    app_state.clear_library()?;
    Ok(app_state.snapshot())
}

#[tauri::command]
pub fn clear_citations(state: State<'_, SharedAppState>) -> Result<AppSnapshot, String> {
    let mut app_state = state
        .write()
        .map_err(|_| "Failed to write app state: lock poisoned".to_string())?;

    app_state.clear_citations()?;
    Ok(app_state.snapshot())
}

#[tauri::command]
pub fn set_next_citation_index(
    next_index: Option<usize>,
    state: State<'_, SharedAppState>,
) -> Result<AppSnapshot, String> {
    let mut app_state = state
        .write()
        .map_err(|_| "Failed to write app state: lock poisoned".to_string())?;

    app_state.set_next_citation_index(next_index)?;
    Ok(app_state.snapshot())
}

fn ensure_bib_extension(path: &str) -> Result<(), String> {
    let extension_ok = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("bib"))
        .unwrap_or(false);

    if extension_ok {
        return Ok(());
    }

    Err("Only .bib files are supported for import".to_string())
}

fn ensure_non_empty_entries(entry_count: usize, path: &str) -> Result<(), String> {
    if entry_count > 0 {
        return Ok(());
    }

    Err(format!("No valid BibTeX entries found in file '{path}'"))
}

#[cfg(test)]
mod tests {
    use super::{ensure_bib_extension, ensure_non_empty_entries};

    #[test]
    fn accepts_bib_extension_case_insensitive() {
        assert!(ensure_bib_extension("/tmp/ref.bib").is_ok());
        assert!(ensure_bib_extension("/tmp/ref.BIB").is_ok());
    }

    #[test]
    fn rejects_non_bib_extension() {
        assert!(ensure_bib_extension("/tmp/ref.txt").is_err());
        assert!(ensure_bib_extension("/tmp/ref").is_err());
    }

    #[test]
    fn rejects_empty_parsed_entry_set() {
        let result = ensure_non_empty_entries(0, "/tmp/empty.bib");
        assert!(result
            .expect_err("empty entry set should fail")
            .contains("No valid BibTeX entries found"));
    }

    #[test]
    fn accepts_non_empty_parsed_entry_set() {
        assert!(ensure_non_empty_entries(1, "/tmp/data.bib").is_ok());
    }
}
