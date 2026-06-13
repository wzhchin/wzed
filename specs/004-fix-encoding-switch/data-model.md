# Data Model: Fix Encoding Switch Data Loss

**Date**: 2026-06-13 | **Phase**: 1

## Entities

### Tab Encoding (existing, modified)

The `Tab` struct in `tab.rs` already has an `encoding: &'static encoding_rs::Encoding` field. No structural changes needed.

**Fields**:
- `encoding`: The character encoding used to decode this tab's file content. Set on file open (auto-detected) or on manual encoding switch.

**State transitions**:
1. Tab created → encoding = UTF-8 (default)
2. File opened → encoding = auto-detected via `chardetng`
3. User switches encoding → encoding = user-selected value (only after successful decode)
4. Error during switch → encoding unchanged (previous value preserved)

### SessionTab (existing, modified)

The `SessionTab` struct in `workspace.rs` gains an `encoding` field for persistence.

**Fields** (after modification):
- `path: Option<String>` — file path (existing)
- `unsaved_content: Option<String>` — dirty buffer snapshot (existing)
- `pinned: bool` — tab pin state (existing)
- `encoding: Option<String>` — encoding label (e.g., "GBK"), only stored when not UTF-8 (new)

**Serialization rules**:
- When saving: `encoding = Some(label)` if `tab.encoding != UTF_8`, else `None`
- When restoring: if `encoding` is `Some(label)`, override the auto-detected encoding with the stored one

### Encoding Preference (no new entity)

Encoding preferences are not a separate entity — they are stored inline in `SessionTab` as part of session persistence. This avoids creating any new data structure or file.

## Validation Rules

| Rule | Location | Enforcement |
|------|----------|-------------|
| Tab encoding must be a valid `encoding_rs::Encoding` reference | `tab.encoding` | Type system guarantees (`&'static Encoding`) |
| Encoding switch on untitled buffer is a no-op | Command center callback | Check `tab.path.is_none()` before reload |
| Encoding switch with dirty buffer is blocked | Command center callback | Check `tab.is_dirty(cx)` before reload |
| Failed decode preserves previous encoding | Command center callback | Only update `tab.encoding` on successful decode |
| Session encoding label must be parseable | Session restore | Use `encoding_from_label()` with fallback to UTF-8 |
