# Quickstart: Remove Top Menubar

**Feature**: Remove Top Menubar
**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

## Prerequisites

- Rust toolchain installed
- Zed source checkout at `../zed`

## Build

```bash
cargo build
```

Must complete with zero warnings.

## Validation Scenarios

### Scenario 1: No toolbar on launch

1. Run `cargo run`
2. **Expected**: The main window shows the tab sidebar and editor content directly below the window title bar. No row of buttons (New, Open, Save, Find, Replace, Compare, Recent) appears at the top.
3. The status bar at the bottom should still be visible with tab title, encoding, and tab count.

### Scenario 2: Keyboard shortcuts still work

While the editor is running:

1. Press `Ctrl+O` (or platform equivalent) → file picker should open
2. Press `Ctrl+S` → file should save (or save-as dialog if untitled)
3. Press `Ctrl+F` → search bar should appear
4. Press `Ctrl+H` → replace option should toggle
5. Press `Ctrl+N` → new tab should appear

### Scenario 3: Command center access

1. Press the command center shortcut (M-x or Ctrl+Shift+P)
2. Search for "Save", "Open", "Find", "Replace", "Compare"
3. **Expected**: All actions are listed and invokable

### Scenario 4: Unit tests pass

```bash
cargo test
```

All existing tests must pass — no new tests needed for a removal-only change.
