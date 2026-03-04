use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const STATE_VERSION: u32 = 1;

const fn default_state_version() -> u32 {
    STATE_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LibraryEntry {
    pub key: String,
    pub entry_type: String,
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedState {
    #[serde(default = "default_state_version")]
    pub version: u32,
    #[serde(default)]
    pub entries: BTreeMap<String, LibraryEntry>,
    #[serde(default)]
    pub citation_order: Vec<String>,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            entries: BTreeMap::new(),
            citation_order: Vec::new(),
        }
    }
}

impl PersistedState {
    pub fn imported_keys(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub total_entries: usize,
    pub imported_keys: Vec<String>,
    pub citation_order: Vec<String>,
}

impl AppSnapshot {
    pub fn from_persisted(state: &PersistedState) -> Self {
        Self {
            total_entries: state.entries.len(),
            imported_keys: state.imported_keys(),
            citation_order: state.citation_order.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub total: usize,
    pub imported: usize,
    pub new_count: usize,
    pub overwritten_count: usize,
    pub failed: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CiteResult {
    pub citation_text: String,
    pub cited_references_text: String,
    pub newly_added_count: usize,
}
