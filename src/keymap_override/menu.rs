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
        _context: &Context,
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

        if key_event.original == key!("space") {
            return Ok(close_dispatches);
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
