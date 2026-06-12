# Research: Code Quality Fixes

**Date**: 2026-06-13

## R1: Error Handling Patterns

### Decision: Replace `.ok()` with appropriate error handling per call site

**Rationale**: The codebase has 19 `.ok()` calls in non-test code. Each falls into one of three categories:

1. **User-facing operations** (file open, save, IPC) — must show notification via existing `show_notification()`
2. **Background tasks** (file watcher metadata, snapshot cleanup) — must log via `eprintln!()` or `.log_err()`
3. **Entity closure internals** (timer updates, state mutations) — can use `.log_err()` since failure is non-critical but should not be silent

**Alternatives considered**:
- `anyhow`/`thiserror` error types: Rejected — would require defining error types across the codebase for a project that uses `eprintln!()` and notifications as its error strategy
- `log` crate: Rejected — adds a dependency for functionality already served by `eprintln!()` and notifications

### Decision: Eliminate all `unwrap()`/`expect()` in non-test code

**Rationale**: Constitution Principle II mandates no panics in production. Two `expect()` in `main.rs` (L137, L402) are platform init and window creation — these should use `?` with proper error reporting. One `unwrap()` in `workspace.rs:1143` is notification rendering — should use `if let`.

**Affected calls**:
- `main.rs:137` — Windows platform init `expect()` → `?` with error print
- `main.rs:402` — window opening `expect()` → `?` with error print
- `workspace.rs:1143` — notification rendering `unwrap()` → `if let` pattern

## R2: Workspace Decomposition Strategy

### Decision: Extract search and diff view logic into their existing module files

**Rationale**: `workspace.rs` is 1171 lines. The Render impl alone is 281 lines (L890-1171). Analysis shows these blocks can move to existing files:

| Block | Lines | Target | Estimated reduction |
|-------|-------|--------|-------------------|
| Search find/replace logic | ~200 lines | `search.rs` | 200 |
| Diff comparison logic | ~50 lines | `diff_view.rs` | 50 |
| Notification mechanism | ~15 lines | `workspace.rs` (keep — core state) | 0 |

The Render impl (281 lines) must stay in `workspace.rs` since it's the `Render` trait implementation for `LiteWorkspace`.

After extraction: 1171 - 250 = ~920 lines. To reach under 800, also extract:
- Autosave/snapshot logic (~40 lines) → can stay, logic is tightly coupled to workspace state
- Session save/load helpers (~30 lines) → can be simplified inline

**Alternatives considered**:
- New module files (e.g., `workspace_search.rs`): Rejected — violates Constitution Principle IV
- Trait-based decomposition: Rejected — over-engineering for a ~4000-line codebase
- Keep as-is: Rejected — 1171 lines is past the "hard to maintain" threshold

### Constraint: No circular imports
- `search.rs` and `diff_view.rs` already import from `workspace.rs` types
- Moving logic OUT of workspace into these files maintains the existing dependency direction
- The moved functions will take `&mut LiteWorkspace` or specific state as parameters

## R3: Notification Mechanism

### Decision: Reuse existing `show_notification()` — extend call sites, don't rebuild

**Rationale**: workspace.rs already has a working notification system (4-second transient overlay, bottom-right positioned). The problem is that it's only called from 3 places (save failure, encoding reload failure). The fix is to call it from all error sites that the user needs to see.

**For IPC errors** (main.rs): IPC runs before workspace exists. These should use `eprintln!()` (which the user sees in the terminal they launched from) rather than trying to route to the GUI.

**Classification of all 19 `.ok()` sites**:

| Site | Category | Action |
|------|----------|--------|
| workspace.rs:105 | Cleanup | `.log_err()` |
| workspace.rs:223 | Timer | `.log_err()` |
| workspace.rs:495 | File open | `show_notification()` |
| workspace.rs:497 | Async update | `.log_err()` |
| workspace.rs:530 | Save | Already has notification |
| workspace.rs:884 | Diff state | `.log_err()` |
| workspace.rs:1166 | Drag-drop open | `show_notification()` |
| main.rs:253 | IPC file open | `eprintln!()` + proper return |
| main.rs:345 | IPC | `eprintln!()` + proper return |
| main.rs:384 | IPC file open | `eprintln!()` + proper return |
| main.rs:411 | IPC | `eprintln!()` + proper return |
| command_center.rs:166 | File open | Needs workspace ref → notification |
| file_watcher.rs:37,38 | Metadata | `.log_err()` |
| file_watcher.rs:75,76 | Metadata | `.log_err()` |
| recent_files.rs:39,40 | Parse | `.log_err()` |
| search.rs:233 | Regex | Return error to caller → notification |
| topbar.rs:173 | Operation | `.log_err()` |

## R4: Configuration Constants

### Decision: Add `AppConfig` struct to `utils.rs`

**Rationale**: `utils.rs` is a general utility module (232 lines). Adding a small config struct here is natural and doesn't create a new file.

**Constants to extract**:
- `AUTOSAVE_INTERVAL_SECS: u64 = 30` (workspace.rs:185)
- `FILE_WATCHER_POLL_SECS: u64 = 5` (workspace.rs:197)
- `NOTIFICATION_DISPLAY_SECS: u64 = 4` (workspace.rs:217)
- `SNAPSHOT_RETENTION_DAYS: u64 = 7` (workspace.rs:100)
- `MAX_RECENT_FILES: usize = 20` (recent_files.rs:45)
- `IPC_BUFFER_SIZE: usize = 8192` (main.rs — if present)

**Alternatives considered**:
- TOML config file: Rejected — over-engineering; these are developer-tunable, not user-facing
- Separate `config.rs` file: Rejected — Constitution Principle IV
