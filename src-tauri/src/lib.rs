pub mod bib_parser;
pub mod citation_engine;
mod commands;
pub mod formatter;
pub mod models;
pub mod state;
pub mod storage;

use std::io;

use commands::SharedAppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_state = state::AppState::initialize(app.handle()).map_err(io::Error::other)?;
            app.manage(SharedAppState::new(app_state));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_snapshot,
            commands::get_storage_path,
            commands::get_cited_references_text,
            commands::import_bib_file,
            commands::cite_keys,
            commands::clear_library,
            commands::clear_citations,
            commands::set_next_citation_index
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
