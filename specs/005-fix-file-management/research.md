# Research: Fix File Management Correctness & Safety

**Feature**: specs/005-fix-file-management
**Date**: 2026-06-13

This document resolves the technical unknowns that gate the implementation
plan. Each finding was verified against the actual Zed source / `encoding_rs`
crate source — API signatures quoted below are real, not assumed.

---

## Decision 1: Save-path encoding (fixes FR-001, FR-002, FR-003)

### Decision
On save, encode the editor text back into the file's encoding before writing
bytes. Source the encoding from the buffer (Zed already tracks it), not from
a hand-maintained `Tab` field. Abort the save and surface an error if the
text contains characters unrepresentable in the target encoding.

### Rationale — verified facts
- **`encoding_rs::Encoding::encode`** signature (verified in
  `~/.cargo/registry/.../encoding_rs-*/src/lib.rs`):
  ```rust
  pub fn encode<'a>(&'static self, string: &'a str) -> (Cow<'a, [u8]>, &'static Encoding, bool)
  ```
  Returns `(encoded_bytes, encoding_used, had_errors)`. For UTF-8 it returns the
  borrowed byte slice with `had_errors = false` — so the common path is zero-copy
  and behaviorally identical to today. The current bug is simply that this is
  never called: `write_editor_to_file` writes `std::fs::write(path, &content)`
  with `content: String` (always UTF-8).
- **The buffer already tracks encoding.** Verified in
  `zed/crates/language/src/buffer.rs`:
  - `buffer.encoding() -> &'static Encoding` (line 1426)
  - `buffer.set_encoding(&'static Encoding)` (line 1431)
  - field `encoding: &'static Encoding` (line 138)
  This means the fix can read `buffer.encoding()` rather than threading the
  `Tab.encoding` field through `write_editor_to_file`. It also makes `Tab.encoding`
  redundant with the buffer's encoding — a cleanup opportunity, but out of scope
  for this fix (see Open Question below).
- **`did_save` does NOT take an encoding.** Verified signature
  (`buffer.rs:1526`): `did_save(&mut self, version: clock::Global, mtime:
  Option<MTime>, cx)`. The earlier draft assumption that it took an encoding
  arg was wrong — line 1552's encoding arg belongs to the unrelated
  `reload_with_encoding`. So the existing `did_save(version, None, cx)` call is
  correct and unchanged.
- **Unencodable handling**: `encode` sets `had_errors = true` and, per
  `encoding_rs` docs, substitutes HTML numeric character references for
  unmappable chars. That substitution is a corruption risk — we MUST NOT write
  it silently. FR-003 is satisfied by checking `had_errors` and returning an
  `Err` (surfaced via the existing notification banner) before any `fs::write`.

### Alternatives considered
- **Delegate to a Zed "save with encoding" API**: investigated; does not exist.
  Zed itself only writes UTF-8; its encoding tracking is for read/decode
  (`reload_with_encoding`). Hand-encoding via `encoding.encode(&text)` is the
  minimal, correct path. Satisfies Constitution Principle V (delegate) at the
  crate level — we use `encoding_rs`, we don't reimplement encoding.
- **Reuse `Tab.encoding` instead of `buffer.encoding()`**: rejected. The buffer
  is the source of truth for what's displayed; reading its encoding avoids a
  second field that can drift. (Whether to later delete `Tab.encoding` is
  tracked as an Open Question.)

### Open Question (deferred, not blocking)
`Tab.encoding` and `buffer.encoding()` are now redundant. This fix reads
`buffer.encoding()` for the save path but leaves `Tab.encoding` in place
(session persistence and the encoding picker still use it) to keep the change
surface minimal. Reconciling them is a separate cleanup, out of scope here.

---

## Decision 2: Snapshot recovery (fixes FR-004, FR-005)

### Decision
Wire the existing snapshot files into the restore path: when `session.json` is
unreadable or missing, recover dirty tabs from their snapshots. Give snapshots
stable, content-addressable identity (not volatile tab index) so recovery maps
to the right content after reorder/close.

### Rationale
- Today `restore_session` (`workspace.rs:238`) only reads `session.json`. On
  any read/parse failure it falls straight to a blank untitled tab — the
  snapshots written by `save_dirty_snapshots` are never consulted. The recovery
  path is the missing piece, not the write path.
- The current snapshot filename `tab-{index}-{epoch}.txt` (`workspace.rs:605`)
  uses tab **index**, which changes on drag/pin/close. This makes "which
  snapshot belongs to which tab" ambiguous on recovery. Recovery needs a
  stable key. Options evaluated:
  - **Path-of-origin** (the file path for saved tabs): stable and meaningful,
    but untitled tabs have no path.
  - **Untitled tabs**: keyed by nothing stable; these are exactly the tabs most
    in need of recovery. Use the existing `unsaved_content` in session.json as
    the primary recovery channel for them, and snapshots as the fallback when
    even session.json is gone.
- `prune_old_snapshots` (7-day retention) is correct and stays. The fix is
  read-side; write-side only changes the filename to be content-stable.

### Alternatives considered
- **Delete the snapshot subsystem entirely**: rejected — Constitution Principle
  VI mandates snapshot backups exist. The fix is to make them functional.
- **Deduplicate session.json vs snapshot content**: addressed under Decision 3.

---

## Decision 3: Stop full-buffer serialization into session.json (fixes FR-006)

### Decision
Decouple the session file from raw buffer contents. The session file records
**tab identity + metadata** (path, pinned, encoding, active index). Unsaved
content is recovered through the snapshot mechanism (Decision 2), not embedded
in session.json.

### Rationale
- Today `unsaved_content` is `Some(full_text)` for every path-less or dirty tab
  (`workspace.rs:60`), so session.json grows with the largest dirty buffer and
  is fully rewritten every 30s. With snapshot recovery in place, the session
  file no longer needs to carry buffer text — that removes the per-interval
  full-buffer duplication (SC-003).
- **Sequencing matters**: Decision 2 (snapshot recovery) must land before
  Decision 3 (drop `unsaved_content`), otherwise there is a window where
  unsaved content is neither in session.json nor recoverable. This ordering is
  encoded in tasks.

### Alternatives considered
- **Incremental/diff serialization**: rejected as premature complexity. Removing
  the full text from session.json entirely (since snapshots now own recovery)
  is simpler and meets SC-003.
- **Keep `unsaved_content` but compress**: rejected — adds a dependency and
  doesn't fix the "rewrite everything every tick" write-amplification.

---

## Decision 4: File-system-event watching via the `fs` crate (fixes FR-007, FR-009)

### Decision
Replace the hand-rolled 5s polling watcher with `Fs::watch` (the already-dep'd
Zed `fs` crate). Keep the dirty-tab protection (never reload a tab with unsaved
edits). On external change, **notify the user** instead of silently swapping
content; surface conflicts when local edits + external change coexist.

### Rationale — verified facts
- **`Fs::watch` API** (verified in `zed/crates/fs`):
  ```rust
  async fn watch(&self, path: &Path, latency: Duration)
      -> (Pin<Box<dyn Send + Stream<Item = Vec<PathEvent>>>>, Arc<dyn Watcher>)
  ```
  Event model: `PathEvent { path: PathBuf, kind: Option<PathEventKind> }` with
  `PathEventKind::{Created, Changed, Removed, Rescan}`. Batched + debounced via
  `latency`. **Works on arbitrary absolute paths — no worktree/project
  required** (verified via `settings_file.rs` usage, which watches config dirs
  directly).
- **Global registration**: `RealFs::new(git_binary_path, background_executor)`
  set as global via `<dyn Fs>::set_global(...)`, retrieved with `Fs::global(cx)`.
  wzed does not currently initialize `Fs`; the plan adds this to `main.rs`
  alongside the existing GPUI init.
- **Self-write suppression (FR-009)**: the stream fires `Changed` on any write,
  including our own saves. The existing `update_mtime` pattern is the model —
  keep a known mtime per path and ignore events whose mtime matches our last
  write. The `fs` events give us the path; we still stat to compare mtime, same
  as today, but triggered by events instead of a fixed poll.
- Real usage references: `zed/crates/settings/src/settings_file.rs:88`
  (`watch_config_dir`), `zed/crates/worktree/src/worktree.rs`.

### Alternatives considered
- **Keep polling**: rejected — Constitution Principle V (delegate to Zed crates,
  don't reimplement). `fs` is already a dependency.
- **`notify` crate**: rejected — would add a new runtime dependency (Principle
  III) when `fs` already exists.

---

## Decision 5: Unified IPC dispatch via `build_action` (fixes FR-010)

### Decision
Replace the hand-written command-string `match` in `main.rs` with
`cx.build_action(name, None)` + `window.dispatch_action(action, cx)`, the same
path the command center already uses.

### Rationale — verified facts
- **`App::build_action`** (verified `zed/crates/gpui/src/app.rs:2034`):
  ```rust
  pub fn build_action(&self, name: &str, data: Option<serde_json::Value>)
      -> std::result::Result<Box<dyn Action>, ActionBuildError>
  ```
- **`Window::dispatch_action`** (verified `zed/crates/gpui/src/window.rs:1879`):
  ```rust
  pub fn dispatch_action(&mut self, action: Box<dyn Action>, cx: &mut App)
  ```
- **All wzed actions are unit structs** (verified `main.rs:30-74` `actions!`
  macro) → `build_action(name, None)` always works, no JSON payload needed.
- **command_center.rs already does exactly this** (`command_center.rs:104-106`):
  `cx.build_action(entry.action_name, None)` then `window.dispatch_action(...)`.
  The IPC change copies a proven pattern, not new ground.
- Other Zed callers using this exact pattern: `keymap_file.rs:536`,
  `vim/src/command.rs:1195`.

### Alternatives considered
- **Keep the match, add a lint/test**: rejected — a duplicated dispatch table
  is a defect factory; `build_action` makes it structurally impossible to drift.

### Note on the special IPC payloads
`IpcMessage::{SetText, SaveAs, SwitchTab}` carry payloads that are NOT plain
unit actions. These stay as explicit handlers in the IPC pump — they have no
keyboard-action equivalent to unify with. Only `ExecuteCommand` (the 18-entry
match) is replaced. The plan documents this boundary explicitly so no one
mistakenly tries to unify the payload variants.

---

## Cross-cutting: Constitution compliance

Every decision was checked against the six principles (see plan.md
Constitution Check). None require an amendment:
- No new runtime deps (Principle III): `encoding_rs`, `fs`, GPUI are all
  existing path/registry deps.
- No new source files, no `mod.rs` (Principle IV): all changes land in
  `workspace.rs`, `file_watcher.rs`, `main.rs`, `encoding.rs`.
- No `unwrap()` (Principle II): `build_action` returns `Result`, `encode`
  returns `had_errors`, `Fs::global` returns an `Arc` — all handled with `?`
  or `match`.
- Data safety (Principle VI): strengthened, not weakened — snapshot recovery is
  the headline improvement.
