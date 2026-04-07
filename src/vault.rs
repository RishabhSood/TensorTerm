use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::app::PaperEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedPaper {
    pub title: String,
    pub authors: String,
    pub date: String,
    pub domain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Vault {
    pub collections: BTreeMap<String, Vec<String>>, // name → arxiv_ids
    pub paper_cache: HashMap<String, CachedPaper>,  // arxiv_id → metadata
}

const DEFAULT_COLLECTIONS: &[&str] = &["Reading List"];

impl Vault {
    fn path() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".config")
            });
        base.join("tensor_term").join("vault.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            // First run — seed with defaults
            let mut vault = Self::default();
            for name in DEFAULT_COLLECTIONS {
                vault.collections.entry(name.to_string()).or_insert_with(Vec::new);
            }
            vault
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

    pub fn add_paper(&mut self, collection: &str, arxiv_id: &str, paper: &PaperEntry) {
        let ids = self
            .collections
            .entry(collection.to_string())
            .or_insert_with(Vec::new);

        if !ids.contains(&arxiv_id.to_string()) {
            ids.push(arxiv_id.to_string());
        }

        self.paper_cache.entry(arxiv_id.to_string()).or_insert_with(|| {
            CachedPaper {
                title: paper.title.clone(),
                authors: paper.authors.clone(),
                date: paper.date.clone(),
                domain: paper.domain.clone(),
            }
        });

        self.save();
    }

    pub fn remove_paper(&mut self, collection: &str, arxiv_id: &str) {
        if let Some(ids) = self.collections.get_mut(collection) {
            ids.retain(|id| id != arxiv_id);
        }
        // Don't remove from paper_cache — paper may be in other collections
        self.save();
    }

    pub fn create_collection(&mut self, name: &str) {
        self.collections
            .entry(name.to_string())
            .or_insert_with(Vec::new);
        self.save();
    }

    pub fn delete_collection(&mut self, name: &str) {
        self.collections.remove(name);
        self.save();
    }

    pub fn collection_names(&self) -> Vec<&str> {
        self.collections.keys().map(|s| s.as_str()).collect()
    }

    pub fn papers_in(&self, collection: &str) -> Vec<&str> {
        self.collections
            .get(collection)
            .map(|ids| ids.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Return names of all collections that contain this arxiv_id.
    pub fn collections_containing(&self, arxiv_id: &str) -> Vec<&str> {
        self.collections
            .iter()
            .filter(|(_, ids)| ids.iter().any(|id| id == arxiv_id))
            .map(|(name, _)| name.as_str())
            .collect()
    }
}
