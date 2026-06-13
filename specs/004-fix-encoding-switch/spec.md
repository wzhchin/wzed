# Feature Specification: Fix Encoding Switch Data Loss

**Feature Branch**: `004-fix-encoding-switch`

**Created**: 2026-06-13

**Status**: Draft

**Input**: User description: "encoding 不工作，当切换编码时，东西都丢了，但是还是 utf-8"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Safe Encoding Reload (Priority: P1)

A user opens a non-UTF-8 file (e.g., GBK-encoded Chinese text) that displays as garbled characters. They press the encoding switch shortcut to cycle through encodings. The file is re-read from disk using the selected encoding and displayed correctly. If the user has made unsaved edits, they are warned before the reload proceeds.

**Why this priority**: This is the core value proposition — viewing files in the correct encoding without losing work. Without this, the encoding feature is actively harmful.

**Independent Test**: Open a GBK-encoded file, press Ctrl+Shift+E, verify the text becomes readable. Make an edit, press Ctrl+Shift+E again, verify a warning appears before reloading.

**Acceptance Scenarios**:

1. **Given** a file encoded in GBK is open and displays garbled text, **When** the user presses the encoding switch shortcut, **Then** the file is re-read from disk using the next encoding in the cycle and the text is displayed correctly
2. **Given** a file is open with unsaved edits, **When** the user presses the encoding switch shortcut, **Then** a confirmation prompt warns the user that unsaved changes will be lost, and the reload only proceeds if the user confirms
3. **Given** a file is open with no unsaved edits, **When** the user presses the encoding switch shortcut, **Then** the file is re-read from disk with the next encoding immediately, without any prompt

---

### User Story 2 - Encoding Visibility (Priority: P2)

A user wants to know which encoding is currently applied to a file, and wants to select a specific encoding rather than cycling blindly through all 15 options.

**Why this priority**: Blind cycling through 15 encodings is impractical. Users need to see the current encoding and pick the right one directly. This makes the encoding feature actually usable.

**Independent Test**: Open a file, invoke the encoding picker, verify the current encoding is highlighted, select a different encoding, verify the file reloads with that encoding.

**Acceptance Scenarios**:

1. **Given** a file is open, **When** the user invokes the encoding command, **Then** a picker shows all supported encodings with the current one highlighted
2. **Given** the encoding picker is open, **When** the user selects an encoding from the list, **Then** the file is re-read from disk using the selected encoding and the picker closes
3. **Given** the encoding picker is open, **When** the user cancels (Escape), **Then** the picker closes without changing the encoding

---

### User Story 3 - Encoding Persistence (Priority: P3)

A user opens a file that was previously opened with a specific encoding (e.g., Shift_JIS). The editor remembers the encoding used for that file and applies it automatically on reopen.

**Why this priority**: Convenience feature — avoids re-selecting encoding every time. Important for users who regularly work with non-UTF-8 files but not critical for data safety.

**Independent Test**: Open a file, switch to Shift_JIS encoding, close the tab, reopen the same file, verify it opens with Shift_JIS encoding automatically.

**Acceptance Scenarios**:

1. **Given** a file was previously opened with a non-default encoding, **When** the user reopens that file, **Then** the file is read using the previously selected encoding
2. **Given** a file is opened for the first time, **When** no previous encoding preference exists, **Then** the file is opened using UTF-8 as the default

---

### Edge Cases

- What happens when the file on disk cannot be decoded with the selected encoding? The editor should display a clear error and keep the previous content rather than replacing it with garbage or an empty buffer.
- What happens when the user switches encoding on a new untitled buffer with no file on disk? The operation should be a no-op since there is no file to re-read.
- What happens when the file on disk has been deleted or is unreadable? The editor should show an error notification and preserve the current buffer content.
- What happens when encoding auto-detection conflicts with the user's manual selection? The user's manual selection takes precedence.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: When the user switches encoding, the editor MUST re-read the file from disk using the selected encoding and replace the buffer content
- **FR-002**: If the buffer has unsaved changes, the editor MUST warn the user before reloading and require explicit confirmation to proceed
- **FR-003**: If the file cannot be decoded with the selected encoding, the editor MUST display an error notification and preserve the current buffer content unchanged
- **FR-004**: If the tab has no associated file (untitled buffer), the encoding switch MUST be a no-op
- **FR-005**: The editor MUST provide a picker UI listing all supported encodings, with the current encoding highlighted
- **FR-006**: The editor MUST remember the encoding used for each file path across sessions via session persistence
- **FR-007**: When reopening a file, the editor MUST apply the previously used encoding if one was recorded
- **FR-008**: The status bar MUST display the current encoding label for the active tab

### Key Entities

- **Tab Encoding**: The character encoding associated with an open file tab. Tracks the encoding name (e.g., "GBK", "UTF-8") and whether it was auto-detected or manually selected.
- **Encoding Preference**: A per-file-path encoding memory that persists across sessions. Maps file paths to their last-used encoding.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users never lose unsaved edits when switching encoding — a warning always appears first if the buffer is dirty
- **SC-002**: Users can identify the current encoding and select a target encoding from a visible list without blind cycling
- **SC-003**: Encoding preference persists so that reopening a file automatically uses the correct encoding
- **SC-004**: Failed encoding switches never corrupt or clear the buffer — the previous content is always preserved on error

## Assumptions

- Users primarily work with UTF-8 files and only occasionally need other encodings (CJK users being the main exception)
- The editor does not need to save files in non-UTF-8 encodings — it only needs to read and display them correctly
- Encoding preferences are stored locally per user, not shared or synced
- The set of supported encodings (15 encodings currently defined) is sufficient for the target user base
- Files may be opened from any path on the local filesystem; encoding preferences should be keyed by absolute file path
