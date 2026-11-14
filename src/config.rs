use std::borrow::Cow;
use std::io::Read;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use anyhow::Context;
use itertools::Itertools;
use regex::Regex;

use crate::app::StatusLine;
use crate::components::editor_keymap::{builtin_layout_map, KeyboardLayout, KeyboardLayoutKeys};
use crate::scripting::{Keybinding, Script};
use crate::themes::Theme;
use figment::providers;
use figment::providers::Format;
use figment::Provider;
use once_cell::sync::{Lazy, OnceCell};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use shared::absolute_path::AbsolutePath;
use shared::language::{self, Language};

mod lenient;
use lenient::{extract_lenient, Extracted};

pub struct AppConfig {
    languages: HashMap<String, Language>,
    keyboard_layout: KeyboardLayout,
    theme: ConfigTheme,
    status_lines: Vec<StatusLine>,
    leader_keymap: LeaderKeymap,
    keyboard_layouts: HashMap<String, KeyboardLayout>,
    indent_char: char,
    indent_width: usize,
    show_key_in_keymap: bool,
    format_on_save: bool,
    command_menu_invocation: CommandMenuInvocation,
    quick_marks_menu_invocation: QuickMarksMenuInvocation,
    bookmark_menu_invocation: BookmarkMenuInvocation,
    swap_key: SwapKey,
    pick_menu_df_swap: bool,
    e_normal_mode: ENormalMode,
    filetree_open_with_path: bool,
    icon_config: shared::icons::IconsConfig,
    navigation_keys: NavigationKeys,
    hjkl_config: HjklConfig,
    hud_visibility: HudVisibilityConfig,
    load_errors: Vec<String>,
    modeline: bool,
    modelines: usize,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RawConfig {
    languages: HashMap<String, Language>,
    keyboard_layout: String,
    theme: ConfigThemeName,
    status_lines: Vec<StatusLine>,
    leader_keymap: LeaderKeymap,
    #[serde(default)]
    custom_keyboard_layouts: HashMap<String, KeyboardLayoutKeys>,
    indent_char: IndentChar,
    indent_width: usize,
    show_key_in_keymap: bool,
    #[serde(default)]
    format_on_save: bool,
    #[serde(default)]
    command_menu_invocation: CommandMenuInvocation,
    #[serde(default)]
    #[serde(alias = "quick-marks_menu_invocation")]
    quick_marks_menu_invocation: Option<QuickMarksMenuInvocation>,
    #[serde(default)]
    #[serde(alias = "bookmarks_menu_invocation")]
    bookmark_menu_invocation: Option<BookmarkMenuInvocation>,
    #[serde(default)]
    swap_key: SwapKey,
    #[serde(default)]
    pick_menu_df_swap: bool,
    #[serde(default)]
    e_normal_mode: ENormalMode,
    #[serde(default)]
    filetree_open_with_path: bool,
    #[serde(default)]
    icon_style: shared::icons::IconStyle,
    #[serde(default)]
    navigation_keys: NavigationKeys,
    #[serde(default = "default_hjkl_y_semicolon_action")]
    #[serde(rename = "hjkl_y;_action", alias = "hjkl_y_semicolon_action")]
    hjkl_y_semicolon_action: HjklPairAction,
    #[serde(default = "default_hjkl_io_action")]
    hjkl_io_action: HjklPairAction,
    #[serde(default = "default_hjkl_up_action")]
    hjkl_up_action: HjklPairAction,
    #[serde(default)]
    jhlk_np_rl_inverse: bool,
    #[serde(default)]
    empty_hud: Option<String>,
    #[serde(default)]
    allow_hud: Option<String>,
    #[serde(default)]
    modeline: bool,
    #[serde(default = "default_modelines")]
    modelines: usize,
}

fn default_modelines() -> usize {
    5
}

fn default_hjkl_y_semicolon_action() -> HjklPairAction {
    HjklPairAction::Insert
}

fn default_hjkl_io_action() -> HjklPairAction {
    HjklPairAction::Np
}

fn default_hjkl_up_action() -> HjklPairAction {
    HjklPairAction::Fl
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum HudMenu {
    Extend,      // 'f'
    Space,       // 'S'
    Command,     // 'C'
    CopyDup,     // 'c'
    History,     // 'q'
    MarkBuffer,  // 'm', 'B'
    Cut,         // 'R', 'x'
    Swap,        // 't'
    Open,        // 'g'
    UndoRedo,    // 'z'
    Paste,       // 'v'
    MultiCursor, // 'b'
    Find,        // 'n'
    Surround,    // ','
    DeleteEat,   // 'd', 'e'
}

#[derive(Clone, Debug, PartialEq)]
pub enum HudVisibilityConfig {
    /// Default: All HUDs are displayed normally
    Default,
    /// empty_hud: "all"
    EmptyAll,
    /// allow_hud: "all"
    AllowAll,
    /// empty_hud: specific menus
    Empty(HashSet<HudMenu>),
    /// allow_hud: specific menus allowed, rest empty
    Allow(HashSet<HudMenu>),
}

pub fn parse_hud_menus(s: &str) -> HashSet<HudMenu> {
    let mut set = HashSet::new();
    for c in s.chars() {
        match c {
            'f' => {
                set.insert(HudMenu::Extend);
            }
            'S' => {
                set.insert(HudMenu::Space);
            }
            'C' => {
                set.insert(HudMenu::Command);
            }
            'q' => {
                set.insert(HudMenu::History);
            }
            'm' | 'B' => {
                set.insert(HudMenu::MarkBuffer);
            }
            'b' => {
                set.insert(HudMenu::MultiCursor);
            }
            'd' | 'e' => {
                set.insert(HudMenu::DeleteEat);
            }
            'R' | 'x' => {
                set.insert(HudMenu::Cut);
            }
            't' => {
                set.insert(HudMenu::Swap);
            }
            'g' => {
                set.insert(HudMenu::Open);
            }
            'z' => {
                set.insert(HudMenu::UndoRedo);
            }
            'c' => {
                set.insert(HudMenu::CopyDup);
            }
            'v' => {
                set.insert(HudMenu::Paste);
            }
            'n' => {
                set.insert(HudMenu::Find);
            }
            ',' => {
                set.insert(HudMenu::Surround);
            }
            _ => {}
        }
    }
    set
}

pub fn menu_for_title(title: &str) -> Option<HudMenu> {
    if title == "Extend" {
        Some(HudMenu::Extend)
    } else if title == "Space" || title == "Editor" || title == "Pick" || title == "Context" {
        Some(HudMenu::Space)
    } else if title == "Command" {
        Some(HudMenu::Command)
    } else if title == "Copy/≡ Dup" {
        Some(HudMenu::CopyDup)
    } else if title == "≡ Move Hist" || title.contains("Move Hist") {
        Some(HudMenu::History)
    } else if title == "≡ Buffer"
        || title == "≡ Mark"
        || title == "≡ Buffer/Mark"
        || title == "≡ Mark/Buffer"
        || title.contains("Buffer")
        || (title.contains("Mark") && !title.contains("Quick-mark") && !title.contains("Bookmark"))
    {
        Some(HudMenu::MarkBuffer)
    } else if title == "≡ Cut" || title.contains("Cut") {
        Some(HudMenu::Cut)
    } else if title == "≡ Swap" || title.contains("Swap") {
        Some(HudMenu::Swap)
    } else if title == "≡ Open" {
        Some(HudMenu::Open)
    } else if title == "≡ Undo/Redo" || title.contains("Undo/Redo") {
        Some(HudMenu::UndoRedo)
    } else if title == "≡ Paste" || title.contains("Paste") {
        Some(HudMenu::Paste)
    } else if title == "≡ Multi-cursor" || title.contains("Multi-cursor") {
        Some(HudMenu::MultiCursor)
    } else if title.starts_with("Find") || title.contains("QUICKFIX") {
        Some(HudMenu::Find)
    } else if title.contains("Surround") {
        Some(HudMenu::Surround)
    } else if title == "≡ Delete"
        || title == "≡ Eat"
        || title == "≡ Delete/Eat"
        || title.contains("Delete")
        || title.contains("Eat")
    {
        Some(HudMenu::DeleteEat)
    } else {
        None
    }
}

#[derive(Deserialize, Serialize, JsonSchema, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HjklPairAction {
    #[serde(rename = "insert", alias = "Insert", alias = "INSERT")]
    Insert,
    #[serde(rename = "fl", alias = "FL", alias = "first_last", alias = "first-last", alias = "FirstLast")]
    Fl,
    #[serde(rename = "np", alias = "NP", alias = "prev_next", alias = "prev-next", alias = "PrevNext", alias = "next_prev")]
    Np,
}

impl HjklPairAction {
    pub fn start_target(&self) -> char {
        self.start_target_with_inverse(false)
    }

    pub fn end_target(&self) -> char {
        self.end_target_with_inverse(false)
    }

    pub fn start_target_with_inverse(&self, inverse_np: bool) -> char {
        match self {
            HjklPairAction::Insert => 'h',
            HjklPairAction::Np => if inverse_np { 'o' } else { 'u' },
            HjklPairAction::Fl => 'y',
        }
    }

    pub fn end_target_with_inverse(&self, inverse_np: bool) -> char {
        match self {
            HjklPairAction::Insert => ';',
            HjklPairAction::Np => if inverse_np { 'u' } else { 'o' },
            HjklPairAction::Fl => 'p',
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HjklConfig {
    pub y_semicolon_action: HjklPairAction,
    pub io_action: HjklPairAction,
    pub up_action: HjklPairAction,
    pub jhlk_np_rl_inverse: bool,
}

impl Default for HjklConfig {
    fn default() -> Self {
        Self {
            y_semicolon_action: HjklPairAction::Insert,
            io_action: HjklPairAction::Np,
            up_action: HjklPairAction::Fl,
            jhlk_np_rl_inverse: false,
        }
    }
}

#[derive(Deserialize, Serialize, JsonSchema, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NavigationKeys {
    #[default]
    #[serde(rename = "ijkl", alias = "IJKL")]
    Ijkl,
    #[serde(rename = "hjkl", alias = "HJKL")]
    Hjkl,
    #[serde(rename = "jhlk", alias = "JHLK")]
    Jhlk,
}

#[derive(Deserialize, Serialize, JsonSchema, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ENormalMode {
    #[default]
    #[serde(rename = "buffer", alias = "Buffer")]
    Buffer,
    #[serde(rename = "mark", alias = "Mark")]
    Mark,
}

#[derive(Deserialize, Serialize, JsonSchema, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SwapKey {
    #[default]
    #[serde(rename = "/")]
    Slash,
    #[serde(rename = "x", alias = "X")]
    X,
}

#[derive(Deserialize, Serialize, JsonSchema, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CommandMenuInvocation {
    #[default]
    #[serde(rename = "Space", alias = "space")]
    Space,
    #[serde(rename = "c")]
    LowerC,
    #[serde(rename = "C")]
    UpperC,
    #[serde(rename = ":", alias = "colon", alias = "shift+;", alias = "shift+semicolon")]
    ShiftSemicolon,
}

impl CommandMenuInvocation {
    pub fn matches(&self, event: &event::KeyEvent) -> bool {
        match self {
            Self::Space => {
                event.code == crossterm::event::KeyCode::Char(' ')
                    && !event.modifiers.ctrl
                    && !event.modifiers.alt
            }
            Self::LowerC => {
                event.code == crossterm::event::KeyCode::Char('c')
                    && !event.modifiers.shift
                    && !event.modifiers.ctrl
                    && !event.modifiers.alt
            }
            Self::UpperC => {
                (event.code == crossterm::event::KeyCode::Char('C')
                    || (event.code == crossterm::event::KeyCode::Char('c') && event.modifiers.shift))
                    && !event.modifiers.ctrl
                    && !event.modifiers.alt
            }
            Self::ShiftSemicolon => {
                (event.code == crossterm::event::KeyCode::Char(':')
                    || (event.code == crossterm::event::KeyCode::Char(';') && event.modifiers.shift))
                    && !event.modifiers.ctrl
                    && !event.modifiers.alt
            }
        }
    }
}

#[derive(Deserialize, Serialize, JsonSchema, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum QuickMarksMenuInvocation {
    #[default]
    #[serde(rename = ":", alias = "colon", alias = "shift+;", alias = "shift+semicolon")]
    Colon,
    #[serde(rename = "1", alias = "One", alias = "one")]
    One,
}

impl QuickMarksMenuInvocation {
    pub fn matches(&self, event: &event::KeyEvent) -> bool {
        match self {
            Self::Colon => {
                (event.code == crossterm::event::KeyCode::Char(':')
                    || (event.code == crossterm::event::KeyCode::Char(';') && event.modifiers.shift))
                    && !event.modifiers.ctrl
                    && !event.modifiers.alt
            }
            Self::One => {
                event.code == crossterm::event::KeyCode::Char('1')
                    && !event.modifiers.shift
                    && !event.modifiers.ctrl
                    && !event.modifiers.alt
            }
        }
    }

    pub fn display(&self) -> &'static str {
        match self {
            Self::Colon => ":",
            Self::One => "1",
        }
    }
}

#[derive(Deserialize, Serialize, JsonSchema, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BookmarkMenuInvocation {
    #[default]
    #[serde(rename = "B", alias = "b")]
    B,
    #[serde(rename = ":", alias = "colon", alias = "shift+;", alias = "shift+semicolon")]
    Colon,
    #[serde(rename = "1", alias = "One", alias = "one")]
    One,
}

impl BookmarkMenuInvocation {
    pub fn matches(&self, event: &event::KeyEvent) -> bool {
        match self {
            Self::B => {
                (event.code == crossterm::event::KeyCode::Char('B')
                    || (event.code == crossterm::event::KeyCode::Char('b') && event.modifiers.shift))
                    && !event.modifiers.ctrl
                    && !event.modifiers.alt
            }
            Self::Colon => {
                (event.code == crossterm::event::KeyCode::Char(':')
                    || (event.code == crossterm::event::KeyCode::Char(';') && event.modifiers.shift))
                    && !event.modifiers.ctrl
                    && !event.modifiers.alt
            }
            Self::One => {
                event.code == crossterm::event::KeyCode::Char('1')
                    && !event.modifiers.shift
                    && !event.modifiers.ctrl
                    && !event.modifiers.alt
            }
        }
    }

    pub fn display(&self) -> &'static str {
        match self {
            Self::B => "B",
            Self::Colon => ":",
            Self::One => "1",
        }
    }
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum IndentChar {
    Space,
    Tab,
}

const DEFAULT_CONFIG: &str = include_str!("config_default.json");

/// A JSON [`providers::Format`] backed by the [`json5`] parser, so
/// user-facing `config.json` files can contain `//` and `/* */` comments
/// (and other JSON5 conveniences like trailing commas).
struct Json5;

impl providers::Format for Json5 {
    type Error = json5::Error;

    const NAME: &'static str = "JSON";

    fn from_str<T: serde::de::DeserializeOwned>(string: &str) -> Result<T, Self::Error> {
        json5::from_str(string)
    }
}

impl Default for RawConfig {
    fn default() -> Self {
        RawConfig {
            languages: shared::languages::languages(),
            ..serde_json::from_str(DEFAULT_CONFIG)
                .expect("Default config doesn't parse, this is a bug!")
        }
    }
}

impl TryFrom<RawConfig> for AppConfig {
    type Error = anyhow::Error;
    fn try_from(value: RawConfig) -> Result<Self, Self::Error> {
        let (hjkl_y_semicolon_action, hjkl_io_action, hjkl_up_action, collision_errors) = {
            let y_act = value.hjkl_y_semicolon_action;
            let io_act = value.hjkl_io_action;
            let up_act = value.hjkl_up_action;
            if y_act == io_act || y_act == up_act || io_act == up_act {
                let msg = format!(
                    "Collision in HJKL key action mappings: 'hjkl_y;_action' is {y_act:?}, 'hjkl_io_action' is {io_act:?}, 'hjkl_up_action' is {up_act:?}. Reverting all to default (y;=insert, io=np, up=fl)."
                );
                (
                    HjklPairAction::Insert,
                    HjklPairAction::Np,
                    HjklPairAction::Fl,
                    vec![msg],
                )
            } else {
                (y_act, io_act, up_act, Vec::new())
            }
        };
        let hjkl_config = HjklConfig {
            y_semicolon_action: hjkl_y_semicolon_action,
            io_action: hjkl_io_action,
            up_action: hjkl_up_action,
            jhlk_np_rl_inverse: value.jhlk_np_rl_inverse,
        };

        let keyboard_layouts: HashMap<_, _> = builtin_layout_map()
            .into_iter()
            .chain(
                value
                    .custom_keyboard_layouts
                    .into_iter()
                    .map(|(name, keys)| (name.clone(), KeyboardLayout::new(name, keys))),
            )
            .map(|(name, mut layout)| {
                layout.set_navigation_keys(value.navigation_keys);
                layout.set_hjkl_config(hjkl_config);
                (name, layout)
            })
            .collect();
        let keyboard_layout = keyboard_layouts
            .get(&value.keyboard_layout)
            .context(format!(
                "Unknown keyboard layout {}, possible values are {:?}",
                value.keyboard_layout,
                keyboard_layouts.keys().collect::<Vec<_>>()
            ))?
            .clone();
        let (quick_marks_menu_invocation, bookmark_menu_invocation, mut load_errors) = {
            if value.command_menu_invocation == CommandMenuInvocation::ShiftSemicolon {
                let mut errs = Vec::new();
                if value.quick_marks_menu_invocation == Some(QuickMarksMenuInvocation::Colon)
                    || value.bookmark_menu_invocation == Some(BookmarkMenuInvocation::Colon)
                {
                    errs.push("Command menu is configured for ':'; Quick-marks assigned '1' and Bookmarks assigned 'B'.".to_string());
                }
                (QuickMarksMenuInvocation::One, BookmarkMenuInvocation::B, errs)
            } else {
                let desired_quick = value.quick_marks_menu_invocation.unwrap_or(QuickMarksMenuInvocation::Colon);
                let desired_bookmark = value.bookmark_menu_invocation.unwrap_or(BookmarkMenuInvocation::B);

                match (desired_quick, desired_bookmark) {
                    (QuickMarksMenuInvocation::Colon, BookmarkMenuInvocation::Colon) => {
                        (
                            QuickMarksMenuInvocation::Colon,
                            BookmarkMenuInvocation::One,
                            vec!["Both Quick-marks and Bookmarks configured for ':'; Quick-marks gets ':' and Bookmarks gets '1'.".to_string()],
                        )
                    }
                    (QuickMarksMenuInvocation::One, BookmarkMenuInvocation::One) => {
                        (
                            QuickMarksMenuInvocation::One,
                            BookmarkMenuInvocation::Colon,
                            vec!["Both Quick-marks and Bookmarks configured for '1'; Quick-marks gets '1' and Bookmarks gets ':'.".to_string()],
                        )
                    }
                    _ => (desired_quick, desired_bookmark, Vec::new()),
                }
            }
        };
        load_errors.extend(collision_errors);

        let hud_visibility = match (value.empty_hud.as_deref(), value.allow_hud.as_deref()) {
            (Some(_), Some(_)) => {
                load_errors.push(
                    "Both 'empty_hud' and 'allow_hud' are specified in config; both settings will be ignored."
                        .to_string(),
                );
                HudVisibilityConfig::Default
            }
            (Some(empty), None) => {
                if empty.trim().eq_ignore_ascii_case("all") {
                    HudVisibilityConfig::EmptyAll
                } else {
                    HudVisibilityConfig::Empty(parse_hud_menus(empty))
                }
            }
            (None, Some(allow)) => {
                if allow.trim().eq_ignore_ascii_case("all") {
                    HudVisibilityConfig::AllowAll
                } else {
                    HudVisibilityConfig::Allow(parse_hud_menus(allow))
                }
            }
            (None, None) => HudVisibilityConfig::Default,
        };

        Ok(Self {
            languages: value.languages,
            keyboard_layout: keyboard_layout.clone(),
            theme: ConfigTheme::try_from(value.theme)?,
            status_lines: value.status_lines,
            leader_keymap: value.leader_keymap,
            keyboard_layouts,
            indent_char: match value.indent_char {
                IndentChar::Space => ' ',
                IndentChar::Tab => '\t',
            },
            indent_width: value.indent_width,
            show_key_in_keymap: value.show_key_in_keymap,
            format_on_save: value.format_on_save,
            command_menu_invocation: value.command_menu_invocation,
            quick_marks_menu_invocation,
            bookmark_menu_invocation,
            swap_key: value.swap_key,
            pick_menu_df_swap: value.pick_menu_df_swap,
            e_normal_mode: value.e_normal_mode,
            filetree_open_with_path: value.filetree_open_with_path,
            icon_config: shared::icons::build_icon_config(&value.icon_style),
            navigation_keys: value.navigation_keys,
            hjkl_config,
            hud_visibility,
            load_errors,
            modeline: value.modeline,
            modelines: if value.modelines == 0 { 5 } else { value.modelines },
        })
    }
}

/// The leader keymap is a 3x10 matrix representing three rows of 10 columns.
///
/// Assuming the keyboard layout is Qwerty, then:  
///
/// 1st row is "qwertyuiop",  
/// 2nd row is "asdfghjkl;",  
/// and 3rd row is "zxcvbnm,./".  
#[derive(Clone, Deserialize, Serialize, JsonSchema)]
pub struct LeaderKeymap([[Option<Keybinding>; 10]; 3]);
impl LeaderKeymap {
    pub fn keybindings(&self) -> &[[Option<Keybinding>; 10]; 3] {
        &self.0
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(transparent)]
struct ConfigThemeName(String);

#[derive(Clone)]
struct ConfigTheme(Theme);

impl TryFrom<ConfigThemeName> for ConfigTheme {
    type Error = anyhow::Error;

    fn try_from(value: ConfigThemeName) -> Result<Self, Self::Error> {
        crate::themes::from_name(&value.0).map(Self)
    }
}

impl From<ConfigTheme> for ConfigThemeName {
    fn from(value: ConfigTheme) -> Self {
        Self(value.0.name)
    }
}

impl JsonSchema for ConfigThemeName {
    fn schema_name() -> Cow<'static, str> {
        "Theme".into()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        let valid_themes: Vec<_> = crate::themes::theme_descriptor::all()
            .into_iter()
            .map(|descriptor| descriptor.name().to_string())
            .sorted()
            .collect();

        schemars::json_schema!({
            "type": "string",
            "enum": valid_themes
        })
    }
}

pub fn ki_workspace_directory() -> anyhow::Result<PathBuf> {
    Ok(std::env::current_dir()?.join(".ki"))
}

pub fn ki_global_directory() -> PathBuf {
    ::grammar::config_dir()
}

/// A config file, in one of the formats Ki understands. Unifies the three
/// concrete `figment` provider types into one so a directory's providers can
/// be folded over as a homogeneous list.
enum ConfigFile {
    Json5(figment::providers::Data<Json5>),
    Yaml(figment::providers::Data<providers::Yaml>),
    Toml(figment::providers::Data<providers::Toml>),
}

impl figment::Provider for ConfigFile {
    fn metadata(&self) -> figment::Metadata {
        match self {
            ConfigFile::Json5(provider) => provider.metadata(),
            ConfigFile::Yaml(provider) => provider.metadata(),
            ConfigFile::Toml(provider) => provider.metadata(),
        }
    }

    fn data(
        &self,
    ) -> Result<figment::value::Map<figment::Profile, figment::value::Dict>, figment::Error> {
        match self {
            ConfigFile::Json5(provider) => provider.data(),
            ConfigFile::Yaml(provider) => provider.data(),
            ConfigFile::Toml(provider) => provider.data(),
        }
    }
}

/// Merges `provider` into `figment` only if it parses cleanly, returning an
/// error message instead if it doesn't. A file with a syntax error (invalid
/// JSON/YAML/TOML) is excluded from the merge instead of being included and
/// later poisoning the whole merged figment, so that a broken config file
/// only takes down its own contents rather than every other config source
/// (e.g. a broken workspace config no longer discards an otherwise-valid
/// global config, and vice versa).
struct MergeValid {
    figment: figment::Figment,
    error: Option<String>,
}

fn merge_valid(figment: figment::Figment, provider: ConfigFile) -> MergeValid {
    match provider.data() {
        Ok(_) => MergeValid {
            figment: figment.merge(provider),
            error: None,
        },
        Err(error) => {
            let metadata = provider.metadata();
            let source = metadata
                .source
                .as_ref()
                .map(|source| source.to_string())
                .unwrap_or_else(|| metadata.name.to_string());
            MergeValid {
                figment,
                error: Some(format!(
                    "{source} could not be parsed (invalid syntax), skipping this file: {error}"
                )),
            }
        }
    }
}

struct MergeAllValid {
    figment: figment::Figment,
    errors: Vec<String>,
}

/// Folds [`merge_valid`] over `providers`, collecting the resulting figment
/// alongside every error message produced along the way.
fn merge_all_valid(figment: figment::Figment, providers: Vec<ConfigFile>) -> MergeAllValid {
    providers.into_iter().fold(
        MergeAllValid {
            figment,
            errors: Vec::new(),
        },
        |acc, provider| {
            let result = merge_valid(acc.figment, provider);
            MergeAllValid {
                figment: result.figment,
                errors: acc.errors.into_iter().chain(result.error).collect(),
            }
        },
    )
}

pub fn load_script(script_name: &str) -> anyhow::Result<Script> {
    // Trying reading from workspace directory first
    let workspace_path = ki_workspace_directory()?.join("scripts").join(script_name);
    let global_path = ki_global_directory().join("scripts").join(script_name);
    if workspace_path.exists() {
        Ok(Script {
            path: workspace_path.try_into().unwrap(),
            name: script_name.to_string(),
        })
    }
    // Then try reading from global directory
    else if global_path.exists() {
        Ok(Script {
            path: global_path.try_into().unwrap(),
            name: script_name.to_string(),
        })
    } else {
        Err(anyhow::anyhow!(
            "Unable to find script {script_name:?} in both {} and {}",
            workspace_path.display(),
            global_path.display()
        ))
    }
}

impl AppConfig {
    fn default() -> Self {
        RawConfig::default()
            .try_into()
            .expect("Default config can't be converted to an AppConfig, this is a bug!")
    }

    /// Loads the config leniently: a malformed or missing value for any
    /// individual field (or language entry) falls back to its default
    /// instead of failing the entire config, so that Ki always starts up.
    /// Anything that couldn't be parsed is recorded in [`AppConfig::load_errors`];
    /// [`AppConfig::singleton`] is responsible for surfacing it to the user.
    ///
    /// Global and workspace config files are each validated for syntax
    /// independently before being merged in: a syntax error in one file (e.g.
    /// the workspace config) only drops that file, so the other (e.g. the
    /// global config) still applies instead of the whole config collapsing
    /// to hardcoded defaults.
    pub fn load_from_current_directory() -> Self {
        let (workspace_dir, workspace_dir_error) = match ki_workspace_directory() {
            Ok(dir) => (Some(dir), None),
            Err(error) => (
                None,
                Some(format!("Failed to determine workspace directory: {error}")),
            ),
        };

        let figment =
            figment::Figment::from(providers::Serialized::defaults(&RawConfig::default()));

        #[cfg(not(test))]
        let (figment, global_errors) = {
            let global_config =
                |extension: &str| ki_global_directory().join(format!("config.{extension}"));
            let result = merge_all_valid(
                figment,
                vec![
                    ConfigFile::Json5(Json5::file(global_config("json"))),
                    ConfigFile::Yaml(providers::Yaml::file(global_config("yaml"))),
                    ConfigFile::Toml(providers::Toml::file(global_config("toml"))),
                ],
            );
            (result.figment, result.errors)
        };
        #[cfg(test)]
        let global_errors: Vec<String> = Vec::new();

        let (figment, workspace_errors) = if let Some(workspace_dir) = &workspace_dir {
            let workspace_config =
                |extension: &str| workspace_dir.join(format!("config.{extension}"));
            let result = merge_all_valid(
                figment,
                vec![
                    ConfigFile::Json5(Json5::file(workspace_config("json"))),
                    ConfigFile::Yaml(providers::Yaml::file(workspace_config("yaml"))),
                    ConfigFile::Toml(providers::Toml::file(workspace_config("toml"))),
                ],
            );
            (result.figment, result.errors)
        } else {
            (figment, Vec::new())
        };

        let Extracted {
            value: raw_config,
            errors: field_errors,
        } = extract_lenient(&figment);

        let mut conflict_error = None;
        if raw_config.command_menu_invocation == CommandMenuInvocation::ShiftSemicolon
            && raw_config.quick_marks_menu_invocation == Some(QuickMarksMenuInvocation::Colon)
        {
            conflict_error = Some("Both 'command_menu_invocation' and 'quick-marks_menu_invocation' are configured for ':'. Defaulting quick-marks to '1' and using ':' for Command menu.".to_string());
        }

        let (app_config, apply_error) = match AppConfig::try_from(raw_config) {
            Ok(app_config) => (app_config, None),
            Err(error) => (
                AppConfig::default(),
                Some(format!("Failed to apply config, using defaults: {error}")),
            ),
        };

        let load_errors = workspace_dir_error
            .into_iter()
            .chain(global_errors)
            .chain(workspace_errors)
            .chain(field_errors)
            .chain(apply_error)
            .chain(conflict_error)
            .chain(app_config.load_errors)
            .collect();

        AppConfig {
            load_errors,
            ..app_config
        }
    }

    pub fn singleton() -> &'static AppConfig {
        static INSTANCE: OnceCell<AppConfig> = OnceCell::new();
        INSTANCE.get_or_init(|| {
            let app_config = AppConfig::load_from_current_directory();
            if !app_config.load_errors.is_empty() {
                eprintln!(
                    "Ki config warning:\n{}",
                    app_config.load_errors.join("\n\n")
                );
                println!("\n[Press any key to continue]");
                // Best-effort: if stdin isn't interactive (e.g. under a test
                // harness or piped input), don't block startup on it.
                let _ = std::io::stdin().read(&mut [0u8]);
            }
            app_config
        })
    }

    /// Human-readable descriptions of any config values that failed to parse
    /// and were replaced by their defaults during [`AppConfig::load_from_current_directory`].
    pub fn load_errors(&self) -> &[String] {
        &self.load_errors
    }

    pub fn languages(&self) -> &HashMap<std::string::String, language::Language> {
        &self.languages
    }

    pub fn keyboard_layout(&self) -> &KeyboardLayout {
        &self.keyboard_layout
    }

    pub fn keyboard_layouts(&self) -> &HashMap<String, KeyboardLayout> {
        &self.keyboard_layouts
    }

    pub fn theme(&self) -> &Theme {
        &self.theme.0
    }

    pub fn status_lines(&self) -> Vec<crate::app::StatusLine> {
        self.status_lines.clone()
    }

    pub fn leader_keymap(&self) -> &LeaderKeymap {
        &self.leader_keymap
    }

    pub fn indent_width(&self) -> usize {
        self.indent_width
    }

    pub fn indent_char(&self) -> char {
        self.indent_char
    }

    pub fn show_key_in_keymap(&self) -> bool {
        self.show_key_in_keymap
    }

    pub fn format_on_save(&self) -> bool {
        self.format_on_save
    }

    pub fn command_menu_invocation(&self) -> CommandMenuInvocation {
        self.command_menu_invocation
    }

    pub fn quick_marks_menu_invocation(&self) -> QuickMarksMenuInvocation {
        self.quick_marks_menu_invocation
    }

    pub fn bookmark_menu_invocation(&self) -> BookmarkMenuInvocation {
        self.bookmark_menu_invocation
    }

    pub fn swap_key(&self) -> SwapKey {
        self.swap_key
    }

    pub fn pick_menu_df_swap(&self) -> bool {
        self.pick_menu_df_swap
    }

    pub fn e_normal_mode(&self) -> ENormalMode {
        self.e_normal_mode
    }

    pub fn filetree_open_with_path(&self) -> bool {
        self.filetree_open_with_path
    }

    pub fn icon_config(&self) -> &shared::icons::IconsConfig {
        &self.icon_config
    }

    pub fn navigation_keys(&self) -> NavigationKeys {
        self.navigation_keys
    }

    pub fn hjkl_config(&self) -> HjklConfig {
        self.hjkl_config
    }

    pub fn hud_visibility(&self) -> &HudVisibilityConfig {
        &self.hud_visibility
    }

    pub fn is_empty_hud(&self, title: &str) -> bool {
        match &self.hud_visibility {
            HudVisibilityConfig::Default => false,
            HudVisibilityConfig::EmptyAll => true,
            HudVisibilityConfig::AllowAll => false,
            HudVisibilityConfig::Empty(set) => {
                if let Some(menu) = menu_for_title(title) {
                    set.contains(&menu)
                } else {
                    false
                }
            }
            HudVisibilityConfig::Allow(set) => {
                if let Some(menu) = menu_for_title(title) {
                    !set.contains(&menu)
                } else {
                    true
                }
            }
        }
    }

    pub fn modeline(&self) -> bool {
        self.modeline
    }

    pub fn modelines(&self) -> usize {
        self.modelines
    }
}

pub fn from_path(path: &AbsolutePath) -> Option<Language> {
    path.extension()
        .and_then(from_extension)
        .or_else(|| from_filename(path))
}

pub fn from_extension(extension: &str) -> Option<Language> {
    AppConfig::singleton()
        .languages()
        .iter()
        .find(|(_, language)| language.extensions().contains(&extension.to_string()))
        .map(|(_, language)| (*language).clone())
}

pub fn from_filename(path: &AbsolutePath) -> Option<Language> {
    let file_name = path.file_name()?;
    AppConfig::singleton()
        .languages()
        .iter()
        .find(|(_, language)| language.file_names().contains(&file_name))
        .map(|(_, language)| (*language).clone())
}

static KI_MODELINE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)\bki\s*:\s*lang\s*=\s*['"]?([a-zA-Z0-9_#+\-]+)['"]?"#).unwrap()
});

static OTHER_MODELINE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)\b(?:mode:\s*|ft\s*=\s*)['"]?([a-zA-Z0-9_#+\-]+)['"]?"#).unwrap()
});

static SHEBANG_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)^#!.*/(?:env\s+)?['"]?([a-zA-Z0-9_#+\-]+)['"]?"#).unwrap()
});

pub fn resolve_language(id: &str) -> Option<Language> {
    let raw = id.trim();
    let lower = raw.to_ascii_lowercase();

    // Map common aliases
    let normalized = match lower.as_str() {
        "c++" | "cpp" | "cxx" | "cc" => "cpp",
        "c#" | "cs" | "csharp" => "c_sharp",
        "f#" | "fs" => "fsharp",
        "py" | "python3" => "python",
        "rs" => "rust",
        "js" => "javascript",
        "ts" => "typescript",
        "sh" => "bash",
        "md" => "markdown",
        _ => lower.as_str(),
    };

    let languages = AppConfig::singleton().languages();

    // 1. Direct key match (e.g. "rust", "python", "cpp")
    if let Some(lang) = languages.get(normalized) {
        return Some(lang.clone());
    }

    // 2. Case-insensitive key match
    if let Some((_, lang)) = languages
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(normalized) || key.eq_ignore_ascii_case(&lower))
    {
        return Some(lang.clone());
    }

    // 3. Tree-sitter grammar id match
    if let Some((_, lang)) = languages.iter().find(|(_, lang)| {
        lang.tree_sitter_grammar_id()
            .is_some_and(|g| g.eq_ignore_ascii_case(normalized) || g.eq_ignore_ascii_case(&lower))
    }) {
        return Some(lang.clone());
    }

    // 4. LSP language ID match
    if let Some((_, lang)) = languages.iter().find(|(_, lang)| {
        lang.lsp_language_id().as_ref().is_some_and(|lsp| {
            lsp.to_string().eq_ignore_ascii_case(normalized)
                || lsp.to_string().eq_ignore_ascii_case(&lower)
        })
    }) {
        return Some(lang.clone());
    }

    // 5. Extensions match (e.g. if someone writes "rs" or "py" or "ts" directly)
    if let Some((_, lang)) = languages.iter().find(|(_, lang)| {
        lang.extensions().iter().any(|ext| {
            ext.eq_ignore_ascii_case(normalized) || ext.eq_ignore_ascii_case(&lower)
        })
    }) {
        return Some(lang.clone());
    }

    None
}

fn get_candidate_lines<'a>(content: &'a str, n: usize) -> Vec<&'a str> {
    let n = if n == 0 { 5 } else { n };
    let mut first_lines = Vec::with_capacity(n);
    for line in content.lines().take(n) {
        first_lines.push(line);
    }
    if first_lines.len() < n {
        return first_lines;
    }
    let total_lines = content.lines().count();
    if total_lines <= n * 2 {
        content.lines().collect()
    } else {
        first_lines.reserve(n);
        for line in content.lines().skip(total_lines - n) {
            first_lines.push(line);
        }
        first_lines
    }
}

pub fn from_modeline(content: &str, modelines: usize) -> Option<Language> {
    let candidates = get_candidate_lines(content, modelines);

    // 1. Check for ki: lang=... directives first
    for &line in &candidates {
        if let Some(captures) = KI_MODELINE_REGEX.captures(line) {
            if let Some(m) = captures.get(1) {
                if let Some(lang) = resolve_language(m.as_str()) {
                    return Some(lang);
                }
            }
        }
    }

    // 2. Check for other editor modelines (mode: or ft=)
    for &line in &candidates {
        if let Some(captures) = OTHER_MODELINE_REGEX.captures(line) {
            if let Some(m) = captures.get(1) {
                if let Some(lang) = resolve_language(m.as_str()) {
                    return Some(lang);
                }
            }
        }
    }

    // 3. Check for shebang on first line
    if let Some(&first_line) = candidates.first() {
        if let Some(captures) = SHEBANG_REGEX.captures(first_line) {
            if let Some(m) = captures.get(1) {
                if let Some(lang) = resolve_language(m.as_str()) {
                    return Some(lang);
                }
            }
        }
    }

    None
}

pub fn parse_modeline_directive_from_line(line: &str) -> Option<Language> {
    if let Some(captures) = KI_MODELINE_REGEX.captures(line) {
        if let Some(m) = captures.get(1) {
            if let Some(lang) = resolve_language(m.as_str()) {
                return Some(lang);
            }
        }
    }
    if let Some(captures) = OTHER_MODELINE_REGEX.captures(line) {
        if let Some(m) = captures.get(1) {
            if let Some(lang) = resolve_language(m.as_str()) {
                return Some(lang);
            }
        }
    }
    if let Some(captures) = SHEBANG_REGEX.captures(line) {
        if let Some(m) = captures.get(1) {
            if let Some(lang) = resolve_language(m.as_str()) {
                return Some(lang);
            }
        }
    }
    None
}

/// Detect the language from the first line of the file content.
///
/// Standard shebang format is checked as well as vim's `ft=` method and various
/// other editors supporting `mode:`.
///
/// For example, a file opened that has any of the following first lines will be
/// detected as bash.
///
/// - `#!/bin/bash`
/// - `# vim: ft=bash`
/// - `# mode: bash`
///
/// Spaces and other content on the line do not matter.
pub fn from_content_directive(content: &str) -> Option<Language> {
    let first_line = content.lines().next()?;
    parse_modeline_directive_from_line(first_line)
}

pub fn detect_language(path: Option<&AbsolutePath>, content: &str) -> Option<Language> {
    let app_config = AppConfig::singleton();
    if app_config.modeline() {
        if let Some(language) = from_modeline(content, app_config.modelines()) {
            return Some(language);
        }
    }

    if let Some(path) = path {
        if let Some(language) = from_path(path) {
            return Some(language);
        }
    }

    from_content_directive(content)
}

#[cfg(test)]
mod test_language {
    use super::*;
    use std::fs::File;
    #[test]
    fn test_from_path() -> anyhow::Result<()> {
        fn run_test_case(filename: &str, expected_language_id: &'static str) -> anyhow::Result<()> {
            let tempdir = tempfile::tempdir()?;
            let path = tempdir.path().join(filename);
            File::create(path.clone())?;
            let result = from_path(&path.to_string_lossy().to_string().try_into()?).unwrap();
            assert_eq!(
                result.tree_sitter_grammar_id().unwrap(),
                expected_language_id
            );
            Ok(())
        }
        run_test_case("hello.rs", "rust")?;
        run_test_case("justfile", "just")?;
        Ok(())
    }

    #[test]
    fn test_from_content_directive() -> anyhow::Result<()> {
        fn run_test_case(content: &str, expected_language_id: &'static str) -> anyhow::Result<()> {
            let result = from_content_directive(content).unwrap();
            assert_eq!(
                result.tree_sitter_grammar_id().unwrap(),
                expected_language_id
            );
            Ok(())
        }

        run_test_case("#!/bin/bash", "bash")?;
        run_test_case("#!/usr/local/bin/bash", "bash")?;
        run_test_case("// mode: python", "python")?;
        run_test_case("-- tab_spaces: 5, mode: bash, use_tabs: false", "bash")?;
        run_test_case("-- tab_spaces: 5, mode:bash, use_tabs: false", "bash")?;
        run_test_case("-- vim: ft = bash", "bash")?;
        run_test_case("// ki: lang=rust", "rust")?;
        run_test_case("# ki : lang = python", "python")?;
        run_test_case("/* ki: lang = cpp */", "cpp")?;
        run_test_case("// ki : lang = 'c_sharp'", "c_sharp")?;
        run_test_case("<!-- ki: lang=html -->", "html")?;

        Ok(())
    }

    #[test]
    fn test_from_modeline() -> anyhow::Result<()> {
        let content_top = "// ki: lang=rust\nfn main() {}\n";
        assert_eq!(
            from_modeline(content_top, 5).unwrap().tree_sitter_grammar_id().unwrap(),
            "rust"
        );

        let content_bottom = "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\n// ki: lang=python\n";
        assert_eq!(
            from_modeline(content_bottom, 5).unwrap().tree_sitter_grammar_id().unwrap(),
            "python"
        );

        let content_flexible = "/* ki : lang = c++ */\nint main() {}";
        assert_eq!(
            from_modeline(content_flexible, 5).unwrap().tree_sitter_grammar_id().unwrap(),
            "cpp"
        );

        let content_invalid = "// ki: lang=nonexistent_xyz\nfn main() {}\n";
        assert!(from_modeline(content_invalid, 5).is_none());

        Ok(())
    }
}

mod test_config {
    #[test]
    /// This test case is necessary to prevent invalid `config_default.json`
    /// from being committed to the master branch.
    fn default_app_config_should_be_construtable() {
        super::AppConfig::default();
    }

    #[test]
    fn doc_assets_default_config_json() {
        let path = "docs/static/config_default.json";
        std::fs::write(path, super::DEFAULT_CONFIG).unwrap();
    }

    /// Regression test: an empty (e.g. freshly auto-created) workspace config
    /// file must never fail startup or block on stdin; it should be treated
    /// as "no config" and reported as a single collapsed error.
    #[test]
    fn empty_config_file_falls_back_to_defaults() {
        let tempdir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tempdir.path().join(".ki")).unwrap();
        std::fs::write(tempdir.path().join(".ki/config.json"), "").unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(tempdir.path()).unwrap();
        let config = super::AppConfig::load_from_current_directory();
        std::env::set_current_dir(original_dir).unwrap();

        assert_eq!(config.load_errors().len(), 1);
        assert!(config.load_errors()[0].contains(".ki/config.json"));
        assert_eq!(
            config.indent_width(),
            super::AppConfig::default().indent_width()
        );
    }

    /// Regression test: a syntax error in one config file (here, a broken
    /// workspace `config.json`) must not discard the values contributed by
    /// another, syntactically valid config file (here, `config.toml`) merged
    /// into the same figment. This is the same mechanism that lets a broken
    /// workspace config fall back to an otherwise-valid global config instead
    /// of collapsing the whole config to hardcoded defaults.
    #[test]
    fn broken_file_does_not_discard_another_valid_files_values() {
        let tempdir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tempdir.path().join(".ki")).unwrap();
        std::fs::write(tempdir.path().join(".ki/config.json"), "{ not valid json").unwrap();
        std::fs::write(tempdir.path().join(".ki/config.toml"), "indent_width = 7").unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(tempdir.path()).unwrap();
        let config = super::AppConfig::load_from_current_directory();
        std::env::set_current_dir(original_dir).unwrap();

        assert_eq!(config.load_errors().len(), 1);
        assert!(config.load_errors()[0].contains(".ki/config.json"));
        assert_eq!(config.indent_width(), 7);
    }

    /// A malformed field and a malformed language entry should each fall back
    /// to their own default independently, instead of the whole config (or
    /// even the whole `languages` map) failing.
    #[test]
    fn malformed_field_and_language_fall_back_individually() {
        let tempdir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tempdir.path().join(".ki")).unwrap();
        std::fs::write(
            tempdir.path().join(".ki/config.json"),
            r#"{"indent_width": "two", "languages": {"rust": {"lsp_command": "not-an-object"}}}"#,
        )
        .unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(tempdir.path()).unwrap();
        let config = super::AppConfig::load_from_current_directory();
        std::env::set_current_dir(original_dir).unwrap();

        assert_eq!(config.load_errors().len(), 2);
        assert_eq!(
            config.indent_width(),
            super::AppConfig::default().indent_width()
        );
        assert!(!config.languages().is_empty());
        assert!(config
            .load_errors()
            .iter()
            .all(|error| error.contains(".ki/config.json")));
    }

    /// `config.json` is allowed to contain `//` and `/* */` comments (JSONC),
    /// since that's convenient for annotating settings.
    #[test]
    fn config_json_allows_comments() {
        let tempdir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tempdir.path().join(".ki")).unwrap();
        std::fs::write(
            tempdir.path().join(".ki/config.json"),
            r#"{
                // line comment
                "indent_width": 7 /* inline comment */
            }"#,
        )
        .unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(tempdir.path()).unwrap();
        let config = super::AppConfig::load_from_current_directory();
        std::env::set_current_dir(original_dir).unwrap();

        assert!(config.load_errors().is_empty());
        assert_eq!(config.indent_width(), 7);
    }

    /// A malformed field *within* a language override (e.g. `formatter`)
    /// should only reset that field, not drop the whole language override.
    #[test]
    fn malformed_language_field_falls_back_individually() {
        let tempdir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tempdir.path().join(".ki")).unwrap();
        std::fs::write(
            tempdir.path().join(".ki/config.json"),
            r#"{"languages": {"javascript": {
                "formatter": {"command": "prettier", "arguments": "format --stdin-file-path=_.js"},
                "line_comment_prefix": "//"
            }}}"#,
        )
        .unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(tempdir.path()).unwrap();
        let config = super::AppConfig::load_from_current_directory();
        std::env::set_current_dir(original_dir).unwrap();

        assert_eq!(config.load_errors().len(), 1);
        assert!(config.load_errors()[0].contains("languages.javascript.formatter.arguments"));
        assert!(config.load_errors()[0].contains(".ki/config.json"));

        let javascript = config.languages().get("javascript").unwrap();
        // `line_comment_prefix` was valid and should still be applied...
        assert_eq!(javascript.line_comment_prefix(), Some("//".to_string()));
        // ...while `formatter` should fall back to the builtin default
        // (which is `Some`), instead of the whole language override being
        // discarded.
        assert!(javascript.formatter().is_some());
    }

    /// One malformed status line (e.g. a typo'd component name) should only
    /// drop that line, not reset `status_lines` to the default set.
    #[test]
    fn malformed_status_line_falls_back_individually() {
        let tempdir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tempdir.path().join(".ki")).unwrap();
        std::fs::write(
            tempdir.path().join(".ki/config.json"),
            r#"{"status_lines": [
                {"components": ["Mode"]},
                {"components": ["NotAComponent"]}
            ]}"#,
        )
        .unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(tempdir.path()).unwrap();
        let config = super::AppConfig::load_from_current_directory();
        std::env::set_current_dir(original_dir).unwrap();

        assert_eq!(config.load_errors().len(), 1);
        assert!(config.load_errors()[0].contains("status_lines.1"));
        assert!(config.load_errors()[0].contains(".ki/config.json"));
        assert_eq!(config.status_lines().len(), 1);
    }

    /// One malformed keybinding in `leader_keymap` should only clear that
    /// cell, not wipe out every other configured keybinding.
    #[test]
    fn malformed_leader_keymap_cell_falls_back_individually() {
        let tempdir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tempdir.path().join(".ki")).unwrap();
        let mut grid = vec![vec![serde_json::Value::Null; 10]; 3];
        grid[0][0] = serde_json::json!({"name": "not-an-object"});
        std::fs::write(
            tempdir.path().join(".ki/config.json"),
            serde_json::json!({ "leader_keymap": grid }).to_string(),
        )
        .unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(tempdir.path()).unwrap();
        let config = super::AppConfig::load_from_current_directory();
        std::env::set_current_dir(original_dir).unwrap();

        assert_eq!(config.load_errors().len(), 1);
        assert!(config.load_errors()[0].contains("leader_keymap.0.0"));
        assert!(config.load_errors()[0].contains(".ki/config.json"));
        assert!(config.leader_keymap().keybindings()[0][0].is_none());
    }

    /// One malformed custom keyboard layout should only drop that entry,
    /// not every custom layout the user defined.
    #[test]
    fn malformed_custom_keyboard_layout_falls_back_individually() {
        let tempdir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tempdir.path().join(".ki")).unwrap();
        std::fs::write(
            tempdir.path().join(".ki/config.json"),
            r#"{"custom_keyboard_layouts": {"bad": "not-a-grid"}}"#,
        )
        .unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(tempdir.path()).unwrap();
        let config = super::AppConfig::load_from_current_directory();
        std::env::set_current_dir(original_dir).unwrap();

        assert_eq!(config.load_errors().len(), 1);
        assert!(config.load_errors()[0].contains("custom_keyboard_layouts.bad"));
        assert!(config.load_errors()[0].contains(".ki/config.json"));
    }
}
