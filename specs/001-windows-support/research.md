# Research: Windows Support

**Feature**: Windows Support (`001-windows-support`)
**Date**: 2026-06-13

## R1: GPUI Platform Selection Interface

**Decision**: Use `cfg`-guarded platform initialization with a shared wrapper.

**Rationale**: `gpui_linux` exposes `current_platform(headless: bool) -> Rc<dyn Platform>`.
`gpui_windows` exposes `WindowsPlatform::new(headless: bool) -> Result<WindowsPlatform>`
where `WindowsPlatform: Platform`. The API differs — Linux returns `Rc<dyn Platform>`
directly, Windows returns a concrete `Result`. A thin wrapper normalizes this:

```rust
// Linux
Application::with_platform(gpui_linux::current_platform(false))

// Windows
let platform = gpui_windows::WindowsPlatform::new(false)
    .expect("failed to initialize Windows platform");
Application::with_platform(Rc::new(platform))
```

**Alternatives considered**:
- A trait-based `current_platform()` function in `gpui` itself — rejected because
  it doesn't exist in the Zed API and we shouldn't modify Zed crates.
- A build.rs script that generates a platform module — rejected as over-engineering
  for a two-branch cfg.

## R2: Conditional Cargo Dependencies

**Decision**: Use `[target.'cfg(unix)'.dependencies]` and
`[target.'cfg(windows)'.dependencies]` sections in `Cargo.toml`.

**Rationale**: Cargo supports target-specific dependency tables. This avoids
feature flags and keeps the platform split explicit:

```toml
[target.'cfg(unix)'.dependencies]
gpui_linux = { path = "../zed/crates/gpui_linux" }

[target.'cfg(windows)'.dependencies]
gpui_windows = { path = "../zed/crates/gpui_windows" }
```

**Alternatives considered**:
- A single dependency with a feature flag (`gpui_linux`/`gpui_windows` features) —
  rejected because these are different crates with different APIs.
- A workspace-level `[patch]` — rejected; not applicable to different crate names.

## R3: IPC Port Lock Stale Detection

**Decision**: Attempt TCP connection to detect stale locks. If connection fails,
remove the port file and proceed.

**Rationale**: The current `#[cfg(windows)]` IPC code writes a port number to
`wzed.port` but has no stale-lock cleanup (unlike the Unix path which detects
`ConnectionRefused`). On startup, if the port file exists, try connecting. If it
fails, the previous instance is gone — remove the file and start fresh.

**Alternatives considered**:
- Windows named mutex for single-instance detection — more idiomatic but adds
  complexity; TCP approach is already implemented and works.
- File locking (`LockFileEx`) — platform-specific, no advantage over TCP check.

## R4: Configuration Path Handling

**Decision**: No changes needed — the `dirs` crate already returns correct paths
on both platforms.

**Rationale**: `dirs::config_dir()` returns `~/.config` on Linux and
`%APPDATA%` on Windows. `dirs::data_dir()` returns `~/.local/share` on Linux
and `%APPDATA%` on Windows. All path construction in `utils.rs` uses `dirs`
consistently.

**Alternatives considered**: None — already correct.

## R5: File Watcher Cross-Platform Compatibility

**Decision**: No changes needed — `std::fs::metadata` and `std::fs::read`
work on both platforms.

**Rationale**: The polling-based file watcher uses only `std::fs` APIs which are
cross-platform. Performance characteristics are identical (5-second polling
interval).

**Alternatives considered**: None — already cross-platform.
