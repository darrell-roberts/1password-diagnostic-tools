# diagnostic-tui

A terminal user interface for viewing, searching, and filtering 1Password
`.1pdiagnostics` diagnostic report files. Built with
[ratatui](https://github.com/ratatui/ratatui) on top of the
[diagnostic-parser](../diagnostic-parser) library crate.

## Features

- **Three-tab interface** — Overview, Logs, and Crash Reports, each tailored to
  a specific aspect of the diagnostic report.
- **Live search** — Incrementally filter log entries by message, source
  component, thread, log file title, or stack-trace content (case-insensitive).
- **Log level filtering** — Cycle through severity thresholds
  (ALL → ERROR → WARN+ → INFO+ → DEBUG+ → ALL) with a single keypress.
- **Source component filtering** — Cycle through the distinct source components
  found in the report (e.g. `1P`, `client`, `status`), or open a picker popup
  to select one directly.
- **Log file filtering** — Filter entries by the log file they belong to. Cycle
  through available log files, open a picker popup, or press a single key to
  combine all logs back into the default view.
- **Split-pane log viewer** — Scrollable entry list on the left with a detail
  pane on the right showing full metadata, message text, and color-coded stack
  traces.
- **Crash report correlation** — Automatically links each crash report entry to
  its corresponding panic log entry and displays the full call stack.
- **Vim-style navigation** — `j`/`k` movement, `g`/`G` for home/end, `/` for
  search, plus arrow keys, Page Up/Down, and Home/End.
- **Help overlay** — Press `?` at any time for a quick-reference keybinding
  guide.

## Requirements

- Rust **1.85+** (edition 2024)
- The `diagnostic-parser` crate located at `../diagnostic-parser` (sibling
  directory)

## Installation

Install directly via Cargo:

```sh
cargo install --path .
```

This places the `diagnostic-tui` binary in your Cargo bin directory
(`~/.cargo/bin` by default). Make sure it is on your `PATH`.

## Run

```sh
diagnostic-tui path/to/file.1pdiagnostics
```

## Tabs

### 1 — Overview

Displays a scrollable summary of the entire diagnostic report:

- **Report information** — UUID and creation timestamp.
- **System** — Client name, build number, OS, processor, memory, disk space,
  locale, lock state, and install path.
- **Overview counters** — Number of accounts, vaults, active items, and inactive
  items.
- **Accounts** — Per-account metadata (URL, type, state, billing status, storage
  used) with nested vault listings showing item counts.
- **Feature flags** — All active feature flags on the client.
- **Log file statistics** — File count, total line count, parsed entry count,
  and a per-level breakdown (ERROR, WARN, INFO, DEBUG, TRACE).
- **Crash report count**.

### 2 — Logs

A two-pane log viewer with a search bar and filter controls:

| Area | Description |
|------|-------------|
| Search bar | Type `/` to activate. Filters entries in real time across message, source, thread, log file title, and continuation lines. |
| Filter bar | Shows the active level filter (`f` to cycle), source filter (`s` to cycle), log file filter (`l` to cycle), and the matched/total entry count. |
| Entry list (left) | Scrollable list showing level, timestamp, truncated message, and a `+` marker for entries with stack traces. |
| Detail pane (right) | Full entry metadata: level, timestamp, thread, source component, file path, line number, log file title, message text, and stack trace (if present). |

Press `→`, `Enter`, or `d` to focus the detail pane for scrolling. Press `←` or
`Esc` to return focus to the list.

### 3 — Crash Reports

A two-pane crash report viewer:

| Area | Description |
|------|-------------|
| Crash list (left) | All crash report entries showing type, timestamp, and report ID. |
| Crash detail (right) | Report ID, type, timestamp, and diagnostic tag. If a matching panic log entry is found, displays the linked entry's log file, thread, source, timestamp, message, and full call stack with alternating colors for readability. |

## Keybindings

### Global

| Key | Action |
|-----|--------|
| `Tab` | Next tab |
| `Shift+Tab` | Previous tab |
| `1` / `2` / `3` | Jump to Overview / Logs / Crash Reports |
| `?` | Toggle help overlay |
| `q` | Quit |
| `Ctrl+c` | Force quit |

### Navigation (all tabs)

| Key | Action |
|-----|--------|
| `↑` / `k` | Move up / scroll up |
| `↓` / `j` | Move down / scroll down |
| `Page Up` | Page up |
| `Page Down` | Page down |
| `Home` / `g` | Jump to first item |
| `End` / `G` | Jump to last item |

### Logs & Crash Reports tabs

| Key | Action |
|-----|--------|
| `→` / `Enter` / `d` | Focus detail pane (toggle) |
| `←` | Return to list pane |
| `Esc` | Unfocus detail / clear search |

### Logs tab only

| Key | Action |
|-----|--------|
| `/` | Open search bar |
| `Esc` (in search) | Close search bar |
| `Enter` (in search) | Confirm and close search bar |
| `Backspace` (in search) | Delete last character |
| `f` | Cycle log level filter: ALL → ERROR → WARN+ → INFO+ → DEBUG+ → ALL |
| `s` | Cycle source component filter |
| `S` | Open source component picker |
| `a` | Reset to all sources |
| `l` | Cycle log file filter |
| `L` | Open log file picker |
| `A` | Combine all logs (reset log file filter to default view) |

## Project Structure

```
src/
├── main.rs     Entry point — CLI arg parsing, terminal setup, event loop
├── app.rs      Application state, input handling, filtering logic
│                 Tab, InputMode, LevelFilter, SourceFilter, LogFileFilter,
│                 ListState, App
└── ui.rs       Rendering for all views
                  Tab bar, status bar, overview, log list/detail,
                  crash list/detail, search bar, filter bar,
                  source picker, log file picker, help overlay
```

### Module Responsibilities

**`main.rs`** — Parses the `.1pdiagnostics` file path from command-line
arguments, loads the report via `DiagnosticReport::from_file()`, initializes the
crossterm backend and ratatui terminal, and runs the synchronous event loop.

**`app.rs`** — Owns all mutable application state. `App::new()` parses log
entries once at startup and pre-computes the source component list. Filtering is
done via `App::refilter()` which rebuilds a `filtered_indices` vector of indices
into the immutable `all_entries` list. All keyboard input is routed through
`App::handle_key()` which dispatches to mode-specific handlers (normal
navigation vs. search input).

**`ui.rs`** — Pure rendering code. The top-level `draw()` function dispatches to
per-tab drawing functions. Log levels are color-coded consistently throughout
(red for ERROR, yellow for WARN, green for INFO, cyan for DEBUG, dark gray for
TRACE). The detail panes clone entry data before rendering to avoid borrow
conflicts with scroll state. Picker popups (source component and log file) are
rendered as centered overlays on top of the main content.

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `diagnostic-parser` | path | Parsing and data model for `.1pdiagnostics` files |
| `ratatui` | 0.29 | Terminal UI framework (widgets, layout, styling) |
| `crossterm` | 0.28 | Cross-platform terminal I/O and event handling |
| `chrono` | 0.4 | Timestamp formatting in the UI |

## License

This project is licensed under the [MIT License](LICENSE).
