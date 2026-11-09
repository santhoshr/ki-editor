use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use itertools::Itertools;
use my_proc_macros::key;
use shared::absolute_path::AbsolutePath;

use crate::{
    app::{Dispatch, DispatchParser, Dispatches},
    buffer::Buffer,
    components::{component::ComponentId, editor::DispatchEditor},
    context::Context,
    lsp::completion::Completion,
    search::parse_search_config,
    selection::{CharIndex, Selection, SelectionMode, SelectionSet},
    thread::Callback,
};

use super::{
    component::Component,
    dropdown_sync::DropdownItem,
    editor::{Editor, Mode},
    suggestive_editor::{DispatchSuggestiveEditor, SuggestiveEditor, SuggestiveEditorFilter},
};

#[derive(Clone, PartialEq)]
pub enum PromptOnEnter {
    ParseCurrentLine {
        parser: DispatchParser,
        history_key: PromptHistoryKey,
        current_line: Option<String>,
        suggested_items: Vec<DropdownItem>,
    },
    ParseWholeBuffer {
        parser: DispatchParser,
        initial_lines: Vec<String>,
    },
    SelectsFirstMatchingItem {
        items: PromptItems,
    },
}

pub struct Prompt {
    editor: SuggestiveEditor,
    on_enter: PromptOnEnter,
    on_change: Option<PromptOnChangeDispatch>,
    on_cancelled: Option<Dispatches>,
    on_empty_enter: Option<Dispatches>,
    on_equal_enter: Option<Dispatches>,
    last_line: std::cell::RefCell<Option<String>>,
    ignore_completion_on_enter: bool,
    workspace_items: std::sync::Arc<std::sync::Mutex<Vec<DropdownItem>>>,
    was_path_nav: std::cell::Cell<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PromptOnChangeDispatch {
    RequestWorkspaceSymbol(AbsolutePath),
    SetIncrementalSearchConfig { component_id: ComponentId },
    UpdateSuggestedItemsWithChildPaths,
    UpdateSuggestedItemsWithChildPathsAndFiles,
    UpdateSuggestedItemsForArbitraryFile,
}

impl PromptOnChangeDispatch {
    fn to_dispatches(&self, context: &PromptContext) -> Dispatches {
        match self {
            PromptOnChangeDispatch::RequestWorkspaceSymbol(path) => {
                Dispatches::one(Dispatch::RequestWorkspaceSymbols {
                    query: context.current_line.to_string(),
                    path: path.clone(),
                })
            }
            PromptOnChangeDispatch::SetIncrementalSearchConfig { component_id } => {
                let search_config = parse_search_config(&context.current_line)
                    .unwrap_or_default()
                    .local_config;
                Dispatches::one(Dispatch::UpdateCurrentComponentTitle(format!(
                    "Local search ({})",
                    search_config.mode.display()
                )))
                .append(Dispatch::SetIncrementalSearchConfig {
                    config: search_config,
                    component_id: Some(*component_id),
                })
            }
            PromptOnChangeDispatch::UpdateSuggestedItemsWithChildPaths => {
                let expanded_line = if let Ok(abs) = AbsolutePath::try_from(&context.current_line) {
                    PathBuf::from(abs.display_absolute())
                } else {
                    PathBuf::from(&context.current_line)
                };
                let path = if expanded_line.is_dir() {
                    Some(expanded_line.as_path())
                } else {
                    expanded_line.parent()
                };
                if let Some(path) = path {
                    if let Ok(paths) = get_child_directories(path) {
                        Dispatches::one(Dispatch::ToSuggestiveEditor(
                            DispatchSuggestiveEditor::Completion(Completion {
                                items: paths
                                    .into_iter()
                                    .map(|path| DropdownItem::new(path.display_absolute()))
                                    .collect_vec(),
                                trigger_characters: Vec::new(),
                            }),
                        ))
                    } else {
                        Dispatches::default()
                    }
                } else {
                    Dispatches::default()
                }
            }
            PromptOnChangeDispatch::UpdateSuggestedItemsWithChildPathsAndFiles => {
                let items = get_child_paths_and_files_for_prompt(&context.current_line);
                if items.is_empty() {
                    Dispatches::default()
                } else {
                    Dispatches::one(Dispatch::ToSuggestiveEditor(
                        DispatchSuggestiveEditor::Completion(Completion {
                            items,
                            trigger_characters: Vec::new(),
                        }),
                    ))
                }
            }
            PromptOnChangeDispatch::UpdateSuggestedItemsForArbitraryFile => Dispatches::default(),
        }
    }
}

fn get_child_paths_and_files_for_prompt(line: &str) -> Vec<DropdownItem> {
    let line = line.trim();
    let home_dir = AbsolutePath::home_dir().ok();

    let (lookup_path, is_home_scoped) = if line.is_empty() {
        (
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            false,
        )
    } else if line == "~" || line == "~/" {
        let p = home_dir.clone().unwrap_or_else(|| PathBuf::from("."));
        (p, true)
    } else if let Some(rest) = line.strip_prefix("~/") {
        let base = if let Some(home) = &home_dir {
            home.join(rest)
        } else {
            PathBuf::from(line)
        };
        let dir = if line.ends_with('/') && base.is_dir() {
            base
        } else {
            base.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
        };
        (dir, true)
    } else if line.starts_with('/') {
        let base = PathBuf::from(line);
        let dir = if line.ends_with('/') && base.is_dir() {
            base
        } else {
            base.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("/"))
        };
        (dir, false)
    } else if line == ".." || line == "../" || line.starts_with("../") {
        let base = AbsolutePath::try_from(line)
            .map(|p| p.to_path_buf().clone())
            .unwrap_or_else(|_| PathBuf::from(".."));
        let dir = if (line.ends_with('/') || line == "..") && base.is_dir() {
            base
        } else {
            base.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."))
        };
        (dir, false)
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let base = cwd.join(line);
        let dir = if line.ends_with('/') && base.is_dir() {
            base
        } else {
            base.parent().map(|p| p.to_path_buf()).unwrap_or(cwd)
        };
        (dir, false)
    };

    let Ok(entries) = std::fs::read_dir(&lookup_path) else {
        return Vec::new();
    };

    let mut formatted_items = Vec::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let is_dir = path.is_dir();

        let display_str = if is_home_scoped {
            if let Some(home) = &home_dir {
                if let Ok(rel) = path.strip_prefix(home) {
                    if rel.as_os_str().is_empty() {
                        "~/".to_string()
                    } else if is_dir {
                        format!("~/{}/", rel.display())
                    } else {
                        format!("~/{}", rel.display())
                    }
                } else if is_dir {
                    format!("{}/", path.display())
                } else {
                    path.display().to_string()
                }
            } else if is_dir {
                format!("{}/", path.display())
            } else {
                path.display().to_string()
            }
        } else if line.starts_with('/') {
            if is_dir {
                format!("{}/", path.display())
            } else {
                path.display().to_string()
            }
        } else if line.starts_with("..") {
            let parent_dir = AbsolutePath::try_from("..")
                .map(|p| p.to_path_buf().clone())
                .unwrap_or_else(|_| PathBuf::from(".."));
            if let Ok(rel) = path.strip_prefix(&parent_dir) {
                if is_dir {
                    format!("../{}/", rel.display())
                } else {
                    format!("../{}", rel.display())
                }
            } else if is_dir {
                format!("{}/", path.display())
            } else {
                path.display().to_string()
            }
        } else {
            // Relative
            if let Ok(cwd) = std::env::current_dir() {
                if let Ok(rel) = path.strip_prefix(&cwd) {
                    if rel.as_os_str().is_empty() {
                        "./".to_string()
                    } else if is_dir {
                        format!("{}/", rel.display())
                    } else {
                        rel.display().to_string()
                    }
                } else if is_dir {
                    format!("{}/", path.display())
                } else {
                    path.display().to_string()
                }
            } else if is_dir {
                format!("{}/", path.display())
            } else {
                path.display().to_string()
            }
        };

        let dispatches = if is_dir {
            if let Ok(abs_path) = AbsolutePath::try_from(path.clone()) {
                Dispatches::one(Dispatch::ChangeWorkingDirectory(abs_path))
            } else {
                Dispatches::default()
            }
        } else {
            Dispatches::one(Dispatch::OpenFileFromPathBuf {
                path,
                owner: crate::buffer::BufferOwner::User,
                focus: true,
            })
        };

        formatted_items.push((
            display_str.clone(),
            DropdownItem::new(display_str).set_dispatches(dispatches),
        ));
    }

    formatted_items.sort_by(|a, b| a.0.cmp(&b.0));

    formatted_items.into_iter().map(|(_, item)| item).collect()
}

fn get_child_directories(path: &Path) -> anyhow::Result<Vec<AbsolutePath>> {
    Ok(std::fs::read_dir(path)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter_map(|path| AbsolutePath::try_from(path.canonicalize().ok()?).ok())
        .sorted()
        .collect())
}

struct PromptContext {
    current_line: String,
}

#[derive(Clone, PartialEq)]
pub struct PromptConfig {
    pub on_enter: PromptOnEnter,
    pub title: String,

    /// If defined, the `Dispatches` here is used for undoing the dispatches fired on change.
    pub on_cancelled: Option<Dispatches>,
    pub on_change: Option<PromptOnChangeDispatch>,
    pub show_all_when_empty: bool,
    pub ignore_completion_on_enter: bool,
    pub auto_select_at_dir: bool,
    pub initial_text: Option<String>,
    pub on_empty_enter: Option<Dispatches>,
    pub on_equal_enter: Option<Dispatches>,
}

impl PromptConfig {
    pub fn new(title: String, on_enter: PromptOnEnter) -> Self {
        let auto_select_at_dir = !matches!(on_enter, PromptOnEnter::ParseCurrentLine { .. });
        Self {
            title,
            on_enter,
            on_cancelled: None,
            on_change: None,
            show_all_when_empty: false,
            ignore_completion_on_enter: false,
            auto_select_at_dir,
            initial_text: None,
            on_empty_enter: None,
            on_equal_enter: None,
        }
    }

    pub fn set_initial_text(self, initial_text: Option<String>) -> Self {
        Self {
            initial_text,
            ..self
        }
    }

    pub fn set_on_empty_enter(self, on_empty_enter: Option<Dispatches>) -> Self {
        Self {
            on_empty_enter,
            ..self
        }
    }

    pub fn set_on_equal_enter(self, on_equal_enter: Option<Dispatches>) -> Self {
        Self {
            on_equal_enter,
            ..self
        }
    }

    pub fn set_show_all_when_empty(self, show_all_when_empty: bool) -> Self {
        Self {
            show_all_when_empty,
            ..self
        }
    }

    pub fn set_ignore_completion_on_enter(self, ignore_completion_on_enter: bool) -> Self {
        Self {
            ignore_completion_on_enter,
            ..self
        }
    }

    pub fn set_auto_select_at_dir(self, auto_select_at_dir: bool) -> Self {
        Self {
            auto_select_at_dir,
            ..self
        }
    }
    pub fn items(&self) -> Vec<DropdownItem> {
        match &self.on_enter {
            PromptOnEnter::ParseCurrentLine {
                suggested_items, ..
            } => suggested_items.clone(),
            PromptOnEnter::ParseWholeBuffer { .. } => Vec::new(),
            PromptOnEnter::SelectsFirstMatchingItem { items } => match &items {
                PromptItems::None => Vec::default(),
                PromptItems::Precomputed(dropdown_items) => dropdown_items.clone(),
                PromptItems::BackgroundTask { .. } => Vec::default(),
            },
        }
    }

    pub fn set_on_change(self, on_change: Option<PromptOnChangeDispatch>) -> Self {
        Self { on_change, ..self }
    }

    pub fn set_on_cancelled(self, on_cancelled: Option<Dispatches>) -> PromptConfig {
        Self {
            on_cancelled,
            ..self
        }
    }
}

impl std::fmt::Debug for PromptConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptConfig")
            .field("title", &self.title)
            .field("on_cancelled", &self.on_cancelled)
            .finish()
    }
}

#[derive(Clone, PartialEq, Default)]
pub enum PromptItems {
    #[default]
    None,
    Precomputed(Vec<DropdownItem>),
    BackgroundTask {
        task: PromptItemsBackgroundTask,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum PromptItemsBackgroundTask {
    NonGitIgnoredFiles { working_directory: AbsolutePath },
    NonGitIgnoredFilesAndDirectories { working_directory: AbsolutePath },
    HandledByMainEventLoop,
}
impl PromptItemsBackgroundTask {
    fn execute(self, injector: Callback<DropdownItem>) {
        match self {
            PromptItemsBackgroundTask::NonGitIgnoredFiles { working_directory } => {
                std::thread::spawn(move || {
                    crate::list::WalkBuilderConfig::get_non_git_ignored_files(
                        working_directory.to_path_buf().clone(),
                        Arc::new(move |path_buf| {
                            let item = DropdownItem::from_path_buf(&working_directory, path_buf);
                            injector.call(item);
                        }),
                    );
                });
            }
            PromptItemsBackgroundTask::NonGitIgnoredFilesAndDirectories { working_directory } => {
                std::thread::spawn(move || {
                    crate::list::WalkBuilderConfig::get_non_git_ignored_files_and_directories(
                        working_directory.to_path_buf().clone(),
                        Arc::new(move |path_buf| {
                            let item = DropdownItem::from_path_buf(&working_directory, path_buf);
                            injector.call(item);
                        }),
                    );
                });
            }
            PromptItemsBackgroundTask::HandledByMainEventLoop => {
                // Do nothing if the background task is handled by the app main event loop
                // For example: LSP Workspace Symbols
            }
        }
    }
}

#[derive(
    Hash, PartialEq, Eq, Debug, Clone, Copy, serde::Serialize, serde::Deserialize, Default,
)]
pub enum PromptHistoryKey {
    MoveToIndex,
    Search,
    Rename,
    AddPath,
    MovePath,
    CopyFile,
    Symbol,
    OpenFile,
    CodeAction,
    #[default]
    Null,
    Theme,
    PipeToShell,
    FilterSelectionsMatchingSearch {
        maintain: bool,
    },
    KeyboardLayout,
    SurroundXmlTag,
    ResolveBufferSaveConflict,
    WorkspaceSymbol,
    ChangeWorkingDirectory,
    SaveAs,
}

impl Prompt {
    pub fn new(
        config: PromptConfig,
        context: &Context,
        on_nucleo_tick_debounced: Callback<()>,
    ) -> (Self, Dispatches) {
        let (text, leaves_current_line_empty, has_multiple_lines) = match &config.on_enter {
            PromptOnEnter::ParseCurrentLine {
                history_key,
                current_line,
                ..
            } => {
                let history = context.get_prompt_history(*history_key);
                let history = if history.is_empty() {
                    "".to_string()
                } else {
                    format!("{}\n", history.join("\n"))
                };
                let text = if let Some(current_line) = current_line {
                    if history.is_empty() {
                        current_line.to_string()
                    } else {
                        format!("{history}{current_line}")
                    }
                } else {
                    history
                };
                (text, current_line.is_none(), false)
            }
            PromptOnEnter::ParseWholeBuffer { initial_lines, .. } => {
                (initial_lines.join("\n"), false, true)
            }
            PromptOnEnter::SelectsFirstMatchingItem { .. } => {
                if let Some(initial) = &config.initial_text {
                    (initial.clone(), false, false)
                } else {
                    ("".to_string(), true, false)
                }
            }
        };
        let mut editor = SuggestiveEditor::from_buffer(
            Rc::new(RefCell::new(Buffer::new(None, &text))),
            SuggestiveEditorFilter::CurrentLine,
            on_nucleo_tick_debounced,
        );
        let dispatches = if leaves_current_line_empty {
            Dispatches::one(Dispatch::ToEditor(DispatchEditor::MoveToLastChar))
        } else if config.initial_text.is_some() {
            Dispatches::new(vec![
                Dispatch::ToEditor(DispatchEditor::SelectAll),
                Dispatch::ToEditor(DispatchEditor::EnterInsertMode(super::editor::Direction::End)),
            ])
        } else {
            Dispatches::new(
                [
                    Dispatch::ToEditor(DispatchEditor::SetSelectionMode(
                        super::editor::IfCurrentNotFound::LookForward,
                        SelectionMode::Line,
                    )),
                    Dispatch::ToEditor(DispatchEditor::MoveSelection(
                        super::editor::Movement::Last,
                    )),
                ]
                .to_vec(),
            )
            .append_some(if has_multiple_lines {
                Some(Dispatch::ToEditor(DispatchEditor::CursorAddToAllSelections))
            } else {
                None
            })
            .append(Dispatch::ToEditor(DispatchEditor::MoveToLineEnd))
        };
        // TODO: set cursor to last line
        editor.set_title(config.title.clone());
        editor.set_show_all_when_empty(config.show_all_when_empty);
        editor.set_completion(Completion {
            items: config.items(),
            trigger_characters: vec![" ".to_string()],
        });
        let dispatches = dispatches.chain(editor.render_completion_dropdown(true));

        let workspace_items = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        if let PromptOnEnter::SelectsFirstMatchingItem {
            items: PromptItems::BackgroundTask { task },
        } = &config.on_enter
        {
            let ws_clone = workspace_items.clone();
            let injector = editor.injector();
            let caching_injector = Callback::new(std::sync::Arc::new(move |item: DropdownItem| {
                ws_clone.lock().unwrap().push(item.clone());
                injector.call(item);
            }));
            task.to_owned().execute(caching_injector);
        }

        if let Some(initial) = &config.initial_text {
            let len = CharIndex(initial.chars().count());
            let mut sel = Selection::default();
            sel.range = (CharIndex(0)..len).into();
            sel.initial_range = Some((CharIndex(0)..CharIndex(0)).into());
            let _ = editor.editor_mut().set_selection_set(
                SelectionSet::new(nonempty::NonEmpty::singleton(sel)),
                context,
            );
        }
        editor.set_auto_select_at_dir(config.auto_select_at_dir);

        (
            Prompt {
                editor,
                on_enter: config.on_enter,
                on_cancelled: config.on_cancelled,
                on_change: config.on_change,
                on_empty_enter: config.on_empty_enter,
                on_equal_enter: config.on_equal_enter,
                last_line: std::cell::RefCell::new(None),
                ignore_completion_on_enter: config.ignore_completion_on_enter,
                workspace_items,
                was_path_nav: std::cell::Cell::new(false),
            },
            dispatches,
        )
    }

    fn replace_current_query_with_focused_item(
        &mut self,
        context: &Context,
        event: event::KeyEvent,
    ) -> Result<Dispatches, anyhow::Error> {
        if self.editor.completion_dropdown_opened() {
            if let Some(item) = self.editor.completion_dropdown_current_item() {
                let clean_text = if let Some(dispatch) = item.dispatches.clone().into_vec().first() {
                    match dispatch {
                        Dispatch::OpenFileFromPathBuf { path, .. } => {
                            if let Ok(cwd) = std::env::current_dir() {
                                path.strip_prefix(&cwd)
                                    .map(|p| p.display().to_string())
                                    .unwrap_or_else(|_| path.display().to_string())
                            } else {
                                path.display().to_string()
                            }
                        }
                        Dispatch::ChangeWorkingDirectory(path) => {
                            let mut s = if let Ok(cwd) = std::env::current_dir() {
                                path.to_path_buf()
                                    .strip_prefix(&cwd)
                                    .map(|p| p.display().to_string())
                                    .unwrap_or_else(|_| path.to_path_buf().display().to_string())
                            } else {
                                path.to_path_buf().display().to_string()
                            };
                            if !s.ends_with('/') {
                                s.push('/');
                            }
                            s
                        }
                        _ => item.display(),
                    }
                } else {
                    item.display()
                };
                let dispatches = self.editor.update_current_line(context, &clean_text)?;
                Ok(dispatches.chain(self.editor_mut().move_to_line_end()?))
            } else {
                Ok(Dispatches::default())
            }
        } else {
            self.editor_mut().handle_key_event(context, event)
        }
    }

    pub fn handle_nucleo_notify(&mut self) -> Dispatches {
        self.editor.handle_nucleo_notify()
    }

    pub fn clear_and_update_matcher_items(&mut self, items: Vec<DropdownItem>) {
        *self.last_line.borrow_mut() = None;
        self.editor.set_completion(Completion {
            items,
            trigger_characters: vec![" ".to_string()],
        });
    }

    pub fn get_on_change_dispatches(&self) -> Dispatches {
        let current_line = self.editor().current_line().unwrap_or_default();
        if self.last_line.borrow().as_deref() == Some(&current_line) {
            return Dispatches::default();
        }
        *self.last_line.borrow_mut() = Some(current_line.clone());
        self.editor.reset_dismissed_completion();

        if self.on_change == Some(PromptOnChangeDispatch::UpdateSuggestedItemsForArbitraryFile) {
            let line = current_line.trim();
            let is_path_nav = line.starts_with('/')
                || line.starts_with('~')
                || line.starts_with("..")
                || line == "."
                || line.starts_with("./");

            if is_path_nav {
                self.was_path_nav.set(true);
                let items = get_child_paths_and_files_for_prompt(line);
                return Dispatches::one(Dispatch::ToSuggestiveEditor(
                    DispatchSuggestiveEditor::Completion(Completion {
                        items,
                        trigger_characters: Vec::new(),
                    }),
                ));
            } else if self.was_path_nav.get() {
                self.was_path_nav.set(false);
                let items = self.workspace_items.lock().unwrap().clone();
                return Dispatches::one(Dispatch::ToSuggestiveEditor(
                    DispatchSuggestiveEditor::Completion(Completion {
                        items,
                        trigger_characters: Vec::new(),
                    }),
                ));
            } else {
                return Dispatches::default();
            }
        }

        self.on_change
            .as_ref()
            .map(|on_change| {
                on_change.to_dispatches(&PromptContext {
                    current_line,
                })
            })
            .unwrap_or_default()
    }
}

impl Component for Prompt {
    fn editor(&self) -> &Editor {
        self.editor.editor()
    }

    fn editor_mut(&mut self) -> &mut Editor {
        self.editor.editor_mut()
    }

    fn handle_dispatch_editor(
        &mut self,
        context: &Context,
        dispatch: super::editor::DispatchEditor,
    ) -> anyhow::Result<Dispatches> {
        self.editor.handle_dispatch_editor(context, dispatch)
    }

    fn handle_key_event(
        &mut self,
        context: &Context,
        event: event::KeyEvent,
    ) -> anyhow::Result<Dispatches> {
        match event {
            key!("esc") => {
                if self.editor.completion_dropdown_opened() && !self.editor.is_completion_dismissed() {
                    return Ok(self.editor.dismiss_completion());
                }
                if self.editor().mode == Mode::Normal {
                    return Ok(Dispatches::one(Dispatch::CloseCurrentWindow)
                        .chain(self.on_cancelled.clone().unwrap_or_default()));
                }
                let mut dispatches = self.editor.dismiss_completion();
                dispatches = dispatches.chain(self.editor.handle_key_event(context, event)?);
                Ok(dispatches)
            }
            key!("tab") => self.replace_current_query_with_focused_item(context, event),
            _ if event.display() == "alt+x" => {
                self.replace_current_query_with_focused_item(context, event)
            }
            key!("ctrl+k") | key!("alt+c") => {
                let all_items = self.editor.completion_dropdown_all_filtered_items();
                let mut paths: Vec<AbsolutePath> = Vec::new();
                for item in &all_items {
                    for dispatch in item.dispatches.clone().into_vec() {
                        if let Dispatch::OpenFileFromPathBuf { path, .. } = dispatch {
                            if let Ok(abs_path) = path.try_into() {
                                paths.push(abs_path);
                            }
                        }
                    }
                }

                if !paths.is_empty() {
                    Ok(Dispatches::one(Dispatch::CloseCurrentWindow)
                        .chain(Dispatches::one(Dispatch::CloseBuffers(paths))))
                } else {
                    Ok(Dispatches::empty())
                }
            }
            key!("ctrl+o") | key!("alt+g") | key!("alt+o") => {
                let all_items = self.editor.completion_dropdown_all_filtered_items();
                if !all_items.is_empty() {
                    let mut paths: Vec<AbsolutePath> = Vec::new();
                    for item in &all_items {
                        for dispatch in item.dispatches.clone().into_vec() {
                            match dispatch {
                                Dispatch::OpenFileFromPathBuf { path, .. } => {
                                    if let Ok(abs_path) = AbsolutePath::try_from(path) {
                                        if !abs_path.to_path_buf().is_dir() {
                                            paths.push(abs_path);
                                        }
                                    }
                                }
                                Dispatch::OpenFile { path, .. } => {
                                    if !path.to_path_buf().is_dir() {
                                        paths.push(path);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    if let Some(non_empty_paths) = nonempty::NonEmpty::from_vec(paths) {
                        Ok(Dispatches::one(Dispatch::CloseCurrentWindow)
                            .chain(Dispatches::one(Dispatch::OpenAndMarkFiles(non_empty_paths))))
                    } else {
                        let dispatches = all_items
                            .into_iter()
                            .flat_map(|item| item.dispatches.into_vec())
                            .collect();
                        Ok(Dispatches::one(Dispatch::CloseCurrentWindow).chain(Dispatches::new(dispatches)))
                    }
                } else {
                    Ok(Dispatches::empty())
                }
            }
            key!("enter") => {
                let dispatches = match &self.on_enter {
                    PromptOnEnter::SelectsFirstMatchingItem { .. } => {
                        let line = self.editor().current_line().unwrap_or_default();
                        let text = line.trim();
                        if text == "=" {
                            if let Some(on_equal) = &self.on_equal_enter {
                                return Ok(Dispatches::one(Dispatch::CloseCurrentWindow).chain(on_equal.clone()));
                            }
                        }
                        if text.is_empty() {
                            if let Some(on_empty) = &self.on_empty_enter {
                                return Ok(Dispatches::one(Dispatch::CloseCurrentWindow).chain(on_empty.clone()));
                            }
                        }

                        let current_item = if !self.editor.is_completion_dismissed()
                            && self.editor.completion_dropdown_opened()
                        {
                            self.editor.completion_dropdown_current_item()
                        } else {
                            None
                        };

                        if let Some(item) = current_item {
                            item.dispatches
                        } else {
                            if !text.is_empty() {
                                if let Ok(abs_path) = AbsolutePath::try_from(text) {
                                    if abs_path.to_path_buf().is_dir() {
                                        Dispatches::one(Dispatch::ChangeWorkingDirectory(abs_path))
                                    } else {
                                        Dispatches::one(Dispatch::OpenFileFromPathBuf {
                                            path: abs_path.into_path_buf(),
                                            owner: crate::buffer::BufferOwner::User,
                                            focus: true,
                                        })
                                    }
                                } else {
                                    Dispatches::default()
                                }
                            } else {
                                self.on_empty_enter.clone().unwrap_or_default()
                            }
                        }
                    }
                    PromptOnEnter::ParseCurrentLine {
                        parser,
                        history_key: prompt_history_key,
                        ..
                    } => {
                        let is_text_search = self.ignore_completion_on_enter
                            || matches!(
                                prompt_history_key,
                                PromptHistoryKey::Search
                                    | PromptHistoryKey::FilterSelectionsMatchingSearch { .. }
                            );
                        let entry = if !is_text_search
                            && !self.editor.is_completion_dismissed()
                            && self.editor.completion_dropdown_opened()
                            && self.editor.is_completion_selected()
                        {
                            if let Some(item) = self.editor.completion_dropdown_current_item() {
                                item.display()
                            } else {
                                self.editor().current_line()?
                            }
                        } else {
                            self.editor().current_line()?
                        };
                        parser.parse(&entry)?.append(Dispatch::PushPromptHistory {
                            key: *prompt_history_key,
                            line: entry.clone(),
                        })
                    }
                    PromptOnEnter::ParseWholeBuffer { parser, .. } => {
                        let entry = self.editor().content();
                        parser.parse(&entry)?
                    }
                };

                Ok(Dispatches::one(Dispatch::CloseCurrentWindow).chain(dispatches))
            }
            _ => {
                let dispatches = self.editor.handle_key_event(context, event)?;
                Ok(dispatches.chain(
                    self.editor
                        .completion_dropdown_current_item()
                        .map(|item| item.on_focused())
                        .unwrap_or_default(),
                ))
            }
        }
    }

    fn post_handle_event(&self, dispatches: Dispatches) -> anyhow::Result<Dispatches> {
        Ok(dispatches
            .append(Dispatch::GetAndHandlePromptOnChangeDispatches)
            .append(Dispatch::ToSuggestiveEditor(
                DispatchSuggestiveEditor::UpdateFilter,
            )))
    }
}
impl Prompt {
    pub fn handle_dispatch_suggestive_editor(
        &mut self,
        dispatch: DispatchSuggestiveEditor,
    ) -> anyhow::Result<Dispatches> {
        self.editor.handle_dispatch(dispatch)
    }
}
#[cfg(test)]
mod test_prompt {
    use crate::{
        app::{LocalSearchConfigUpdate, Scope},
        buffer::BufferOwner,
        components::{
            editor::{Direction, IfCurrentNotFound},
            suggestive_editor::Info,
        },
        list::grep::RegexConfig,
        lsp::completion::CompletionItem,
        test_app::*,
    };
    use my_proc_macros::keys;

    use crate::{app::Dispatch, position::Position};

    use super::*;

    #[test]
    fn leaves_current_line_empty_if_current_line_is_not_defined() {
        fn test(
            leaves_current_line_empty: bool,
            expected_text: &'static str,
            expected_cursor_position: Position,
        ) {
            execute_test(|s| {
                Box::new([
                    App(OpenFile {
                        path: s.main_rs(),
                        owner: BufferOwner::User,
                        focus: true,
                    }),
                    App(OpenPrompt {
                        config: PromptConfig::new(
                            "".to_string(),
                            PromptOnEnter::ParseCurrentLine {
                                parser: DispatchParser::AddPath,
                                history_key: PromptHistoryKey::AddPath,
                                current_line: if leaves_current_line_empty {
                                    None
                                } else {
                                    Some("hello\nworld".to_string())
                                },
                                suggested_items: Vec::new(),
                            },
                        ),
                    }),
                    Expect(CurrentComponentContent(expected_text)),
                    Expect(EditorCursorPosition(expected_cursor_position)),
                ])
            })
            .unwrap();
        }
        test(true, "", Position::new(0, 0));
        test(false, "hello\nworld", Position::new(1, 5));
    }

    #[test]
    fn prompt_history() -> anyhow::Result<()> {
        execute_test(|s| {
            let open_prompt = OpenPrompt {
                config: PromptConfig::new(
                    "".to_string(),
                    PromptOnEnter::ParseCurrentLine {
                        parser: DispatchParser::Null,
                        history_key: PromptHistoryKey::Null,
                        current_line: None,
                        suggested_items: Vec::new(),
                    },
                ),
            };
            Box::new([
                App(OpenFile {
                    path: s.main_rs(),
                    owner: BufferOwner::User,
                    focus: true,
                }),
                App(open_prompt.clone()),
                App(HandleKeyEvents(keys!("h e l l o enter").to_vec())),
                App(open_prompt.clone()),
                Expect(CurrentComponentContent("hello\n")),
                App(HandleKeyEvents(keys!("h e l l o enter").to_vec())),
                App(open_prompt.clone()),
                // Duplicates should not be repeated
                Expect(CurrentComponentContent("hello\n")),
                App(open_prompt.clone()),
                App(HandleKeyEvents(keys!("y o enter").to_vec())),
                App(open_prompt.clone()),
                // Latest entry should appear last
                Expect(CurrentComponentContent("hello\nyo\n")),
                // Enter 'hello' again
                App(open_prompt.clone()),
                App(HandleKeyEvents(keys!("h e l l o enter").to_vec())),
                // 'hello' should appear last
                App(open_prompt.clone()),
                Expect(CurrentComponentContent("yo\nhello\n")),
            ])
        })
    }

    #[test]
    fn should_not_contain_newline_if_empty() -> Result<(), anyhow::Error> {
        execute_test(|s| {
            Box::new([
                App(OpenFile {
                    path: s.main_rs(),
                    owner: BufferOwner::User,
                    focus: true,
                }),
                Editor(SetContent("".to_string())),
                Editor(EnterInsertMode(Direction::Start)),
                App(Dispatch::OpenPrompt {
                    config: super::PromptConfig::new(
                        "".to_string(),
                        PromptOnEnter::ParseCurrentLine {
                            parser: DispatchParser::Null,
                            history_key: PromptHistoryKey::Null,
                            current_line: None,
                            suggested_items: Vec::new(),
                        },
                    ),
                }),
                Expect(CurrentComponentContent("")),
            ])
        })
    }

    #[test]
    fn enter_selects_first_matching_item() -> anyhow::Result<()> {
        execute_test(|s| {
            Box::new([
                App(OpenFile {
                    path: s.main_rs(),
                    owner: BufferOwner::User,
                    focus: true,
                }),
                App(OpenPrompt {
                    config: super::PromptConfig::new(
                        "".to_string(),
                        PromptOnEnter::SelectsFirstMatchingItem {
                            items: PromptItems::Precomputed(
                                ["foo".to_string(), "bar".to_string()]
                                    .into_iter()
                                    .map(|str| {
                                        let item: DropdownItem = str.clone().into();
                                        item.set_dispatches(Dispatches::one(ToEditor(SetContent(
                                            str,
                                        ))))
                                    })
                                    .collect(),
                            ),
                        },
                    ),
                }),
                Expect(CompletionDropdownIsOpen(true)),
                App(HandleKeyEvents(event::parse_key_events("f").unwrap())),
                App(HandleKeyEvent(key!("enter"))),
                Expect(CurrentComponentContent("foo")),
            ])
        })
    }

    #[test]
    fn tab_replace_current_content_with_first_highlighted_suggestion() -> anyhow::Result<()> {
        execute_test(|_| {
            Box::new([
                App(Dispatch::OpenPrompt {
                    config: super::PromptConfig::new(
                        "".to_string(),
                        PromptOnEnter::ParseCurrentLine {
                            parser: DispatchParser::SetContent,
                            history_key: PromptHistoryKey::Null,
                            current_line: None,
                            suggested_items: ["foo_bar".to_string()]
                                .into_iter()
                                .map(|item| item.into())
                                .collect(),
                        },
                    ),
                }),
                App(HandleKeyEvents(keys!("f o o _ b tab").to_vec())),
                Expect(CurrentComponentContent("foo_bar")),
                Expect(EditorCursorPosition(Position { line: 0, column: 7 })),
            ])
        })
    }

    #[test]
    fn test_on_cancelled() -> anyhow::Result<()> {
        execute_test(|_| {
            Box::new([
                App(Dispatch::OpenPrompt {
                    config: super::PromptConfig::new(
                        "".to_string(),
                        PromptOnEnter::SelectsFirstMatchingItem {
                            items: PromptItems::Precomputed(
                                [
                                    "foo_bar".to_string(),
                                    "zazam".to_string(),
                                    "boque".to_string(),
                                ]
                                .into_iter()
                                .map(|item| item.into())
                                .map(|item: DropdownItem| {
                                    let content = item.display();
                                    item.set_on_focused(Dispatches::one(Dispatch::ShowEditorInfo(
                                        Info::new("".to_string(), content),
                                    )))
                                })
                                .collect(),
                            ),
                        },
                    )
                    .set_on_cancelled(Some(Dispatches::one(
                        Dispatch::ShowEditorInfo(Info::new(
                            "".to_string(),
                            "back to square one".to_string(),
                        )),
                    ))),
                }),
                App(HandleKeyEvents(keys!("f o o _").to_vec())),
                Expect(EditorInfoContents(&["foo_bar"])),
                App(HandleKeyEvents(
                    keys!("alt+v alt+y release-alt+v z a m").to_vec(),
                )),
                Expect(EditorInfoContents(&["zazam"])),
                App(HandleKeyEvents(
                    keys!("alt+v alt+y release-alt+v q").to_vec(),
                )),
                Expect(EditorInfoContents(&["boque"])),
                App(HandleKeyEvents(keys!("esc esc").to_vec())),
                Expect(EditorInfoContents(&["back to square one"])),
            ])
        })
    }

    #[test]
    fn filter_without_matching_items_should_clear_suggestions() -> anyhow::Result<()> {
        execute_test(|_| {
            Box::new([
                App(Dispatch::OpenPrompt {
                    config: super::PromptConfig::new(
                        "".to_string(),
                        PromptOnEnter::ParseCurrentLine {
                            parser: DispatchParser::SetContent,
                            history_key: PromptHistoryKey::Null,
                            current_line: None,
                            suggested_items: [CompletionItem::from_label(
                                "spongebob squarepants".to_string(),
                            )]
                            .into_iter()
                            .map(|item| item.into())
                            .collect(),
                        },
                    ),
                }),
                App(TerminalDimensionChanged(crate::app::Dimension {
                    height: 10,
                    width: 50,
                })),
                Expect(AppGridContains("squarepants")),
                App(HandleKeyEvents(keys!("f o o").to_vec())),
                Expect(Not(Box::new(AppGridContains("squarepants")))),
            ])
        })
    }

    #[test]
    fn suggestion_should_update_with_kill_line() -> Result<(), anyhow::Error> {
        execute_test(|s| {
            Box::new([
                App(OpenFile {
                    path: s.main_rs(),
                    owner: BufferOwner::User,
                    focus: true,
                }),
                Editor(SetContent("".to_string())),
                Editor(EnterInsertMode(Direction::Start)),
                App(Dispatch::OpenPrompt {
                    config: super::PromptConfig::new(
                        "".to_string(),
                        PromptOnEnter::ParseCurrentLine {
                            parser: DispatchParser::SetContent,
                            history_key: PromptHistoryKey::Null,
                            current_line: None,
                            suggested_items: ["Patrick", "Spongebob", "Squidward"]
                                .into_iter()
                                .map(|item| item.to_string().into())
                                .collect(),
                        },
                    ),
                }),
                // Expect the completion dropdown to be open,
                Expect(CompletionDropdownContent("Patrick\nSpongebob\nSquidward")),
                // Type in 'pa'
                App(HandleKeyEvents(keys!("p a").to_vec())),
                // Expect only 'Patrick' remains in the completion dropdown
                Expect(CompletionDropdownContent("Patrick")),
                // Clear 'pa' using kill line forward
                App(HandleKeyEvents(keys!("alt+v alt+y release-alt+v").to_vec())),
                // Expect all items are shown again
                Expect(CompletionDropdownContent("Patrick\nSpongebob\nSquidward")),
                //
                //
                // Perform the same test for kill line backward
                App(HandleKeyEvents(keys!("p a").to_vec())),
                Expect(CompletionDropdownContent("Patrick")),
                App(HandleKeyEvents(
                    keys!("alt+y alt+v alt+p release-alt+v").to_vec(),
                )),
                Expect(CompletionDropdownContent("Patrick\nSpongebob\nSquidward")),
            ])
        })
    }

    #[test]
    fn replace_current_query_with_focused_item_should_replace_only_current_line(
    ) -> Result<(), anyhow::Error> {
        execute_test(|s| {
            Box::new([
                App(OpenFile {
                    path: s.main_rs(),
                    owner: BufferOwner::User,
                    focus: true,
                }),
                Editor(SetContent("foo bar spam".to_string())),
                App(OpenSearchPrompt {
                    scope: Scope::Local,
                    if_current_not_found: IfCurrentNotFound::LookForward,
                }),
                App(HandleKeyEvents(keys!("f o enter").to_vec())), // Populate search history with "fo"
                App(OpenSearchPrompt {
                    scope: Scope::Local,
                    if_current_not_found: IfCurrentNotFound::LookForward,
                }),
                Expect(CurrentComponentContent("fo\n")),
                App(HandleKeyEvents(keys!("f o alt+x").to_vec())),
                Expect(CurrentComponentContent("fo\nfoo")),
            ])
        })
    }

    #[test]
    fn using_history_entries() -> Result<(), anyhow::Error> {
        execute_test(|s| {
            Box::new([
                App(OpenFile {
                    path: s.main_rs(),
                    owner: BufferOwner::User,
                    focus: true,
                }),
                Editor(SetContent("foox bar spam fooy".to_string())),
                App(UpdateLocalSearchConfig {
                    update: LocalSearchConfigUpdate::Mode(
                        crate::context::LocalSearchConfigMode::Regex(RegexConfig {
                            escaped: false,
                            case_sensitive: false,
                            match_whole_word: false,
                        }),
                    ),
                    scope: Scope::Local,
                    if_current_not_found: IfCurrentNotFound::LookForward,
                    run_search_after_config_updated: false,
                }),
                App(OpenSearchPrompt {
                    scope: Scope::Local,
                    if_current_not_found: IfCurrentNotFound::LookForward,
                }),
                App(HandleKeyEvents(keys!("f o o . enter").to_vec())), // Populate search history with "foo."
                App(OpenSearchPrompt {
                    scope: Scope::Local,
                    if_current_not_found: IfCurrentNotFound::LookForward,
                }),
                Expect(CurrentComponentContent("foo.\n")),
                // Navigate upwards and use the history
                Editor(EnterNormalMode),
                Editor(MoveSelection(Left)),
                App(HandleKeyEvent(key!("enter"))),
                Expect(CurrentSearch(Scope::Local, "foo.")),
            ])
        })
    }

    #[test]
    fn parse_whole_buffer_auto_add_cursor_to_all_line() -> Result<(), anyhow::Error> {
        execute_test(|s| {
            Box::new([
                App(RevealInExplorer(s.main_rs())),
                Expect(FileExplorerContent(
                    "
 - ../
 - 📁  .git/ :
 - 📂  src/ :
   - 🦀  foo.rs
   - 📘  hello.ts
   - 🦀  main.rs
 - 🙈  .gitignore
 - 🔒  Cargo.lock
 - 📄  Cargo.toml
"
                    .trim_matches('\n')
                    .to_string(),
                )),
                Editor(MatchLiteral("Cargo.lock".to_owned())),
                Editor(SetSelectionMode(IfCurrentNotFound::LookForward, Line)),
                Editor(EnableSelectionExtension),
                Editor(MoveSelection(Right)),
                Expect(CurrentSelectedTexts(&[
                    "- 🔒  Cargo.lock\n - 📄  Cargo.toml",
                ])),
                App(OpenMovePathsPrompt),
                Expect(CurrentMode(Mode::Insert)),
                Expect(CurrentSelectedTexts(&["", ""])),
                Editor(EnterNormalMode),
                Editor(SetSelectionMode(
                    IfCurrentNotFound::LookForward,
                    SelectionMode::Line,
                )),
                Editor(SwapCursor),
                Editor(SetSelectionMode(IfCurrentNotFound::LookForward, Word)),
                Editor(EnableSelectionExtension),
                Editor(MoveSelection(Left)),
                Expect(CurrentSelectedTexts(&["Cargo.lock", "Cargo.toml"])),
            ])
        })
    }
}
