# Implementation Plan: Fix Encoding Switch Data Loss

**Branch**: `004-fix-encoding-switch` | **Date**: 2026-06-13 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/004-fix-encoding-switch/spec.md`

## Summary

Fix the encoding switch feature which currently destroys unsaved buffer content when cycling encodings via Ctrl+Shift+E. The fix adds: (1) dirty-buffer warning before reload, (2) an encoding picker submenu in the command center (replacing blind cycling), (3) encoding persistence in session state, and (4) error-safe reload that preserves buffer content on decode failure.

## Technical Context

**Language/Version**: Rust edition 2024

**Primary Dependencies**: Zed GPUI framework (editor, gpui, language crates from `../zed`), `encoding_rs` for encoding/decoding, `chardetng` for auto-detection

**Storage**: File-based session persistence at `~/.config/wzed/session.json`

**Testing**: `cargo test` for unit tests in `encoding.rs` (6 existing); manual integration testing via `test-step.md`

**Target Platform**: Linux (primary), Windows (secondary)

**Project Type**: Desktop GUI application (single binary, single window)

**Performance Goals**: Encoding picker must feel instant (< 16ms render); file reload time bounded by disk I/O

**Constraints**: No new external crates; all changes in existing source files (no new files per Constitution IV)

**Scale/Scope**: 15 supported encodings; single-window single-instance editor

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Minimalist Scope | ✅ PASS | Encoding display/switching is core text editing functionality |
| II. Error Resilience | ✅ PASS | Plan requires error-safe reload (preserve buffer on failure), no `unwrap()` |
| III. No External Runtime Dependencies | ✅ PASS | Uses existing `encoding_rs` and `chardetng` crates already in dependency tree |
| IV. Existing File Discipline | ✅ PASS | All changes in `workspace.rs`, `tab.rs`, `encoding.rs`, `command_center.rs` — no new files |
| V. Zed Framework Delegation | ✅ PASS | Uses GPUI picker pattern from command center, Zed Editor for buffer manipulation |
| VI. Session and Data Safety | ✅ PASS | Dirty-buffer warning prevents data loss; encoding persisted in session; error-safe reload preserves content |

**Post-Phase 1 re-check**: All gates remain PASS. No new files created. Encoding preference added to existing `SessionTab` struct.

## Project Structure

### Documentation (this feature)

```text
specs/004-fix-encoding-switch/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
src/
├── encoding.rs          # Core encoding utilities (add error-safe read, label lookup)
├── workspace.rs         # Main workspace: dirty check before reload, session persistence
├── tab.rs               # Tab struct (encoding field already exists)
├── command_center.rs    # Encoding picker submenu (already has ChangeEncoding)
├── topbar.rs            # Status bar (encoding display already works)
└── main.rs              # Action/keybinding (no changes needed)
```

**Structure Decision**: All changes in existing files. The command center already has a `ChangeEncoding` submenu — it just needs dirty-check and error-recovery logic added to its callback. The `SessionTab` struct in `workspace.rs` gains an `encoding` field.

## Complexity Tracking

No violations to justify. All gates pass.
