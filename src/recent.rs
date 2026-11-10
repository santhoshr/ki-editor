use std::{fs, path::PathBuf, time::SystemTime};
use serde::{Deserialize, Serialize};
use shared::absolute_path::AbsolutePath;

const MAX_RECENT_FILES: usize = 200;
const MAX_RECENT_FOLDERS: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecentStoreData {
    #[serde(default)]
    pub files: Vec<AbsolutePath>,
    #[serde(default)]
    pub folders: Vec<AbsolutePath>,
}

#[derive(Debug, Clone)]
pub struct RecentStore {
    file_path: PathBuf,
    last_modified: Option<SystemTime>,
    data: RecentStoreData,
}

impl Default for RecentStore {
    fn default() -> Self {
        Self::load_or_default()
    }
}

impl RecentStore {
    pub fn load_or_default() -> Self {
        let dir = grammar::config_dir();
        let file_path = dir.join("recent.json");
        let last_modified = fs::metadata(&file_path).ok().and_then(|m| m.modified().ok());
        let data = if file_path.exists() {
            fs::read_to_string(&file_path)
                .ok()
                .and_then(|content| serde_json::from_str::<RecentStoreData>(&content).ok())
                .unwrap_or_default()
        } else {
            RecentStoreData::default()
        };
        Self {
            file_path,
            last_modified,
            data,
        }
    }

    pub fn reload_if_changed(&mut self) -> bool {
        if let Ok(metadata) = fs::metadata(&self.file_path) {
            if let Ok(modified) = metadata.modified() {
                if self.last_modified != Some(modified) {
                    if let Ok(content) = fs::read_to_string(&self.file_path) {
                        if let Ok(new_data) = serde_json::from_str::<RecentStoreData>(&content) {
                            self.data = new_data;
                            self.last_modified = Some(modified);
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        if let Some(parent) = self.file_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let content = serde_json::to_string_pretty(&self.data)?;
        fs::write(&self.file_path, content)?;
        if let Ok(metadata) = fs::metadata(&self.file_path) {
            self.last_modified = metadata.modified().ok();
        }
        Ok(())
    }

    pub fn files(&self) -> Vec<AbsolutePath> {
        self.data.files.clone()
    }

    pub fn folders(&self) -> Vec<AbsolutePath> {
        self.data.folders.clone()
    }

    pub fn add_file(&mut self, path: AbsolutePath) {
        if path.to_path_buf().is_dir() {
            self.add_folder(path);
            return;
        }

        self.data.files.retain(|p| p != &path);
        self.data.files.insert(0, path);
        if self.data.files.len() > MAX_RECENT_FILES {
            self.data.files.truncate(MAX_RECENT_FILES);
        }

        let _ = self.save();
    }

    pub fn add_folder(&mut self, path: AbsolutePath) {
        if !path.to_path_buf().is_dir() {
            return;
        }
        self.add_folder_internal(path);
        let _ = self.save();
    }

    fn add_folder_internal(&mut self, path: AbsolutePath) {
        self.data.folders.retain(|p| p != &path);
        self.data.folders.insert(0, path);
        if self.data.folders.len() > MAX_RECENT_FOLDERS {
            self.data.folders.truncate(MAX_RECENT_FOLDERS);
        }
    }
}
