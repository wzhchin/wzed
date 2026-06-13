# Implementation Plan: Fix File Management Correctness & Safety

**Branch**: `005-fix-file-management` | **Date**: 2026-06-13 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/005-fix-file-management/spec.md`

## Summary

Fix six defects across wzed's file-management subsystem, ordered by data-safety
impact. The two P1 fixes eliminate **silent data corruption** (saving non-UTF-8
files rewrites them as UTF-8) and **unrecoverable loss** (snapshot backups are
written but never read back). The P2/P3 fixes address write-amplification in
session persistence, replace hand-rolled polling with the existing `fs` crate,
surface external file changes instead of silently overwriting them, and unify
IPC command dispatch through the GPUI action registry so the dispatch table can
no longer drift from the action registry.

Technical approach is fixed by research.md: all fixes reuse existing dependencies
(`encoding_rs`, Zed `fs`, GPUI `build_action`) — no new runtime deps, no new
source files.

## Technical Context

**Language/Version**: Rust, edition 2024

**Primary Dependencies**: Zed GPUI (`gpui`, `editor`, `language`, `multi_buffer`),
Zed `fs` crate (file-system event watching — already a path dep, currently
uninitialized), `encoding_rs` (encode/decode — already a dep), `similar`,
`anyhow`, `serde`/`serde_json`.

**Storage**: Local filesystem — `~/.config/wzed/{session.json, recent.json}`,
`~/.config/wzed/snapshots/`. Session/recent use atomic write (tmp+rename).

**Testing**: `cargo test` for pure-logic unit modules (`utils.rs`, `encoding.rs`,
`recent_files.rs`); manual integration testing via `test-step.md` scenarios for
UI/IPC/file-watching (see quickstart.md).

**Target Platform**: Linux (primary), Windows (secondary). File watching and IPC
must work on both.

**Project Type**: Desktop app (single binary, single window, single
`LiteWorkspace` entity).

**Performance Goals**: Per-autosave-interval disk write volume must not scale
with dirty-buffer size (SC-003). External-change detection within seconds (SC-004).

**Constraints**: No `unwrap()` in non-test code (Constitution II, enforced by
clippy). No silent error discards. Atomic writes for persisted state. No new
source files; no `mod.rs` (Constitution IV). Delegate to Zed crates (V). No new
runtime deps (III).

**Scale/Scope**: ~3700 LOC, 12 source files. This fix touches 4 of them
(`workspace.rs`, `file_watcher.rs`, `main.rs`, `encoding.rs`).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Checked against `.specify/memory/constitution.md` (v1.0.0). All principles pass
**without amendment**.

| Principle | Verdict | Note |
|-----------|---------|------|
| I. Minimalist Scope | ✅ PASS | All changes fix existing file-management behavior. No new features/UI surfaces beyond the existing notification banner. |
| II. Error Resilience | ✅ PASS | `build_action`→`Result`, `encode`→`had_errors`, `Fs::global`→`Arc` all handled with `?`/`match`. No `unwrap`, no `let _ =`. |
| III. No External Runtime Deps | ✅ PASS | Reuses existing deps only: `encoding_rs`, `fs`, GPUI. No new registry/path deps. |
| IV. Existing File Discipline | ✅ PASS | Changes land in `workspace.rs`, `file_watcher.rs`, `main.rs`, `encoding.rs`. No new files, no `mod.rs`. |
| V. Zed Framework Delegation | ✅ PASS | Replaces hand-rolled polling with `fs` crate; uses `build_action` instead of a dispatch table. Delegation increases. |
| VI. Session and Data Safety | ✅ PASS | **Strengthens** this principle: snapshot recovery (currently dead) is wired in; encoding corruption fixed. Atomic writes retained. |

**Gate result**: PASS. No Complexity Tracking entries needed (no violations to
justify).

## Project Structure

### Documentation (this feature)

```text
specs/005-fix-file-management/
├── plan.md              # This file
├── spec.md              # Feature spec (/speckit-specify output)
├── research.md          # Phase 0 — verified API findings
├── data-model.md        # Phase 1 — entities & state transitions
├── quickstart.md        # Phase 1 — validation scenarios
├── contracts/
│   ├── save-path-contract.md     # encoding-aware save invariants
│   └── ipc-dispatch-contract.md  # unified IPC dispatch invariants
└── tasks.md             # Phase 2 (/speckit-tasks — NOT created here)
```

### Source Code (repository root)

```text
src/
├── workspace.rs     # save path re-encode, snapshot recovery, session slimming, restore wiring
├── file_watcher.rs  # replace polling with fs::watch; notify-on-change + self-write suppression
├── main.rs          # init Fs global; replace IPC match with build_action dispatch
└── encoding.rs      # encode helper (encode + had_errors check) + unit test
```

**Structure Decision**: Single-project (existing layout). This fix modifies 4
existing files; it introduces no new modules, consistent with Constitution IV.
The four files are the natural homes for each concern: save/session logic in
`workspace.rs`, watching in `file_watcher.rs`, app init + IPC pump in `main.rs`,
encode primitive in `encoding.rs`.

## Complexity Tracking

> Not applicable — Constitution Check passes with no violations to justify.
