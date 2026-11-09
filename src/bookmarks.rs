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

#[derive(Debug, Clone)]
pub struct Bookmarks {
    nested_file_path: PathBuf,
    quick_marks_file_path: PathBuf,
    nested_last_modified: Option<std::time::SystemTime>,
    quick_marks_last_modified: Option<std::time::SystemTime>,
    nested_entries: BTreeMap<char, BookmarkNode>,
    single_entries: BTreeMap<char, Vec<AbsolutePath>>,
}

impl Default for Bookmarks {
    fn default() -> Self {
        Self::load_or_default()
    }
}

impl Bookmarks {
    pub fn load_or_default() -> Self {
        let dir = grammar::config_dir();
        let nested_file_path = dir.join("bookmarks.json");
        let quick_marks_file_path = dir.join("quick_marks.json");

        let nested_last_modified = std::fs::metadata(&nested_file_path)
            .ok()
            .and_then(|m| m.modified().ok());
        let quick_marks_last_modified = std::fs::metadata(&quick_marks_file_path)
            .ok()
            .and_then(|m| m.modified().ok());

        let nested_entries = if nested_file_path.exists() {
            std::fs::read_to_string(&nested_file_path)
                .ok()
                .and_then(|content| serde_json::from_str::<BTreeMap<char, BookmarkNode>>(&content).ok())
                .unwrap_or_default()
        } else {
            BTreeMap::new()
        };

        let single_entries = if quick_marks_file_path.exists() {
            std::fs::read_to_string(&quick_marks_file_path)
                .ok()
                .and_then(|content| serde_json::from_str::<BTreeMap<char, Vec<AbsolutePath>>>(&content).ok())
                .unwrap_or_default()
        } else {
            // Also check legacy bookmarks_single.json if quick_marks.json does not exist yet
            let legacy_path = dir.join("bookmarks_single.json");
            if legacy_path.exists() {
                std::fs::read_to_string(&legacy_path)
                    .ok()
                    .and_then(|content| serde_json::from_str::<BTreeMap<char, Vec<AbsolutePath>>>(&content).ok())
                    .unwrap_or_default()
            } else {
                BTreeMap::new()
            }
        };

        Self {
            nested_file_path,
            quick_marks_file_path,
            nested_last_modified,
            quick_marks_last_modified,
            nested_entries,
            single_entries,
        }
    }

    pub fn file_path(&self) -> &std::path::Path {
        &self.nested_file_path
    }

    pub fn quick_marks_file_path(&self) -> &std::path::Path {
        &self.quick_marks_file_path
    }

    pub fn reload_if_changed(&mut self) -> bool {
        let mut changed = false;

        if let Ok(metadata) = std::fs::metadata(&self.nested_file_path) {
            if let Ok(modified) = metadata.modified() {
                if self.nested_last_modified != Some(modified) {
                    if let Ok(content) = std::fs::read_to_string(&self.nested_file_path) {
                        if let Ok(new_entries) = serde_json::from_str::<BTreeMap<char, BookmarkNode>>(&content) {
                            self.nested_entries = new_entries;
                            self.nested_last_modified = Some(modified);
                            changed = true;
                        }
                    }
                }
            }
        }

        if let Ok(metadata) = std::fs::metadata(&self.quick_marks_file_path) {
            if let Ok(modified) = metadata.modified() {
                if self.quick_marks_last_modified != Some(modified) {
                    if let Ok(content) = std::fs::read_to_string(&self.quick_marks_file_path) {
                        if let Ok(new_entries) = serde_json::from_str::<BTreeMap<char, Vec<AbsolutePath>>>(&content) {
                            self.single_entries = new_entries;
                            self.quick_marks_last_modified = Some(modified);
                            changed = true;
                        }
                    }
                }
            }
        }

        changed
    }

    pub fn save(&mut self) -> anyhow::Result<()> {
        if let Some(parent) = self.nested_file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let content = serde_json::to_string_pretty(&self.nested_entries)?;
        std::fs::write(&self.nested_file_path, content)?;
        if let Ok(metadata) = std::fs::metadata(&self.nested_file_path) {
            self.nested_last_modified = metadata.modified().ok();
        }
        Ok(())
    }

    pub fn save_quick_marks(&mut self) -> anyhow::Result<()> {
        if let Some(parent) = self.quick_marks_file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let content = serde_json::to_string_pretty(&self.single_entries)?;
        std::fs::write(&self.quick_marks_file_path, content)?;
        if let Ok(metadata) = std::fs::metadata(&self.quick_marks_file_path) {
            self.quick_marks_last_modified = metadata.modified().ok();
        }
        Ok(())
    }

    // --- Nested Bookmarks Methods ---

    pub fn entries_at(&self, path: &[char]) -> Option<&BTreeMap<char, BookmarkNode>> {
        let mut current = &self.nested_entries;
        for &step in path {
            match current.get(&step)? {
                BookmarkNode::Submenu(submap) => current = submap,
                BookmarkNode::File(_) => return None,
            }
        }
        Some(current)
    }

    pub fn entries_at_mut(&mut self, path: &[char]) -> Option<&mut BTreeMap<char, BookmarkNode>> {
        let mut current = &mut self.nested_entries;
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
        let mut current = &mut self.nested_entries;
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

    pub fn entries(&self) -> &BTreeMap<char, BookmarkNode> {
        &self.nested_entries
    }

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

    // --- Quick-marks (Single) Methods ---

    pub fn get_single(&self, key: char) -> Option<&Vec<AbsolutePath>> {
        self.single_entries.get(&key)
    }

    pub fn has_single(&self, key: char) -> bool {
        self.single_entries.contains_key(&key)
    }

    pub fn single_entries(&self) -> &BTreeMap<char, Vec<AbsolutePath>> {
        &self.single_entries
    }

    pub fn add_single(&mut self, key: char, file: AbsolutePath) -> anyhow::Result<()> {
        let list = self.single_entries.entry(key).or_default();
        if !list.contains(&file) {
            list.push(file);
        }
        self.save_quick_marks()
    }

    pub fn set_single(&mut self, key: char, files: Vec<AbsolutePath>) -> anyhow::Result<()> {
        if files.is_empty() {
            self.single_entries.remove(&key);
        } else {
            self.single_entries.insert(key, files);
        }
        self.save_quick_marks()
    }

    pub fn remove_single(&mut self, key: char) -> anyhow::Result<bool> {
        let removed = self.single_entries.remove(&key).is_some();
        if removed {
            self.save_quick_marks()?;
        }
        Ok(removed)
    }

    pub fn all_single_files(&self) -> Vec<AbsolutePath> {
        let mut files = indexmap::IndexSet::new();
        for list in self.single_entries.values() {
            for p in list {
                files.insert(p.clone());
            }
        }
        files.into_iter().collect()
    }

    // --- All Files ---

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
        collect_from_map(&self.nested_entries, &mut files);
        for list in self.single_entries.values() {
            for p in list {
                files.insert(p.clone());
            }
        }
        files.into_iter().collect()
    }
}
