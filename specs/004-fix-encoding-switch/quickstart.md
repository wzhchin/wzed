# Quickstart: Fix Encoding Switch Data Loss

**Date**: 2026-06-13 | **Phase**: 1

## Prerequisites

- Build the project: `cargo build`
- A test file in a non-UTF-8 encoding (e.g., GBK). Create one with:
  ```bash
  echo "你好世界" | iconv -f UTF-8 -t GBK > /tmp/test_gbk.txt
  ```

## Scenario 1: Safe Encoding Reload (P1)

**Validates**: FR-001, FR-002, FR-003, FR-004

1. Open wzed with the GBK test file: `cargo run -- /tmp/test_gbk.txt`
2. Text appears garbled (GBK bytes misinterpreted as UTF-8)
3. Press `Ctrl+Shift+E` → command center opens with encoding submenu
4. Select "GBK" from the list
5. **Expected**: Text displays correctly as "你好世界"
6. Make an edit (type some text)
7. Press `Ctrl+Shift+E` again, select "UTF-8"
8. **Expected**: Notification appears saying unsaved changes must be saved first; buffer unchanged
9. Save the file (`Ctrl+S`), then switch encoding again
10. **Expected**: File reloads with UTF-8 (garbled again since content is GBK on disk)

**Error recovery test**:
1. Create an empty file: `touch /tmp/empty.txt`
2. Open it in wzed
3. Switch to "Shift_JIS" encoding
4. **Expected**: Either loads successfully or shows error notification; buffer never cleared

## Scenario 2: Encoding Picker (P2)

**Validates**: FR-005

1. Open any file in wzed
2. Press `Ctrl+Shift+E` → command center opens showing all 15 encodings
3. **Expected**: Current encoding is visually highlighted in the list
4. Press Escape → picker closes, no change
5. Press `Ctrl+Shift+E` again, type "gb" to filter
6. **Expected**: List filters to show "GBK" and "GB18030"
7. Select one → file reloads with that encoding

## Scenario 3: Encoding Persistence (P3)

**Validates**: FR-006, FR-007

1. Open a file and switch to GBK encoding (per Scenario 1)
2. Close the tab (`Ctrl+W`)
3. Reopen the same file
4. **Expected**: File opens with GBK encoding (not auto-detected or UTF-8)
5. Close wzed entirely
6. Restart wzed (session should restore)
7. **Expected**: File tab restores with GBK encoding and content displays correctly

## Scenario 4: Status Bar Display

**Validates**: FR-008

1. Open a file with any encoding
2. Look at the status bar at the bottom
3. **Expected**: Current encoding label is visible (already works — verify it updates after switch)
