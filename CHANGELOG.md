# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-06-19

### Added

- Windows platform support with conditional GPUI backend
- Arch Linux PKGBUILD (Wayland-only, builds from local source, no remote download)
- Top toolbar visibility now persists across sessions

### Changed

- Shrink release binary ~47% (147 MiB → 78 MiB) via LTO, strip, and panic=abort
- Drop the X11 backend entirely — the binary links only libxkbcommon, no libxcb

### Fixed

- Code quality issues and missing project infrastructure
- 16 improvements from codebase review

## [0.1.0] - 2026-06-12

### Added

- Text editor built on Zed GPUI framework
- Tab management with sidebar, icons, drag-and-drop, pinning, context menu
- Find/replace with regex support (Ctrl+F, Ctrl+H, Alt+R)
- Search across all open tabs (Ctrl+Shift+F)
- File open dialog and save-as support
- Save all tabs (Ctrl+Shift+S)
- Autosave and snapshot backup
- File watcher for external changes
- Recent files list in toolbar dropdown
- Encoding detection and conversion (Ctrl+Shift+E)
- Side-by-side file comparison view
- Command center with dynamic action discovery (Alt+X)
- Single-instance IPC (Unix socket / Windows TCP)
- Session persistence (tabs restored on reopen)
- Toolbar with common action buttons
- Tab groups with visual separators (Ctrl+G)
- Linux desktop file for file type association
- Emacs-like command architecture with dynamic discovery
- Soft wrap enabled by default
- Unit tests for utils, encoding, and recent_files modules
