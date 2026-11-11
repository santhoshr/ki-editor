# KustomKi: Developer Reference & Technical Specification

This document provides a comprehensive technical reference for the modifications, architectural refactors, ergonomic enhancements, and bug fixes implemented in the `kustom_ki` branch of Ki Editor. It is structured for core developers and maintainers, detailing design rationales, state transitions, keymap changes, and source file locations.

---

## 1. Architectural Philosophy & Design Principles

The `kustom_ki` branch refactors Ki's reactive component model, layout tree, and prompt subsystems around four operational tenets:

1. **Non-destructive Buffer Retention (Zero Data Loss):** Unsaved edits must never be silently discarded. Any file with modifications is automatically pinned as a persistent tab, and closing or unmarking dirty buffers is strictly guarded.
2. **Modal Ergonomics & Unified Commands:** Modal isolation in original Ki created friction for standard terminal editor workflows. Universal escape sequences (`ctrl+g`, `ctrl+[`, `jk`) ensure predictable exits across prompts, dropdowns, and HUD layers. The primary Command menu is mapped directly to `Space` in Normal mode, embedding immediate buffer switching without sub-layer traversal.
3. **Sub-millisecond Rendering Loop:** Eliminates global buffer string allocations, per-frame hash set comparisons, and unbounded regex evaluations during terminal rendering and viewport scrolling.
4. **POSIX Filesystem Fidelity:** Full transparency for symbolic links across recursive walks, directory pickers, file trees, and global searches, ensuring standard UNIX symlinked workflows function seamlessly.

---

## 2. Evolution of Requirements (Technical Summary)

The changes in `kustom_ki` address 172 iterative development requests, categorized into seven technical domains:

### Input & Modality
- Implement universal `Escape` alternatives (`ctrl+g`, `ctrl+[`) across all components, modal prompts, and dropdowns.
- Provide a 500ms `jk` escape sequence in Insert mode that transitions to Normal mode and sanitizes the undo/redo ring.
- Map text search forward to `/` and backward to `?`.
- Unify register/character swapping to `x` and move line-cut to `Shift+R` to protect the primary clipboard.
- Support `line:col` and `line col` jumping in `Shift+M` while preserving AST selection modes (e.g., Tree-sitter Node mode).
- Support configurable navigation key layouts (`"navigation_keys"`: `"ijkl"`, `"hjkl"`, or `"jhlk"`), remapping home-row navigation across normal, selection, extend, and modal layers (`hjkl`: `h`=Left, `j`=Down, `k`=Up, `l`=Right; `jhlk`: `j`=Left, `h`=Up, `l`=Down, `k`=Right).
- Provide configurable pair roles for `y/;`, `i/o`, and `u/p` in `hjkl` and `jhlk` modes (`"insert"`, `"np"`, `"fl"`) with automatic collision resolution and literal-slot HUD protection for Bookmarks and Quick-marks.
- Support `"jhlk_np_rl_inverse"` option to invert Right/Left motions (`j`/`k`) and configured Next/Previous pair directions in `jhlk` mode.
- Enable `Space` to dismiss open HUD menus without losing momentary hold/release semantics.
- Allow `Enter` on momentary layers to execute tap actions and close the layer.

### Command Architecture & Menus
- Bind the primary Command Menu directly to `Space` in Normal mode (retiring the modal `:` keybinding).
- Embed high-frequency Buffer operations directly onto the right side of the Space Command HUD.
- Chain `Space` inside the Command Menu to open the secondary Space Menu of Legends.
- Provide configurable Pick menu mappings (`"pick_menu_df_swap"`): default `Space k d` for File and `Space k f` for Buffer; when swapped (`true`), `Space k f` for File and `Space k d` for Buffer.
- Add `<Ctrl-O>` in file and bookmark pickers to batch-open all filtered matches simultaneously.

### Buffer Lifecycle & Tabstrip Management
- Accept multiple CLI path arguments (`ki file1 file2 ...`), opening all in the background, auto-marking them as tabs, and focusing the first.
- Automatically promote unmarked preview buffers to marked tabs when modified (`dirty_status == true`).
- Guard `close_current_window` and `toggle_file_mark` against closing or unmarking buffers with unsaved changes.
- Manage untitled buffers (`[No Name]`) with clean disposal on discard.
- Implement a bounded, dynamic column-grid tabstrip (1 to 5 rows, max 20% viewport height) with vertically aligned column brackets `[-]`.
- Disambiguate duplicate buffer basenames in tabs (e.g., `app.rs (src)` vs `app.rs (tests)`).
- Update terminal emulator window titles dynamically via OSC escape sequences (`ki - filename [+][-]`).

### Prompts, Pickers & Autocompletion
- Build the "Open File (or CD)" prompt: defaults to recursive workspace indexing; typing `/`, `~`, `..`, or `./` switches dynamically to filesystem path completion; selecting a directory updates current working directory.
- Isolate prompt search histories from document undo rings and auto-persist to disk.
- Suppress slow Nerd Font icon lookups in completion dropdowns to eliminate UI micro-stutters.
- Prevent directory autoselection from prematurely changing directories when entering text queries.

### Filesystem, Tree & Symlink Transparency
- Add a directory path pre-picker before launching the File Explorer.
- In File Explorer, bind <kbd>Tab</kbd> to close the tree and open the "Open File (or CD)" prompt rooted at the focused node (parent directory for files, directory itself for folders).
- Support POSIX symbolic links across `ignore::WalkBuilder` traversals (`get_non_git_ignored_files`, `get_non_git_ignored_files_and_directories`, and `find_in_files`).
- Classify symlinked directories properly in File Explorer via `path.is_dir()`.

### Persistent Hierarchical Bookmarks
- Implement persistent bookmark storage in `~/.config/ki/bookmarks.json`.
- Support single-key mnemonic access (`a`-`z`, `A`-`Z`, `0`-`9`) via `Space b` (or `B` / `Shift+Space` in the Command menu).
- Allow dynamic assignment via `= <key>`, with `/` suffix creating nested submenus.
- Provide a <kbd>Tab</kbd> filter picker listing all bookmarks with full absolute paths.
- Automatically reload bookmarks from disk on external modifications via filesystem notifications.

### Performance, Git & Error Diagnostics
- Optimize `Screen::diff` with an index-by-index equality fast path, bypassing hash set construction.
- Slice visible line byte ranges for regex syntax highlighting, eliminating whole-buffer `Rope::to_string()` heap allocations.
- Clamp scrolling and viewport ranges to prevent assertion panics on small terminal windows or overshoot.
- Fix single-operation assertion panics in `src/git/hunk.rs` by flattening multi-operation inline diff iterators.
- Broaden file watcher event matching and add forced reload bypass (`Space j d`) for external file modifications.
- Replace modal error popups with ephemeral non-blocking errors that auto-dismiss on movement, with error recall via `T` / `Shift+T`.
- Live-reload theme previews dynamically from `~/.config/ki/themes` with graceful JSON error recovery.

---

## 3. Keyboard Architecture & Modal Mappings

### 3.1 Normal Mode Space Command Menu

In Normal mode, pressing `Space` directly opens the Command Menu HUD (`command_keymap`) by default. This keybinding is fully configurable via `"command_menu_invocation"` in `config.json`.

Supported invocation keys:
- `"Space"` (default): `Space` opens the Command Menu; pressing `Space` inside opens the secondary Space Menu.
- `"c"`: `c` in Normal mode opens the Command Menu; `Space` in Normal mode directly opens the Space Menu; Normal mode's original `c` action (`Copy/≡ Dup`) is moved inside the Command Menu under `c`.
- `"C"`: `C` (or `Shift+c`) opens the Command Menu; `Space` in Normal mode directly opens the Space Menu.
- `"Shift+;"` (or `":"`): `:` opens the Command Menu; `Space` in Normal mode directly opens the Space Menu.

```
+------------------------------------+------------------------------------+
| COMMANDS (Left)                    | BUFFERS (Right)                    |
+------------------------------------+------------------------------------+
| f          Open File (or CD)       | j          << Marked File (Prev)   |
| b          Buffer List Picker      | l          Marked File >> (Next)   |
| T, S-T     Show Last Error         | y          |< Marked File (First)  |
| B, S-B     Bookmarks Menu          | p          Marked File >| (Last)   |
| C, S-C     Current File Directory  | k          Marked File ^           |
| -          Reveal in Explorer      | m          Unmarked File v         |
| _, S-_     Explorer in Working Dir | u          Alternate Buffer        |
| w          Close Window / Tab      | s          Save Current Buffer     |
| q          Safe Quit               | a          Save As                 |
| v          Quit Without Saving     | d          Reload File From Disk   |
| Enter      Toggle Selection Mark   | h          < New Untitled (Left)   |
| Space      Open Full Space MoL     | ;          New Untitled > (Right)  |
| S-Space    Bookmarks Menu          |                                    |
| /          Show Help Legend        |                                    |
+------------------------------------+------------------------------------+
```

#### Behavioral Details:
- **`Space` -> `Space`:** Pressing `Space` inside the Command Menu opens the full Space Menu of Legends (`space_keymap_legend_config`), maintaining backward compatibility with all deep MoL subsystems (Pick, Jump, Buffer, Git, View, Window).
- **Embedded Buffer Operations:** High-frequency tab navigation (`j`/`l`), save (`s`), reload (`d`), and alternate buffer switching (`u`) execute directly from the Space HUD without entering submenus.
- **HUD Dismissal:** Pressing `Space` while already inside the Command HUD dismisses it immediately (`MenuKeymapOverride::handle_press`). Momentary layer locks remain supported.
- **HUD Footer Help:** In Space HUD (`Space` / `Command`), the footer displays `* Pick Keyboard    \ Leader    - File Tree (current file)    shift+-/_ File Tree (cd)`. In Bookmarks HUD, it displays `Key/S-Key/Alt-Key Pick file/folder to bookmark, empty path to clear, input = to nest(equals key)`.

### 3.2 Universal Escape & Modality Chords

| Keybinding | Context | Behavior |
| :--- | :--- | :--- |
| `ctrl+g` | Universal | Normalizes to `KeyCode::Esc` before component routing; dismisses menus, prompts, dropdowns, and selections. |
| `ctrl+[` | Universal | Normalizes to `KeyCode::Esc` identically to `ctrl+g`. |
| `jk` (chord) | Insert Mode | When `k` is pressed within 500ms of `j`, backspaces `j`, dispatches `EnterNormalMode`, dismisses completions, and cleans the undo ring. |
| `Space` | Menu HUD | Closes active menu HUD immediately when pressed. |
| `Enter` | Momentary Layer | Triggers layer tap action and dismisses the layer. |

### 3.3 Core Editing, Search & Motion

The swap and cut key assignments are configurable via `"swap_key"` in `config.json` (`"/"` or `"x"`, defaulting to `"/"`):

- **Default (`"swap_key": "/"`):**
  - `/`: Unified cursor swap (`DispatchEditor::SwapCursor`).
  - `x`: Cut momentary layer (`≡ Cut`) with tap action Cut One (`DispatchEditor::CutOne`), moved from `Shift+R`.
  - `?`: Search text backward.
  - Forward search remains available via `s d` / `S d`.

- **Alternative (`"swap_key": "x"`):**
  - `x`: Unified cursor swap (`DispatchEditor::SwapCursor`).
  - `Shift+R` (`R`): Cut momentary layer (`≡ Cut`) with tap action Cut One (`DispatchEditor::CutOne`).
  - `/`: Search text forward (`PromptHistoryKey::Search`).
  - `?`: Search text backward.

| Keybinding | Context | Action |
| :--- | :--- | :--- |
| `/` | Normal Mode (default) | Unified swap register and cursor operation (`"swap_key": "/"`). |
| `x` | Normal Mode (default) | Cut line/selection (`≡ Cut` momentary layer, tap `Cut One`) when `"swap_key": "/"`. |
| `x` | Normal Mode (alt) | Unified swap register and cursor operation when `"swap_key": "x"`. |
| `Shift+R` | Normal Mode (alt) | Cut line/selection (`≡ Cut` momentary layer, tap `Cut One`) when `"swap_key": "x"`. |
| `?` | Normal Mode | Search text backward. |
| `Alt+/` | Normal Mode | Multi-window switcher: cycles focus between Editor, File Explorer, Git Blame, and Hunk Diff views. Falls back to Buffer Picker if only one view is open. |
| `Shift+M` | Normal Mode | Jump to position: accepts `line:col` or `line col` (e.g. `42:15`), preserving active selection mode (e.g. Tree-sitter AST Node mode). |

### 3.4 E Key Momentary Menu Configuration (`src/config.rs`, `src/keymap.rs`)

The normal mode `e` key behavior is configurable via `"e_normal_mode"` in `config.json` (`"buffer"` or `"mark"`, defaulting to `"buffer"`):

- **Default (`"e_normal_mode": "buffer"`):**
  - **Hold `e`:** Opens the original buffer menu (`≡ Buffer`):
    - `j` / `l`: cycle marked files (`<<` / `>>`)
    - `u` / `o`: cycle opened files (`<` / `>`)
    - `y` / `p`: first / last marked file
    - `k`: toggle file mark, `n`: close window, `i`: unmark other buffers, `m`: alternate buffer
    - `h` / `;`: open untitled file left / right
  - **Tap `e`:** Toggles selection mark (`ToggleSelectionMark`).

- **Alternative (`"e_normal_mode": "mark"`):**
  - **Hold `e`:** Opens the customized mark navigation menu (`≡ Mark`):
    - `j` / `l`: local marks (`<` / `>`)
    - `u` / `o`: global marks (`<` / `>`)
    - `y` / `p`: first / last local mark (`|<` / `>|`)
    - `k`: toggle file mark, `n`: close window, `i`: unmark other buffers, `m`: alternate buffer
    - `h` / `;`: open untitled file left / right
  - **Tap `e`:** Toggles selection mark (`ToggleSelectionMark`).

### 3.5 Configurable Navigation Keys (IJKL / HJKL / JHLK) (`src/config.rs`, `src/components/editor_keymap.rs`)

Ki navigation can be set via `"navigation_keys"` in `config.json` (`"ijkl"`, `"hjkl"`, or `"jhlk"`, default `"ijkl"`).

#### Core Navigation Motions

| Layout | Left (`<<`) | Down (`v`) | Up (`^`) | Right (`>>`) |
| :--- | :--- | :--- | :--- | :--- |
| `"ijkl"` (Ki standard) | `j` | `k` | `i` | `l` |
| `"hjkl"` (Vim style) | `h` | `j` | `k` | `l` |
| `"jhlk"` | `j` | `l` | `h` | `k` |

- **Modifiers:** Shift/Caps counterparts (Dedent, Break, Join, Indent) and Alt counterparts (Scroll Down, Scroll Up) follow the mapped directions consistently across normal, selection, extend, and modal layers.
  - In `"hjkl"`: `H`=Dedent, `J`=Break, `K`=Join, `L`=Indent, `Alt+j`=Scroll Down, `Alt+k`=Scroll Up.
  - In `"jhlk"`: `J`=Dedent, `L`=Break, `H`=Join, `K`=Indent, `Alt+l`=Scroll Down, `Alt+h`=Scroll Up.

#### Configurable Action Pairs (`hjkl_y;_action`, `hjkl_io_action`, `hjkl_up_action`)
In both `"hjkl"` and `"jhlk"` modes, the three remaining key pairs (`y/;`, `i/o`, and `u/p`) are individually assignable to one of three roles (`"insert"`, `"np"`, `"fl"`):

| Config Key | Default Role | Keys Controlled |
| :--- | :--- | :--- |
| `"hjkl_y;_action"` | `"insert"` | `y` and `;` |
| `"hjkl_io_action"` | `"np"` (Prev/Next) | `i` and `o` |
| `"hjkl_up_action"` | `"fl"` (First/Last) | `u` and `p` |

*(Note: `"hjkl_y_semicolon_action"` is also supported as an alias if JSON linters or tooling disallow literal semicolons in property keys.)*

- **Role Definitions:**
  - `"insert"`: First key acts as `h` (`← Insert`), second key acts as `;` (`Insert →`).
  - `"np"`: First key acts as `u` (`< Previous`), second key acts as `o` (`> Next`).
  - `"fl"`: First key acts as `y` (`|< First`), second key acts as `p` (`>| Last`).
- **Default Layouts:**
  - In `"hjkl"` (`y/;-insert, io-np, up-fl`):
    - Row 0: `y` (Insert ←), `u` (|< First), `i` (< Prev), `o` (> Next), `p` (>| Last)
    - Row 1: `h` (<< Left), `j` (v Down), `k` (^ Up), `l` (>> Right), `;` (Insert →)
  - In `"jhlk"` (`y/;-insert, io-np, up-fl`):
    - Row 0: `y` (Insert ←), `u` (|< First), `i` (< Prev), `o` (> Next), `p` (>| Last)
    - Row 1: `h` (^ Up), `j` (<< Left), `k` (>> Right), `l` (v Down), `;` (Insert →)
- **Collision Guard:** If any of the three settings share the same role or collide, all three automatically revert to default (`insert`, `np`, `fl`) and a descriptive warning is added to `load_errors`.
- **Text Insertion Safety:** Literal character typing in Insert mode (`original.code`) is unaffected.
- **HUD Alignment:** Keymap HUDs reflect the remapped actions under each physical cell, while literal-slot keymaps (Quick-marks and Bookmarks) bypass navigation remapping to prevent letter jumbling.

#### Right/Left and Next/Previous Inversion (`"jhlk_np_rl_inverse"`)
- **Config Key:** `"jhlk_np_rl_inverse": false` (boolean, default `false`).
- When set to `true` while using `"navigation_keys": "jhlk"`:
  - Core Right/Left keys invert: `j` navigates Right (`l` in Ki) and `k` navigates Left (`j` in Ki), along with their Shift (Indent/Dedent) and Alt counterparts.
  - The configured Next/Previous pair (whichever of `io`, `up`, or `y;` has the `"np"` action) inverts: the first key navigates Next (`o` in Ki) and the second key navigates Previous (`u` in Ki).
  - Core Up (`h`), Down (`l`), First/Last (`"fl"`), and Insert (`"insert"`) actions remain unchanged.

---

## 4. Buffer Lifecycle & Tab Engine

### 4.1 Non-destructive Buffer Retention
- **CLI Ingestion (`src/cli.rs`, `src/app.rs`):** `ki file1.rs file2.rs ...` parses multiple arguments into `Vec<AbsolutePath>`, opens all files in the background, inserts them into `self.context.marked_files()`, and sets focus to the first file.
- **Auto-marking on Modification (`src/app.rs`):** `set_file_dirty_status` detects when an unmarked preview buffer becomes dirty (`dirty_status == true`) and automatically adds it to `marked_files`. This guarantees preview buffers do not get overwritten by subsequent file opens.
- **Close Safeguard (`src/app.rs`):** `close_current_window` and `toggle_file_mark` check buffer dirty status. If unsaved edits exist, Ki halts closure and requests user resolution.
- **Safe Quit Workflow (`src/app.rs`):** When Quit (`q` / `Dispatch::SafeQuit`) is executed, all clean buffers are automatically closed and unmarked. If all buffers are clean, Ki exits immediately. If any unsaved buffers remain, Ki leaves them open, focuses the first unsaved buffer, and displays a warning prompt, enabling incremental save/discard iteration.
- **Non-file Path Sanitization (`src/context.rs`):** Session mark restoration passes paths through `.filter(|p| p.is_file())` to prevent directory symlinks (e.g. `~/.nix-profile`) from corrupting the tab strip.

### 4.2 Dynamic Column-Grid Tabstrip (`src/components/render_editor.rs`)
- **Vertical Viewport Bounds:** Capped strictly between 1 and 5 rows (bounded by 20% of terminal height).
- **Column Alignment:**
  - Calculates column widths greedily up to terminal width.
  - If a lower row accommodates more files, the wider column layout propagates upward so columns align vertically across all rows.
  - Tab brackets `[-]` and `[+]` align flush against the line number gutter border `│`.
- **Title Disambiguation:** When multiple open buffers share identical file names, Ki computes the shortest unique relative parent path (e.g., `app.rs (src)` vs `app.rs (tests)`).
- **Indicator Gutter:** Multi-row tab bars display aligned line numbers (`01│`, `02│`), followed by a dedicated summary line:
  - Left: `01│ (tab rows)`
  - Right: `[src/components/render_editor.rs]`
- **Terminal Title Synchronization (`src/frontend/crossterm.rs`):** Emits OSC 0/2 escape sequences updating the host terminal window title with the active file and dirty state: `ki - src/main.rs [+]`.

### 4.3 Format on Save Configuration (`src/config.rs`, `src/buffer.rs`)
- **Config Key:** `"format_on_save": false` (added to `config.json` and `src/config_default.json`).
- **Default State:** Disabled (`false`). Saving a buffer writes contents directly to disk without spawning external language formatters.
- **Opt-in Formatting:** When enabled (`"format_on_save": true`), `Buffer::save` runs the language formatter (e.g. `prettier`, `rustfmt`) and updates the buffer content before committing to disk.

---

## 5. Pickers, Prompts & Autocompletion

### 5.1 "Open File (or CD)" Prompt (`src/components/prompt.rs`, `src/app.rs`)
Accessible via `"` (<kbd>Shift+'</kbd>) directly in Normal mode, or `f` from the Space Command menu (`Dispatch::OpenArbitraryFilePrompt`):
- **Recursive Workspace Scan:** Spawns `PromptItemsBackgroundTask::NonGitIgnoredFilesAndDirectories`, listing all project files and directories with relative paths.
- **Dynamic Filesystem Navigation:** When the input string begins with `/`, `~`, `..`, `.`, or `./`, Ki switches from cached workspace matching to active directory listing via `get_child_paths_and_files_for_prompt`.
- **Directory CD:** Selecting a directory entry executes `Dispatch::ChangeWorkingDirectory(path)`, updating the active working directory for subsequent pickers.
- **Batch Opening:** Pressing `<Ctrl-O>` in the picker opens all currently filtered matches into background tabs simultaneously.

### 5.2 Direct Normal Mode & Command Menu Shortcuts (`src/keymap.rs`, `src/recent.rs`)
- **`'` (Single Quote in Normal Mode):** Opens the persistent **Recent Files** filter prompt (`Dispatch::OpenRecentFilesPicker`). Files visited across sessions (persisted in `~/.config/ki/recent.json`) are displayed in MRU order. Supports fuzzy filtering, Enter to open, and `<Ctrl-O>` to batch open filtered files.
- **`'` (Single Quote in Command Menu):** Opens the persistent **Recent Folders** filter prompt (`Dispatch::OpenRecentFoldersPicker`). Folders visited across sessions (persisted in `~/.config/ki/recent.json`) are displayed in MRU order. Selecting a folder switches the active working directory with confirmation notice.

### 5.2 Performance & History
- **Completion Icon Suppression (`src/components/dropdown_sync.rs`):** Suppresses slow Nerd Font glyph resolutions during rapid keystroke filtering in dropdowns.
- **Query Precedence (`src/components/prompt.rs`):** Exact typed query text takes precedence over unselected completion rows when pressing Enter in text search prompts.
- **History Isolation:** Segregated by `PromptHistoryKey`, preventing prompt queries from polluting document edit history rings, with persistence under `~/.config/ki/history/`.

### 5.3 Pick Menu D/F Configuration (`src/config.rs`, `src/keymap.rs`)
- **Config Key:** `"pick_menu_df_swap": false` (added to `config.json` and `src/config_default.json`).
- **Default State (`false`):** In the Space Pick menu (`Space k`), `d` opens File picker (`FilePickerKind::NonGitIgnored`) and `f` opens Buffer picker (`FilePickerKind::Opened`).
- **Swapped State (`true`):** In the Space Pick menu (`Space k`), `f` opens File picker (`FilePickerKind::NonGitIgnored`) and `d` opens Buffer picker (`FilePickerKind::Opened`).

---

## 6. File Explorer & Filesystem Ingestion

### 6.1 Path Pre-picker & Direct Opening (`src/components/file_explorer.rs`, `src/keymap.rs`)
- **Config Key:** `"filetree_open_with_path": false` (added to `config.json` and `src/config_default.json`).
- **Default State (`false`):** Pressing `Space ;` opens the File Explorer directly rooted at the current buffer or working directory (`Dispatch::RevealInExplorer`), matching original Ki behavior.
- **Path Picker State (`true`):** Pressing `Space ;` opens the path picker prompt before launching the file tree (`Dispatch::OpenFileTreeWithPathPicker`).
- **Node <kbd>Tab</kbd> Dispatch:** Pressing <kbd>Tab</kbd> on any node in the File Explorer:
  1. Inspects the focused node:
     - If `NodeKind::File`, resolves to parent directory (`node.path.parent()`).
     - If `NodeKind::Directory` or `NodeKind::ParentDirectory`, resolves to the directory itself (`node.path`).
  2. Closes the File Explorer view (`layout.close_file_explorer`).
  3. Dispatches `Dispatch::OpenArbitraryFilePromptAt(target_dir)`, opening the "Open File (or CD)" prompt rooted at that directory with standard recursive filtering.

### 6.2 POSIX Symlink Transparency (`src/list/mod.rs`)
Rust's `ignore::WalkBuilder` emits raw directory entry `FileType` metadata without traversing symlinks by default (`is_symlink() == true`, `is_file() == false`, `is_dir() == false`).
- **File Ingestion:** Updated `get_non_git_ignored_files` to check `file_type.is_file() || (file_type.is_symlink() && path.path().is_file())`.
- **Directory Ingestion:** Updated `get_non_git_ignored_files_and_directories` to check `file_type.is_file() || file_type.is_dir() || (file_type.is_symlink() && (entry.path().is_file() || entry.path().is_dir()))`.
- **Global Search:** Updated `find_in_files` to index files through valid symlinks.
- **Tree Classification:** Updated `FileExplorer::get_nodes` to use `path.is_dir()` (which follows symlinks), preventing directory symlinks from being treated as plain files.

---

## 7. Persistent Bookmarks & Quick-marks Engine

Implemented in `src/bookmarks.rs` and `src/keymap_override/bookmark.rs`. Both systems run simultaneously and maintain independent persistent JSON configuration files:

- **Bookmarks (Nested):** `~/.config/ki/bookmarks.json`
- **Quick-marks (Single):** `~/.config/ki/quick_marks.json`

### 7.1 Architecture & Comparison

| Feature | Bookmarks (`Shift+B` / `B`) | Quick-marks (`:` or `1`) |
| :--- | :--- | :--- |
| **Storage File** | `~/.config/ki/bookmarks.json` | `~/.config/ki/quick_marks.json` |
| **Architecture** | Hierarchical submenus and directory/file leaves | Flat key-to-multiple-bookmarks map |
| **Capacity per Key** | 1 target (file, directory, or nested submenu) | Multiple paths (`Vec<AbsolutePath>`) per key |
| **Key Activation** | Jumps to file/directory or enters submenu | If 1 target: jumps directly. If >1: opens searchable picker |
| **Unassigned Key** | Opens assign prompt pre-filled with current file | Opens assign prompt pre-filled with current file |
| **Shift+Key** | Assigns / navigates shifted character key | Opens prompt pre-filled with current file path to add another bookmark |
| **Alt+Key** | Re-assigns key with pre-filled path | Opens editable panel (multi-line buffer in Normal line mode with first item selected) |
| **Editable Panel (Alt+Key)** | N/A (Submenus used instead) | Opens in Normal mode, Line selection mode with first item selected. Navigate with `j`/`k`, delete line with `r` or `x`, delete multiple with `v`/`l` selection + `r`/`x`, delete all with `SelectAll` + `r`/`x`. `<Enter>` opens focused bookmark and saves edits; `<Esc>` saves edits |
| **HUD Footer** | `Key/S-Key/Alt-Key Pick file/folder to bookmark, empty path to clear, input = to nest(equals key)` | `Key Open/Pick, S-Key Add file, Alt-Key Edit panel` |

### 7.2 Configuration & Invocations
- **Invocations:**
  - `"bookmark_menu_invocation"`: `"B"` (default), `":"` (or `"colon"`, `"shift+;"`, `"shift+semicolon"`), or `"1"`.
  - `"quick_marks_menu_invocation"`: `":"` (default, or `"colon"`, `"shift+;"`, `"shift+semicolon"`) or `"1"`.
  - `"command_menu_invocation"`: `"Space"`, `"c"`, `"C"`, or `":"` (or `"colon"`, `"shift+;"`, `"shift+semicolon"`).
  - *(Note: For `:` invocations, `"colon"`, `"shift+;"`, or `"shift+semicolon"` can be used interchangeably in configs and CLI/shell scripts.)*
- **Conflict Resolution Priority:**
  - If **Command menu** is configured for `":"`: Command menu gets `":"`, Quick-marks gets `"1"`, Bookmarks gets `"B"`.
  - If both **Quick-marks** and **Bookmarks** are configured for `":"`: Quick-marks gets `":"` and Bookmarks gets `"1"`.
  - If both **Quick-marks** and **Bookmarks** are configured for `"1"`: Quick-marks gets `"1"` and Bookmarks gets `":"`.
  - Otherwise, each menu uses its configured invocation (or default).
- **Filter Picker (<kbd>Tab</kbd>):** Pressing <kbd>Tab</kbd> in either HUD opens a searchable dropdown with full absolute paths across all marks in that system. Supports `<Ctrl-O>` for batch opening.
- **Live Disk Synchronization:** Automatically detects modifications to `bookmarks.json` and `quick_marks.json` on disk via `notify` events and reloads state in real time.

---

## 8. Git Integration & External Sync

### 8.1 Multi-Op Inline Hunk Parsing (`src/git/hunk.rs`)
Original Ki panicked with `assertion failed: diff_ops.len() <= 1` when parsing complex renames, consecutive modifications, or multi-operation diffs.
- **Resolution:** Removed the single-operation assertion and replaced the change collection iterator with:
  ```rust
  diff.ops().iter().flat_map(|op| diff.iter_inline_changes(op))
  ```

### 8.2 External Disk Modification Watcher (`src/buffer.rs`, `src/file_watcher/mod.rs`)
- **Force Reload Bypass:** `Buffer::reload` previously skipped disk reads if the cached modification timestamp matched. Added `force: true` to bypass timestamp equality checks during manual reloads (`Space j d`).
- **Broadened Watcher Events:** Updated `file_watcher` to capture `Modify(Data)`, atomic replace/rename operations, and file recreation events, ensuring external changes (git checkouts, formatters) reflect immediately in buffers.

---

## 9. Performance Engineering & Rendering

### 9.1 Screen Diff Fast Path (`src/screen.rs`)
Original `Screen::diff` built an `IndexSet` with full cell hashing on every frame.
- **Optimization:** Added a direct index-by-index equality loop when incoming and current screen cell counts match. Bypasses hash set creation entirely for standard typing and scrolling.

### 9.2 Viewport Sliced Regex Highlighting (`src/components/render_editor.rs`)
Original regex syntax highlighting converted the entire buffer to a `String` via `Rope::to_string()` on every frame, causing noticeable GC pauses and dropped frames on multi-megabyte files.
- **Optimization:** Restricted regex highlighting to visible line slices:
  ```rust
  let (start_byte, end_byte) = buffer.visible_line_range_to_byte_range(visible_range);
  let visible_slice = buffer.slice_to_cow(start_byte..end_byte);
  ```
  Eliminates heap re-allocations and ensures sustained 60+ FPS scrolling.

### 9.3 Boundary Clamping & Panic Prevention
- **Scroll Clamping (`src/components/editor/mod.rs`):** Clamps `scroll_offset` to `buffer.len_lines().saturating_sub(1)`.
- **Range Safety (`src/components/render_editor.rs`):** `visible_line_range_given_scroll_offset_and_height` clamps start line to total lines, and `line_range_to_byte_range` returns empty ranges on inverted offsets, preventing panics on terminal resize.

---

## 10. Source File Reference

| Subsystem | Primary Source Files | Key Functions / Structs |
| :--- | :--- | :--- |
| **Command Menu on Space** | `src/keymap.rs`<br>`src/components/editor/mod.rs` | `command_keymap`<br>`command_keymap_legend_config`<br>`Editor::press_space` |
| **Escape & Event Loop** | `src/app.rs` | `App::handle_event`<br>`last_insert_j_press` |
| **Buffer Safety & Tabs** | `src/app.rs`<br>`src/context.rs`<br>`src/components/render_editor.rs` | `App::close_current_window`<br>`App::set_file_dirty_status`<br>`Editor::render_window_title` |
| **Prompts & Completions** | `src/components/prompt.rs`<br>`src/components/dropdown_sync.rs` | `Prompt::new`<br>`PromptConfig`<br>`DropdownItem::from_path_buf` |
| **File Tree & Tab Action** | `src/components/file_explorer.rs` | `FileExplorer::handle_key_event`<br>`open_file_prompt_for_current_node` |
| **Symlink Resolution** | `src/list/mod.rs` | `WalkBuilderConfig::get_non_git_ignored_files`<br>`get_non_git_ignored_files_and_directories` |
| **Bookmarks Engine** | `src/bookmarks.rs`<br>`src/keymap_override/bookmark.rs` | `Bookmarks`<br>`BookmarkKeymapOverride` |
| **Navigation Keys & HJKL** | `src/config.rs`<br>`src/components/editor_keymap.rs`<br>`src/components/editor_keymap_printer.rs` | `NavigationKeys`<br>`HjklPairAction`<br>`remap_navigation_key` |
| **Git Hunk Parser** | `src/git/hunk.rs` | `Hunk::from_diff` |
| **Screen Diff Perf** | `src/screen.rs` | `Screen::diff` |
| **Regex Slice Perf** | `src/components/render_editor.rs` | `Editor::render_syntax_highlighting` |
| **Verification & Logs** | `AGENTS.md` | Chronological log of requests 1-205 |

---

# <picture><source media="(prefers-color-scheme: dark)" srcset="https://ki-editor.org/img/logo-white.svg"><img src="https://avatars.githubusercontent.com/u/170408312" height="30"></picture> Ki Editor

Multi-cursor combinatoric modal editor.

- For installation instructions, building instructions, and complete documentation, please consult the [Ki wiki](https://ki-editor.org).
- For directly downloading the latest release binaries refer to [Latest master build](https://github.com/ki-editor/ki-editor/releases/tag/latest)
- If you need help, please reach out on [Ki Editor community chat room](https://ki-editor.zulipchat.com/).

[![asciicast](https://asciinema.org/a/1087449.svg)](https://asciinema.org/a/1087449)

Thank you Zulip Chat for the generosity of free Cloud hosting for Ki Editor community chat room.
