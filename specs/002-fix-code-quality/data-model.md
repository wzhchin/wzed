# Data Model: Code Quality Fixes

**Date**: 2026-06-13

## Entities

### AppConfig

Centralized configuration constants. Read-only after creation. Lives in `utils.rs`.

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| autosave_interval_secs | u64 | 30 | How often to autosave snapshots |
| file_watcher_poll_secs | u64 | 5 | File watcher polling interval |
| notification_display_secs | u64 | 4 | How long error notifications stay visible |
| snapshot_retention_days | u64 | 7 | Days to keep autosave snapshots |
| max_recent_files | usize | 20 | Maximum entries in recent files list |

**Relationships**: None — standalone constants struct.

**Validation**: All numeric values must be > 0.

**State transitions**: None — immutable after creation.

### ErrorNotification (existing, extended)

Already exists as `Option<(String, Instant)>` on `LiteWorkspace`. No schema change needed — only the *call sites* change (more errors trigger it instead of being silently swallowed).

| Field | Type | Purpose |
|-------|------|---------|
| message | String | Human-readable error description |
| created_at | Instant | When notification was created (for auto-dismiss) |

**Relationships**: Owned by `LiteWorkspace`.

**State transitions**:
- `None` → `Some((msg, now))` — error occurs
- `Some` → `None` — auto-dismiss after `notification_display_secs`

## Impact on Existing Code

No new entities are created. `AppConfig` is a new struct but contains only constants previously scattered as literals. The notification mechanism (`show_notification`) is unchanged — only its call sites expand.
