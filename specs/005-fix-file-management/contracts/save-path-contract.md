# Contract: Encoding-Aware Save Path

**Feature**: specs/005-fix-file-management

This editor exposes no external API (single binary, single window). "Contracts"
here are internal invariants the save path MUST uphold — the spec against which
`/speckit-tasks` will write tests and `test-step.md` scenarios.

## The save path contract

### Input
- A dirty editor buffer (text + an encoding, sourced from the buffer).
- A target filesystem path.

### Output / guarantees
1. **Encoding-preserving (FR-001)**: bytes written to disk are the editor text
   re-encoded into the buffer's encoding. Non-UTF-8 files stay non-UTF-8.
2. **Indicator consistency (FR-002)**: after save, the encoding shown to the user
   equals the on-disk encoding.
3. **No silent corruption (FR-003)**: if the text contains any character the
   target encoding cannot represent, NO bytes are written and the user is
   notified. The save is rejected, not approximated.
4. **Dirty cleared**: on successful save, the buffer's dirty flag is cleared
   (`did_save`) so the tab no longer appears edited.
5. **Watcher self-suppression (FR-009)**: after writing, the watcher's known-mtime
   for that path is updated so the editor's own write does not register as an
   external change.

### Failure handling (Constitution Principle II)
- Encode error / unencodable chars → return `Err`, surface via notification
  banner. No `unwrap`, no `let _ =`.
- Filesystem write failure → return `Err`, surface via notification banner.

### Applies to all three save entry points
- `SaveFile` (active tab; with or without existing path)
- `SaveAs` (active tab to a new path)
- `SaveAll` (all tabs with paths)

All three route through the same encoding-aware write primitive.
