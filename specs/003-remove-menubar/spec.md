# Feature Specification: Remove Top Menubar

**Feature Branch**: `003-remove-menubar`

**Created**: 2026-06-13

**Status**: Draft

**Input**: User description: "去掉顶部的 menubar"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Clean Editor View Without Menubar (Priority: P1)

As a user, I want the editor to open with no top menubar so that I have maximum vertical space for editing text and a distraction-free interface.

**Why this priority**: This is the core request — removing the menubar is the primary value of this feature.

**Independent Test**: Can be fully tested by launching the editor and verifying the toolbar area (New, Open, Save, Find, Replace, Compare, Recent buttons) no longer appears at the top of the window.

**Acceptance Scenarios**:

1. **Given** the editor is launched, **When** the main window renders, **Then** no toolbar row appears between the window title and the tab sidebar
2. **Given** the editor is launched, **When** the main window renders, **Then** the tab sidebar and editor content fill the full height between the window chrome and the status bar

---

### User Story 2 - Existing Keyboard Shortcuts Still Work (Priority: P2)

As a user, I want all file operations (new, open, save, find, replace, compare) to remain accessible via keyboard shortcuts even though the toolbar buttons are gone.

**Why this priority**: Removing the menubar must not remove functionality — only the visual toolbar. Keyboard access to all operations is essential.

**Independent Test**: Can be tested by using each keyboard shortcut (e.g., Cmd/Ctrl+S for save, Cmd/Ctrl+O for open) and verifying the actions still work.

**Acceptance Scenarios**:

1. **Given** the editor is running without a menubar, **When** the user presses the "save" shortcut, **Then** the current file is saved
2. **Given** the editor is running without a menubar, **When** the user presses the "open" shortcut, **Then** the file picker appears
3. **Given** the editor is running without a menubar, **When** the user presses the "find" shortcut, **Then** the search bar appears
4. **Given** the editor is running without a menubar, **When** the user presses the "new file" shortcut, **Then** a new tab is created

---

### User Story 3 - Command Center Still Provides Access (Priority: P3)

As a user, I want to access file operations through the command center (M-x panel) as a fallback for any toolbar functionality.

**Why this priority**: The command center is an existing feature that already provides access to all actions. Confirming it still works is important but secondary since it was not changed.

**Independent Test**: Can be tested by opening the command center and searching for actions like "New", "Open", "Save", "Find", "Replace", "Compare".

**Acceptance Scenarios**:

1. **Given** the editor is running without a menubar, **When** the user opens the command center, **Then** all file operation actions are listed and invokable

---

### Edge Cases

- What happens when the user had `show_toolbar: false` in their session? → No change in behavior; the setting becomes irrelevant since the toolbar is always hidden.
- What happens to the Recent Files feature that was only accessible via the toolbar dropdown? → Recent files remain accessible via keyboard shortcut or command center; the dropdown is removed.
- What happens to the `ToggleToolbar` action? → It is removed since there is no toolbar to toggle.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The top toolbar (containing New, Open, Save, Find, Replace, Compare, and Recent buttons) MUST NOT be rendered in the workspace
- **FR-002**: All file operations previously accessible via toolbar buttons MUST remain accessible via keyboard shortcuts
- **FR-003**: All file operations previously accessible via toolbar buttons MUST remain accessible via the command center
- **FR-004**: The `show_toolbar` state field and `ToggleToolbar` action MUST be removed from the workspace
- **FR-005**: The `render_toolbar()` function and related toolbar rendering code (button helpers, separator, recent menu dropdown) MUST be removed from `topbar.rs`
- **FR-006**: The status bar at the bottom of the window MUST remain unchanged
- **FR-007**: Session persistence MUST NOT include `show_toolbar` after this change
- **FR-008**: The Recent Files feature MUST continue to track opened files (via `recent_files.rs`) even though the toolbar dropdown is removed

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The editor launches and no toolbar row is visible at the top of the window — the tab sidebar starts directly below the window title bar
- **SC-002**: Users can perform all file operations (new, open, save, find, replace, compare) using only keyboard shortcuts with no change in behavior from before
- **SC-003**: The editor compiles and runs with zero warnings related to dead code from removed toolbar components
- **SC-004**: The total line count of the codebase decreases as unused toolbar rendering code is removed

## Assumptions

- The existing keyboard shortcuts (defined in keymap) already cover all toolbar button actions, so no new shortcuts are needed
- Users who relied on the toolbar for discovery of features can use the command center (M-x) to find and invoke any action
- The "Recent Files" dropdown was the only toolbar-only feature without a keyboard shortcut — the underlying recent files tracking continues to work, and recent files remain accessible via the command center
- The `topbar.rs` module still contains the `render_status_bar()` function, so the file is not deleted entirely
