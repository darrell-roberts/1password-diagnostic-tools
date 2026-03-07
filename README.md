# diagnostic-tools

A Rust workspace for parsing and exploring 1Password `.1pdiagnostics` diagnostic report files.

## Crates

### diagnostic-parser

A library (and small demo binary) for parsing `.1pdiagnostics` files. A `.1pdiagnostics` file is a single JSON document containing system information, account metadata, structured log entries, and crash reports from a 1Password client.

**Highlights**

- Strongly-typed model covering system info, accounts, vaults, log files, and crash reports.
- Structured log-entry parser with level, source, thread, timestamp, and continuation/stack-trace extraction.
- Zero-copy parsing path (`parse_log_entries_ref`) with string interning for memory-efficient processing of large diagnostic files.

### diagnostic-tui

A terminal UI built with [ratatui](https://github.com/ratatui/ratatui) for interactively browsing diagnostic reports.

https://github.com/user-attachments/assets/4f2cfa4f-da9a-4c67-acd5-2fa5073fa981

**Features**

- **Overview** tab — system details, account and vault summaries.
- **Logs** tab — scrollable log viewer with:
  - Level filtering (trace / debug / info / warn / error)
  - Source and log-file filtering
  - Full-text search
  - Detail pane with stack traces
- **Crash Reports** tab — crash report list with linked panic log entries.
- Mouse scroll support and keyboard navigation.

### Installation
1. Clone the repository. 
2. Install the TUI binary.

```bash
cargo install --path diagnostic-tui
```

## Building

Requires **Rust 1.85+** (edition 2024).

```
cargo build
```

To build in release mode:

```
cargo build --release
```

## Usage

### CLI summary

### TUI viewer

```
diagnostic-tui path/to/file.1pdiagnostics
```

Navigate with the keyboard:

| Key | Action |
|---|---|
| `Tab` / `Shift+Tab` | Switch tabs |
| `↑` / `↓` / `PgUp` / `PgDn` | Scroll lists |
| `Home` / `End` | Jump to start / end |
| `Enter` | Toggle detail pane |
| `/` | Start search |
| `Esc` | Cancel search / close picker |
| `l` | Cycle log-level filter |
| `s` | Open source filter picker |
| `f` | Open log-file filter picker |
| `?` | Toggle help |
| `q` / `Ctrl+c` | Quit |

## License

This project is licensed under the [MIT License](LICENSE).
