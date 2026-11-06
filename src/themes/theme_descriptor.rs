use std::{
    collections::HashMap,
    sync::{LazyLock, RwLock},
};

use super::{from_zed_theme, very_dark, vscode_dark, vscode_light, Theme};
use zed_theme::get_zed_themes;

#[derive(Clone, Debug, PartialEq)]
pub struct ThemeDescriptor(String);

impl ThemeDescriptor {
    pub fn name(&self) -> &str {
        &self.0
    }

    pub fn to_theme(&self) -> Theme {
        let theme_map = THEMES.read().unwrap();
        theme_map
            .get(&self.0)
            .expect("Theme descriptor had no matching theme?")
            .clone()
    }
}

impl Default for ThemeDescriptor {
    fn default() -> Self {
        Self("VS Code (Light)".to_string())
    }
}

fn load_themes_from_disk() -> HashMap<String, Theme> {
    let mut themes = HashMap::new();

    // Non-zed builtin themes
    themes.insert("VS Code (Light)".to_string(), vscode_light());
    themes.insert("VS Code (Dark)".to_string(), vscode_dark());
    themes.insert("Very Dark".to_string(), very_dark());

    // Zed builtin themes
    for (name, theme) in get_zed_themes() {
        themes.insert(name, from_zed_theme::from_theme_content(theme));
    }

    // while running the tests we don't want to load the custom user config
    #[cfg(not(test))]
    {
        // Zed ~/.config/themes/ directory loaded themes
        let global_glob_os_string = crate::config::ki_global_directory()
            .join("themes/*.json")
            .into_os_string();
        if let Some(global_glob) = global_glob_os_string.to_str() {
            zed_theme::get_custom_themes(global_glob)
                .into_iter()
                .for_each(|(name, theme)| {
                    themes.insert(name, from_zed_theme::from_theme_content(theme));
                });
        }

        // Zed .ki/themes/ directory loaded themes
        let workspace_glob_os_string = crate::config::ki_workspace_directory()
            .map(|path| path.join("themes/*.json").into_os_string());

        if let Ok(workspace_glob_os_string) = workspace_glob_os_string {
            if let Some(workspace_glob) = workspace_glob_os_string.to_str() {
                zed_theme::get_custom_themes(workspace_glob)
                    .into_iter()
                    .for_each(|(name, theme)| {
                        themes.insert(name, from_zed_theme::from_theme_content(theme));
                    });
            }
        };
    }

    themes
}

static THEMES: LazyLock<RwLock<HashMap<String, Theme>>> =
    LazyLock::new(|| RwLock::new(load_themes_from_disk()));

pub fn reload_themes() {
    let reloaded = load_themes_from_disk();
    if let Ok(mut lock) = THEMES.write() {
        *lock = reloaded;
    }
}

pub fn all() -> Vec<ThemeDescriptor> {
    let lock = THEMES.read().unwrap();
    lock.keys().cloned().map(ThemeDescriptor).collect()
}

#[cfg(test)]
mod test {
    #[test]
    fn test_all_themes_work() {
        for theme_descriptor in super::all() {
            theme_descriptor.to_theme();
        }
    }
}
