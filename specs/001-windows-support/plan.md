# Implementation Plan: Windows Support

**Branch**: `001-windows-support` | **Date**: 2026-06-13 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/001-windows-support/spec.md`

## Summary

Add Windows 10/11 build support to WZed. The core change is making the
platform layer compile-time conditional: `gpui_linux` on Linux, `gpui_windows`
on Windows. The IPC layer already has `#[cfg(windows)]` TCP implementation.
The remaining work is wiring the GPUI platform selector in `main.rs` and
adding a conditional Cargo dependency on `gpui_windows`.

## Technical Context

**Language/Version**: Rust edition 2024

**Primary Dependencies**: Zed GPUI framework (`gpui`, `gpui_linux`/`gpui_windows`,
`editor`, `language`, `theme`, etc.)

**Storage**: Filesystem — `dirs` crate returns `%APPDATA%/wzed/` on Windows

**Testing**: `cargo test` (35 unit tests); manual integration testing on Windows

**Target Platform**: Windows 10/11 (x86_64-pc-windows-msvc) + Linux (existing)

**Project Type**: Desktop application (single binary, single window)

**Performance Goals**: Same as Linux — 60 fps rendering, <100ms file open

**Constraints**: Must not break existing Linux build; no runtime platform detection

**Scale/Scope**: ~20 lines of code changes in 2 files (`main.rs`, `Cargo.toml`)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Minimalist Scope | ✅ PASS | Platform support is fundamental to text editing |
| II. Error Resilience | ✅ PASS | `WindowsPlatform::new()` returns `Result`, must propagate |
| III. No External Runtime Dependencies | ✅ PASS | `gpui_windows` is a build-time dependency from `../zed` |
| IV. Existing File Discipline | ✅ PASS | Changes in `main.rs` and `Cargo.toml` only |
| V. Zed Framework Delegation | ✅ PASS | Uses `gpui_windows` directly, no reimplementation |
| VI. Session and Data Safety | ✅ PASS | `dirs` crate already handles Windows paths |

No violations. No complexity tracking required.

## Project Structure

### Documentation (this feature)

```text
specs/001-windows-support/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── quickstart.md        # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
src/
├── main.rs              # Platform init (cfg conditional)
├── ipc.rs               # Already has #[cfg(windows)] IPC
├── workspace.rs         # No changes needed
├── tab.rs               # No changes needed
├── search.rs            # No changes needed
├── command_center.rs    # No changes needed
├── diff_view.rs         # No changes needed
├── topbar.rs            # No changes needed
├── app_theme.rs         # No changes needed
├── encoding.rs          # No changes needed
├── file_watcher.rs      # No changes needed (uses std::fs)
├── recent_files.rs      # No changes needed
└── utils.rs             # No changes needed (dirs crate handles paths)

Cargo.toml               # Conditional gpui_windows dependency
dist/                    # Linux-only, no changes needed
```

**Structure Decision**: Existing single-project structure. Only `main.rs` and
`Cargo.toml` need changes. All other modules are platform-agnostic.

## Complexity Tracking

> No violations — section intentionally empty.
