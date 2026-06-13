# Data Model: Fix File Management Correctness & Safety

**Feature**: specs/005-fix-file-management

Describes the data entities and their state transitions for this fix. Field
types are conceptual (no Rust syntax), focused on meaning and validation rules.
Implementation lives in existing source files (Constitution Principle IV).

---

## Entity: Encoded File

A text file on disk paired with its character encoding. The encoding is part
of the file's identity for round-tripping.

**Attributes**
- **path** — absolute filesystem location.
- **encoding** — the character encoding (UTF-8 by default, else detected or
  user-selected: GBK, Shift_JIS, ISO-8859-1, etc.).
- **text** — the Unicode string representation held in the editor buffer.

**Invariant (FR-001, FR-002)**
Saving MUST write `text` re-encoded into `encoding`. The on-disk bytes' encoding
equals the declared `encoding` before and after a save. The encoding indicator
shown to the user always equals the actual on-disk encoding.

**Validation rule (FR-003)**
If `text` contains any character that `encoding` cannot represent, the save is
**rejected** and the user is notified. No partial/corrupted write reaches disk.

**State transition: Open → Edit → Save**
```
disk bytes (encoding E)
  --decode (E)-->  editor text  --edit-->  editor text'
  --encode (E)-->  disk bytes (encoding E)   [round-trip preserves E]
```

**Note**: In this editor, the encoding source of truth for the save path becomes
the Zed **buffer**'s encoding field (`buffer.encoding()`). The existing per-tab
encoding field remains for session persistence and the encoding picker; they are
not reconciled in this fix (see research.md Open Question).

---

## Entity: Snapshot Backup

A recovery copy of a dirty tab's content, written on autosave, intended as the
fallback when session state is lost.

**Attributes**
- **identity key** — a STABLE identifier (file path for path-backed tabs; a
  generated id for untitled tabs) that survives tab reorder/close. Replaces the
  current volatile tab-index naming.
- **content** — the full Unicode text at snapshot time.
- **timestamp** — write time, for retention pruning.

**Invariant (FR-004, FR-005)**
Snapshots MUST be both written AND read back. After session loss, each dirty tab
recovers its content from the snapshot whose identity key matches.

**State transitions**
```
dirty tab  --autosave-->  snapshot file (key K)
session loss
restore: for each recoverable tab, read snapshot by key K --> tab content
retention: snapshot older than 7 days  --prune-->  deleted
```

**Validation rule**
Recovery MUST NOT fabricate content. A tab with no matching snapshot falls back
gracefully (reload file from disk, or empty untitled), never invented text.

---

## Entity: Session State

The lightweight descriptor of the editor's open-tab set. Survives crashes via
atomic writes.

**Attributes**
- **tabs** — ordered list of tab descriptors.
- **active** — index of the focused tab.
- per tab: **path** (optional), **pinned**, **encoding** (when non-UTF-8).

**Invariant (FR-006)**
The session file records tab **identity + metadata only**. It MUST NOT carry the
full text of every dirty buffer on every autosave interval. (Unsaved content is
recovered via Snapshot Backup, not embedded here.) This bounds per-interval write
volume independent of buffer size.

**State transition**
```
any state change  --save_session (atomic tmp+rename)-->  session.json (metadata only)
crash/restart  --restore_session-->  tabs reconstructed
   ├─ path-backed tab  --> reopen from disk (+ recover unsaved via snapshot if dirty)
   ├─ untitled tab     --> restore content via snapshot
   └─ session unreadable --> recover all dirty tabs from snapshots (Decision 2)
```

**Sequencing constraint**
Snapshot-recovery (read path) MUST be implemented before session.json is stripped
of embedded content. Otherwise unsaved work has nowhere to survive.

---

## Entity: External Change

A modification to an open file's on-disk content originating outside the editor.

**Attributes**
- **path** — the affected file.
- **kind** — created / changed / removed / watcher-resync-needed.

**Invariants (FR-007, FR-008, FR-009)**
- Detection is event-driven (not fixed-interval polling).
- The editor's own saves MUST NOT trigger a reload (suppressed via known-mtime
  comparison).
- A clean tab with an external change → user is notified, reload is deliberate.
- A dirty tab with an external change → conflict surfaced to the user; neither
  side silently discarded.

**State transition**
```
external write to watched path
  --fs event-->  detection
  ├─ mtime == our last write?  --> ignore (self-write)
  ├─ tab clean      --> notify user (option to reload)
  └─ tab dirty      --> notify user of conflict (both sides preserved)
```
