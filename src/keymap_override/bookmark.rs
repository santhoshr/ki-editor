use my_proc_macros::key;
use crate::{
    app::{Dispatch, Dispatches},
    components::{
        editor::DispatchEditor, editor_keymap::CombinedKeyEvent, keymap_legend::KeymapLegendConfig,
        suggestive_editor::Info,
    },
    context::Context,
    keymap_override::KeymapOverrideTrait,
};

#[derive(Debug, Clone, PartialEq)]
pub struct BookmarkKeymapOverride {
    config: KeymapLegendConfig,
    current_path: Vec<char>,
    is_quick_marks: bool,
}

impl BookmarkKeymapOverride {
    pub fn new(config: KeymapLegendConfig, current_path: Vec<char>) -> Self {
        Self {
            config,
            current_path,
            is_quick_marks: false,
        }
    }

    pub fn new_quick_marks(config: KeymapLegendConfig) -> Self {
        Self {
            config,
            current_path: Vec::new(),
            is_quick_marks: true,
        }
    }

    pub fn config(&self) -> &KeymapLegendConfig {
        &self.config
    }

    pub fn current_path(&self) -> &[char] {
        &self.current_path
    }
}

impl KeymapOverrideTrait for BookmarkKeymapOverride {
    fn handle_press(
        &mut self,
        context: &Context,
        key_event: CombinedKeyEvent,
    ) -> anyhow::Result<Dispatches> {
        let close_dispatches = Dispatches::from(vec![
            Dispatch::CloseKeymapLegend,
            Dispatch::ToEditor(DispatchEditor::SetKeymapOverride(None)),
        ]);

        if key_event.original == key!("esc") || key_event.original == key!("space") {
            return Ok(close_dispatches);
        }

        if self.is_quick_marks {
            if key_event.original == key!("tab") || key_event.translated == key!("tab") {
                return Ok(close_dispatches.chain(Dispatches::one(Dispatch::OpenQuickMarksFilesPicker)));
            }

            let is_alt = key_event.original.modifiers.alt || key_event.translated.modifiers.alt;
            let is_shift = key_event.original.modifiers.shift || key_event.translated.modifiers.shift;

            let char_opt = match key_event.original.code {
                crossterm::event::KeyCode::Char(c) => Some(c),
                _ => match key_event.translated.code {
                    crossterm::event::KeyCode::Char(c) => Some(c),
                    _ => None,
                },
            };

            if let Some(c) = char_opt {
                if c == ' ' {
                    return Ok(close_dispatches);
                }

                let is_shift_press = is_shift || (c.is_ascii_alphabetic() && c.is_ascii_uppercase());
                let base_key = crate::components::editor_keymap::unshifted_char(c);

                // Alt+key: open editable panel (multi-line buffer in Normal line mode)
                if is_alt {
                    if context.has_single_bookmark(base_key) {
                        return Ok(close_dispatches.chain(Dispatches::one(
                            Dispatch::OpenManageSingleBookmarksPicker(base_key),
                        )));
                    } else {
                        return Ok(close_dispatches.chain(Dispatches::one(
                            Dispatch::ShowEditorInfo(Info::new(
                                "Quick-mark Not Found".to_string(),
                                format!("Quick-mark '{base_key}' is not assigned"),
                            )),
                        )));
                    }
                }

                // Shift+key: show current file path prefilled for addition
                if is_shift_press {
                    return Ok(close_dispatches.chain(Dispatches::one(
                        Dispatch::OpenAssignQuickMarkPrompt {
                            key: base_key,
                            prefill_current_file: true,
                        },
                    )));
                }

                // Normal key activation
                if !context.has_single_bookmark(base_key) {
                    return Ok(close_dispatches.chain(Dispatches::one(
                        Dispatch::OpenAssignQuickMarkPrompt {
                            key: base_key,
                            prefill_current_file: true,
                        },
                    )));
                } else {
                    let count = context.get_single_bookmarks(base_key).map(|b| b.len()).unwrap_or(0);
                    if count > 1 {
                        return Ok(close_dispatches.chain(Dispatches::one(
                            Dispatch::OpenSearchableSingleBookmarkPicker(base_key),
                        )));
                    } else {
                        return Ok(close_dispatches.chain(Dispatches::one(
                            Dispatch::ActivateSingleBookmark(base_key),
                        )));
                    }
                }
            }

            return Ok(Dispatches::default());
        }

        // Nested Bookmarks mode:
        if key_event.original == key!("backspace") {
            return Ok(close_dispatches.chain(Dispatches::one(Dispatch::BookmarkGoBack)));
        }

        if key_event.original == key!("tab") || key_event.translated == key!("tab") {
            return Ok(close_dispatches.chain(Dispatches::one(Dispatch::OpenBookmarkFilesPicker)));
        }

        let is_alt = key_event.original.modifiers.alt || key_event.translated.modifiers.alt;
        let is_shift = key_event.original.modifiers.shift || key_event.translated.modifiers.shift;

        let char_opt = match key_event.original.code {
            crossterm::event::KeyCode::Char(c) => Some(c),
            _ => match key_event.translated.code {
                crossterm::event::KeyCode::Char(c) => Some(c),
                _ => None,
            },
        };

        if let Some(c) = char_opt {
            if c == ' ' {
                return Ok(close_dispatches);
            }

            // If shift is pressed, map to shifted char (e.g. 'a' -> 'A', '1' -> '!')
            let target_char = if is_shift {
                crate::components::editor_keymap::shifted_char(c)
            } else {
                c
            };

            // Alt+key (including Alt+Shift+key): prefill/preselect current file name (or clear if empty)
            if is_alt {
                return Ok(close_dispatches.chain(Dispatches::one(
                    Dispatch::OpenAssignBookmarkPrompt {
                        key: target_char,
                        prefill_current_file: true,
                    },
                )));
            }

            // Normal key or Shift+key:
            if context.has_bookmark_at(&self.current_path, target_char) {
                return Ok(close_dispatches.chain(Dispatches::one(Dispatch::OpenBookmark(target_char))));
            } else {
                // Initial assignment: also prefill and preselect file name
                return Ok(close_dispatches.chain(Dispatches::one(
                    Dispatch::OpenAssignBookmarkPrompt {
                        key: target_char,
                        prefill_current_file: true,
                    },
                )));
            }
        }

        Ok(Dispatches::default())
    }
}
