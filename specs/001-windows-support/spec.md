# Feature Specification: Windows Support

**Feature Branch**: `001-windows-support`

**Created**: 2026-06-13

**Status**: Draft

**Input**: User description: "增加 windows 支持" (Add Windows support)

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Build and Launch on Windows (Priority: P1)

A user on Windows downloads the source, runs `cargo build`, and launches
WZed. The editor window appears with the same dark theme, toolbar, and tab
layout as on Linux. Opening, editing, and saving files works identically.

**Why this priority**: Without the app even starting on Windows, no other
functionality matters. This is the foundational gate.

**Independent Test**: Clone repo on a Windows machine, run `cargo build` then
`cargo run`, verify the editor window opens and a file can be opened, edited,
and saved.

**Acceptance Scenarios**:

1. **Given** a Windows machine with Rust toolchain installed and Zed source
   at `../zed`, **When** the user runs `cargo build`, **Then** the build
   completes without errors.
2. **Given** a successful build, **When** the user runs `cargo run`,
   **Then** the editor window opens with correct title, theme, and layout.
3. **Given** the editor is running, **When** the user opens a file, edits
   content, and saves, **Then** the file is written correctly to disk.

---

### User Story 2 - Single-Instance IPC on Windows (Priority: P2)

When a user double-clicks a file or runs `wzed file.rs` from a second terminal
while WZed is already running, the file opens in the existing window rather
than launching a new instance.

**Why this priority**: Single-instance behavior is a core UX expectation. The
TCP-based IPC skeleton already exists but needs validation and integration
testing on Windows.

**Independent Test**: Launch WZed, then from a second terminal run
`wzed second_file.rs` — verify the file appears as a new tab in the running
instance.

**Acceptance Scenarios**:

1. **Given** WZed is already running, **When** the user runs
   `wzed file.rs` from another terminal, **Then** the file opens as a new
   tab in the existing window without spawning a new process.
2. **Given** WZed is already running, **When** the user runs
   `wzed -c "new-file"`, **Then** a new empty tab opens in the existing
   window.
3. **Given** WZed exits, **When** the port lock file exists from a previous
   session, **Then** a new instance detects the stale lock, removes it, and
   starts successfully.

---

### User Story 3 - Configuration and Session on Windows (Priority: P3)

WZed stores settings, session state, recent files, and autosave snapshots in
the standard Windows configuration directory. After closing and reopening the
editor, the previous session (open tabs, cursor positions) is restored.

**Why this priority**: Session persistence is a key usability feature but
depends on P1 (app must launch) being solid first.

**Independent Test**: Open multiple files in WZed, close the editor, reopen it
— verify all tabs and their content are restored.

**Acceptance Scenarios**:

1. **Given** the user creates `settings.json` with custom font settings,
   **When** WZed starts, **Then** the editor uses those settings.
2. **Given** the user has multiple tabs open, **When** the user closes and
   reopens WZed, **Then** all previously open tabs are restored.
3. **Given** an unsaved file with snapshot backup, **When** WZed restarts
   after a crash, **Then** the snapshot content is recovered.

---

### Edge Cases

- What happens when `../zed` contains Windows-incompatible path separators
  or missing Win32 platform hooks in GPUI?
- How does the editor behave with Windows line endings (CRLF) — are they
  preserved or converted to LF?
- What happens when a file path contains non-ASCII characters common on
  Windows (e.g., CJK user directory names)?
- How does the port lock file behave if WZed crashes without cleanup — does
  the next instance handle the stale lock gracefully?
- Does the file watcher handle Windows file locking (a file open in another
  program may not be readable)?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The application MUST compile and run on Windows 10/11 with the
  Rust MSVC toolchain.
- **FR-002**: The editor MUST use `gpui_windows` for platform integration on
  Windows, replacing the hardcoded `gpui_linux` platform selection.
- **FR-003**: The `main.rs` platform initialization MUST select the correct
  GPUI platform at compile time via `cfg` attributes.
- **FR-004**: Single-instance IPC on Windows MUST use TCP loopback
  (already implemented) and MUST handle stale port lock files from crashed
  previous instances.
- **FR-005**: Configuration, session, and data files MUST be stored in the
  platform-appropriate directory returned by `dirs` (`%APPDATA%/wzed/` on
  Windows, `~/.config/wzed/` on Linux).
- **FR-006**: The `Cargo.toml` dependency on `gpui_linux` MUST be extended
  with a conditional dependency on `gpui_windows` for Windows targets.
- **FR-007**: File paths in IPC messages MUST be handled correctly on both
  platforms (backslash vs forward slash, drive letters).
- **FR-008**: The autosave snapshot directory and backup mechanism MUST work
  with Windows path conventions.
- **FR-009**: The desktop entry file (`dist/dev.wzed.editor.desktop`) is
  Linux-only and MUST NOT be included in Windows packaging concerns.

### Key Entities

- **PlatformConfig**: The GPUI platform selection logic — currently a single
  hardcoded call, needs to become a compile-time conditional.
- **IpcTransport**: Already split into Unix datagram (Linux) and TCP
  (Windows) — the Windows path exists but needs build validation.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `cargo build` on Windows completes with zero errors and zero
  warnings from WZed code (Zed crate warnings are out of scope).
- **SC-002**: All 35 existing unit tests pass on Windows via `cargo test`.
- **SC-003**: A user can open, edit, and save a file on Windows with the same
  steps and keyboard shortcuts as on Linux.
- **SC-004**: Single-instance IPC works on Windows — second launch sends files
  to the running instance instead of opening a new window.
- **SC-005**: Session persistence works identically on Windows — closing and
  reopening restores the previous tab state.

## Assumptions

- The `gpui_windows` crate in the Zed source tree is functional and
  provides the same `current_platform` interface as `gpui_linux`.
- The Zed framework already handles Windows-specific rendering concerns
  (font loading, window management, input handling) — WZed does not need
  to implement these.
- The `dirs` crate correctly returns `%APPDATA%` for `config_dir()` and
  `data_dir()` on Windows.
- The Rust MSVC toolchain is the standard Windows Rust target
  (`x86_64-pc-windows-msvc`).
- CRLF line ending handling is managed by Zed's buffer implementation;
  WZed does not need special handling.
- The `similar` crate, `chardetng`, `encoding_rs`, and all other
  cross-platform dependencies work on Windows without modification.
