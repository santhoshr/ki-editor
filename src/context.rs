use std::collections::{HashMap, HashSet};

use globset::Glob;

use indexmap::IndexSet;
use itertools::{Either, Itertools};
use shared::absolute_path::AbsolutePath;

use crate::{
    app::{GlobalSearchConfigUpdate, LocalSearchConfigUpdate, Scope},
    bookmarks::{BookmarkNode, Bookmarks},
    char_index_range::CharIndexRange,
    clipboard::{Clipboard, RingHistory, Texts},
    components::{editor_keymap::KeyboardLayout, prompt::PromptHistoryKey},
    list::grep::RegexConfig,
    persistence::{Persistence, WorkspaceSession},
    quickfix_list::{DiagnosticSeverityRange, Location, QuickfixList, QuickfixListItem},
    recent::RecentStore,
    selection::SelectionMode,
    themes::Theme,
};

pub struct Context {
    clipboard: Clipboard,
    mode: Option<GlobalMode>,
    theme: Theme,

    #[cfg(test)]
    highlight_configs: crate::syntax_highlight::HighlightConfigs,
    current_working_directory: AbsolutePath,
    local_search_config: LocalSearchConfig,
    global_search_config: GlobalSearchConfig,
    quickfix_list: QuickfixList,
    prompt_histories: HashMap<PromptHistoryKey, IndexSet<String>>,
    last_non_contiguous_selection_mode: Option<Either<SelectionMode, GlobalMode>>,
    keyboard_layout: KeyboardLayout,
    keyboard_layouts: HashMap<String, KeyboardLayout>,
    location_history_backward: Vec<Location>,
    location_history_forward: Vec<Location>,

    marked_files: IndexSet<AbsolutePath>,
    untitled_files: HashSet<AbsolutePath>,
    alternate_file: Option<AbsolutePath>,

    marks: HashMap<AbsolutePath, Vec<CharIndexRange>>,
    bookmarks: Bookmarks,
    recent: RecentStore,

    /// This is true, for example, when Ki is running as a VS Code's extension
    is_running_as_embedded: bool,

    persistence: Option<Persistence>,

    lsp_progress: String,
    kill_ring: RingHistory<Texts>,

    file_dirty_status: HashMap<AbsolutePath, bool>,
    indent_char: char,
    indent_width: usize,
    format_on_save: bool,
    command_menu_invocation: crate::config::CommandMenuInvocation,
    quick_marks_menu_invocation: crate::config::QuickMarksMenuInvocation,
    bookmark_menu_invocation: crate::config::BookmarkMenuInvocation,
    swap_key: crate::config::SwapKey,
    pick_menu_df_swap: bool,
    e_normal_mode: crate::config::ENormalMode,
    filetree_open_with_path: bool,
    navigation_keys: crate::config::NavigationKeys,
    hjkl_config: crate::config::HjklConfig,

    /// This is used to prevent the same non-existent formatter error from being reported more than once.
    non_existent_formatter_commands: Vec<FormatterCommand>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FormatterCommand(pub String);

#[derive(Debug)]
pub enum QuickfixListSource {
    Diagnostic(DiagnosticSeverityRange),
    Mark,
    Custom(Vec<QuickfixListItem>),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GlobalMode {
    QuickfixListItem,
}
impl GlobalMode {
    pub fn display(&self) -> String {
        match self {
            GlobalMode::QuickfixListItem => "QFIX".to_string(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Search {
    pub mode: LocalSearchConfigMode,
    pub search: String,
}

impl Context {
    #[cfg(test)]
    pub fn default() -> Self {
        Self::new(AbsolutePath::try_from(".").unwrap(), false, None)
    }

    pub fn persist_data(&mut self) {
        if let Some(persistence) = self.persistence.as_mut() {
            let current_working_directory = self.current_working_directory.to_path_buf();
            let untitled_files = &self.untitled_files;
            persistence.set_workspace_session(
                current_working_directory,
                WorkspaceSession {
                    marked_files: self
                        .marked_files
                        .iter()
                        .filter(|path| !untitled_files.contains(*path))
                        .map(|path| path.to_path_buf().clone())
                        .collect_vec(),
                    marks: self
                        .marks
                        .iter()
                        .map(|(path, marks)| (path.to_path_buf().clone(), marks.clone()))
                        .collect(),
                    prompt_histories: self.prompt_histories.clone(),
                },
            );

            if let Err(error) = persistence.write() {
                log::error!("Failed to write persistence due to {error:?}");
            }
        }
    }

    pub fn quickfix_list_items(&self) -> &[QuickfixListItem] {
        self.quickfix_list.items()
    }

    pub fn handle_applied_edits(&mut self, path: AbsolutePath, edits: Vec<crate::edit::Edit>) {
        self.quickfix_list
            .handle_applied_edits(&path, &edits, &self.current_working_directory);

        self.marks = std::mem::take(&mut self.marks)
            .into_iter()
            .map(|(p, marks)| {
                if p == path {
                    (
                        p,
                        marks
                            .into_iter()
                            .filter_map(|mark| {
                                edits
                                    .iter()
                                    .try_fold(mark, |mark, edit| mark.apply_edit(edit))
                            })
                            .collect(),
                    )
                } else {
                    (p, marks)
                }
            })
            .collect();
    }

    pub fn save_marks(&mut self, path: AbsolutePath, marks: Vec<CharIndexRange>) {
        let old_ranges = self
            .marks
            .get(&path)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect::<HashSet<_>>();

        let new_ranges = marks.into_iter().collect::<HashSet<_>>();

        // We take the symmetric difference between the old ranges and the new ranges
        // so that user can unmark existing mark
        self.marks.insert(
            path,
            new_ranges
                .symmetric_difference(&old_ranges)
                .cloned()
                .collect_vec(),
        );
    }

    pub fn get_marks(&self, path: Option<AbsolutePath>) -> Vec<CharIndexRange> {
        path.map(|path| self.marks.get(&path).cloned().unwrap_or_default().to_vec())
            .unwrap_or_default()
    }

    pub fn marks(&self) -> &HashMap<AbsolutePath, Vec<CharIndexRange>> {
        &self.marks
    }

    pub fn bookmarks(&self) -> &Bookmarks {
        &self.bookmarks
    }

    pub fn bookmarks_mut(&mut self) -> &mut Bookmarks {
        &mut self.bookmarks
    }

    pub fn recent(&self) -> &RecentStore {
        &self.recent
    }

    pub fn recent_mut(&mut self) -> &mut RecentStore {
        &mut self.recent
    }

    pub fn reload_bookmarks_if_changed(&mut self) -> bool {
        self.bookmarks.reload_if_changed()
    }

    pub fn get_bookmark_at(&self, path: &[char], key: char) -> Option<&BookmarkNode> {
        self.bookmarks.get_at(path, key)
    }

    pub fn has_bookmark_at(&self, path: &[char], key: char) -> bool {
        self.bookmarks.contains_at(path, key)
    }

    pub fn set_bookmark_file(
        &mut self,
        path: &[char],
        key: char,
        file: AbsolutePath,
    ) -> anyhow::Result<()> {
        self.bookmarks.set_file(path, key, file)
    }

    pub fn set_bookmark_submenu(&mut self, path: &[char], key: char) -> anyhow::Result<()> {
        self.bookmarks.set_submenu(path, key)
    }

    pub fn remove_bookmark_at(&mut self, path: &[char], key: char) -> anyhow::Result<bool> {
        self.bookmarks.remove_at(path, key)
    }

    pub fn get_bookmark(&self, key: char) -> Option<&BookmarkNode> {
        self.bookmarks.get_at(&[], key)
    }

    pub fn has_bookmark(&self, key: char) -> bool {
        self.bookmarks.contains_at(&[], key)
    }

    pub fn set_bookmark(&mut self, key: char, path: AbsolutePath) -> anyhow::Result<()> {
        self.bookmarks.set_file(&[], key, path)
    }

    pub fn remove_bookmark(&mut self, key: char) -> anyhow::Result<bool> {
        self.bookmarks.remove_at(&[], key)
    }

    pub fn has_single_bookmark(&self, key: char) -> bool {
        self.bookmarks.has_single(key)
    }

    pub fn get_single_bookmarks(&self, key: char) -> Option<&Vec<AbsolutePath>> {
        self.bookmarks.get_single(key)
    }

    pub fn add_single_bookmark(&mut self, key: char, file: AbsolutePath) -> anyhow::Result<()> {
        self.bookmarks.add_single(key, file)
    }

    pub fn set_single_bookmarks(&mut self, key: char, files: Vec<AbsolutePath>) -> anyhow::Result<()> {
        self.bookmarks.set_single(key, files)
    }

    pub fn remove_single_bookmark(&mut self, key: char) -> anyhow::Result<bool> {
        self.bookmarks.remove_single(key)
    }

    pub fn all_single_bookmarks(&self) -> Vec<AbsolutePath> {
        self.bookmarks.all_single_files()
    }

    pub fn has_quick_mark(&self, key: char) -> bool {
        self.has_single_bookmark(key)
    }

    pub fn get_quick_marks(&self, key: char) -> Option<&Vec<AbsolutePath>> {
        self.get_single_bookmarks(key)
    }

    pub fn add_quick_mark(&mut self, key: char, file: AbsolutePath) -> anyhow::Result<()> {
        self.add_single_bookmark(key, file)
    }

    pub fn set_quick_marks(&mut self, key: char, files: Vec<AbsolutePath>) -> anyhow::Result<()> {
        self.set_single_bookmarks(key, files)
    }

    pub fn remove_quick_mark(&mut self, key: char) -> anyhow::Result<bool> {
        self.remove_single_bookmark(key)
    }

    pub fn handle_file_renamed(&mut self, source: std::path::PathBuf, destination: AbsolutePath) {
        if let Some(path) = self
            .marked_files
            .iter()
            .find(|path| path.to_path_buf() == &source)
        {
            self.marked_files.shift_remove(&path.clone());
            self.marked_files.insert(destination);
        }
    }

    pub fn mark_files(&mut self, paths: nonempty::NonEmpty<AbsolutePath>) {
        for path in paths {
            self.mark_file(path);
        }
    }

    pub fn change_working_directory(&mut self, path: AbsolutePath) -> Result<(), std::io::Error> {
        self.recent.add_folder(path.clone());
        self.current_working_directory = path;
        std::env::set_current_dir(self.current_working_directory.clone())?;
        Ok(())
    }

    pub(crate) fn update_lsp_progress(&mut self, lsp_progress: String) {
        self.lsp_progress = lsp_progress;
    }

    pub(crate) fn lsp_progress(&self) -> String {
        self.lsp_progress.clone()
    }

    pub(crate) fn quickfix_list(&self) -> &QuickfixList {
        &self.quickfix_list
    }

    pub fn quickfix_list_mut(&mut self) -> &mut QuickfixList {
        &mut self.quickfix_list
    }

    pub(crate) fn get_quickfix_list_item(
        &mut self,
        movement: crate::components::editor::Movement,
    ) -> Option<(usize, crate::app::Dispatches)> {
        self.quickfix_list.get_item(movement)
    }

    pub fn set_quickfix_list_items(
        &mut self,
        items: Vec<QuickfixListItem>,
        kind: Option<QuickfixListKind>,
    ) {
        self.quickfix_list
            .set_items(items, &self.current_working_directory);
        self.quickfix_list.set_kind(kind);
    }

    pub fn set_quickfix_list_title(&mut self, title: &str) {
        self.quickfix_list.set_title(title);
    }

    pub(crate) fn quickfix_list_items_count(&self) -> usize {
        self.quickfix_list.items_count()
    }

    pub(crate) fn extend_quickfix_list_items(&mut self, items: Vec<QuickfixListItem>) {
        self.quickfix_list
            .extend_items(items, &self.current_working_directory);
    }

    pub(crate) fn indent_width(&self) -> usize {
        self.indent_width
    }

    pub(crate) fn indent_char(&self) -> char {
        self.indent_char
    }

    /// Returns `true` if the given `formatter_command` is not seen before.
    pub(crate) fn add_formatter_not_exist_error(
        &mut self,
        formatter_command: &FormatterCommand,
    ) -> bool {
        if self
            .non_existent_formatter_commands
            .contains(formatter_command)
        {
            false
        } else {
            self.non_existent_formatter_commands
                .push(formatter_command.clone());
            true
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickfixListKind {
    Mark,
}

impl Context {
    pub fn new(
        current_working_directory: AbsolutePath,
        is_running_as_embedded: bool,
        persistence: Option<Persistence>,
    ) -> Self {
        let marked_files: IndexSet<AbsolutePath> = persistence
            .as_ref()
            .and_then(|persistence| {
                Some(
                    persistence
                        .get_marked_files(current_working_directory.to_path_buf())?
                        .into_iter()
                        .filter_map(|path| AbsolutePath::try_from(path).ok())
                        .filter(|path| path.is_file())
                        .collect(),
                )
            })
            .unwrap_or_default();

        let marks = persistence
            .as_ref()
            .and_then(|persistence| {
                Some(
                    persistence
                        .get_marks(current_working_directory.to_path_buf())?
                        .into_iter()
                        .filter_map(|(path, marks)| Some((path.try_into().ok()?, marks)))
                        .collect(),
                )
            })
            .unwrap_or_default();

        let prompt_histories = persistence
            .as_ref()
            .and_then(|persistence| {
                persistence.get_prompt_histories(current_working_directory.to_path_buf())
            })
            .unwrap_or_default();
        let mut recent = RecentStore::load_or_default();
        recent.add_folder(current_working_directory.clone());
        if recent.files().is_empty() {
            for file in &marked_files {
                recent.add_file(file.clone());
            }
        }
        let app_config = crate::config::AppConfig::singleton();
        Self {
            clipboard: Clipboard::new(),
            theme: app_config.theme().clone(),
            mode: None,
            #[cfg(test)]
            highlight_configs: crate::syntax_highlight::HighlightConfigs::new(),
            current_working_directory,
            local_search_config: LocalSearchConfig::default(),
            global_search_config: GlobalSearchConfig::default(),
            prompt_histories,
            last_non_contiguous_selection_mode: None,
            keyboard_layout: app_config.keyboard_layout().clone(),
            keyboard_layouts: app_config.keyboard_layouts().clone(),
            location_history_backward: Vec::new(),
            location_history_forward: Vec::new(),
            marked_files,
            untitled_files: HashSet::new(),
            alternate_file: None,
            is_running_as_embedded,
            persistence,
            marks,
            bookmarks: Bookmarks::load_or_default(),
            recent,
            lsp_progress: "".to_string(),
            quickfix_list: QuickfixList::default(),
            kill_ring: RingHistory::new(),
            file_dirty_status: HashMap::new(),
            indent_char: app_config.indent_char(),
            indent_width: app_config.indent_width(),
            format_on_save: app_config.format_on_save(),
            command_menu_invocation: app_config.command_menu_invocation(),
            quick_marks_menu_invocation: app_config.quick_marks_menu_invocation(),
            bookmark_menu_invocation: app_config.bookmark_menu_invocation(),
            swap_key: app_config.swap_key(),
            pick_menu_df_swap: app_config.pick_menu_df_swap(),
            e_normal_mode: app_config.e_normal_mode(),
            filetree_open_with_path: app_config.filetree_open_with_path(),
            navigation_keys: app_config.navigation_keys(),
            hjkl_config: app_config.hjkl_config(),
            non_existent_formatter_commands: Vec::new(),
        }
    }

    /// Checks if the contents in both the system clipboard and the app clipboard is the same
    pub fn clipboards_synced(&self) -> bool {
        let history_offset = 0;
        let Some(app_clipboard_content) = self.clipboard.get(history_offset) else {
            return false;
        };

        let Some(system_clipboard_content) = self.clipboard.get_from_system_clipboard().ok() else {
            return false;
        };

        app_clipboard_content == system_clipboard_content
    }

    pub(crate) fn add_kill_ring_entry(&mut self, texts: Texts) {
        self.kill_ring.add(texts);
    }

    pub fn get_kill_ring_content(&self, history_offset: isize) -> Option<Texts> {
        self.kill_ring.get(history_offset)
    }

    /// Note: `history_offset` is ignored when `use_system_clipboard` is true.
    ///
    /// This method should never fail, if `use_system_clipboard` is true but
    /// the system clipboard is inaccessible, the app clipboard will be used.
    pub fn get_clipboard_content(&self, history_offset: isize) -> Option<Texts> {
        // Always use the system clipboard if the content of the system clipboard is no longer the same
        // with the content of the app clipboard
        let use_system_clipboard = !self.clipboards_synced();

        if use_system_clipboard {
            match self.clipboard.get_from_system_clipboard() {
                Ok(copied_texts) => return Some(copied_texts),
                Err(err) => {
                    log::error!("Context::get_clipboard_content: cannot access system clipboard due to {err:?}");
                }
            }
        }

        self.clipboard.get(history_offset)
    }

    pub fn set_clipboard_content(&mut self, contents: Texts) {
        self.kill_ring.add(contents.clone());
        self.clipboard.set(contents);
    }

    pub fn mode(&self) -> Option<GlobalMode> {
        self.mode.clone()
    }
    pub fn set_mode(&mut self, mode: Option<GlobalMode>) {
        self.mode = mode.clone();
        if let Some(mode) = mode {
            self.last_non_contiguous_selection_mode = Some(Either::Right(mode));
        }
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub fn format_on_save(&self) -> bool {
        self.format_on_save
    }

    pub fn command_menu_invocation(&self) -> crate::config::CommandMenuInvocation {
        self.command_menu_invocation
    }

    pub fn quick_marks_menu_invocation(&self) -> crate::config::QuickMarksMenuInvocation {
        self.quick_marks_menu_invocation
    }

    pub fn bookmark_menu_invocation(&self) -> crate::config::BookmarkMenuInvocation {
        self.bookmark_menu_invocation
    }

    pub fn swap_key(&self) -> crate::config::SwapKey {
        self.swap_key
    }

    pub fn pick_menu_df_swap(&self) -> bool {
        self.pick_menu_df_swap
    }

    pub fn e_normal_mode(&self) -> crate::config::ENormalMode {
        self.e_normal_mode
    }

    pub fn filetree_open_with_path(&self) -> bool {
        self.filetree_open_with_path
    }

    #[cfg(test)]
    pub fn highlight(
        &mut self,
        language: shared::language::Language,
        source_code: &str,
    ) -> anyhow::Result<crate::syntax_highlight::HighlightedSpans> {
        use std::sync::atomic::AtomicUsize;

        self.highlight_configs
            .highlight(language, source_code, &AtomicUsize::new(0))
    }

    pub fn current_working_directory(&self) -> &AbsolutePath {
        &self.current_working_directory
    }

    pub fn set_file_dirty_status(&mut self, path: &AbsolutePath, dirty_status: bool) {
        self.file_dirty_status.insert(path.clone(), dirty_status);
    }

    pub fn get_file_dirty_status(&self, path: &AbsolutePath) -> Option<&bool> {
        self.file_dirty_status.get(path)
    }

    pub fn get_dirty_files(&self) -> Vec<&AbsolutePath> {
        self.file_dirty_status
            .iter()
            .filter(|(_path, dirty_status)| **dirty_status)
            .map(|(path, _dirty_status)| path)
            .collect()
    }

    pub fn global_search_config(&self) -> &GlobalSearchConfig {
        &self.global_search_config
    }

    pub fn update_local_search_config(&mut self, update: LocalSearchConfigUpdate, scope: Scope) {
        match scope {
            Scope::Local => &mut self.local_search_config,
            Scope::Global => &mut self.global_search_config.local_config,
        }
        .update(update);
    }

    pub fn update_global_search_config(
        &mut self,
        update: GlobalSearchConfigUpdate,
    ) -> anyhow::Result<()> {
        match update {
            GlobalSearchConfigUpdate::Config(config) => self.global_search_config = config,
        };
        Ok(())
    }

    pub fn local_search_config(&self, scope: Scope) -> &LocalSearchConfig {
        match scope {
            Scope::Local => &self.local_search_config,
            Scope::Global => &self.global_search_config.local_config,
        }
    }

    pub fn set_quickfix_list_current_item_index(&mut self, current_item_index: usize) {
        self.quickfix_list
            .set_current_item_index(current_item_index);
    }

    pub fn push_history_prompt(&mut self, key: PromptHistoryKey, line: String) {
        if let Some(map) = self.prompt_histories.get_mut(&key) {
            map.shift_remove(&line);
            let inserted = map.insert(line);
            debug_assert!(inserted);
        } else {
            self.prompt_histories.insert(key, {
                let mut set = IndexSet::new();
                set.insert(line);
                set
            });
        }
    }

    pub fn get_prompt_history(&self, key: PromptHistoryKey) -> Vec<String> {
        self.prompt_histories
            .get(&key)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect_vec()
    }

    pub fn set_last_non_contiguous_selection_mode(
        &mut self,
        selection_mode: Either<crate::selection::SelectionMode, GlobalMode>,
    ) {
        self.last_non_contiguous_selection_mode = Some(selection_mode);
    }

    pub fn last_non_contiguous_selection_mode(
        &self,
    ) -> Option<&Either<crate::selection::SelectionMode, GlobalMode>> {
        self.last_non_contiguous_selection_mode.as_ref()
    }

    pub fn keyboard_layout(&self) -> &KeyboardLayout {
        &self.keyboard_layout
    }

    pub fn navigation_keys(&self) -> crate::config::NavigationKeys {
        self.navigation_keys
    }

    pub fn hjkl_config(&self) -> crate::config::HjklConfig {
        self.hjkl_config
    }

    pub fn set_keyboard_layout(&mut self, mut keyboard_layout: KeyboardLayout) {
        keyboard_layout.set_navigation_keys(self.navigation_keys);
        keyboard_layout.set_hjkl_config(self.hjkl_config);
        self.keyboard_layout = keyboard_layout;
    }

    pub fn keyboard_layouts(&self) -> &HashMap<String, KeyboardLayout> {
        &self.keyboard_layouts
    }

    pub fn push_location_history(&mut self, location: Location, backward: bool) {
        if backward {
            self.location_history_backward.push(location);
        } else {
            self.location_history_forward.push(location);
        }
    }

    pub fn clear_forward_history(&mut self) {
        self.location_history_forward.clear();
    }

    pub fn location_previous(&mut self) -> Option<Location> {
        self.location_history_backward.pop()
    }

    pub fn location_next(&mut self) -> Option<Location> {
        self.location_history_forward.pop()
    }

    pub fn get_marked_files(&self) -> Vec<&AbsolutePath> {
        self.marked_files.iter().collect()
    }

    /// Returns some path if we should focus another file.
    /// If the action is to unmark a file, and the file is not the only marked file left,
    /// then we return the nearest neighbor.
    pub fn toggle_path_mark(&mut self, path: AbsolutePath) -> Option<&AbsolutePath> {
        if let Some(index) = self.marked_files.get_index_of(&path) {
            if self.is_dirty(&path) {
                // Never unmark file needed saving
                return self.marked_files.get(&path);
            }
            self.unmark_path_impl(index, path)
        } else {
            let _ = self.mark_file(path);
            None
        }
    }

    pub fn mark_file(&mut self, path: AbsolutePath) -> (usize, bool) {
        self.marked_files.insert_full(path)
    }

    /// Returns true if the path to be removed is in the list
    pub fn unmark_path(&mut self, path: AbsolutePath) -> Option<&AbsolutePath> {
        if self.is_dirty(&path) {
            // Never unmark file needed saving
            return self.marked_files.get(&path);
        }
        if let Some(index) = self.marked_files.get_index_of(&path) {
            self.unmark_path_impl(index, path)
        } else {
            None
        }
    }

    fn unmark_path_impl(&mut self, index: usize, path: AbsolutePath) -> Option<&AbsolutePath> {
        let _ = self.marked_files.shift_remove(&path);
        self.marked_files
            .get_index(if index == self.marked_files.len() {
                index.saturating_sub(1)
            } else {
                index
            })
    }

    pub fn get_alternate_file(&self) -> Option<&AbsolutePath> {
        self.alternate_file.as_ref()
    }

    pub fn set_alternate_file(&mut self, path: AbsolutePath) {
        self.alternate_file = Some(path);
    }

    pub fn clear_alternate_file(&mut self) {
        self.alternate_file = None;
    }

    pub fn location_history_backward(&self) -> &Vec<Location> {
        &self.location_history_backward
    }

    pub fn is_running_as_embedded(&self) -> bool {
        self.is_running_as_embedded
    }

    pub fn register_untitled_file(&mut self, path: AbsolutePath) {
        self.untitled_files.insert(path);
    }

    pub fn unregister_untitled_file(&mut self, path: &AbsolutePath) {
        self.untitled_files.remove(path);
    }

    pub fn is_untitled_file(&self, path: &AbsolutePath) -> bool {
        self.untitled_files.contains(path)
    }

    pub fn is_dirty(&self, path: &AbsolutePath) -> bool {
        self.get_file_dirty_status(path)
            .copied()
            .unwrap_or(false)
    }

    pub fn is_dirty_untitled(&self, path: &AbsolutePath) -> bool {
        self.untitled_files.contains(path) && self.is_dirty(path)
    }

    pub fn is_marked(&self, path: &AbsolutePath) -> bool {
        self.marked_files.contains(path)
    }

    pub fn insert_marked_file_at(&mut self, index: usize, path: AbsolutePath) {
        let index = index.min(self.marked_files.len());
        self.marked_files.shift_insert(index, path);
    }

    pub fn replace_marked_file_at_same_index(&mut self, from: &AbsolutePath, to: AbsolutePath) {
        if let Some(index) = self.marked_files.get_index_of(from) {
            self.marked_files.shift_remove(from);
            let index = index.min(self.marked_files.len());
            self.marked_files.shift_insert(index, to);
        } else {
            self.marked_files.insert(to);
        }
    }

    pub fn rename_path_mark(&mut self, from: &AbsolutePath, to: &AbsolutePath) {
        if let Some(index) = self.marked_files.get_index_of(from) {
            self.marked_files.shift_remove(from);
            let index = index.min(self.marked_files.len());
            self.marked_files.shift_insert(index, to.clone());
        } else {
            self.marked_files.insert(to.clone());
        }
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct GlobalSearchConfig {
    pub include_glob: Option<Glob>,
    pub exclude_glob: Option<Glob>,
    pub local_config: LocalSearchConfig,
}
impl GlobalSearchConfig {
    pub fn local_config(&self) -> &LocalSearchConfig {
        &self.local_config
    }

    pub fn include_glob(&self) -> Option<Glob> {
        self.include_glob.clone()
    }

    pub fn exclude_glob(&self) -> Option<Glob> {
        self.exclude_glob.clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum LocalSearchConfigMode {
    Regex(RegexConfig),
    AstGrep,
    NamingConventionAgnostic,
}
impl LocalSearchConfigMode {
    pub fn display(&self) -> String {
        match self {
            LocalSearchConfigMode::Regex(regex) => regex.display(),

            LocalSearchConfigMode::AstGrep => "AST Grep".to_string(),
            LocalSearchConfigMode::NamingConventionAgnostic => {
                "Naming Convention Agnostic".to_string()
            }
        }
    }
}

impl Default for LocalSearchConfigMode {
    fn default() -> Self {
        Self::Regex(RegexConfig::default())
    }
}

impl RegexConfig {
    fn display(&self) -> String {
        format!(
            "{}{}{}",
            if self.escaped { "Literal" } else { "Regex" },
            if self.case_sensitive {
                " A=a".to_string()
            } else {
                String::new()
            },
            if self.match_whole_word {
                " [Ab]".to_string()
            } else {
                String::new()
            }
        )
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct LocalSearchConfig {
    pub mode: LocalSearchConfigMode,
    search: Option<String>,
    replacement: Option<String>,
}

impl LocalSearchConfig {
    pub fn new(mode: LocalSearchConfigMode) -> Self {
        Self {
            mode,
            search: None,
            replacement: None,
        }
    }

    fn update(&mut self, update: LocalSearchConfigUpdate) {
        match update {
            #[cfg(test)]
            LocalSearchConfigUpdate::Mode(mode) => self.mode = mode,
            #[cfg(test)]
            LocalSearchConfigUpdate::Replacement(replacement) => {
                self.set_replacment(replacement);
            }
            #[cfg(test)]
            LocalSearchConfigUpdate::Search(search) => {
                self.set_search(search);
            }
            LocalSearchConfigUpdate::Config(config) => *self = config,
        }
    }

    pub fn set_search(&mut self, search: String) -> &mut Self {
        let _ = self.search.insert(search);
        self
    }

    pub fn search(&self) -> String {
        self.search.clone().unwrap_or_default()
    }

    pub fn set_replacment(&mut self, replacement: String) -> &mut Self {
        let _ = self.replacement.insert(replacement);
        self
    }

    pub fn last_search(&self) -> Option<Search> {
        self.search.clone().map(|search| Search {
            search,
            mode: self.mode,
        })
    }

    pub fn replacement(&self) -> String {
        self.replacement.clone().unwrap_or_default()
    }

    pub fn require_tree_sitter(&self) -> bool {
        self.mode == LocalSearchConfigMode::AstGrep
    }
}

#[cfg(test)]
mod test_context {
    use std::collections::HashMap;

    use indexmap::IndexSet;
    use itertools::Itertools;
    use shared::absolute_path::AbsolutePath;

    use crate::{
        char_index_range::CharIndexRange, components::prompt::PromptHistoryKey, context::Context,
        persistence::Persistence, selection::CharIndex,
    };

    #[test]
    fn test_persistence() -> anyhow::Result<()> {
        let temp_data_file = tempfile::NamedTempFile::new()?.path().to_path_buf();
        std::fs::write(temp_data_file.clone(), "")?;

        let temp_cwd = tempfile::tempdir()?.path().to_path_buf();
        std::fs::create_dir_all(temp_cwd.clone())?;

        let random_file = tempfile::NamedTempFile::new()?.path().to_path_buf();
        std::fs::write(random_file.clone(), "foo")?;

        let marks = [(CharIndex(0)..CharIndex(2)).into()].to_vec();

        // Save data
        {
            let persistence = Persistence::load_or_default(temp_data_file.clone());
            let mut context = Context::new(temp_cwd.clone().try_into()?, false, Some(persistence));
            let mut index_set = IndexSet::new();
            index_set.insert("Github Light".to_string());
            context.toggle_path_mark(random_file.clone().try_into()?);
            context.save_marks(random_file.clone().try_into()?, marks.clone());
            context
                .prompt_histories
                .insert(PromptHistoryKey::Theme, index_set);
            context.persist_data();
        }

        // Load data
        {
            let persistence = Persistence::load_or_default(temp_data_file);
            let context = Context::new(temp_cwd.try_into()?, false, Some(persistence));
            let actual_marked_files = context
                .get_marked_files()
                .into_iter()
                .cloned()
                .collect_vec();

            let expected_marked_files = vec![AbsolutePath::try_from(random_file.clone())?];

            assert_eq!(actual_marked_files, expected_marked_files);

            let actual_marks = context.marks;

            let expected_marks: HashMap<AbsolutePath, Vec<CharIndexRange>> =
                [(random_file.try_into()?, marks)].into_iter().collect();

            assert_eq!(actual_marks, expected_marks);

            let actual_prompt_histories = context.prompt_histories;
            assert_eq!(
                actual_prompt_histories
                    .get(&PromptHistoryKey::Theme)
                    .unwrap()
                    .len(),
                1
            );
        }

        Ok(())
    }
}
