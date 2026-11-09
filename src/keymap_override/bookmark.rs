use my_proc_macros::key;
use crate::{
    app::{Dispatch, Dispatches},
    components::{
        editor::DispatchEditor, editor_keymap::CombinedKeyEvent, keymap_legend::KeymapLegendConfig,
    },
    context::Context,
    keymap_override::KeymapOverrideTrait,
};

#[derive(Debug, Clone, PartialEq)]
pub struct BookmarkKeymapOverride {
    config: KeymapLegendConfig,
}

impl BookmarkKeymapOverride {
    pub fn new(config: KeymapLegendConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &KeymapLegendConfig {
        &self.config
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
            if context.has_bookmark(target_char) {
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
