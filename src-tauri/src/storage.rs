use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::models::{PersistedState, STATE_VERSION};

#[derive(Debug)]
pub enum StorageError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Deserialize {
        path: PathBuf,
        source: serde_json::Error,
    },
    Serialize(serde_json::Error),
    UnsupportedVersion(u32),
    MissingParentDirectory(PathBuf),
}

impl Display for StorageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "I/O error at {}: {}", path.display(), source)
            }
            Self::Deserialize { path, source } => {
                write!(f, "Failed to parse {}: {}", path.display(), source)
            }
            Self::Serialize(source) => write!(f, "Failed to serialize state: {}", source),
            Self::UnsupportedVersion(version) => {
                write!(
                    f,
                    "Unsupported state file version {} (expected {})",
                    version, STATE_VERSION
                )
            }
            Self::MissingParentDirectory(path) => {
                write!(f, "State path has no parent directory: {}", path.display())
            }
        }
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Deserialize { source, .. } => Some(source),
            Self::Serialize(source) => Some(source),
            Self::UnsupportedVersion(_) | Self::MissingParentDirectory(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Storage {
    path: PathBuf,
}

impl Storage {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load_or_default(&self) -> Result<PersistedState, StorageError> {
        if !self.path.exists() {
            return Ok(PersistedState::default());
        }

        let content = fs::read_to_string(&self.path).map_err(|source| StorageError::Io {
            path: self.path.clone(),
            source,
        })?;

        if content.trim().is_empty() {
            return Ok(PersistedState::default());
        }

        let parsed: PersistedState =
            serde_json::from_str(&content).map_err(|source| StorageError::Deserialize {
                path: self.path.clone(),
                source,
            })?;

        if parsed.version != STATE_VERSION {
            return Err(StorageError::UnsupportedVersion(parsed.version));
        }

        Ok(parsed)
    }

    pub fn save(&self, state: &PersistedState) -> Result<(), StorageError> {
        let Some(parent) = self.path.parent() else {
            return Err(StorageError::MissingParentDirectory(self.path.clone()));
        };

        fs::create_dir_all(parent).map_err(|source| StorageError::Io {
            path: parent.to_path_buf(),
            source,
        })?;

        let mut normalized_state = state.clone();
        normalized_state.version = STATE_VERSION;

        let payload =
            serde_json::to_vec_pretty(&normalized_state).map_err(StorageError::Serialize)?;

        let temp_path = temporary_file_path(&self.path);
        fs::write(&temp_path, payload).map_err(|source| StorageError::Io {
            path: temp_path.clone(),
            source,
        })?;

        fs::rename(&temp_path, &self.path).map_err(|source| StorageError::Io {
            path: self.path.clone(),
            source,
        })?;

        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn temporary_file_path(target_path: &Path) -> PathBuf {
    if let Some(file_name) = target_path.file_name() {
        let mut temp_name = file_name.to_os_string();
        temp_name.push(".tmp");
        return target_path.with_file_name(temp_name);
    }

    target_path.with_extension("tmp")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::env;
    use std::path::PathBuf;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::models::{LibraryEntry, PersistedState};

    use super::{Storage, StorageError};

    fn unique_state_path(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after UNIX_EPOCH")
            .as_nanos();

        env::temp_dir().join(format!(
            "reference-tool-{}-{}-{}.json",
            test_name,
            process::id(),
            nanos
        ))
    }

    #[test]
    fn load_missing_state_returns_default() {
        let path = unique_state_path("missing");
        let storage = Storage::new(path);

        let loaded = storage
            .load_or_default()
            .expect("missing state should return default");

        assert_eq!(loaded, PersistedState::default());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let path = unique_state_path("roundtrip");
        let storage = Storage::new(path.clone());

        let mut fields = BTreeMap::new();
        fields.insert(
            "title".to_string(),
            "Throughput Maximization for RIS-UAV Relaying Communications".to_string(),
        );

        let mut state = PersistedState::default();
        state.entries.insert(
            "9750059".to_string(),
            LibraryEntry {
                key: "9750059".to_string(),
                entry_type: "ARTICLE".to_string(),
                fields,
                raw: None,
            },
        );
        state.citation_order.push("9750059".to_string());

        storage
            .save(&state)
            .expect("save should succeed for roundtrip test");

        let loaded = storage
            .load_or_default()
            .expect("load should succeed for roundtrip test");

        assert_eq!(loaded, state);

        std::fs::remove_file(path).expect("cleanup state file");
    }

    #[test]
    fn unsupported_version_returns_error() {
        let path = unique_state_path("version");
        let storage = Storage::new(path.clone());

        std::fs::write(&path, r#"{"version":999,"entries":{},"citation_order":[]}"#)
            .expect("write version fixture");

        let error = storage
            .load_or_default()
            .expect_err("unsupported version should fail");

        assert!(matches!(error, StorageError::UnsupportedVersion(999)));

        std::fs::remove_file(path).expect("cleanup state file");
    }

    #[test]
    fn invalid_json_returns_deserialize_error() {
        let path = unique_state_path("invalid-json");
        let storage = Storage::new(path.clone());

        std::fs::write(&path, "{invalid json").expect("write invalid json fixture");

        let error = storage
            .load_or_default()
            .expect_err("invalid json should fail");

        assert!(matches!(error, StorageError::Deserialize { .. }));

        std::fs::remove_file(path).expect("cleanup state file");
    }
}
