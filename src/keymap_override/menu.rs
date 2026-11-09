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
pub struct MenuKeymapOverride {
    config: KeymapLegendConfig,
}

impl KeymapOverrideTrait for MenuKeymapOverride {
    fn handle_press(
        &mut self,
        context: &Context,
        key_event: CombinedKeyEvent,
    ) -> anyhow::Result<Dispatches> {
        let close_dispatches = Dispatches::from(vec![
            Dispatch::CloseKeymapLegend,
            Dispatch::ToEditor(DispatchEditor::SetKeymapOverride(None)),
        ]);
        if key_event.original == key!("esc") {
            return Ok(close_dispatches);
        }

        if let Some(binding) = self.config.keymap.get(&key_event) {
            return Ok(close_dispatches.chain(binding.get_dispatches()));
        }

        if let Some(binding) = self.config.keymap.iter().find(|b| {
            b.event() == &key_event.original
                || (key_event.original.modifiers.shift
                    && match (b.event().code, key_event.original.code) {
                        (
                            crossterm::event::KeyCode::Char(b_c),
                            crossterm::event::KeyCode::Char(k_c),
                        ) => b_c.to_ascii_uppercase() == k_c.to_ascii_uppercase(),
                        _ => false,
                    })
        }) {
            return Ok(close_dispatches.chain(binding.get_dispatches()));
        }

        if key_event.original == key!("space") {
            return Ok(close_dispatches);
        }

        if self.config.title == "Command" {
            let invocation = context.command_menu_invocation();
            if invocation != crate::config::CommandMenuInvocation::Space
                && (invocation.matches(&key_event.original) || invocation.matches(&key_event.translated))
            {
                return Ok(close_dispatches);
            }
        }

        Ok(Dispatches::default())
    }
}

impl MenuKeymapOverride {
    pub fn new(config: KeymapLegendConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &KeymapLegendConfig {
        &self.config
    }
}
