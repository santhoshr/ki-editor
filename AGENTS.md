# AGENTS.md

This document records the user's change requests verbatim from the very beginning of the project history, concise technical responses, and the context/architecture required for future agent sessions.

---

## 1. Verbatim User Requests and Short Responses

### Request 1
> "check git diff I added ctrl+g shortcut for escape but keybinding is not working"
**Response:**
Inspected `git diff`. The binding was added to `keymap.rs` under Normal mode, but modal inputs, prompts, and dropdowns bypass `keymap.rs` or have their own handlers.

### Request 2
> "you can remove it I want it to work as esc replacement universally"
**Response:**
Removed the `keymap.rs` edits and intercepted `ctrl+g` directly in `App::handle_event` (`src/app.rs`) so it maps to `KeyCode::Esc` before any component or mode handles the key event.

### Request 3
> "stop  with the tests and verifications, just do the change I will manually test it"
**Response:**
Stopped running automated test suites; shifted strictly to direct code changes and compilation checks via `cargo build`.

### Request 4
> "it works, when we are at it let us also add ctrl+["
**Response:**
Added `ctrl+[` alongside `ctrl+g` as a universal escape key in `App::handle_event` (`src/app.rs`).

### Request 5
> "the documentation is pure madness of philosophical ramblings, convulted way saying things. I have add newdoc folder, add single single html documentation, that covers the functionality and key bindings."
**Response:**
Created `newdoc/index.html` with an interactive, searchable single-page guide covering normal, insert, selection motions, space menus, command options, and keybinding cheat sheets.

### Request 6
> "don't fix answer me, why no cursor in insert mode?"
**Response:**
In `src/components/render_editor.rs` (lines 819–831), `primary_selection_primary_cursor` was guarded with `if !is_focused_file`, resulting in `None` for the active focused file so no cell had `is_cursor = true`.

### Request 7
> "don't over analyze just answer based on current available session info"
**Response:**
Summarized concisely: the hardware cursor position calculation in `render_editor.rs` evaluates to `None` on the focused buffer during render.

### Request 8
> "why"
**Response:**
Explained the logic inversion in the conditional block where `!is_focused_file` was used instead of `is_focused_file`.

### Request 9
> "is it stylistic decision?/"
**Response:**
No, not stylistic; it was a bug where the cursor was inadvertently suppressed on the active editor buffer.

### Request 10
> "can you do git blame to find any comment on why it is done?"
**Response:**
Ran `git log` and `git blame` on the line; the commit message simply refactored cursor and selection rendering without intention to hide the cursor in insert mode.

### Request 11
> "any way to define config file to exit insert mode to jk"
**Response:**
Analyzed configuration architecture: `config.json` only allowed custom shell-out leader bindings, not sequence-based chord timeouts in insert mode. It required an event handler in the editor.

### Request 12
> "implement it and let me test"
**Response:**
Added `last_insert_j_press: Option<Instant>` in `App` (`src/app.rs`). If `k` is pressed within 500ms of `j` in insert mode, it automatically backspaces `j`, dispatches `EnterNormalMode`, and closes dropdowns.

### Request 13
> "update newdoc with missing features/keybindings from code knowledge, also give examples/locations of config files"
**Response:**
Expanded `newdoc/index.html` with:
- `jk`, `ctrl+g`, and `ctrl+[` escape mechanisms.
- Extend submode (`f`), Leader protocol (`\`) with JSON schema.
- Config file paths across Linux (`~/.config/ki/config.json`), macOS (`~/Library/Application Support/ki/config.json`), Windows (`%APPDATA%\ki\config.json`).
- Schema configurations including Nerd Font icons, indent preferences, and custom commands.

### Request 14
> "what git features ki includes, add a section for git usage as well"
**Response:**
Added Section 13 ("Git Integration") to `newdoc/index.html` detailing:
- Dual-reference diffs: `@` (main branch) vs `^` (head branch).
- Gutter diff markers (`+`, `-`, `~`).
- Hunk navigation and reverts (`Space j r` / `Space j R`).
- Git blame view (`Space j b`).
- Git status file pickers (`Space k g` / `Space k G`).

### Request 15
> "the performance seems terrible when doing scrolling do you see memory leaks are code issuess, generally it is very slower than vim or kakoune"
> "is it possible to rearch and fix it with minimal change?"
**Response:**
Identified and fixed two rendering bottlenecks:
1. `Screen::diff` built an `IndexSet` with hashing per frame. Added direct index-by-index equality fast-path when cell count matches (`src/screen.rs`).
2. `Editor::render_syntax_highlighting` converted the entire buffer to a `String` and ran regex on the entire file per frame. Replaced with byte-range slicing on only visible lines (`src/components/render_editor.rs`).

### Request 16
> "thread '<unnamed>' (11981) panicked at src/git/hunk.rs:102:9: assertion failed: diff_ops.len() <= 1"
**Response:**
Fixed in `src/git/hunk.rs` by removing the single-op assertion and using `.ops().iter().flat_map(|op| diff.iter_inline_changes(op))`.

### Request 17
> "commit our performance change"
**Response:**
Committed changes as `cc4b10b3f` (*"perf: optimize screen diff and visible line regex rendering, fix hunk panic"*).

### Request 18
> "reload file (even before our changes) never reloads from the system for some reason, can you what is the issue, if I change it externally, the file itself shown as unavailable in ki"
**Response:**
Diagnosed and fixed two reload issues:
1. `Buffer::reload` skipped reading if `last_modified_time` matched even if `force: true` was requested. Added `!force` check (`src/buffer.rs`).
2. In `src/file_watcher/mod.rs`, broadened notify event matching from `Modify(Data)` to all modify events, atomic file replace renames, and create events.
3. Updated manual "Reload File" keybinding (`Space j d`) to use `force: true` in `src/keymap.rs`.

### Request 19
> "commit this change"
**Response:**
Committed changes as `7cb7f1f21` (*"fix(buffer): reload file reliably on manual trigger and external edits"*).

### Request 20
> "some decisions are questionable, for e.g. files are always in preview mode, so opening another file replace current view, they require mark toggle, when file is opened from command line, do a mark automatically, when file is opened from explorer, let it be unmarked, but when user does modification mark it, so another explorer open will open new unmarked view. Also confirm my assertions are right, before changes (no verfication/test running)"
**Response:**
Confirmed 100% accurate:
1. `Layout::replace_and_focus_current_suggestive_editor` always replaces the single `SuggestiveEditor` under root. Files not in `marked_files` are treated as preview (`[ ]`).
2. CLI startup (`App::run`) dispatched `Dispatch::OpenFile` unmarked, causing files opened via CLI to be replaced on next open.
3. Modifying a buffer dispatched `SetFileDirtyStatus`, but did not add the file to `marked_files`.

### Request 21
> "also another weird thing I found is for text editor, it does not open more than one file, confirm this as well"
**Response:**
Confirmed: Ki has no window-split mechanism (`vsplit`/`split`). The UI tree strictly enforces one active `SuggestiveEditor` at a time. All multi-file management is purely tab/buffer switching in `layout.background_suggestive_editors`.

### Request 22
> "another thing I found peculier for an editor is there is no way to open files, it is burried with menus like change folder then pick file, is it right? is there any reason to do this like ki can't handle multi folder files? for some reason"
**Response:**
Confirmed: Ki handles multi-folder/external paths via `AbsolutePath`, but its UI relies on strict Menus of Legends (MoL). Fuzzy file picking is placed at `Space k d` (Pick > File). The fuzzy search scopes to the current working directory, which is why `Space j f` (Change Work Dir) is exposed to re-scope fuzzy search.

### Request 23
> "one more thing change work dir does indexing everytime? for e.g. setting home directory will have thousands of files? how it works"
**Response:**
Changing work directory does not index immediately. Indexing triggers dynamically when `Space k d` is opened. Ki does not keep a persistent database; it spawns a parallel `ignore::WalkBuilder` stream every time the prompt opens. In large un-ignored paths (like `$HOME`), this causes CPU and memory spikes because every discovered path creates an item in memory.

### Request 24
> "fix what we discussed for marking files, and opening multiple files from cli, also add my queries and requests to AGENTS.md first"
**Response:**
1. Added all user queries and responses to `AGENTS.md`.
2. Updated `EditArgs` to take multiple paths (`Vec<String>`).
3. Updated `RunConfig` and `App::run` to accept `Vec<AbsolutePath>`:
   - All specified CLI files are opened in background and pinned into `marked_files` (`[-]`).
   - The first file is focused as the active editor.
4. Updated `set_file_dirty_status`:
   - When a buffer becomes dirty (`dirty_status == true`), the file is automatically marked in `self.context.mark_file(...)` so preview files pin as tabs when modified.

### Request 25
> "reverse Pick d and f to f and d. f should be file and d should be Buffer. also this has any conceptual impact anywhere / or any other menus"
**Response:**
Confirmed no conceptual or functional side effects anywhere else. Ki's submenus are isolated modal scopes. Swapped `f` -> File (`FilePickerKind::NonGitIgnored`) and `d` -> Buffer (`FilePickerKind::Opened`) in `space_pick_keymap_legend_config` (`src/keymap.rs`) and updated `newdoc/index.html`.

---

## 2. Agent Required Context & Architecture Guide

### Project Architecture
- **Language / Framework:** Rust, Crossterm (terminal frontend), Tree-sitter (syntax parsing), Ropey (rope-backed text buffer), LSP Types (LSP client integration).
- **Branch:** `kustom_ki` (working branch for customizations).
- **Core State Tree:**
  - `src/app.rs`: Main event loop, input dispatch handling (`handle_event`, `handle_dispatches`), background task receivers.
  - `src/layout.rs`: Manages UI component tree (`ui_tree.rs`) and background editor map (`background_suggestive_editors: IndexMap<AbsolutePath, Rc<RefCell<SuggestiveEditor>>>`).
  - `src/buffer.rs`: Document state, rope manipulations, dirty status, disk reload and conflict logic.
  - `src/components/render_editor.rs`: Terminal cell rendering, visible viewport calculations, syntax styling, custom regex rules.
  - `src/screen.rs`: Terminal double-buffering diff algorithm.
  - `src/file_watcher/mod.rs`: `notify`-based filesystem watcher for open buffers and active directory trees.
  - `src/keymap.rs`: Normal, Insert, Selection, and Space Menu of Legends (MoL) configurations.

### Custom Features Implemented in `kustom_ki`
1. **Escape Alternatives:**
   - `ctrl+g` and `ctrl+[` act universally as `esc` in `App::handle_event`.
   - `jk` typed within 500ms in Insert mode exits to Normal mode (deletes `j` via Backspace and enters Normal mode).
2. **Scroll & Rendering Performance:**
   - Direct cell comparison fast-path in `Screen::diff` bypassing hash set creation.
   - Sliced visible byte-range regex highlighting in `render_editor.rs` avoiding full-buffer `Rope::to_string()` allocations per frame.
3. **Hunk Diff Stability:**
   - Multi-op inline changes iteration in `src/git/hunk.rs` preventing panic on complex diffs.
4. **File Reloading:**
   - Manual `Space j d` sends `force: true` to bypass equal timestamp caching.
   - Watcher accepts all modification events and file recreate/replacement events.

### Pending Agreed Next Steps
1. **Auto-mark CLI files on launch:**
   - In `App::run` (`src/app.rs`), mark `entry_path` so files passed on the command line start pinned as tabs (`[-]`).
2. **Auto-mark files when dirtied/modified:**
   - In `src/app.rs` (`set_file_dirty_status`), automatically mark the path into `self.context.get_marked_files()` when modified (`dirty_status == true`).

### Important Directives for Agents
- **Strict User Policy:** Do NOT run automatic verification/test loops unless explicitly asked. Make changes directly and use `cargo build` for syntax/compilation checking only.
