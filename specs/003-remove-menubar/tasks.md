# Tasks: Remove Top Menubar

**Input**: Design documents from `/specs/003-remove-menubar/`

**Prerequisites**: plan.md (required), spec.md (required), quickstart.md (available)

**Tests**: Not requested — no test tasks included.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: No project setup needed — existing project, removal-only change.

No setup tasks required.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core removal that MUST be complete before user story validation.

- [x] T001 [US1] Remove `render_toolbar()`, `render_recent_menu()`, `toolbar_btn()`, and `toolbar_separator()` functions from `src/topbar.rs`
- [x] T002 [US1] Remove unused imports from `src/topbar.rs` (`PathBuf`, `CompareFiles`, `OpenFile`, `NewFile`, `SaveFile`, `ToggleFind`, `ToggleReplace`) — keep only `Tab`, `colors`, and used GPUI imports
- [x] T003 [US1] Remove `show_toolbar` and `show_recent_menu` fields from `LiteWorkspace` struct in `src/workspace.rs` (lines 120, 123)
- [x] T004 [US1] Remove `show_toolbar: true` and `show_recent_menu: false` initializations from `LiteWorkspace::new()` in `src/workspace.rs` (lines 153, 156)
- [x] T005 [US1] Delete `handle_toggle_toolbar()` method from `src/workspace.rs` (lines 786-794)
- [x] T006 [US1] Remove toolbar rendering from `Render::render()` in `src/workspace.rs`: delete `let toolbar = ...` (line 865) and `.children(toolbar)` (line 1014)
- [x] T007 [US1] Remove `.on_action(cx.listener(Self::handle_toggle_toolbar))` from `Render::render()` in `src/workspace.rs` (line 1062)
- [x] T008 [US1] Remove `ToggleToolbar` from `use crate::{...}` import in `src/workspace.rs` (line 29)
- [x] T009 [US1] Remove `ToggleToolbar` action from `actions!` macro in `src/main.rs` (lines 61-62, comment + action)
- [x] T010 [US1] Remove `"lite_editor::ToggleToolbar"` match arm from command center dispatch in `src/main.rs` (lines 300-302)

**Checkpoint**: `cargo build` succeeds with zero warnings. Toolbar code is fully removed.

---

## Phase 3: User Story 1 - Clean Editor View Without Menubar (Priority: P1) 🎯 MVP

**Goal**: Editor launches with no top toolbar — sidebar and editor fill the full height.

**Independent Test**: Launch `cargo run` and verify no button row appears between window title and sidebar.

### Implementation for User Story 1

All implementation tasks are in Phase 2 (T001-T010). This is a removal-only feature — the "implementation" is the removal itself.

**Checkpoint**: Run `cargo run`, confirm no toolbar row visible. Status bar still present at bottom.

---

## Phase 4: User Story 2 - Keyboard Shortcuts Still Work (Priority: P2)

**Goal**: All file operations (new, open, save, find, replace, compare) remain accessible via keyboard shortcuts.

**Independent Test**: While running, press Ctrl+S, Ctrl+O, Ctrl+F, Ctrl+H, Ctrl+N — each should trigger its action.

### Verification for User Story 2

- [x] T011 [US2] Verify no keyboard shortcut keybindings were removed — confirm keymap references in `src/main.rs` only reference actions that still exist
- [ ] T012 [US2] Manual validation: launch editor and test each keyboard shortcut per `quickstart.md` Scenario 2

**Checkpoint**: All keyboard shortcuts work as before.

---

## Phase 5: User Story 3 - Command Center Access (Priority: P3)

**Goal**: File operations are discoverable and invokable via the command center (M-x).

**Independent Test**: Open command center, search for "Save", "Open", "Find" — all should appear and work.

### Verification for User Story 3

- [x] T013 [US3] Verify command center dispatch table in `src/main.rs` still includes entries for NewFile, OpenFile, SaveFile, ToggleFind, ToggleReplace, CompareFiles
- [ ] T014 [US3] Manual validation: open command center and test action discovery per `quickstart.md` Scenario 3

**Checkpoint**: Command center lists and executes all file operation actions.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final cleanup and validation.

- [x] T015 Run `cargo build` and confirm zero warnings
- [x] T016 Run `cargo test` and confirm all existing tests pass
- [ ] T017 Run full quickstart.md validation (all 4 scenarios)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 2 (Foundational)**: No dependencies — start immediately
- **Phase 3 (US1)**: Depends on Phase 2 completion — validates the removal
- **Phase 4 (US2)**: Depends on Phase 2 completion — validates shortcuts still work
- **Phase 5 (US3)**: Depends on Phase 2 completion — validates command center still works
- **Phase 6 (Polish)**: Depends on all prior phases

### Task Dependencies Within Phase 2

- T001 and T002 (`topbar.rs`): T002 depends on T001 (remove functions first, then clean imports)
- T003 and T004 (`workspace.rs` fields): Can be done together
- T005, T006, T007 (`workspace.rs` render): T006 depends on T003 (remove field before removing usage)
- T008 (`workspace.rs` import): Depends on T005 (remove handler before removing import)
- T009 and T010 (`main.rs`): Can be done together

### Parallel Opportunities

- T001+T002 (`topbar.rs`) and T009+T010 (`main.rs`) can run in parallel (different files)
- T003+T004+T005+T006+T007+T008 (`workspace.rs`) must be sequential (same file)

---

## Parallel Example: Phase 2

```bash
# Parallel track A: topbar.rs cleanup
Task T001: "Remove toolbar functions from src/topbar.rs"
Task T002: "Remove unused imports from src/topbar.rs"

# Parallel track B: main.rs cleanup
Task T009: "Remove ToggleToolbar action from src/main.rs"
Task T010: "Remove ToggleToolbar dispatch from src/main.rs"

# Sequential track: workspace.rs (must be done after tracks A & B finish)
Task T003 → T004 → T005 → T006 → T007 → T008
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 2: All removal tasks (T001-T010)
2. **STOP and VALIDATE**: `cargo build` + `cargo run`, confirm no toolbar visible
3. This delivers the entire feature — US2 and US3 are validation-only (no code changes)

### Incremental Delivery

1. Phase 2 → Toolbar removed (MVP!)
2. Phase 4 → Confirm shortcuts work (verification only)
3. Phase 5 → Confirm command center works (verification only)
4. Phase 6 → Final build + test validation

---

## Notes

- This is a pure subtraction feature — no new code is written
- US2 and US3 require no code changes, only manual verification
- The `topbar.rs` module is kept because `render_status_bar()` still lives there
- Old `session.json` files work without migration (serde ignores unknown fields)
- Commit after Phase 2 completion (the entire functional change)
