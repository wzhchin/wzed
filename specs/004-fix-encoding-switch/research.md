# Research: Fix Encoding Switch Data Loss

**Date**: 2026-06-13 | **Phase**: 0

## R1: How should the dirty-buffer check work before encoding reload?

**Decision**: Check `tab.is_dirty(cx)` before proceeding with reload. If dirty, show a notification asking the user to save first, and abort the reload. Do NOT implement a modal confirmation dialog — the codebase has no modal dialog pattern, and adding one would violate Constitution IV (Existing File Discipline) by requiring significant new UI code.

**Rationale**: The existing notification system (`show_notification`) is lightweight and already used for error feedback. A "save your changes first" notification is clear and prevents data loss without requiring a new UI component.

**Alternatives considered**:
- Modal confirmation dialog: Rejected — no existing pattern in codebase, would require significant new UI code
- Auto-save before reload: Rejected — unexpected side effect, user may not want to save
- Silent abort: Rejected — user wouldn't understand why encoding didn't change

## R2: How should the encoding picker work?

**Decision**: The command center already has a `ChangeEncoding` submenu (`CommandSubmenu::ChangeEncoding`) that lists all 15 supported encodings. This is the primary UI for encoding selection. The Ctrl+Shift+E shortcut should be changed to open the command center filtered to the encoding submenu rather than blindly cycling. The current `handle_reload_encoding` cycling function should be removed.

**Rationale**: The command center picker already implements: search/filter, keyboard navigation, current-item highlighting, click-to-select, escape-to-cancel. Reusing it satisfies FR-005 without any new UI code. The blind cycling via `handle_reload_encoding` is a poor UX with 15 encodings.

**Alternatives considered**:
- Standalone picker: Rejected — would duplicate the command center pattern
- Keep cycling + add picker: Rejected — two paths to same action is confusing
- Status bar dropdown: Rejected — would require new anchored dropdown code

## R3: How should encoding persistence work across sessions?

**Decision**: Add an `encoding: Option<String>` field to the `SessionTab` struct. On save, store `encoding_label(tab.encoding)` if it differs from UTF-8 (the default). On restore, pass the stored encoding to `open_file_path` or use `encoding_from_label` after tab creation to override the auto-detected encoding.

**Rationale**: Minimal change to existing session persistence. Storing as `Option<String>` (label format) is serialization-friendly and human-readable in session.json. Only storing non-UTF-8 keeps the JSON clean for most users. The restore path needs a small modification to `open_file_path` or a post-restore encoding override.

**Alternatives considered**:
- Separate encoding-preferences file: Rejected — unnecessary indirection, session already tracks per-tab state
- Always store encoding (including UTF-8): Rejected — bloats session.json for no benefit
- Use encoding_rs Encoding name: Rejected — labels are already used throughout the codebase

## R4: How should error-safe reload work?

**Decision**: In the `ChangeEncoding` command center callback (and any remaining reload path): attempt `read_file_as_encoding`, and on error, show a notification and do NOT update the buffer or the tab's encoding field. The current code in `command_center.rs` lines 128-142 already uses `if let Ok(content)` which silently ignores errors — change this to show a notification on failure.

**Rationale**: The `if let Ok` pattern in the command center callback silently swallows decode errors, giving the user no feedback. Using the existing notification system for error reporting is consistent with the codebase.

**Alternatives considered**:
- Fallback to previous encoding: Already handled — we just don't update `tab.encoding` on error
- Partial decode with replacement characters: Rejected — could silently corrupt display

## R5: What happens with the existing Ctrl+Shift+E keybinding?

**Decision**: Remap Ctrl+Shift+E from `ReloadWithEncoding` (blind cycling) to opening the command center with the encoding submenu pre-selected. This means Ctrl+Shift+E should trigger the same path as M-x → "Change Encoding". The `ReloadWithEncoding` action and `handle_reload_encoding` function can be removed since the command center picker replaces them.

**Rationale**: Single clear path for encoding switching. Users get a visible list instead of blind cycling. The shortcut is preserved as a quick-access to the encoding picker.

**Alternatives considered**:
- Keep both paths: Rejected — confusing, two behaviors for same conceptual action
- Remove Ctrl+Shift+E entirely: Rejected — power users expect a shortcut
