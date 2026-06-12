# Feature Specification: Code Quality Fixes

**Feature Branch**: `002-fix-code-quality`

**Created**: 2026-06-13

**Status**: Draft

**Input**: User description: "Address all issues identified in REVIEW.md — missing LICENSE, clippy violations, silent error swallowing, god-class workspace, missing tests, no user-visible error notifications, hardcoded magic numbers, missing documentation"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Errors Are Visible to Users (Priority: P1)

A user opens a file that doesn't exist or lacks permissions. Instead of the editor silently doing nothing (current behavior with `.ok()`), the user sees a clear notification describing what went wrong. Similarly, when IPC operations fail (e.g., sending a command to a running instance), the user is informed rather than left guessing.

**Why this priority**: Silent failures are the worst UX for an editor — the user has no idea whether their action was attempted, failed, or succeeded. This is also a constitution violation (Principle II: Error Resilience).

**Independent Test**: Can be fully tested by triggering error conditions (open nonexistent file, IPC to non-listening instance) and verifying that a user-visible notification appears instead of silent failure.

**Acceptance Scenarios**:

1. **Given** the editor is running, **When** the user tries to open a file that doesn't exist, **Then** a notification appears explaining the file could not be opened
2. **Given** a running editor instance, **When** a second instance sends an IPC command that fails, **Then** the second instance shows an error message rather than silently exiting
3. **Given** the editor is running, **When** a file watcher encounters a metadata error, **Then** the error is logged and the editor continues running without panicking

---

### User Story 2 - Code Complies With Its Own Rules (Priority: P1)

A developer runs `cargo clippy` and sees zero warnings related to `unwrap()`/`expect()` in non-test code. The clippy.toml rules are fully enforced — no exceptions, no violations.

**Why this priority**: Rules that aren't enforced are worse than no rules — they create false confidence. This is a constitution compliance issue (Principle II, Development Workflow §2).

**Independent Test**: Can be tested by running `cargo clippy` and verifying zero disallowed-method warnings in non-test code.

**Acceptance Scenarios**:

1. **Given** the codebase, **When** `cargo clippy` runs, **Then** no `unwrap()` or `expect()` warnings appear in non-test code
2. **Given** any error path previously using `.ok()`, **When** that error path is triggered, **Then** the error is either propagated, logged, or shown to the user

---

### User Story 3 - Project Has Required Legal and Documentation Files (Priority: P2)

A visitor lands on the repository and finds a LICENSE file matching the Cargo.toml declaration (GPL-3.0-or-later), a CHANGELOG tracking versions, and contributing guidelines in the README or a CONTRIBUTING file.

**Why this priority**: Without a LICENSE file, the project legally reserves all rights despite claiming GPL. This blocks adoption and contribution.

**Independent Test**: Can be tested by checking that LICENSE file exists and its text matches GPL-3.0-or-later.

**Acceptance Scenarios**:

1. **Given** the repository root, **When** checking for LICENSE, **Then** a GPL-3.0-or-later license file exists
2. **Given** the repository root, **When** checking for CHANGELOG.md, **Then** a changelog file exists with at least the current version entry

---

### User Story 4 - Workspace Module Is Decomposed (Priority: P2)

A developer needs to fix a search-related bug. Instead of scrolling through a 1100+ line god-class file, they open the relevant focused module. The workspace retains its single-entity architecture (constitution requirement) but delegates logic to focused internal modules.

**Why this priority**: `workspace.rs` at 1171 lines is the largest technical debt. Every feature addition makes it harder to maintain. However, this must respect Constitution Principle IV (existing file discipline) — extraction should consolidate, not proliferate files.

**Independent Test**: Can be tested by verifying that workspace.rs is under 800 lines and that search/diff logic has been moved to their existing module files.

**Acceptance Scenarios**:

1. **Given** the codebase, **When** counting lines in workspace.rs, **Then** it is under 800 lines
2. **Given** the refactored codebase, **When** building with `cargo build`, **Then** it compiles without errors
3. **Given** the refactored codebase, **When** running existing tests, **Then** all 33 tests still pass

---

### User Story 5 - Configuration Constants Are Centralized (Priority: P3)

A developer wants to change the notification display duration. Instead of hunting through multiple files for the hardcoded `4.0` value, they find all tunable constants in one place.

**Why this priority**: Hardcoded magic numbers are a maintenance annoyance but not a functional bug. Low risk, low effort.

**Independent Test**: Can be tested by verifying that no literal numeric constants (other than 0, 1, or obvious constants) appear outside a configuration structure.

**Acceptance Scenarios**:

1. **Given** the codebase, **When** searching for numeric literals in source files, **Then** tunable values (poll interval, retention days, buffer size, etc.) are defined in a centralized config structure
2. **Given** the config structure, **When** a value is changed, **Then** the new value takes effect without modifying any other file

---

### Edge Cases

- What happens when a notification display timer overlaps with a new notification? New notification should replace or queue.
- What happens when workspace.rs extraction introduces circular imports? Must not create circular module dependencies.
- What happens if the GPL license text doesn't exactly match the official text? Must use the exact official GPL-3.0-or-later text from FSF.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Editor MUST display a user-visible notification when a file operation fails (open, save, save-as)
- **FR-002**: Editor MUST display a user-visible notification when an IPC operation fails
- **FR-003**: All `.ok()` calls that discard potentially meaningful errors MUST be replaced with proper error handling (propagation, logging, or user notification)
- **FR-004**: No `unwrap()` or `expect()` calls MAY exist in non-test code
- **FR-005**: Repository MUST contain a LICENSE file with GPL-3.0-or-later text matching the Cargo.toml declaration
- **FR-006**: Repository MUST contain a CHANGELOG.md file tracking version history
- **FR-007**: workspace.rs MUST be reduced to under 800 lines by moving logic to existing module files
- **FR-008**: All tunable numeric constants (poll intervals, retention periods, buffer sizes, display durations) MUST be defined in a centralized configuration structure
- **FR-009**: File watcher errors MUST be logged rather than silently discarded
- **FR-010**: Error notification mechanism MUST NOT block the editor's main thread or UI rendering

### Key Entities

- **Error Notification**: A transient, non-blocking message displayed to the user when an operation fails. Has display duration and severity level.
- **App Configuration**: A centralized structure holding all tunable constants (poll intervals, retention periods, buffer sizes, notification durations). Single source of truth for magic numbers.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `cargo clippy` reports zero disallowed-method warnings in non-test code
- **SC-002**: Grep for `.ok()` in non-test source files returns zero results where the discarded error could be user-facing
- **SC-003**: workspace.rs line count is under 800 lines
- **SC-004**: All 33 existing tests pass after refactoring with no regressions
- **SC-005**: A user encountering any file operation failure sees a notification within 1 second of the failure
- **SC-006**: LICENSE file is present and contains exact GPL-3.0-or-later text
- **SC-007**: CHANGELOG.md exists with at least one version entry

## Assumptions

- The notification mechanism will use the existing `show_notification` pattern already present in workspace.rs (4-second transient overlay), not a new UI component from scratch
- `workspace.rs` decomposition will move logic to existing files (`search.rs`, `diff_view.rs`, etc.) rather than creating new module files, per Constitution Principle IV
- The centralized config struct will live in an existing file (likely `utils.rs` or `workspace.rs`) rather than a new dedicated config file
- GPL-3.0-or-later license text will be sourced from the official GNU website
- The CHANGELOG will be retroactively populated from git history for past commits
- Test coverage expansion (REVIEW.md item #5) is out of scope for this feature and should be a separate spec
- File watcher migration to native `notify` crate (REVIEW.md item #7) is out of scope — only error handling in the existing polling implementation will be fixed
