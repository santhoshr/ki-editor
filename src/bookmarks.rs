use std::{collections::BTreeMap, path::PathBuf};
use serde::{Deserialize, Serialize};
use shared::absolute_path::AbsolutePath;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BookmarkNode {
    File(AbsolutePath),
    Submenu(BTreeMap<char, BookmarkNode>),
}

impl BookmarkNode {
    pub fn is_submenu(&self) -> bool {
        matches!(self, BookmarkNode::Submenu(_))
    }

    pub fn as_submenu(&self) -> Option<&BTreeMap<char, BookmarkNode>> {
        match self {
            BookmarkNode::Submenu(map) => Some(map),
            _ => None,
        }
    }

    pub fn as_submenu_mut(&mut self) -> Option<&mut BTreeMap<char, BookmarkNode>> {
        match self {
            BookmarkNode::Submenu(map) => Some(map),
            _ => None,
        }
    }

    pub fn as_file(&self) -> Option<&AbsolutePath> {
        match self {
            BookmarkNode::File(path) => Some(path),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Bookmarks {
    file_path: PathBuf,
    last_modified: Option<std::time::SystemTime>,
    entries: BTreeMap<char, BookmarkNode>,
}

impl Bookmarks {
    pub fn load_or_default() -> Self {
        let dir = grammar::config_dir();
        let file_path = dir.join("bookmarks.json");
        let last_modified = std::fs::metadata(&file_path)
            .ok()
            .and_then(|m| m.modified().ok());
        let entries = if file_path.exists() {
            std::fs::read_to_string(&file_path)
                .ok()
                .and_then(|content| serde_json::from_str::<BTreeMap<char, BookmarkNode>>(&content).ok())
                .unwrap_or_default()
        } else {
            BTreeMap::new()
        };
        Self {
            file_path,
            last_modified,
            entries,
        }
    }

    pub fn file_path(&self) -> &std::path::Path {
        &self.file_path
    }

    pub fn reload_if_changed(&mut self) -> bool {
        let Ok(metadata) = std::fs::metadata(&self.file_path) else {
            return false;
        };
        let Ok(modified) = metadata.modified() else {
            return false;
        };
        if self.last_modified == Some(modified) {
            return false;
        }
        if let Ok(content) = std::fs::read_to_string(&self.file_path) {
            if let Ok(new_entries) = serde_json::from_str::<BTreeMap<char, BookmarkNode>>(&content) {
                self.entries = new_entries;
                self.last_modified = Some(modified);
                return true;
            }
        }
        false
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        if let Some(parent) = self.file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let content = serde_json::to_string_pretty(&self.entries)?;
        std::fs::write(&self.file_path, content)?;
        if let Ok(metadata) = std::fs::metadata(&self.file_path) {
            self.last_modified = metadata.modified().ok();
        }
        Ok(())
    }

    /// Retrieve entries at a given submenu path (empty path = root).
    pub fn entries_at(&self, path: &[char]) -> Option<&BTreeMap<char, BookmarkNode>> {
        let mut current = &self.entries;
        for &step in path {
            match current.get(&step)? {
                BookmarkNode::Submenu(submap) => current = submap,
                BookmarkNode::File(_) => return None,
            }
        }
        Some(current)
    }

    /// Retrieve mutable entries at a given submenu path (empty path = root).
    pub fn entries_at_mut(&mut self, path: &[char]) -> Option<&mut BTreeMap<char, BookmarkNode>> {
        let mut current = &mut self.entries;
        for &step in path {
            match current.get_mut(&step)? {
                BookmarkNode::Submenu(submap) => current = submap,
                BookmarkNode::File(_) => return None,
            }
        }
        Some(current)
    }

    pub fn get_at(&self, path: &[char], key: char) -> Option<&BookmarkNode> {
        self.entries_at(path)?.get(&key)
    }

    pub fn contains_at(&self, path: &[char], key: char) -> bool {
        self.entries_at(path).map(|m| m.contains_key(&key)).unwrap_or(false)
    }

    fn ensure_submenu_path_mut(&mut self, path: &[char]) -> &mut BTreeMap<char, BookmarkNode> {
        let mut current = &mut self.entries;
        for &step in path {
            if !current.contains_key(&step)
                || !current.get(&step).map(|n| n.is_submenu()).unwrap_or(false)
            {
                current.insert(step, BookmarkNode::Submenu(BTreeMap::new()));
            }
            match current.get_mut(&step).unwrap() {
                BookmarkNode::Submenu(submap) => current = submap,
                _ => unreachable!(),
            }
        }
        current
    }

    pub fn set_file(&mut self, path: &[char], key: char, file: AbsolutePath) -> anyhow::Result<()> {
        let map = self.ensure_submenu_path_mut(path);
        map.insert(key, BookmarkNode::File(file));
        self.save()
    }

    pub fn set_submenu(&mut self, path: &[char], key: char) -> anyhow::Result<()> {
        let map = self.ensure_submenu_path_mut(path);
        // Clears existing bindings for small/caps if assigned, creating a clean submenu
        map.insert(key, BookmarkNode::Submenu(BTreeMap::new()));
        self.save()
    }

    pub fn remove_at(&mut self, path: &[char], key: char) -> anyhow::Result<bool> {
        if let Some(map) = self.entries_at_mut(path) {
            let removed = map.remove(&key).is_some();
            if removed {
                self.save()?;
            }
            return Ok(removed);
        }
        Ok(false)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &BTreeMap<char, BookmarkNode> {
        &self.entries
    }

    /// Collects all file/directory paths directly listed in a given submenu.
    pub fn files_at(&self, path: &[char]) -> Vec<AbsolutePath> {
        let mut files = indexmap::IndexSet::new();
        if let Some(entries) = self.entries_at(path) {
            for node in entries.values() {
                if let BookmarkNode::File(p) = node {
                    files.insert(p.clone());
                }
            }
        }
        files.into_iter().collect()
    }

    /// Recursively collects all file paths saved across all bookmarks and submenus without any hard cap.
    pub fn all_files(&self) -> Vec<AbsolutePath> {
        let mut files = indexmap::IndexSet::new();
        fn collect_from_map(
            map: &BTreeMap<char, BookmarkNode>,
            files: &mut indexmap::IndexSet<AbsolutePath>,
        ) {
            for node in map.values() {
                match node {
                    BookmarkNode::File(path) => {
                        files.insert(path.clone());
                    }
                    BookmarkNode::Submenu(submap) => {
                        collect_from_map(submap, files);
                    }
                }
            }
        }
        collect_from_map(&self.entries, &mut files);
        files.into_iter().collect()
    }
}
