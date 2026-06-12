# Quickstart Validation: Code Quality Fixes

**Date**: 2026-06-13

## Prerequisites

- Rust toolchain installed
- `../zed` directory with Zed source checkout
- Current branch with code quality fixes applied

## Validation Scenarios

### V1: Clippy compliance (FR-004)

```bash
cargo clippy 2>&1 | grep -i "disallowed"
# Expected: zero matches (no output)
```

### V2: No `.ok()` in non-test code (FR-003)

```bash
grep -n '\.ok()' src/main.rs src/workspace.rs src/command_center.rs src/file_watcher.rs src/recent_files.rs src/search.rs src/topbar.rs
# Expected: zero matches
```

### V3: No `unwrap()`/`expect()` in non-test code (FR-004)

```bash
grep -n 'unwrap()\|expect(' src/main.rs src/workspace.rs src/command_center.rs src/file_watcher.rs src/recent_files.rs src/search.rs src/topbar.rs src/encoding.rs src/utils.rs src/diff_view.rs src/ipc.rs src/app_theme.rs
# Expected: zero matches (test code excluded by line range)
```

### V4: Workspace under 800 lines (FR-007)

```bash
wc -l src/workspace.rs
# Expected: < 800
```

### V5: All tests pass (SC-004)

```bash
cargo test
# Expected: 33 tests pass, 0 fail
```

### V6: LICENSE file present (FR-005)

```bash
head -1 LICENSE
# Expected: "GNU GENERAL PUBLIC LICENSE"
```

### V7: CHANGELOG present (FR-006)

```bash
head -1 CHANGELOG.md
# Expected: "# Changelog"
```

### V8: Centralized config constants (FR-008)

```bash
grep -n 'pub struct AppConfig' src/utils.rs
# Expected: match found
grep -rn 'from_secs(4)\|from_secs(30)\|from_secs(5)\|7 \* 24' src/
# Expected: zero matches (all moved to AppConfig)
```

### V9: Build succeeds

```bash
cargo build
# Expected: compiles without errors
```

## Full validation run

```bash
cargo build && cargo test && cargo clippy && \
  test $(wc -l < src/workspace.rs) -lt 800 && \
  test -f LICENSE && test -f CHANGELOG.md && \
  echo "ALL CHECKS PASSED"
```
