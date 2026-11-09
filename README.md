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
- Enable `Space` to dismiss open HUD menus without losing momentary hold/release semantics.
- Allow `Enter` on momentary layers to execute tap actions and close the layer.

### Command Architecture & Menus
- Bind the primary Command Menu directly to `Space` in Normal mode (retiring the modal `:` keybinding).
- Embed high-frequency Buffer operations directly onto the right side of the Space Command HUD.
- Chain `Space` inside the Command Menu to open the secondary Space Menu of Legends.
- Provide mnemonic key switches: `Space k f` for Files and `Space k d` for Buffers.
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
- `"c"`: `c` in Normal mode opens the Command Menu; `Space` in Normal mode directly opens the Space Menu.
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

### 3.2 Universal Escape & Modality Chords

| Keybinding | Context | Behavior |
| :--- | :--- | :--- |
| `ctrl+g` | Universal | Normalizes to `KeyCode::Esc` before component routing; dismisses menus, prompts, dropdowns, and selections. |
| `ctrl+[` | Universal | Normalizes to `KeyCode::Esc` identically to `ctrl+g`. |
| `jk` (chord) | Insert Mode | When `k` is pressed within 500ms of `j`, backspaces `j`, dispatches `EnterNormalMode`, dismisses completions, and cleans the undo ring. |
| `Space` | Menu HUD | Closes active menu HUD immediately when pressed. |
| `Enter` | Momentary Layer | Triggers layer tap action and dismisses the layer. |

### 3.3 Core Editing, Search & Motion

| Keybinding | Context | Action |
| :--- | :--- | :--- |
| `/` | Normal Mode | Search text forward (`PromptHistoryKey::Search`). |
| `?` | Normal Mode | Search text backward. |
| `x` | Normal / Selection | Unified swap register and swap character operation. |
| `Shift+R` | Normal / Selection | Cut line to system clipboard (protecting standard paste registers). |
| `Alt+/` | Normal Mode | Multi-window switcher: cycles focus between Editor, File Explorer, Git Blame, and Hunk Diff views. Falls back to Buffer Picker if only one view is open. |
| `Shift+M` | Normal Mode | Jump to position: accepts `line:col` or `line col` (e.g. `42:15`), preserving active selection mode (e.g. Tree-sitter AST Node mode). |

---

## 4. Buffer Lifecycle & Tab Engine

### 4.1 Non-destructive Buffer Retention
- **CLI Ingestion (`src/cli.rs`, `src/app.rs`):** `ki file1.rs file2.rs ...` parses multiple arguments into `Vec<AbsolutePath>`, opens all files in the background, inserts them into `self.context.marked_files()`, and sets focus to the first file.
- **Auto-marking on Modification (`src/app.rs`):** `set_file_dirty_status` detects when an unmarked preview buffer becomes dirty (`dirty_status == true`) and automatically adds it to `marked_files`. This guarantees preview buffers do not get overwritten by subsequent file opens.
- **Close Safeguard (`src/app.rs`):** `close_current_window` and `toggle_file_mark` check buffer dirty status. If unsaved edits exist, Ki halts closure and requests user resolution.
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
Accessible via `f` from the Space Command menu (`Dispatch::OpenArbitraryFilePrompt`):
- **Recursive Workspace Scan:** Spawns `PromptItemsBackgroundTask::NonGitIgnoredFilesAndDirectories`, listing all project files and directories with relative paths.
- **Dynamic Filesystem Navigation:** When the input string begins with `/`, `~`, `..`, `.`, or `./`, Ki switches from cached workspace matching to active directory listing via `get_child_paths_and_files_for_prompt`.
- **Directory CD:** Selecting a directory entry executes `Dispatch::ChangeWorkingDirectory(path)`, updating the active working directory for subsequent pickers.
- **Batch Opening:** Pressing `<Ctrl-O>` in the picker opens all currently filtered matches into background tabs simultaneously.

### 5.2 Performance & History
- **Completion Icon Suppression (`src/components/dropdown_sync.rs`):** Suppresses slow Nerd Font glyph resolutions during rapid keystroke filtering in dropdowns.
- **Query Precedence (`src/components/prompt.rs`):** Exact typed query text takes precedence over unselected completion rows when pressing Enter in text search prompts.
- **History Isolation:** Segregated by `PromptHistoryKey`, preventing prompt queries from polluting document edit history rings, with persistence under `~/.config/ki/history/`.

---

## 6. File Explorer & Filesystem Ingestion

### 6.1 Path Pre-picker & <kbd>Tab</kbd> Action (`src/components/file_explorer.rs`)
- **Starting Path Prompt:** `Space e` prompts for an absolute or relative directory path before opening the tree.
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

## 7. Persistent Hierarchical Bookmarks

Implemented in `src/bookmarks.rs` and `src/keymap_override/bookmark.rs`, backed by `~/.config/ki/bookmarks.json`:

```json
{
  "bookmarks": {
    "c": { "type": "directory", "path": "/home/user/.config" },
    "k": { "type": "file", "path": "/home/user/Developer/ki-editor/src/app.rs" },
    "d": {
      "type": "submenu",
      "name": "docs",
      "children": {
        "m": { "type": "file", "path": "/home/user/Developer/ki-editor/KustomKi.md" }
      }
    }
  }
}
```

### 7.1 Workflow & Keybindings
- **Access:** `Space b` (or `B` / `Shift+B` / `Shift+Space` in Space Command menu).
- **Navigation:** Pressing any assigned alphanumeric key (`a`-`z`, `A`-`Z`, `0`-`9`) immediately opens the file or changes directory.
- **Assignment:** Pressing `= <key>` clears any previous binding and opens the assignment prompt for `<key>`:
  - Default text is the current buffer path (or current directory if no file is open).
  - Trailing slash `/` or typing `=` creates a nested Submenu.
- **Submenus:** Arbitrary recursive nesting with breadcrumb HUD display; `<Esc>` navigates up one level.
- **Filter Picker (<kbd>Tab</kbd>):** Pressing <kbd>Tab</kbd> in any bookmark menu opens a searchable dropdown with full absolute paths. Root displays all bookmarks; submenus scope to their children. Supports `<Ctrl-O>` for batch opening.
- **Live Disk Synchronization:** Automatically detects modifications to `bookmarks.json` on disk via `notify` events and reloads state in real time.

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
| **Git Hunk Parser** | `src/git/hunk.rs` | `Hunk::from_diff` |
| **Screen Diff Perf** | `src/screen.rs` | `Screen::diff` |
| **Regex Slice Perf** | `src/components/render_editor.rs` | `Editor::render_syntax_highlighting` |
| **Verification & Logs** | `AGENTS.md` | Chronological log of requests 1-174 |

---

# <picture><source media="(prefers-color-scheme: dark)" srcset="https://ki-editor.org/img/logo-white.svg"><img src="https://avatars.githubusercontent.com/u/170408312" height="30"></picture> Ki Editor

Multi-cursor combinatoric modal editor.

- For installation instructions, building instructions, and complete documentation, please consult the [Ki wiki](https://ki-editor.org).
- For directly downloading the latest release binaries refer to [Latest master build](https://github.com/ki-editor/ki-editor/releases/tag/latest)
- If you need help, please reach out on [Ki Editor community chat room](https://ki-editor.zulipchat.com/).

[![asciicast](https://asciinema.org/a/1087449.svg)](https://asciinema.org/a/1087449)

Thank you Zulip Chat for the generosity of free Cloud hosting for Ki Editor community chat room.
