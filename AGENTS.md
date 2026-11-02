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

### Request 26
> "ki tries to restore previous file (or marked files?) in any case they don't play well with our changes of marking files on open, another issue even if I clear out all the existing files open newly couple of files, the tab strip as well as commands left/right (or next/prev) no idea where is the file, they switch the buffer view but none of the window elements like tab bar highlights are bottom bar file updates properly"
> "you are in loop, the main issue is just the tabbar last element is not getting highlighted, nothing much"
**Response:**
Diagnosed the highlighting bug: the performance optimization evaluated `regex_highlight_rules` against `visible_slice_str`, which truncated the trailing zero-width space `\u{200B}` for the last tab in the title editor. Restored full-buffer string matching for `regex_highlight_rules` in `src/components/render_editor.rs`. Committed as `77728bcff`.

### Request 27
> "what is the status bar default configuration is ? after language (rust or md etc) what is shows in "quotes" sometimes it is the first file, but doesn't update as I move to another file, is it some search term?"
**Response:**
Confirmed it is `LastSearchString` configured in `status_lines` in `src/config_default.json`. It displays the persisted search history entry (`format!("{search:?}")`) from `PromptHistoryKey::Search`, which is global and does not vary per file buffer.

### Request 28
> "how to specify column in shift+m ?"
> "yes"
**Response:**
Extended `DispatchParser::MoveSelectionByIndex` in `src/app.rs` and added `goto_position` in `src/components/editor/mod.rs` to parse `line:col` or `line col` (e.g. `42:10` or `42 10`) and navigate to the specified line and column (1-indexed).

### Request 29
> "how index works on node"
**Response:**
Explained that in Node mode (`n`), `to_index` iterates Tree-sitter's top-level named AST children (`TopNode.iter`), so entering an index $N$ jumps directly to the $N$-th top-level statement, function, struct, or module definition in the document.

### Request 30
> "what happens with our change if I give 10 12"
**Response:**
Explained that entering two numbers separated by space or colon triggers the `line:col` parser, placing the cursor directly at Line 10, Column 12 rather than using mode-relative indexing.

### Request 31
> "I put it in node and I jump it becomes Line is it right?"
**Response:**
Confirmed the bug: `goto_position` was constructing a default `SelectionSet`, which reverted `mode` back to `SelectionMode::Line`. Updated `goto_position` to preserve `self.selection_set.mode` and snap to the AST node at the target coordinates when in structured modes like Node mode.

### Request 32
> "when I scroll file tree or buffer with mouse scroll after last line exits the view, ki crashes out"
> "assertion failed: visible_line_range.start <= visible_line_range.end"
**Response:**
When scrolling past the end of the file, `scroll_offset` exceeded `buffer.len_lines()`, while `end` was clamped to `buffer.len_lines()`, creating an inverted range `start > end`. Fixed by:
1. Clamping `scroll_offset` in `apply_scroll` to `buffer.len_lines().saturating_sub(1)`.
2. Clamping `start` to `total_lines` in `visible_line_range_given_scroll_offset_and_height`.
3. Guarding `line_range_to_byte_range` against `start >= end`, returning an empty range safely without assertion panic.

### Request 33
> "why space is not used as toggle for HUD (not talking about momemtary layer lock) but when space opens why not dismiss with another space ?"
**Response:**
Explained that `MenuKeymapOverride` only recognized `Esc` as closing the HUD and swallowed any unregistered keys.

### Request 34
> "I think space should close itself for ergonomic reason, keep the lock functionality and every other functionality intact when doing this"
**Response:**
Added `key!("space")` alongside `key!("esc")` as a close trigger in `MenuKeymapOverride::handle_press` (`src/keymap_override/menu.rs`). Momentary layers and joint layers use separate override structs and retain their hold/release lock functionality.

### Request 35
> "I think space hud has a bug when window is small, it tries to to show two halves of keyboad as split view, I gave config with "show_key_in_keymap". when split keyboards are stacked one after another, the right and left part shows same set of left handed keys"
**Response:**
Identified bug in `display_one_side` in `src/components/editor_keymap_printer.rs`: `row.iter().skip(skip).take(take).enumerate()` indexed the keyboard layout using local `col_index` (0..5) instead of `skip + col_index`. For the right-half table (`skip = 5`), it was retrieving keys 0..5 (left hand) instead of 5..10 (right hand). Fixed by using `layout_col_index = skip + col_index`.

### Request 36
> "The file mark "._blah_.swp" is removed from the list as it cannot be opened due to the following error: stream did not contain valid UTF-8"
> "silent unmark without popup"
**Response:**
Removed the global info error popup in `cycle_marked_file` (`src/app.rs`). When encountering an unopenable file during cycling (e.g. non-UTF-8 binary or deleted path), Ki silently removes the mark and immediately cycles to the next available file without interrupting with a modal info box.

### Request 37
> "ki file editor allows search and select commands; so if I do f and it selects all folders and files, resulting some swp and other files, this creates few issues,
> sometimes it error out remove all files from tabbar and view list (ERROR
> 1│Couldn't find component with id ComponentId(1505))
> 2. the tablist keeps growing multiple lines, for e.g. 500 files it has 500 buttons wrapped into lines; we need to make it so it only takes 15 to 20 percent of screen, if more tab buttons are reqired, they should start scrolling horizontally"
**Response:**
1. Fixed `Couldn't find component with id ComponentId(...)` in `src/layout.rs`: `update_highlighted_spans` previously panicked/returned an `Err` when async syntax highlighting responses arrived for components/buffers that were already closed or replaced. Changed it to safely ignore highlighting responses for closed components without raising a global error.
2. Capped tabbar height to at most 15% to 20% of the screen height in `src/components/render_editor.rs`. When there are more tabs than fit in that vertical quota, the tablist scrolls horizontally around the active focused tab so the active file is always visible and editor space is preserved.

### Request 38
> "based on previous saved session trying to close file
> ERROR
> 1│The path "/home/santh/.nix-profile" is not a file.
> also don't move tab buttons location/count on tab rows on navigation it is jarring; also instead of percentage, let us go with percentage+max rows, don't show more than 5 rows even 20pc allows 10 rows, and upto 1 row when no space, it can be between 1 to 5 rows based on available 15/20pc"
> "since I assume the tab strip is also editor, show row numbers and row indicator [row 2/count]"
> "don't keep testing and verifying"
**Response:**
1. In `src/context.rs`, filtered restored persisted marks with `.filter(|path| path.is_file())` to prevent non-file entries (e.g. directory symlinks like `~/.nix-profile`) from being restored as file tabs.
2. In `src/app.rs` (`close_current_window` and `toggle_file_mark`), wrapped unmarking navigation in a loop that silently skips any unopenable non-file paths without throwing an error popup.
3. In `src/components/render_editor.rs`:
   - Configured tab row count between 1 and 5 rows (capped by 20% of window height and min 5).
   - Preserved fixed wrapping structure (`wrap_items(formatted_paths, ...)` over all items) so tab button placement across rows never jumps or changes during navigation.
   - Displayed row numbers and row indicator prefix (`N│[row N/total]`) for multi-row tab lines.

### Request 39
> "row indicator should be at bottom right not at every row"
> "give its own row, it is not visible as part of last tab row"
**Response:**
Updated `src/components/render_editor.rs` so row numbers (`N│`) remain on the left of each row, while the row indicator (`[row N/total]`) is placed on its own dedicated row right-aligned at the bottom of the tab strip.

### Request 40
> "when the names are huge word wrap happens and when it happens, indicator disappears, also use 01/001/0001 based on file count so left is aligned"
**Response:**
1. Zero-padded line numbers on the left (`01│`, `001│`, `0001│`) based on total file count digits (minimum 2 digits) so left-aligned gutters remain perfectly aligned.
2. Fixed indicator disappearance: `wrap_items` previously wrapped using `available_width` without accounting for the line number prefix width (`01│`), which caused long lines to exceed terminal width during `render_content` soft-wrapping, pushing the bottom indicator row out of the vertical viewport. Now `tab_content_width = available_width.saturating_sub(prefix_width)` is used for wrapping so no unexpected soft-wrapping occurs.

### Request 41
> "for aesthetic reason just show row count at start indicator row no other text, so instead of [row 006/045] it will show 045| Total tab rows"
**Response:**
Updated the indicator row in `src/components/render_editor.rs` to start with the aligned total row count gutter followed by "Total tab rows" (e.g. `045│ Total tab rows`) with no extra text.

### Request 42
> "again for aesthetic reason show , 045|, left end , row count, at right end"
**Response:**
Updated `src/components/render_editor.rs` so the indicator row displays `045│` at the left end and right-aligns `Total tab rows` (or total count info) across to the right edge.

### Request 43
> "make it (tab rows) instead of Total tab rows since editor text follows immediately it is jarring"
**Response:**
Changed right-aligned indicator text from `Total tab rows` to `(tab rows)` in `src/components/render_editor.rs`.

### Request 44
> "can you also align tab buttons on [-]"
**Response:**
Stripped the extra leading padding space in `src/components/render_editor.rs` (`strip_prefix(' ')` / `strip_prefix("\u{200b} ")`) from the start of each wrapped line, so that the bracket `[-]` aligns flush against the `│` border on every tab row.

### Request 45
> "not working well, for this to work, it has to be calculated how many buttons can be placed in row, so it can accommodate width of file name currntly not every file marked indicator aligned at every column at every row, also some columns have 5 tabs, some 4 tabs"
> "if row 1 based on file name size (let us be linient to some extent say 25, after that it will be truncated no need of ellipses, now if first files that occupy row 1 let us say 1 - 6, rest of the rows follow same pattern and gets truncated to the first row column regardless of filename size, now if first two file names are monstrous and doesn't even accomodate window width we will only have one row, which means every row will have only one file, to avoid it, if second row can accomodate more rows then it will dictate the column size for subsequent, if that is also bigger or equal to first column, then third, at any point if row that is below can accomodate more files that previous row, that that will dictate column for subsequent rows"
**Response:**
Implemented dynamic column-grid layout in `src/components/render_editor.rs`:
1. Pre-caps tab item display width to 25 characters without ellipses.
2. Determines row 1 column widths greedily up to terminal width.
3. If a subsequent row can accommodate more columns than the current active columns (e.g. following smaller file names after large ones), that improved column count and width configuration takes over for subsequent rows.
4. Pads or truncates each cell to its column width so brackets `[-]` in every column align vertically across all rows.

### Request 46
> "how can we aesthetically show file name in row count bar? left end, 02|(row count), [filename], right end?"
> "option 3 as option 1,2 repeats/clutter, option 4 more numbers/clutter, now you should also show row indicator row even when only one tab is available"
**Response:**
Updated the indicator row in `src/components/render_editor.rs` (Option 3):
- Shown consistently even when only one tab is open.
- Left end: `01│ (tab rows)`
- Right end: `[filename]` (e.g. `[src/components/render_editor.rs]`).
- Row numbers (`01│`) are now always rendered on all tab lines.

### Request 47
> "make the tabbar background and text same as status bar"
> "let us keep using exising fg/bg but fix the theme preview, which is not updating tabbar fg/bg when theme changes, also file icons don't adapt to theme colors, okey even file manager shows terrible icons for some theme, so it is existing issue, so fix the preview for tabbar fg/bg alone for now"
**Response:**
Identified that when the Theme picker prompt is open, the Prompt is the focused component in `self.layout.focused_component_id()`. This caused the underlying `SuggestiveEditor` to be treated as `focused = false`, so its tab bar styled with `StyleKey::UnfocusedWindowTitle` (which in Zed themes uses `tab_inactive_background`, often dark/transparent) instead of `StyleKey::FocusedWindowTitle` (`StyleKey::UiFocusedTab`).
In `src/app.rs` (`render_component_window`), updated `focused` to evaluate to `true` for `SuggestiveEditor` while a Prompt is open, ensuring the tabbar and active tab immediately reflect the previewed theme's focused colors in real time.

### Request 48
> "the buffer picker doesn't list all the files/tabs opened"
**Response:**
`FilePickerKind::Opened` in `App::open_file_picker` previously only queried `self.layout.get_opened_files()`, which only contained buffers loaded into memory (`background_suggestive_editors`), omitting persisted or marked tabs that haven't yet been loaded into memory. Updated it to merge `self.context.get_marked_files()` with `self.layout.get_opened_files()` into an ordered `IndexSet`, ensuring all open tabs and active buffers appear in the buffer picker.

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
5. **Auto-marking & Multi-file CLI:**
   - Multiple CLI paths accepted on startup (`ki file1.rs file2.rs`), auto-marked as tabs with first file focused.
   - Modifying an unmarked preview file auto-marks it (`dirty_status == true`).
6. **Mnemonic Pick Menu:**
   - `Space k f` &rarr; File picker, `Space k d` &rarr; Buffer picker.
7. **Line:Column Jump in Shift+M:**
   - `Shift+M` accepts `line:col` or `line col`, preserving active selection mode (e.g. Node mode).

### Important Directives for Agents
- **Strict User Policy:** Do NOT run automatic verification/test loops unless explicitly asked. Make changes directly and use `cargo build` for syntax/compilation checking only.
