# Contract: Single-Instance IPC Dispatch

**Feature**: specs/005-fix-file-management

Internal invariant for the IPC command channel. wzed is a single-instance app:
a second launch forwards its intent to the running instance over a local socket.

## The dispatch contract

### Message types
- **`ExecuteCommand(name)`** — a registered editor action, by fully-qualified
  name (e.g. `lite_editor::NewFile`).
- **`SetText(content)`** — replace active tab's text. Payload-bearing; NOT a
  keyboard action.
- **`SaveAs(path)`** — save active tab to a path. Payload-bearing; NOT a
  keyboard action.
- **`SwitchTab(index)`** — focus a tab by index. Payload-bearing; NOT a
  keyboard action.
- **`OpenFiles(paths)`** — open files by path.

### Dispatch guarantee (FR-010)
- `ExecuteCommand(name)` dispatches via the **unified action registry**
  (`build_action` + `dispatch_action`) — the SAME path used by keymaps and the
  command center. There is NO hand-maintained command-name → handler table.
  Consequence: any action added via the normal `actions!` registration is
  immediately invocable over IPC with zero extra wiring.
- An unknown/unregistered `name` → safe failure (logged warning), never a panic.
- The payload-bearing variants (`SetText`, `SaveAs`, `SwitchTab`, `OpenFiles`)
  remain explicit handlers — they have no keyboard-action equivalent and are
  intentionally NOT unified.

### Failure handling (Constitution Principle II)
- `build_action` returns `Result`; the `Err` path is logged, not unwrapped.
- Unknown command → `[IPC] unknown/failed command` log line, no crash.

### Round-trip
A second instance launched with `-c new-file` (or the qualified form) MUST
trigger the running instance's `NewFile` action — provable without editing any
dispatch table (SC-005).
