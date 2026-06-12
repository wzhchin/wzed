# Tasks: Windows Support

**Input**: Design documents from `specs/001-windows-support/`

**Prerequisites**: plan.md (required), spec.md (required), research.md (optional), quickstart.md (optional)

**Tests**: Not requested — no test tasks included.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: `src/`, `Cargo.toml` at repository root
- Paths shown below use the actual project layout

---

## Phase 1: Setup

**Purpose**: Configure Cargo for Windows target and verify build infrastructure.

- [x] T001 Add conditional `gpui_windows` dependency in `Cargo.toml` using `[target.'cfg(windows)'.dependencies]`
- [x] T002 Move existing `gpui_linux` dependency into `[target.'cfg(unix)'.dependencies]` section in `Cargo.toml`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Platform initialization that MUST be complete before any user story can be tested on Windows.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T003 Replace hardcoded `gpui_linux::current_platform(false)` in `src/main.rs` with `cfg`-conditional platform initialization — use `gpui_linux::current_platform(false)` on Unix, `Rc::new(gpui_windows::WindowsPlatform::new(false).expect("failed to initialize Windows platform"))` on Windows
- [x] T004 Add `use std::rc::Rc;` import in `src/main.rs` (needed for Windows path wrapping `Rc::new`)
- [x] T005 Verify `src/main.rs` compiles on Linux with `cargo build` — must produce zero new errors

**Checkpoint**: Foundation ready — `cargo build` works on both Linux and Windows.

---

## Phase 3: User Story 1 - Build and Launch on Windows (Priority: P1) 🎯 MVP

**Goal**: WZed compiles and launches on Windows with full editing capability.

**Independent Test**: On a Windows machine, run `cargo build` then `cargo run -- file.rs`. Verify editor opens, file is displayed, editing and saving work.

### Implementation for User Story 1

- [ ] T006 [P] [US1] Verify `cargo build` on Windows (x86_64-pc-windows-msvc) compiles with zero WZed-specific errors ⚠️ BLOCKED: requires Windows machine
- [ ] T007 [P] [US1] Verify editor launches on Windows — window appears with dark theme, toolbar, and correct title "WZed" in `src/main.rs` window options ⚠️ BLOCKED: requires Windows machine
- [ ] T008 [US1] Verify file open (Ctrl+O), edit, and save (Ctrl+S) cycle works on Windows end-to-end ⚠️ BLOCKED: requires Windows machine
- [ ] T009 [US1] Verify all 35 unit tests pass on Windows with `cargo test` ⚠️ BLOCKED: requires Windows machine

**Checkpoint**: At this point, WZed is fully functional as a basic editor on Windows.

---

## Phase 4: User Story 2 - Single-Instance IPC on Windows (Priority: P2)

**Goal**: Second launch sends files/commands to the running instance via TCP IPC.

**Independent Test**: Launch WZed, then from a second terminal run `wzed second_file.rs` — verify the file opens as a new tab in the running instance.

### Implementation for User Story 2

- [x] T010 [US2] Add stale port lock cleanup to `listen_for_instances` in `src/ipc.rs` — before binding TCP listener, check if `wzed.port` exists; if so, try connecting to the stored port; if connection fails, remove the stale file
- [ ] T011 [US2] Verify `try_send_to_existing_instance` works on Windows — second `cargo run -- file.rs` opens file in running instance ⚠️ BLOCKED: requires Windows machine
- [ ] T012 [US2] Verify `try_send_command_to_existing_instance` works on Windows — `cargo run -- -c "new-file"` creates tab in running instance ⚠️ BLOCKED: requires Windows machine
- [ ] T013 [US2] Verify stale lock recovery — kill WZed process, verify new instance starts without manual cleanup of `wzed.port` ⚠️ BLOCKED: requires Windows machine

**Checkpoint**: Single-instance IPC works reliably on Windows including crash recovery.

---

## Phase 5: User Story 3 - Configuration and Session on Windows (Priority: P3)

**Goal**: Settings, session, recent files, and autosave snapshots work with Windows paths.

**Independent Test**: Open multiple files, close WZed, reopen — verify all tabs and content are restored.

### Implementation for User Story 3

- [ ] T014 [P] [US3] Verify `dirs::config_dir()` returns correct path (`%APPDATA%/wzed/`) on Windows — check that `settings.json`, `keymap.json`, `session.json` are created and read from the right location in `src/utils.rs` ⚠️ BLOCKED: requires Windows machine
- [ ] T015 [P] [US3] Verify `dirs::data_dir()` returns correct path for IPC socket/lock in `src/ipc.rs` on Windows ⚠️ BLOCKED: requires Windows machine
- [ ] T016 [US3] Verify session persistence — open 3 files, close editor, reopen; confirm all 3 tabs restore with content ⚠️ BLOCKED: requires Windows machine
- [ ] T017 [US3] Verify autosave snapshot creation and recovery in `src/workspace.rs` — confirm snapshots directory is created under `%APPDATA%/wzed/snapshots/` ⚠️ BLOCKED: requires Windows machine

**Checkpoint**: All session and configuration features work identically on Windows.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Documentation and regression verification.

- [x] T018 [P] Verify Linux regression — run `cargo build`, `cargo test`, and `cargo run` on Linux; confirm zero behavior changes
- [x] T019 [P] Update `README.md` with Windows-specific build prerequisites (MSVC toolchain, Visual Studio Build Tools)
- [x] T020 [P] Update `CLAUDE.md` architecture section to reflect `gpui_linux`/`gpui_windows` conditional dependency

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion — BLOCKS all user stories
- **User Stories (Phase 3–5)**: All depend on Foundational phase completion
  - US1 (Phase 3) can start after Phase 2
  - US2 (Phase 4) can start after Phase 2 (independent of US1)
  - US3 (Phase 5) can start after Phase 2 (independent of US1/US2)
- **Polish (Phase 6)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: No dependencies beyond Foundational — MVP
- **User Story 2 (P2)**: No dependencies beyond Foundational — independently testable
- **User Story 3 (P3)**: No dependencies beyond Foundational — independently testable

### Within Each User Story

- Verification tasks depend on the implementation tasks in Phase 2
- IPC stale-lock cleanup (T010) must complete before IPC verification (T011–T013)
- Path verification (T014, T015) can run in parallel; session verification (T016) depends on them

### Parallel Opportunities

- T006 and T007 can run in parallel (different verification aspects)
- T014 and T015 can run in parallel (different `dirs` functions)
- T018, T019, T020 can all run in parallel (different files)

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (Cargo.toml changes)
2. Complete Phase 2: Foundational (main.rs platform init)
3. Complete Phase 3: User Story 1 (build, launch, edit, save)
4. **STOP and VALIDATE**: Full editing cycle works on Windows
5. Deploy/demo if ready

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Editor works on Windows (MVP!)
3. Add User Story 2 → Test independently → IPC works on Windows
4. Add User Story 3 → Test independently → Session persistence works on Windows
5. Each story adds value without breaking previous stories

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Total tasks: 20
- Files modified: `Cargo.toml`, `src/main.rs`, `src/ipc.rs`, `README.md`, `CLAUDE.md`
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
