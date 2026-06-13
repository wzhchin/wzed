# Tasks: Fix Encoding Switch Data Loss

**Input**: Design documents from `specs/004-fix-encoding-switch/`

**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md

**Tests**: No automated tests requested. Manual integration testing via `quickstart.md`.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Remove Dead Code)

**Purpose**: Remove the blind-cycling encoding action that is being replaced by the command center picker.

- [x] T001 Remove `ReloadWithEncoding` variant from the `actions!` macro in `src/main.rs:60-73`
- [x] T002 Remove `ReloadWithEncoding` keybinding entry (`KeyBinding::new("ctrl-shift-e", ...)`) in `src/main.rs:176`
- [x] T003 Remove `ReloadWithEncoding` IPC handler case (`"lite_editor::ReloadWithEncoding" => { ... }`) in `src/main.rs:311-315`
- [x] T004 Remove `ReloadWithEncoding` import from `src/workspace.rs:27`
- [x] T005 Remove `handle_reload_encoding` method from `src/workspace.rs:817-847`
- [x] T006 Remove `.on_action(cx.listener(Self::handle_reload_encoding))` registration in `src/workspace.rs:1066`

---

## Phase 2: Foundational (No blocking prerequisites)

No foundational infrastructure needed. The command center already has `ChangeEncoding` submenu and `show_notification` already exists. Proceed directly to user stories.

---

## Phase 3: User Story 1 - Safe Encoding Reload (Priority: P1) 🎯 MVP

**Goal**: When a user selects an encoding from the command center picker, the editor safely reloads the file. If the buffer is dirty, show a notification and abort. If the file cannot be decoded, show an error and preserve current content. Untitled buffers are a no-op.

**Independent Test**: Open a GBK file, switch encoding via M-x → switch-encoding, verify correct display. Edit the file, try switching again, verify notification blocks the switch.

### Implementation for User Story 1

- [x] T007 [US1] Add dirty-buffer guard in `CommandSubmenu::ChangeEncoding` branch of `execute_submenu_item` in `src/command_center.rs:128-142`: before attempting reload, check `self.tabs[self.active].is_dirty(cx)`. If dirty, call `self.show_notification("Save changes before switching encoding", cx)` and `cx.notify(); return`. The check must happen before `read_file_as_encoding`.
- [x] T008 [US1] Add untitled-buffer guard in the same `ChangeEncoding` branch in `src/command_center.rs:128-142`: if `self.tabs[self.active].path.is_none()`, show notification "No file to reload" and return early. This check must come before the dirty check.
- [x] T009 [US1] Add error notification for failed decode in the same `ChangeEncoding` branch in `src/command_center.rs:128-142`: restructure the current `if let Ok(content)` chain so that on `Err`, `self.show_notification(format!("Failed to reload: {err:#}"), cx)` is called and `tab.encoding` is NOT updated. Only update `tab.encoding` and `editor.set_text` on success.
- [x] T010 [US1] Add Ctrl+Shift+E keybinding that opens command center with encoding submenu: add a new action `SwitchEncoding` (or reuse `ToggleCommandCenter`) in `src/main.rs:60-73` with keybinding `ctrl-shift-e`, and a handler in `src/workspace.rs` that sets `self.show_command_center = true`, `self.command_submenu = Some(CommandSubmenu::ChangeEncoding)`, clears the search text, and calls `cx.notify()`.

**Checkpoint**: At this point, encoding switching is safe — no data loss possible. Users can pick encoding from command center or Ctrl+Shift+E shortcut. Dirty buffers are protected. Errors show notifications.

---

## Phase 4: User Story 2 - Encoding Visibility (Priority: P2)

**Goal**: The encoding picker shows which encoding is currently active so users can see the current state at a glance.

**Independent Test**: Open command center → switch-encoding, verify current encoding is visually distinct in the list.

### Implementation for User Story 2

- [x] T011 [US2] Highlight current encoding in the command center rendering: in `render_command_center` in `src/command_center.rs:370-412`, when rendering items for `CommandSubmenu::ChangeEncoding`, compare each item's encoding label with the active tab's `encoding_label(tab.encoding)`. For the matching item, render with a distinct visual marker (e.g., prefix with "• " or use a different text color like `colors::TEXT_SECONDARY` for the label part). This requires passing the active tab's encoding into the render function or reading it from `this.tabs[this.active].encoding`.

**Checkpoint**: The encoding picker now shows which encoding is active. Combined with US1, the encoding feature is safe and usable.

---

## Phase 5: User Story 3 - Encoding Persistence (Priority: P3)

**Goal**: When a user reopens a file that was previously opened with a non-UTF-8 encoding, the editor remembers and applies that encoding automatically.

**Independent Test**: Open a file, switch to Shift_JIS, close tab, reopen file, verify Shift_JIS is applied automatically. Restart editor, verify session restores with correct encoding.

### Implementation for User Story 3

- [x] T012 [US3] Add `encoding: Option<String>` field to `SessionTab` struct in `src/workspace.rs:42-47`. The field stores the encoding label (e.g., "GBK") when non-UTF-8.
- [x] T013 [US3] Populate encoding in `save_session` in `src/workspace.rs:49-88`: when building each `SessionTab`, set `encoding` to `Some(encoding_label(tab.encoding).to_string())` if `tab.encoding != encoding_rs::UTF_8`, else `None`.
- [x] T014 [US3] Apply stored encoding during session restore in `src/workspace.rs:276-301`: after `open_file_path` succeeds for a tab with a stored `encoding` value, look up the encoding via `encoding_from_label`, and if found, set `self.tabs[last_idx].encoding = enc` and re-read the file with `read_file_as_encoding(&path, enc)`, then set the editor text. If the encoding label is invalid or the re-read fails, leave the auto-detected encoding unchanged.
- [x] T015 [US3] Handle encoding for tabs where the file no longer exists (unsaved content path in `src/workspace.rs:291-301`): when creating a tab from `unsaved_content` for a missing file, if `tab.encoding` is `Some(label)`, set the new tab's encoding to the resolved encoding (via `encoding_from_label`), falling back to UTF-8 if the label is invalid.

**Checkpoint**: Encoding preferences persist across tab close/reopen and across editor restart. The full encoding workflow is complete.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Final verification and cleanup.

- [x] T016 Run `cargo build` and `cargo test` to verify no compilation errors or test regressions
- [x] T017 Run `cargo clippy` and fix any warnings (especially any `unwrap()` introduced)
- [x] T018 Validate against quickstart.md: test Scenario 1 (safe reload), Scenario 2 (picker visibility), Scenario 3 (persistence), Scenario 4 (status bar)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — remove dead code first to avoid confusion
- **Phase 2 (Foundational)**: Skipped — no blocking prerequisites
- **Phase 3 (US1 — Safe Reload)**: Depends on Phase 1 completion (dead code removed)
- **Phase 4 (US2 — Visibility)**: Depends on Phase 3 (needs the safe reload logic in place)
- **Phase 5 (US3 — Persistence)**: Depends on Phase 3 (needs safe reload logic), independent of Phase 4
- **Phase 6 (Polish)**: Depends on all user stories being complete

### User Story Dependencies

- **US1 (P1)**: Depends on Phase 1 only — core safety fix, MVP
- **US2 (P2)**: Depends on US1 — enhances the picker UI
- **US3 (P3)**: Depends on US1 — needs stable encoding handling; can be done in parallel with US2

### Parallel Opportunities

- T001-T006 can all run in parallel (different locations, but all removals)
- T007, T008, T009 are sequential (same code block, modify incrementally)
- T010 is independent of T007-T009 (different handler)
- T012-T015 are sequential (session persistence chain: struct → save → restore → edge case)

---

## Parallel Example: Phase 1

```bash
# Remove all dead ReloadWithEncoding code (different locations):
Task: "Remove ReloadWithEncoding from actions macro in src/main.rs"
Task: "Remove keybinding in src/main.rs"
Task: "Remove IPC handler in src/main.rs"
Task: "Remove import in src/workspace.rs"
Task: "Remove handle_reload_encoding in src/workspace.rs"
Task: "Remove on_action registration in src/workspace.rs"
```

## Parallel Example: Phase 4 + 5

```bash
# US2 and US3 can be done in parallel by different developers:
Task: "Highlight current encoding in command center (src/command_center.rs)"
Task: "Add encoding to SessionTab and session save/restore (src/workspace.rs)"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Remove dead `ReloadWithEncoding` code
2. Complete Phase 3: Add dirty-buffer guard, untitled guard, error notification, Ctrl+Shift+E shortcut
3. **STOP and VALIDATE**: Test encoding switch is safe — no data loss possible
4. The editor now has a working, safe encoding feature

### Incremental Delivery

1. Remove dead code (Phase 1) → Clean codebase
2. Add safe reload (US1) → Test → Data safety guaranteed (MVP!)
3. Add encoding visibility (US2) → Test → Better UX
4. Add encoding persistence (US3) → Test → Full feature complete
5. Polish (Phase 6) → `cargo clippy` + quickstart validation

---

## Notes

- All tasks modify existing files only (Constitution IV compliance)
- No `unwrap()` allowed (Constitution II compliance)
- The command center `ChangeEncoding` submenu already exists — tasks only add guards and error handling to its callback
- `show_notification` already exists in `LiteWorkspace` — used for all user feedback
- `is_dirty(cx)` already exists on `Tab` — used for dirty-buffer check
- Encoding label display in status bar already works in `topbar.rs` — no changes needed
