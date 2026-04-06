use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScaffoldIndex {
    pub entries: HashMap<String, String>, // arxiv_id → path
}

impl ScaffoldIndex {
    fn path() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".config")
            });
        base.join("tensor_term").join("scaffold_index.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    pub fn insert(&mut self, arxiv_id: String, path: String) {
        self.entries.insert(arxiv_id, path);
        self.save();
    }

    pub fn get(&self, arxiv_id: &str) -> Option<&str> {
        self.entries.get(arxiv_id).map(|s| s.as_str())
    }
}
