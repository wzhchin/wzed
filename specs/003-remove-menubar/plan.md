# Implementation Plan: Remove Top Menubar

**Branch**: `003-remove-menubar` | **Date**: 2026-06-13 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/003-remove-menubar/spec.md`

## Summary

Remove the top toolbar (New, Open, Save, Find, Replace, Compare, Recent buttons) from the editor UI. All underlying actions remain accessible via existing keyboard shortcuts and the command center (M-x). This is a pure subtraction — no new features, no new code, only removal of UI elements and their supporting state.

## Technical Context

**Language/Version**: Rust edition 2024

**Primary Dependencies**: Zed GPUI framework (path dependencies on `../zed/crates/*`)

**Storage**: File-based session persistence (`~/.config/wzed/session.json`)

**Testing**: `cargo test` for unit tests; manual integration testing via `test-step.md`

**Target Platform**: Linux (primary), Windows (secondary)

**Project Type**: Desktop application (single binary, single window)

**Performance Goals**: N/A — removal only, no performance impact expected

**Constraints**: Must compile with zero warnings; no `unwrap()` per clippy.toml

**Scale/Scope**: ~3700 LOC, 12 source files; changes touch 3 files

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Minimalist Scope | ✅ PASS | Removing a UI element aligns with minimalist scope. The toolbar is not essential — all actions have keyboard shortcuts. |
| II. Error Resilience | ✅ PASS | No new error paths; removal only. |
| III. No External Runtime Dependencies | ✅ PASS | No new dependencies. |
| IV. Existing File Discipline | ✅ PASS | Changes go into existing `topbar.rs`, `workspace.rs`, `main.rs`. No new files. |
| V. Zed Framework Delegation | ✅ PASS | No reimplementation of Zed functionality. |
| VI. Session and Data Safety | ✅ PASS | Removing `show_toolbar` from session state is safe — old sessions with it will just ignore the field on deserialization (serde default). Recent files tracking untouched. |

**Gate Result**: ✅ All principles pass. No violations to justify.

## Project Structure

### Documentation (this feature)

```text
specs/003-remove-menubar/
├── plan.md              # This file
├── spec.md              # Feature specification
├── checklists/          # Quality checklists
│   └── requirements.md
└── quickstart.md        # Validation guide
```

### Source Code (repository root)

```text
src/
├── main.rs              # Remove ToggleToolbar action + command_center dispatch entry
├── workspace.rs         # Remove show_toolbar field, show_recent_menu field,
│                         # handle_toggle_toolbar(), toolbar rendering in Render
├── topbar.rs            # Remove render_toolbar(), render_recent_menu(),
│                         # toolbar_btn(), toolbar_separator(). Keep render_status_bar().
└── (all other files)    # No changes needed
```

**Structure Decision**: Single project, changes confined to 3 existing source files. The `topbar.rs` module is kept because it still houses `render_status_bar()`.

## Complexity Tracking

> No violations — table not needed.

## Phase 0: Research

No unknowns or NEEDS CLARIFICATION markers in the spec. The codebase has been fully explored:

- **`topbar.rs`**: Contains `render_toolbar()` (line 13), `render_recent_menu()` (line 53), `toolbar_btn()` (line 192), `toolbar_separator()` (line 207), and `render_status_bar()` (line 215). The first four must be removed; the last must be kept.
- **`workspace.rs`**: `show_toolbar` field (line 120, initialized `true` at line 153), `show_recent_menu` field (line 123), `handle_toggle_toolbar()` method (line 786). In `Render::render()`, toolbar is conditionally built at line 865 and added as `.children(toolbar)` at line 1014. The `on_action` for `ToggleToolbar` is at line 1062.
- **`main.rs`**: `ToggleToolbar` action defined at line 62, dispatched at line 300-301. Imported in `workspace.rs` at line 29.

**Research conclusions**:
- Decision: Remove all toolbar rendering code and state; keep status bar rendering.
- Rationale: The spec says "remove the menubar" and the status bar is explicitly kept (FR-006).
- Alternatives considered: Make toolbar toggleable but default-off → rejected because spec says remove, not hide.

## Phase 1: Design & Contracts

### Data Model

No data model changes — this is a removal-only feature. The only state impact is:

- **Removed fields**: `show_toolbar: bool` and `show_recent_menu: bool` from `LiteWorkspace`
- **Session compatibility**: Old `session.json` files may contain extra fields; serde's default behavior ignores unknown fields on deserialization, so no migration needed.

### Contracts

No external interface changes. All actions (New, Open, Save, Find, Replace, Compare) remain registered and accessible via:
- Keyboard shortcuts (existing keybindings, unchanged)
- Command center (existing `command_center.rs` auto-discovers actions, unchanged)

### Implementation Steps

The changes decompose into a clean sequence across 3 files:

**Step 1: `topbar.rs`** — Remove toolbar rendering code
- Delete `render_toolbar()` function (lines 13-51)
- Delete `render_recent_menu()` function (lines 53-189)
- Delete `toolbar_btn()` function (lines 192-205)
- Delete `toolbar_separator()` function (lines 207-213)
- Remove unused imports: `PathBuf`, `CompareFiles`, `OpenFile`, `NewFile`, `SaveFile`, `ToggleFind`, `ToggleReplace`
- Keep `render_status_bar()` and its imports (`Tab`, `colors`)

**Step 2: `workspace.rs`** — Remove toolbar state and handler
- Remove `show_toolbar: bool` field from `LiteWorkspace` struct (line 120)
- Remove `show_recent_menu: bool` field from `LiteWorkspace` struct (line 123)
- Remove `show_toolbar: true` from constructor (line 153)
- Remove `show_recent_menu: false` from constructor (line 156)
- Delete `handle_toggle_toolbar()` method (lines 786-794)
- In `Render::render()`:
  - Remove `let toolbar = ...` line (line 865)
  - Remove `.children(toolbar)` from the main div (line 1014)
  - Remove `.on_action(cx.listener(Self::handle_toggle_toolbar))` (line 1062)
- Remove `ToggleToolbar` from the `use crate::{...}` import (line 29)

**Step 3: `main.rs`** — Remove action definition and dispatch
- Remove `ToggleToolbar` from the `actions!` macro (line 62, including its doc comment on line 61)
- Remove the `"lite_editor::ToggleToolbar"` match arm in command center dispatch (lines 300-302)

**Step 4: Verify**
- `cargo build` must succeed with zero warnings
- `cargo test` must pass
- Manual test: launch editor, confirm no toolbar row, confirm keyboard shortcuts work (Ctrl+S, Ctrl+O, etc.)
