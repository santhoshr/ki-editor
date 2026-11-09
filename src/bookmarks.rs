use std::{collections::BTreeMap, path::PathBuf};
use shared::absolute_path::AbsolutePath;

#[derive(Debug, Clone, Default)]
pub struct Bookmarks {
    file_path: PathBuf,
    entries: BTreeMap<char, AbsolutePath>,
}

impl Bookmarks {
    pub fn load_or_default() -> Self {
        let dir = grammar::config_dir();
        let file_path = dir.join("bookmarks.json");
        let entries = if file_path.exists() {
            std::fs::read_to_string(&file_path)
                .ok()
                .and_then(|content| serde_json::from_str::<BTreeMap<char, String>>(&content).ok())
                .map(|map| {
                    map.into_iter()
                        .filter_map(|(k, v)| {
                            AbsolutePath::try_from(std::path::PathBuf::from(v))
                                .ok()
                                .map(|p| (k, p))
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            BTreeMap::new()
        };
        Self { file_path, entries }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let map: BTreeMap<String, String> = self
            .entries
            .iter()
            .map(|(k, v)| (k.to_string(), v.display_absolute()))
            .collect();
        let content = serde_json::to_string_pretty(&map)?;
        std::fs::write(&self.file_path, content)?;
        Ok(())
    }

    pub fn get(&self, key: char) -> Option<&AbsolutePath> {
        self.entries.get(&key)
    }

    pub fn contains(&self, key: char) -> bool {
        self.entries.contains_key(&key)
    }

    pub fn set(&mut self, key: char, path: AbsolutePath) -> anyhow::Result<()> {
        self.entries.insert(key, path);
        self.save()
    }

    pub fn remove(&mut self, key: char) -> anyhow::Result<bool> {
        let removed = self.entries.remove(&key).is_some();
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &BTreeMap<char, AbsolutePath> {
        &self.entries
    }
}
