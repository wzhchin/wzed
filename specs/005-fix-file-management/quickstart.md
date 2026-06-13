# Quickstart: Validating the File-Management Fixes

**Feature**: specs/005-fix-file-management

Runnable validation scenarios. The editor has no automated UI tests, so these
are manual integration checks (per the project's `test-step.md` convention)
plus the `cargo test` unit-test gate. Each scenario maps to a spec acceptance
criterion and a success metric.

## Prerequisites

- Build succeeds and unit tests pass:
  ```bash
  cargo build && cargo test
  ```
- `cargo clippy` reports no `unwrap()` violations (Constitution gate).
- A scratch directory for test fixtures, e.g. `/tmp/wzed-fix-test/`.

---

## Scenario 1 — Non-UTF-8 round-trip (P1, FR-001/002, SC-001)

Proves the headline bug fix: saving no longer silently rewrites files as UTF-8.

1. Create a GBK-encoded file outside the editor:
   ```bash
   iconv -f UTF-8 -t GBK <<< '你好，世界 hello' > /tmp/wzed-fix-test/gbk.txt
   ```
2. Launch: `cargo run -- /tmp/wzed-fix-test/gbk.txt`.
3. Append one ASCII line, save (Ctrl-S).
4. Inspect on disk — encoding must still be GBK:
   ```bash
   file /tmp/wzed-fix-test/gbk.txt          # should NOT say UTF-8
   iconv -f GBK -t UTF-8 < /tmp/wzed-fix-test/gbk.txt   # decodes cleanly, edit present
   ```
5. Reopen the file in the editor — text intact, no mojibake, encoding indicator
   shows GBK.
6. Repeat the cycle with a UTF-8 file (the common case) to confirm **no
   regression**: it stays UTF-8 and round-trips.

**Pass**: file stays GBK after save; UTF-8 path unchanged.

## Scenario 2 — Unencodable character rejection (FR-003)

Proves the editor does not silently corrupt on a character the encoding can't
represent.

1. Open/create a file the editor treats as ISO-8859-1 (or force an encoding
   that lacks CJK coverage).
2. Insert a CJK character (e.g. 你) that ISO-8859-1 cannot encode.
3. Save (Ctrl-S).
4. Expect: save is **rejected**, a notification is shown, and the on-disk file
   is **unchanged** (not written with replacement garbage).

**Pass**: notification shown, file unmodified, no `&#...;` entities on disk.

## Scenario 3 — Snapshot recovery after session loss (P1, FR-004/005, SC-002)

Proves the snapshot subsystem finally pays out.

1. Launch the editor, open a file, make unsaved edits. Let at least one autosave
   interval pass (~30s) so a snapshot exists.
2. Simulate session loss: damage/remove the session file:
   ```bash
   rm ~/.config/wzed/session.json
   ```
3. Relaunch the editor.
4. Expect: the tab reappears with the **unsaved edits** recovered from the
   snapshot — not a blank untitled tab.

**Pass**: unsaved content restored despite missing session.json.

## Scenario 4 — Large unsaved file stays light (FR-006, SC-003)

Proves session.json no longer duplicates the full buffer every interval.

1. Create a multi-megabyte file, open it, make a one-line edit so it's dirty.
2. Observe `~/.config/wzed/session.json` size and per-interval disk writes across
   a few autosave cycles (e.g. `watch -n 2 'ls -l ~/.config/wzed/session.json'`).
3. Expect: session.json does NOT balloon to the size of the multi-MB buffer, and
   per-interval write churn is bounded by metadata, not full buffer text.

**Pass**: session.json size independent of the dirty buffer's size.

## Scenario 5 — External change is surfaced, not silent (FR-007/008/009)

Proves event-driven detection and conflict awareness.

1. Open a file in the editor. From another terminal, modify it:
   ```bash
   echo "external edit" >> /tmp/wzed-fix-test/extern.txt
   ```
2. Expect: within a couple seconds the editor **notifies** the user about the
   external change (not a silent content swap).
3. Self-write check: edit and save from within the editor. Expect NO
   "external change" notification fires for the editor's own save (FR-009).
4. Conflict check: make the editor tab dirty (unsaved edit), then modify the
   file externally. Expect a **conflict** surfaced to the user — neither side
   silently discarded.

**Pass**: external changes notify promptly; own saves don't false-trigger;
dirty+external surfaces a conflict.

## Scenario 6 — Action reachable over IPC without a dispatch table (FR-010, SC-005)

Proves unified dispatch.

1. With the editor running, send a command from a second instance:
   ```bash
   cargo run -- -c new-file
   ```
2. Expect: the running instance opens a new tab (no second window).
3. Regression sweep: send each existing command (`save-file`, `open-file`,
   `toggle-find`, etc.) and confirm each still dispatches correctly.
4. (Maintainer proof) Add a trivial new action to the `actions!` macro with a
   keybinding, rebuild, send `-c <new-action>` — expect it to work with **no**
   edits to any IPC dispatch table.

**Pass**: `-c new-file` opens a tab; existing commands still work; a freshly
added action is IPC-invocable with no extra wiring.

---

## Gate (all scenarios)

- `cargo build` ✓, `cargo test` ✓, `cargo clippy` (no `unwrap`) ✓.
- Scenarios 1 and 3 are the two P1 data-safety gates; they MUST pass before this
  feature is considered done. Scenarios 4–6 are P2/P3.
