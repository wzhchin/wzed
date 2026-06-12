# Quickstart: Windows Support

**Feature**: Windows Support (`001-windows-support`)
**Date**: 2026-06-13

## Prerequisites

- Windows 10 or 11
- Rust toolchain installed (`rustup`, MSVC target: `x86_64-pc-windows-msvc`)
- Zed source checkout at `../zed` (same level as the wzed repo)
- Visual Studio Build Tools (for MSVC linker)

## Validation Scenarios

### V1: Build on Windows

```bash
cargo build
```

**Expected**: Build completes with zero errors from WZed code. WZed-specific
warnings are zero. (Zed crate warnings are out of scope.)

### V2: Launch and Basic Editing

```bash
cargo run -- test-file.rs
```

**Expected**: Editor window opens with:
- Dark theme applied
- Toolbar visible at top (New, Open, Save, Find, Replace, Compare buttons)
- `test-file.rs` open in a tab
- Syntax highlighting active (Rust)

**Actions to verify**:
1. Type text — characters appear at cursor
2. `Ctrl+S` — file saves (check file on disk)
3. `Ctrl+N` — new empty tab opens
4. `Ctrl+W` — tab closes

### V3: Single-Instance IPC

Terminal 1:
```bash
cargo run
```

Terminal 2:
```bash
cargo run -- another-file.rs
```

**Expected**: `another-file.rs` opens as a new tab in the running instance.
No second window appears.

Terminal 3 (command):
```bash
cargo run -- -c "new-file"
```

**Expected**: New empty tab appears in the running instance.

### V4: Session Persistence

1. Open WZed, open 3 files in separate tabs
2. Edit content in each tab
3. Close the editor window
4. Run `cargo run` again

**Expected**: All 3 tabs reopen with their content restored.

### V5: Configuration

Create `%APPDATA%/wzed/settings.json`:
```json
{
  "font_family": "Consolas",
  "font_size": 14
}
```

Launch WZed — **Expected**: Editor uses Consolas 14pt.

### V6: Unit Tests

```bash
cargo test
```

**Expected**: All 35 tests pass (same as on Linux).

## Cross-Platform Regression Check

After all Windows validations pass, verify Linux build is unaffected:

```bash
# On Linux
cargo build
cargo test
cargo run -- test-file.rs
```

**Expected**: Identical behavior to before the Windows support changes.
