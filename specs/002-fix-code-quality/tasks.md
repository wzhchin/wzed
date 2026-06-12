# Tasks: Code Quality Fixes

**Input**: Design documents from `/specs/002-fix-code-quality/`

**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/

**Tests**: Not requested — no test tasks generated.

**Organization**: Tasks grouped by user story for independent implementation.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- **Single project**: `src/` at repository root
- All paths relative to project root

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add missing legal files and centralized configuration constants

- [x] T001 Add GPL-3.0-or-later LICENSE file to repository root (source official text from GNU)
- [x] T002 [P] Create CHANGELOG.md at repository root, populated from git log history
- [x] T003 Add `AppConfig` struct to `src/utils.rs` with fields: `autosave_interval_secs` (30), `file_watcher_poll_secs` (5), `notification_display_secs` (4), `snapshot_retention_days` (7), `max_recent_files` (20)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Replace hardcoded magic numbers with `AppConfig` references — must complete before story work begins

- [x] T004 Replace hardcoded duration literals in `src/workspace.rs` with `AppConfig` constants: `from_secs(30)` at L185, `from_secs(5)` at L197, `from_secs(4)` at L217, `7 * 24 * 3600` at L100
- [x] T005 [P] Replace `20` max recent files literal in `src/recent_files.rs` L45 with `AppConfig::max_recent_files`
- [x] T006 [P] Verify `cargo build` and `cargo test` pass after AppConfig changes

**Checkpoint**: All magic numbers centralized, build and tests green

---

## Phase 3: User Story 1 - Errors Are Visible to Users (Priority: P1) 🎯 MVP

**Goal**: Every user-facing operation failure shows a notification instead of silently doing nothing

**Independent Test**: Open a nonexistent file → notification appears. Trigger IPC to dead instance → error printed.

### Implementation for User Story 1

- [x] T007 [US1] Replace `.ok()` at `src/workspace.rs:495` (file open) with `show_notification()` on error
- [x] T008 [US1] Replace `.ok()` at `src/workspace.rs:1166` (drag-drop file open) with `show_notification()` on error
- [x] T009 [P] [US1] Replace `.ok()` at `src/command_center.rs:166` (file open from command palette) with `show_notification()` on error
- [x] T010 [P] [US1] Replace `.ok()` at `src/search.rs:233` (regex compilation) with error propagation to caller, then notification from workspace
- [x] T011 [P] [US1] Replace `.ok()` at `src/main.rs:253` (IPC file open) with `eprintln!()` and early return
- [x] T012 [P] [US1] Replace `.ok()` at `src/main.rs:345` (IPC operation) with `eprintln!()` and early return
- [x] T013 [P] [US1] Replace `.ok()` at `src/main.rs:384` (IPC file open) with `eprintln!()` and early return
- [x] T014 [P] [US1] Replace `.ok()` at `src/main.rs:411` (IPC operation) with `eprintln!()` and early return
- [x] T015 [P] [US1] Replace `.ok()` at `src/file_watcher.rs:37,38` (metadata + modified) with `.log_err()`
- [x] T016 [P] [US1] Replace `.ok()` at `src/file_watcher.rs:75,76` (metadata + modified) with `.log_err()`
- [x] T017 [P] [US1] Replace `.ok()` at `src/recent_files.rs:39,40` (JSON parse errors) with `.log_err()`
- [x] T018 [P] [US1] Replace `.ok()` at `src/workspace.rs:105` (cleanup) with `.log_err()`
- [x] T019 [P] [US1] Replace `.ok()` at `src/workspace.rs:223` (timer update) with `.log_err()`
- [x] T020 [P] [US1] Replace `.ok()` at `src/workspace.rs:497` (async update) with `.log_err()`
- [x] T021 [P] [US1] Replace `.ok()` at `src/workspace.rs:884` (diff state) with `.log_err()`
- [x] T022 [P] [US1] Replace `.ok()` at `src/topbar.rs:173` (operation) with `.log_err()`

**Checkpoint**: Zero `.ok()` calls remain in non-test code; all errors routed to notification/log/eprintln

---

## Phase 4: User Story 2 - Code Complies With Its Own Rules (Priority: P1)

**Goal**: `cargo clippy` reports zero disallowed-method warnings in non-test code

**Independent Test**: Run `cargo clippy` → no unwrap/expect warnings

### Implementation for User Story 2

- [x] T023 [US2] Replace `expect()` at `src/main.rs:137` (Windows platform init) with `?` operator and descriptive error print
- [x] T024 [P] [US2] Replace `expect()` at `src/main.rs:402` (window opening) with `?` operator and descriptive error print
- [x] T025 [P] [US2] Replace `unwrap()` at `src/workspace.rs:1143` (notification rendering) with `if let` pattern
- [x] T026 [US2] Verify `cargo clippy` passes with zero disallowed-method warnings in non-test code

**Checkpoint**: Clippy clean, constitution Principle II fully satisfied

---

## Phase 5: User Story 3 - Required Legal and Documentation Files (Priority: P2)

**Goal**: LICENSE and CHANGELOG exist and are correct

**Independent Test**: `head -1 LICENSE` shows GNU text; `head -1 CHANGELOG.md` shows changelog header

### Implementation for User Story 3

- [x] T027 [US3] Verify LICENSE file from T001 contains exact GPL-3.0-or-later text (compare against official GNU source)
- [x] T028 [P] [US3] Verify CHANGELOG.md from T002 has entries for all past commits

**Checkpoint**: Legal compliance verified

---

## Phase 6: User Story 4 - Workspace Decomposed (Priority: P2)

**Goal**: workspace.rs under 800 lines, logic in focused modules

**Independent Test**: `wc -l src/workspace.rs` shows < 800; `cargo build` succeeds; `cargo test` passes

### Implementation for User Story 4

- [x] T029 [US4] Extract search find/replace navigation methods (~200 lines, find_next, find_previous, replace_current, replace_all, search_all_tabs) from `src/workspace.rs` to `src/search.rs` as functions taking workspace state as parameters
- [x] T030 [US4] Extract file comparison logic (~50 lines, compare_with_file, update_diff_view) from `src/workspace.rs` to `src/diff_view.rs` as functions taking workspace state as parameters
- [x] T031 [US4] Update `src/workspace.rs` to call the extracted functions, verify `cargo build` passes
- [x] T032 [US4] Verify `src/workspace.rs` line count is under 800 and `cargo test` passes (33 tests green)

**Checkpoint**: Workspace decomposed, under 800 lines, all tests pass

---

## Phase 7: User Story 5 - Configuration Constants Centralized (Priority: P3)

**Goal**: All tunable constants accessed via AppConfig

**Independent Test**: `grep -rn 'from_secs(4)\|from_secs(30)\|from_secs(5)\|7 \* 24' src/` returns zero matches

### Implementation for User Story 5

- [x] T033 [US5] Verify no hardcoded tunable constants remain outside `AppConfig` in `src/workspace.rs`
- [x] T034 [US5] Verify no hardcoded tunable constants remain outside `AppConfig` in `src/recent_files.rs`
- [x] T035 [US5] Search all remaining source files for any missed tunable literals and extract to `AppConfig` in `src/utils.rs`

**Checkpoint**: Zero hardcoded magic numbers outside AppConfig

---

## Phase 8: Polish & Validation

**Purpose**: Final verification across all stories

- [x] T036 Run full quickstart.md validation: `cargo build && cargo test && cargo clippy`
- [x] T037 [P] Verify `grep -n '\.ok()' src/*.rs` returns zero results in non-test code
- [x] T038 [P] Verify `wc -l src/workspace.rs` shows under 800
- [x] T039 Verify `test -f LICENSE && test -f CHANGELOG.md`
- [ ] T040 Manual smoke test per `test-step.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 (T003 AppConfig must exist)
- **User Stories (Phase 3-7)**: All depend on Phase 2 completion
  - US1 (Phase 3) and US2 (Phase 4) can proceed in parallel
  - US3 (Phase 5) is verification-only of Phase 1 artifacts
  - US4 (Phase 6) can proceed in parallel with US1/US2 but touches overlapping files — coordinate carefully
  - US5 (Phase 7) is verification-only, depends on Phase 2
- **Polish (Phase 8)**: Depends on all user stories complete

### User Story Dependencies

- **US1 (P1)**: Depends on Phase 2. No dependency on other stories.
- **US2 (P1)**: Depends on Phase 2. No dependency on US1.
- **US3 (P2)**: Depends on Phase 1 (T001, T002). Verification only.
- **US4 (P2)**: Depends on Phase 2. Overlaps with US1 on workspace.rs — implement US4 after US1 or coordinate file edits.
- **US5 (P3)**: Depends on Phase 2. Verification only.

### Within Each User Story

- Error site replacements marked [P] can run in parallel (different locations)
- Workspace decomposition (US4) must be sequential (extract → update → verify)

### Parallel Opportunities

- T001, T002 can run in parallel (different files)
- T007-T022 (US1 .ok() replacements) can mostly run in parallel — different call sites in different files
- T023-T025 (US2 unwrap/expect) can run in parallel
- T015-T022 (background .ok() → .log_err()) can all run in parallel

---

## Parallel Example: Phase 3 (US1)

```bash
# These can all be done simultaneously — different files, different call sites:
T007  workspace.rs:495   → show_notification
T008   workspace.rs:1166 → show_notification
T009  command_center.rs  → show_notification
T011     main.rs:253     → eprintln + return
T012     main.rs:345     → eprintln + return
T013     main.rs:384     → eprintln + return
T014     main.rs:411     → eprintln + return
T015  file_watcher.rs:37 → log_err
T016  file_watcher.rs:75 → log_err
T017  recent_files.rs    → log_err
T018  workspace.rs:105   → log_err
T019  workspace.rs:223   → log_err
T020  workspace.rs:497   → log_err
T021  workspace.rs:884   → log_err
T022  topbar.rs          → log_err
```

---

## Implementation Strategy

### MVP First (US1 + US2)

1. Complete Phase 1: Setup (T001-T003)
2. Complete Phase 2: Foundational (T004-T006)
3. Complete Phase 3: US1 — kill all `.ok()` (T007-T022)
4. Complete Phase 4: US2 — kill all `unwrap()`/`expect()` (T023-T026)
5. **STOP and VALIDATE**: `cargo clippy` clean, no `.ok()` in grep
6. This is the MVP — editor no longer silently swallows errors

### Incremental Delivery

1. MVP (above) → core error handling fixed
2. Add US3 (T027-T028) → legal compliance verified
3. Add US4 (T029-T032) → workspace decomposed
4. Add US5 (T033-T035) → magic numbers verified gone
5. Polish (T036-T040) → full validation pass

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- US4 (workspace decomposition) overlaps with US1 on workspace.rs — do US1 first, then US4
- All `.log_err()` replacements assume Zed's `log_err()` extension on Result types — verify this is available
- LICENSE text must be exact GPL-3.0-or-later from GNU, not a paraphrase
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
