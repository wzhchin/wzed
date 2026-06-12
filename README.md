# WZed

A lightweight text editor built on the [Zed](https://zed.dev) GPUI framework.

Single binary, single window, ~4200 lines of Rust. No debugger, no terminal, no extensions — just text editing.

## Features

- Tabbed editing with drag-and-drop, pinning, and tab groups
- Find & replace with regex support and multi-tab search
- Side-by-side diff view
- 15 languages with syntax highlighting (Rust, C/C++, Go, Python, TypeScript, CSS, JSON, YAML, Markdown, and more)
- Character encoding auto-detection (15 encodings supported)
- Command palette (`Alt+X`)
- Session persistence — reopen where you left off
- Single-instance IPC — files open in the running window
- Autosave with snapshot backups
- User-configurable keymap and settings

## Requirements

- Rust toolchain (edition 2024)
- A local checkout of the [Zed](https://github.com/zed-industries/zed) repository at `../zed`
- Linux (primary) or Windows

## Build & Run

```bash
cargo build                    # debug build
cargo run                      # launch editor
cargo run -- file1.rs file2.rs # open files on launch
```

## CLI Usage

```bash
wzed                       # open editor
wzed file.rs               # open a file
wzed -c "new-file"         # send command to running instance
wzed --list-commands       # list all available commands
```

IPC commands for a running instance:

```bash
wzed -c "open-file"                    # open file dialog
wzed -c "new-file"                     # new tab
wzed -c "save-file"                    # save current tab
wzed -c "set-text:hello world"         # set editor content
wzed -c "save-as:/path/to/file"        # save to path
wzed -c "switch-tab:2"                 # switch to tab index 2
```

## Configuration

Config files live in `~/.config/wzed/`:

| File | Purpose |
|---|---|
| `settings.json` | Font family, font size, tab size |
| `keymap.json` | Custom keybindings (Zed keymap format) |
| `session.json` | Open tabs and window state (auto-managed) |
| `recent.json` | Recently opened files (auto-managed) |

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl+N` | New file |
| `Ctrl+O` | Open file |
| `Ctrl+S` | Save |
| `Ctrl+Shift+S` | Save all |
| `Ctrl+W` | Close tab |
| `Ctrl+F` | Find |
| `Ctrl+H` | Replace |
| `Ctrl+Shift+F` | Search all tabs |
| `Alt+R` | Toggle regex |
| `F3` / `Shift+F3` | Find next / previous |
| `Ctrl+D` | Select next occurrence |
| `Ctrl+Shift+D` | Duplicate line |
| `Ctrl+Shift+K` | Delete line |
| `Alt+↑` / `Alt+↓` | Move line up / down |
| `Ctrl+/` | Toggle comment |
| `Ctrl+G` | Move tab to group |
| `Ctrl+Alt+D` | Compare files (diff) |
| `Ctrl+Shift+E` | Cycle encoding |
| `Alt+X` | Command palette |
| `Escape` | Dismiss panel |

## License

GPL-3.0-or-later
