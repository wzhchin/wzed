# Contracts: Error Handling

**Date**: 2026-06-13

## Internal Contract: Error Response Strategy

This project has no external API. The "contract" is the internal discipline for how errors are handled across modules.

### Three-tier error response

| Tier | When to use | Mechanism | Example |
|------|------------|-----------|---------|
| **User notification** | Error is caused by or affects the user's current action | `show_notification(msg, cx)` | File open fails, save fails |
| **Log** | Background/internal error the user doesn't need to see but developers should know about | `.log_err()` or `eprintln!()` | File watcher metadata, snapshot cleanup |
| **Propagate** | Caller can meaningfully handle the error | `?` operator | Regex compilation, IPC parsing |

### Decision matrix

```
Is there a workspace reference available?
├── YES → Is this a user-initiated action?
│   ├── YES → show_notification()
│   └── NO  → .log_err()
└── NO  → Is this in main.rs / IPC context?
    ├── YES → eprintln!() + return/exit
    └── NO  → .log_err() as fallback
```

### Functions gaining new error returns

| Function | Module | Change |
|----------|--------|--------|
| `show_notification` | workspace.rs | No change — call sites expand |
| File open handlers | workspace.rs, command_center.rs | Replace `.ok()` with `show_notification()` |
| File watcher check | file_watcher.rs | Replace `.ok()` with `.log_err()` |
| Recent files load/save | recent_files.rs | Replace `.ok()` with `.log_err()` |
| Regex builder | search.rs | Replace `.ok()` with `?` propagation |

### Notification content guidelines

Messages must be:
- One sentence, under 80 characters
- State what failed, not why (e.g., "Failed to open file" not "IoError: Permission denied (os error 13)")
- No technical jargon visible to user
