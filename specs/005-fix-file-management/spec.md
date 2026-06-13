# Feature Specification: Fix File Management Correctness & Safety

**Feature Branch**: `005-fix-file-management`

**Created**: 2026-06-13

**Status**: Draft

**Input**: User description: "修复上面提出的所有 bug"

## User Scenarios & Testing *(mandatory)*

WZed is a text editor. Its core promise is: your text is never corrupted, never
lost, and behaves predictably. A code review surfaced six defects across the
file-management subsystem ranging from **silent data corruption** (the worst
possible editor failure) down to redundant work and scalability hazards. This
spec defines the user-visible outcomes each fix must deliver.

### User Story 1 - Round-Trip a Non-UTF-8 File Without Corruption (Priority: P1)

A user opens a file written in a legacy or regional encoding (e.g. GBK,
Shift_JIS, ISO-8859-1), edits it, and saves. They expect the file to **stay in
its original encoding** on disk, readable by the same tools that produced it.
Today, saving silently rewrites the file as UTF-8, destroying its encoding
without warning. This is data loss in disguise: colleagues, build systems, or
other editors that expect the original encoding see a corrupted file.

**Why this priority**: Silent data corruption is the single worst failure mode
for an editor. Every other issue in this spec degrades experience or wastes
resources; this one destroys the user's data. Constitution Principle VI (Session
and Data Safety) makes data integrity mandatory.

**Independent Test**: Open a GBK-encoded file, change one character, save,
reopen it in an external tool that expects GBK. The file must still be valid
GBK and the edit present. Fully testable without any other fix.

**Acceptance Scenarios**:

1. **Given** a file on disk encoded as GBK, **When** the user opens it, edits a
   character, and saves, **Then** the file on disk remains valid GBK and
   contains the edit.
2. **Given** a Shift_JIS file open and edited, **When** the user saves, **Then**
   the file round-trips: reopening it shows the same text with no mojibake and
   the encoding indicator reflects Shift_JIS throughout.
3. **Given** a UTF-8 file (the common case), **When** the user edits and saves,
   **Then** the file stays UTF-8 and round-trips identically to today — no
   regression for the default path.
4. **Given** a file containing characters that cannot be represented in the
   file's encoding, **When** the user saves, **Then** the editor does not
   silently corrupt or silently drop those characters — the failure is surfaced
   to the user rather than written to disk.

---

### User Story 2 - Recover Unsaved Work After a Crash (Priority: P1)

A user has several tabs open, some with unsaved edits. WZed or the OS crashes.
On relaunch, the user expects to get back exactly what they had: the open tabs,
their unsaved content, and the active tab. Today the autosave "snapshot" backup
feature writes files to disk but **never reads them back** — it is a paid-for
insurance policy that pays nothing. When the session file is intact the user is
fine; when it is damaged or missing, their unsaved work is gone despite the
snapshots that were supposed to protect it.

**Why this priority**: Unrecoverable data loss in the exact scenario snapshots
were built for. Constitution Principle VI (Session and Data Safety) mandates
that "user data MUST survive crashes." The snapshot mechanism exists to satisfy
this and currently does not.

**Independent Test**: Open a file, make unsaved edits, simulate a session loss
(damage/remove the session file while snapshots exist), relaunch. The unsaved
edits must reappear from the snapshot recovery path.

**Acceptance Scenarios**:

1. **Given** a tab with unsaved edits and a corresponding snapshot backup,
   **When** the session file is unreadable or missing, **Then** on relaunch the
   editor recovers the tab's content from its snapshot rather than showing a
   blank untitled tab.
2. **Given** multiple dirty tabs and their snapshots, **When** the session is
   lost, **Then** recovery restores each dirty tab's content from its own
   snapshot.
3. **Given** a clean (non-dirty) tab with no snapshot, **When** the session is
   lost, **Then** the editor does not fabricate content for that tab — it falls
   back gracefully (e.g. reloads the file from disk).

---

### User Story 3 - Stay Responsive With Large, Unsaved Files (Priority: P2)

A user edits a large file (multi-megabyte) and leaves it unsaved. Today, every
autosave tick serializes the **entire file text** into the session file, and
writes a second full copy to a snapshot — repeatedly, on a fixed interval. This
produces sustained heavy disk I/O and a session file that grows with the largest
buffer, even when only a few lines changed. The editor should remain lightweight
regardless of file size; autosave must not scale with buffer size on every tick.

**Why this priority**: Performance degradation that grows with use. Not data
loss, but it undermines the "minimal, fast editor" identity and will bite as
files grow. Below the two P1 data-safety stories in urgency.

**Independent Test**: Open a large file, make it dirty, let several autosave
intervals pass, and observe that per-tick disk write volume does not scale with
the full buffer size.

**Acceptance Scenarios**:

1. **Given** a multi-megabyte unsaved file, **When** several autosave intervals
   elapse, **Then** the per-interval disk write volume does not duplicate the
   entire buffer into the session file on every tick.
2. **Given** a dirty file with a small subsequent edit, **When** the next
   autosave runs, **Then** the write volume reflects the incremental nature of
   the change, not a full re-serialization of unrelated content.

---

### User Story 4 - Be Told When an Open File Changes Externally (Priority: P2)

A user has a file open. Another program (a build tool, formatter, git pull,
another editor) modifies it on disk. The user expects to be informed and to
choose whether to reload, rather than having the editor silently overwrite the
tab's content and destroy the cursor position and undo history. Today, for a
clean tab, the change is silently absorbed; for a dirty tab, the change is
never detected at all — a conflict that goes unnoticed until work is lost.

**Why this priority**: Predictability and conflict awareness. Silent reloads
and invisible conflicts both violate user trust, though neither corrupts
already-saved data. P2 because it concerns experience and conflict handling
rather than data destruction.

**Independent Test**: Open a file, modify it with an external tool, and observe
the editor's response (notification / prompt) rather than a silent swap.

**Acceptance Scenarios**:

1. **Given** a clean (no local edits) tab whose file changes externally, **When**
   the change is detected, **Then** the editor notifies the user and reloads
   only with awareness (cursor state handling is explicit, not silently lost).
2. **Given** a tab with unsaved local edits whose file changes externally,
   **When** the change is detected, **Then** the editor surfaces the conflict to
   the user instead of silently dropping either side.
3. **Given** external file changes, **When** detection occurs, **Then** the
   detection is prompt and event-driven rather than delayed by a fixed polling
   interval.

---

### User Story 5 - Add an Editor Action Once, Use It Everywhere (Priority: P3)

A maintainer adds a new editor action (e.g. a new command with a keyboard
shortcut). Today they must register the action, add a keybinding, **and**
separately maintain a hand-written lookup table that maps command names to
handler calls for the single-instance command channel. Forgetting the third
step produces an action that works via keyboard but not via the command channel
— a silent, hard-to-notice gap. Command dispatch should be unified so adding an
action once makes it available everywhere.

**Why this priority**: Maintainability and correctness-consistency. A
duplicated dispatch table is a defect factory, but it produces missing-feature
bugs, not data loss. Lowest priority of the six.

**Independent Test**: Add a trivial new action and confirm it is invocable via
the command channel without editing any separate command-to-handler lookup
table.

**Acceptance Scenarios**:

1. **Given** a newly added editor action with a keybinding, **When** the
   maintainer does nothing beyond the normal action registration, **Then** the
   action is invocable through the single-instance command channel.
2. **Given** the existing set of actions, **When** each is invoked via the
   command channel, **Then** all of them dispatch correctly (no regression vs.
   today's behavior).

---

### Edge Cases

- What happens when a character in the editor cannot be encoded into the file's
  target encoding? (Must surface to user, never silently corrupt.)
- What happens when both the session file AND snapshots are missing/damaged?
  (Graceful fallback to a clean state, no crash — Constitution Principle II.)
- What happens when an external file change and a local autosave race? (Detection
  must distinguish the editor's own write from a genuine external change.)
- What happens when a snapshot exists for a tab index that no longer exists
  after tab reordering/closing? (Recovery must map to the right content, not a
  stale index.)
- What happens when the external file is deleted or becomes unreadable? (Surfaced
  to user, not a silent empty tab.)
- What happens when a new command-channel action has no handler? (Safe no-op or
  logged warning, never a panic — Constitution Principle II.)

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The editor MUST preserve a file's on-disk encoding across an
  open → edit → save cycle. Saving a non-UTF-8 file MUST write bytes in that
  file's encoding, not UTF-8.
- **FR-002**: The editor MUST keep its encoding indicator consistent with the
  actual on-disk encoding after a save.
- **FR-003**: The editor MUST surface an error to the user if content cannot be
  represented in the target encoding, rather than silently corrupting or
  truncating the file.
- **FR-004**: Autosave snapshot backups MUST be recoverable: after a session
  loss, the editor MUST restore dirty tabs' content from their snapshots.
- **FR-005**: Snapshot recovery MUST map each recovered tab to the correct
  content even when tab ordering has changed, and MUST NOT fabricate content for
  tabs that had no snapshot.
- **FR-006**: Autosave MUST NOT duplicate the entire buffer of every dirty tab
  into the session file on every interval. Session write volume per interval
  MUST NOT scale with total dirty-buffer size.
- **FR-007**: The editor MUST detect external changes to open files and inform
  the user, rather than silently overwriting the tab.
- **FR-008**: When a file with unsaved local edits changes externally, the
  editor MUST surface the conflict to the user instead of silently discarding
  either the local edits or the external change.
- **FR-009**: External-change detection MUST distinguish the editor's own saves
  from genuine external modifications (no false-positive self-triggered
  reloads).
- **FR-010**: Editor actions registered normally MUST be invocable through the
  single-instance command channel without maintaining a separate, manually
  synced command-name-to-handler mapping.
- **FR-011**: All changes MUST comply with the Constitution: no `unwrap()` in
  non-test code, no silent error discards, atomic writes for persisted state,
  and delegation to Zed crates rather than reimplementing framework
  functionality.

### Key Entities *(include if feature involves data)*

- **Encoded File**: A text file with an associated character encoding (UTF-8 by
  default, others detected/selected). The encoding is part of the file's
  identity for the purposes of round-tripping — saving must honor it.
- **Session State**: The set of open tabs, their active selection, pinned
  status, and any content not yet saved to a file. Must survive crashes via
  atomic writes.
- **Snapshot Backup**: A recovery copy of a dirty tab's content, written on
  autosave, intended as the fallback when the session state itself is lost.
  Must be both written AND read back for recovery to be meaningful.
- **External Change**: A modification to an open file's on-disk content
  originating outside the editor. Must be detected and surfaced, and must not
  collide silently with local edits.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of non-UTF-8 files round-trip through open → edit → save
  with no encoding change or content corruption (verified for GBK, Shift_JIS,
  and ISO-8859-1 sample files).
- **SC-002**: After a simulated session-file loss, 100% of dirty tabs recover
  their unsaved content from snapshot backups on relaunch.
- **SC-003**: With a multi-megabyte unsaved file open, per-autosave-interval
  disk write volume drops by more than half versus the current full-buffer
  re-serialization behavior.
- **SC-004**: External changes to any open file are detected and surfaced to
  the user within seconds, with zero false-positive reloads triggered by the
  editor's own saves.
- **SC-005**: A newly added editor action is invocable via the command channel
  with zero additional mapping-table edits.
- **SC-006**: `cargo build` and `cargo test` pass, and `cargo clippy` reports
  no `unwrap()` violations (Constitution compliance gate).

## Assumptions

- The scope is strictly **fixing existing behavior** of the file-management
  subsystem. No new editor features (no new file formats, no new UI surfaces
  beyond what notification/prompt already exist). Constitution Principle I
  (Minimalist Scope) applies.
- "Surfacing an error/conflict to the user" reuses the editor's existing
  notification mechanism (the transient notification banner) rather than
  introducing a new dialog framework, consistent with the minimal UI.
- The `fs` Zed crate (already a path dependency) provides file-system event
  watching and is the mandated replacement for the hand-rolled polling watcher
  (Constitution Principle V: delegate to Zed crates, don't reimplement).
- Encoding round-tripping uses the already-validated `encoding_rs` crate (already
  a dependency) for encode/decode; no new encoding dependency is introduced
  (Constitution Principle III: no new runtime dependencies).
- Snapshot recovery reads existing snapshot files; the recovery path is wired
  in, not the snapshot mechanism replaced (Constitution Principle VI mandates
  snapshot backups exist).
- Command-channel unification reuses the existing command-center action
  auto-discovery rather than a new dispatch mechanism.
- Platform support remains Linux (primary) and Windows (secondary); any
  file-watching approach must work on both.
- Performance/scalability targets (SC-003) are about eliminating gratuitous
  full-buffer duplication, not micro-optimizing serialization format.
