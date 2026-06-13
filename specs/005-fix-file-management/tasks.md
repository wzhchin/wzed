---

description: "Task list for fixing file-management correctness & safety"
---

# Tasks: Fix File Management Correctness & Safety

**Input**: Design documents from `/specs/005-fix-file-management/`

**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/

**Tests**: The project has unit tests in pure-logic modules (`utils.rs`, `encoding.rs`, `recent_files.rs`); core UI/file-watching is validated manually via `test-step.md` (see quickstart.md). Test tasks below are included ONLY for the pure-logic changes where `cargo test` is meaningful.

**Organization**: Tasks grouped by user story. Each story is independently implementable and testable. Two P1 stories (data safety) are MVP-critical.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1–US5)
- Exact file paths in every description

## Path Conventions

Single binary project. Sources at repository-root `src/`. No new files this feature — all changes land in existing `src/{workspace,file_watcher,main,encoding,utils}.rs`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Initialize the one new global dependency the feature needs.

- [X] T001 Add a helper to encode a `String` into target-encoding bytes with error detection in `src/encoding.rs` — wraps `encoding_rs::Encoding::encode`, returns `Result<Vec<u8>>` that is `Err` when `had_errors` is true (per research.md Decision 1)
- [X] T002 [P] Add a unit test for the encode helper in `src/encoding.rs` covering: UTF-8 pass-through, GBK round-trip, and an unencodable-char → `Err` case

**Checkpoint**: Encoding primitive ready and unit-tested before any save-path change uses it.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The `Fs` global must exist before any watcher replacement can use it.

**⚠️ CRITICAL**: US4 (event-driven watching) cannot begin until this phase completes.

- [X] T003 Initialize the Zed `Fs` global in `src/main.rs` during app setup — construct `RealFs` with the background executor and register via `<dyn Fs>::set_global(...)`, before the workspace window opens (per research.md Decision 4)
- [X] T004 Verify build compiles and `cargo test` passes after T001–T003 (`Fs` init must not break the existing launch path)

**Checkpoint**: `Fs::global(cx)` is available app-wide; no behavioral change yet.

---

## Phase 3: User Story 1 - Round-Trip a Non-UTF-8 File Without Corruption (Priority: P1) 🎯 MVP

**Goal**: Saving re-encodes into the file's actual encoding instead of writing UTF-8. Files stay in GBK/Shift_JIS/ISO-8859-1 across an open→edit→save cycle.

**Independent Test**: quickstart.md Scenario 1 — open a GBK file, edit one char, save, confirm on disk it is still GBK and the edit is present.

### Implementation for User Story 1

- [X] T005 [US1] Update `write_editor_to_file` in `src/workspace.rs` to read the buffer's encoding via `buffer.encoding()` and write `encoding::encode(text, encoding)?` bytes instead of `&content` (per save-path-contract.md guarantees 1 & 3)
- [X] T006 [US1] Surface encode failures to the user: `write_editor_to_file` returns `Err` and the callers (`save_active_tab`, `save_active_tab_as`, `handle_save_all`) already notify via `show_notification` — confirm the unencodable path produces a user-visible message and writes nothing (save-path-contract.md guarantee 3)
- [X] T007 [US1] Confirm the UTF-8 common path is unchanged: `encode` returns borrowed bytes with `had_errors=false`, so behaviorally identical to today (regression guard)

**Checkpoint**: Non-UTF-8 files round-trip; UTF-8 path unaffected; unencodable chars abort save with a message. **US1 independently testable here.**

---

## Phase 4: User Story 2 - Recover Unsaved Work After a Crash (Priority: P1) 🎯 MVP

**Goal**: Snapshot backups are read back on session loss, so dirty tabs recover their content.

**Independent Test**: quickstart.md Scenario 3 — make unsaved edits, delete `session.json`, relaunch, edits reappear from snapshot.

### Implementation for User Story 2

- [X] T008 [US2] Give snapshots a stable identity key instead of volatile tab index: update `save_snapshot_for_tab` in `src/workspace.rs` to name files by a stable key (path of origin for path-backed tabs; a generated id persisted in session for untitled tabs), not `tab-{index}` (data-model.md Snapshot Backup; research.md Decision 2)
- [X] T009 [US2] Persist the snapshot identity key per untitled tab in session state so recovery can re-map it — extend `SessionTab` in `src/workspace.rs` with the id field and the `save_session`/restore serialization
- [X] T010 [US2] Wire the recovery read-path in `restore_session` (`src/workspace.rs`): when `session.json` is unreadable/missing, enumerate surviving snapshots and rebuild dirty tabs from them instead of falling to a blank untitled tab (research.md Decision 2; data-model.md Session State transition)
- [X] T011 [US2] Ensure recovery never fabricates content: a path-backed tab with no snapshot reloads from disk; a tab with no snapshot and no path becomes an empty untitled — never invented text (save-path/recovery contract; data-model.md validation rule)

**Checkpoint**: Deleted session.json → dirty tabs restored from snapshots. **US2 independently testable here.**

---

## Phase 5: User Story 3 - Stay Responsive With Large Unsaved Files (Priority: P2)

**Goal**: session.json stops duplicating the full dirty buffer every autosave interval.

**Independent Test**: quickstart.md Scenario 4 — open a multi-MB file, dirty it, confirm session.json does not balloon to buffer size across autosave cycles.

### Implementation for User Story 3

**⚠️ SEQUENCING**: This phase depends on US2 (Phase 4) being complete — the snapshot recovery read-path must exist BEFORE unsaved content leaves session.json, or unsaved work has nowhere to survive.

- [X] T012 [US3] Remove `unsaved_content` from `SessionTab` serialization in `src/workspace.rs` `save_session` — session records identity + metadata only (path, pinned, encoding, snapshot id) (research.md Decision 3; data-model.md Session State invariant)
- [X] T013 [US3] Update `restore_session` in `src/workspace.rs` to recover unsaved content via snapshots (US2 path) instead of `unsaved_content`, for both the session-readable and session-lost branches
- [X] T014 [US3] Drop the now-dead `unsaved_content` field and its parsing from `SessionTab` in `src/workspace.rs`; confirm backward-incompatibility is acceptable (old sessions simply reload from disk — no crash per Constitution II)

**Checkpoint**: session.json size independent of dirty-buffer size. **US3 independently testable here (requires US2 done).**

---

## Phase 6: User Story 4 - Be Told When an Open File Changes Externally (Priority: P2)

**Goal**: Replace polling with `Fs::watch`; notify on external change; suppress self-writes; surface dirty+external conflicts.

**Independent Test**: quickstart.md Scenario 5 — externally modify an open file, observe a notification (not a silent swap); own save does not false-trigger; dirty+external surfaces a conflict.

### Implementation for User Story 4

- [X] T015 [US4] Replace the polling spawn loop in `src/workspace.rs` (`new()`, the `FILE_WATCHER_POLL_SECS` timer) with a `Fs::watch`-based async stream that subscribes to each path-backed tab's path; start/stop watching as tabs open/close (research.md Decision 4)
- [X] T016 [US4] Rework `FileWatcher` in `src/file_watcher.rs` around events: on `PathEventKind::Changed/Removed`, compare mtime to the known last-write mtime and skip if it matches our own save (FR-009 self-write suppression, keeping the existing `update_mtime` mechanism)
- [X] T017 [US4] Change the external-change reaction in `src/workspace.rs`/`file_watcher.rs` from silent `reload_tab` to a user notification: clean tab → notify + reload; dirty tab → notify conflict, preserve both sides (FR-007, FR-008; ipc/save contracts)
- [X] T018 [US4] Handle `PathEventKind::Removed` (file deleted externally) and `Rescan` (watcher lost sync) in `src/file_watcher.rs` by surfacing to the user, never a silent empty tab or panic (data-model.md External Change transitions)

**Checkpoint**: External changes notify within seconds; own saves suppressed; conflicts surfaced. **US4 independently testable here.**

---

## Phase 7: User Story 5 - Add an Editor Action Once, Use It Everywhere (Priority: P3)

**Goal**: IPC `ExecuteCommand` dispatches via the GPUI action registry; no hand-written match table.

**Independent Test**: quickstart.md Scenario 6 — `cargo run -- -c new-file` opens a tab in the running instance; adding a new action makes it IPC-invocable with no dispatch-table edit.

### Implementation for User Story 5

- [X] T019 [US5] Replace the 18-arm `match command.as_str()` in the IPC pump (`src/main.rs`, `IpcMessage::ExecuteCommand` branch) with `cx.build_action(&command, None)` + `window.dispatch_action(action, cx)`; log `build_action` errors, never panic (ipc-dispatch-contract.md; research.md Decision 5)
- [X] T020 [US5] Keep the payload-bearing IPC variants (`SetText`, `SaveAs`, `SwitchTab`, `OpenFiles`) as explicit handlers — they have no keyboard-action equivalent and are intentionally NOT unified (ipc-dispatch-contract.md boundary note)
- [X] T021 [US5] Regression-sweep: invoke each previously-matched command over IPC and confirm identical behavior post-refactor (the unified path routes through the same `on_action` listeners as keymaps)

**Checkpoint**: New actions are IPC-invocable with zero dispatch wiring; existing commands unchanged. **US5 independently testable here.**

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Gates that span all stories.

- [X] T022 [P] Run `cargo clippy` and confirm no `unwrap()` / `let _ =` violations introduced (Constitution II gate, enforced by `clippy.toml`)
- [X] T023 [P] Run `cargo test` and confirm all unit tests pass (existing + new encoding encode test)
- [X] T024 Run quickstart.md Scenarios 1 & 3 (the two P1 data-safety gates) manually via `test-step.md` — MUST pass before feature is done
- [ ] T025 Run quickstart.md Scenarios 4, 5, 6 (P2/P3) manually via `test-step.md`
- [X] T026 [P] Verify `encoding.rs` encode helper test count is updated in `CLAUDE.md` line 15 ("encoding.rs (6)" → new count) if the test count changed

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately. T001 before T002.
- **Foundational (Phase 2)**: Depends on Phase 1. T003 (Fs init) before T004.
- **US1 (Phase 3)**: Depends on T001 (encode helper). No other story dependency.
- **US2 (Phase 4)**: Depends on T001 only. No dependency on US1.
- **US3 (Phase 5)**: **Depends on US2 (Phase 4) complete** — snapshot recovery must exist before session.json stops carrying content. ⚠️ Hard ordering.
- **US4 (Phase 6)**: Depends on T003 (Fs init). No dependency on US1/US2/US3.
- **US5 (Phase 7)**: No dependencies. Fully independent.
- **Polish (Phase 8)**: After all implemented stories.

### User Story Dependencies

- **US1 (P1)**: After T001. Independent of other stories. **MVP candidate.**
- **US2 (P1)**: After T001. Independent of US1. **MVP candidate.**
- **US3 (P2)**: After US2. Cannot start until US2's recovery read-path lands.
- **US4 (P2)**: After T003. Independent of US1/US2/US3.
- **US5 (P3)**: Fully independent — can run any time.

### Within Each User Story

- Helper/primitive before caller (T001 → T005).
- Write-path before read-path wiring where applicable.
- Story independently testable at its checkpoint before moving on.

### Parallel Opportunities

- Phase 1: T002 parallel with T003/T004 (different concern, but T002 depends on T001).
- Across stories (with adequate staffing): **US1, US4, US5 are mutually independent** and can proceed in parallel after their prerequisites. US2 can also parallel with these.
- US3 is the lone serialized one (waits on US2).
- Phase 8: T022/T023/T026 are [P] (different checks/files).

---

## Parallel Example: Independent P1/P2/P3 Stories

```text
# After Setup + Foundational (T001–T004), these can run concurrently:
Stream A: US1 — encoding-aware save (src/encoding.rs, src/workspace.rs write path)
Stream B: US2 — snapshot recovery (src/workspace.rs session + restore)
Stream C: US4 — fs::watch watcher (src/file_watcher.rs, src/workspace.rs spawn loop)
Stream D: US5 — IPC dispatch (src/main.rs)

# Then sequentially:
US2 done → US3 (session.json slimming) in src/workspace.rs
```

---

## Implementation Strategy

### MVP First (US1 + US2)

These two are the data-safety P1 stories (silent corruption + unrecoverable loss). Recommend shipping them together as the MVP, since both touch `src/workspace.rs` save/session logic and both are Constitution-VI critical:

1. Phase 1 (Setup): T001, T002
2. Phase 2 (Foundational): T003, T004
3. Phase 3 (US1): T005–T007
4. Phase 4 (US2): T008–T011
5. **STOP and VALIDATE**: quickstart Scenarios 1, 2, 3

### Incremental Delivery

1. MVP (US1+US2) → validate → fixes the two worst failure modes.
2. US3 → validate → bounds session write volume (requires US2).
3. US4 → validate → event-driven, conflict-aware watching.
4. US5 → validate → unified dispatch.

### Single-Developer Note

Because US1, US2, US4, US5 all touch `src/workspace.rs` to some degree, a single developer should do them sequentially to avoid merge churn (the [P] markers indicate logical independence, not zero file overlap). US3 strictly follows US2.

---

## Notes

- [P] tasks = different files OR no dependency on incomplete tasks.
- [Story] label maps task to spec.md user story for traceability.
- Every task names a file path — immediately executable without extra context.
- Constitution gates: no `unwrap`/`let _ =` (II), no new files (IV), no new deps (III), atomic writes retained (VI).
- The `unsaved_content` removal (US3) is a session-format break — acceptable because old sessions fall back to disk reload without crashing.
