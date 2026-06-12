# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
cargo build                    # debug build
cargo run -- file1.rs file2.rs # open files on launch
cargo run -- -c "new-file"     # send IPC command to running instance
```

No Rust tests. Manual integration testing via `test-step.md`.

Unit tests exist in `utils.rs` (23), `encoding.rs` (6), `recent_files.rs` (4) — run with `cargo test`. Core UI modules are untested.

## Architecture

基于 Zed GPUI 框架的**最精简**文本编辑器，依赖 `../zed` 中的核心 crate（editor, gpui, language, theme 等）。单 binary、单窗口、~3700 行、12 个源文件。不需要任何 debug 功能（断点、终端、调试器等），只做纯文本编辑。

- **`main.rs`** — 入口：CLI 解析、IPC 单实例、GPUI 初始化、语言注册、键绑定
- **`workspace.rs`** — 核心状态 `LiteWorkspace`，持有所有 tab/搜索/文件监视器，实现 `Render` 绘制完整 UI，会话持久化到 `~/.config/wzed/session.json`
- **`tab.rs`** — Tab 模型 + 侧栏渲染（图标、拖拽、右键菜单、分组）
- **`command_center.rs`** — M-x 命令面板，自动发现 `lite_editor::*` action
- **`search.rs`** — 查找/替换，支持正则和多 tab 搜索
- **`diff_view.rs`** — 基于 `similar` 的并排 diff
- **`ipc.rs`** — 单实例 IPC（Unix socket / Windows TCP）
- **`file_watcher.rs`** / **`encoding.rs`** / **`recent_files.rs`** / **`app_theme.rs`** / **`topbar.rs`** — 文件监视、编码、最近文件、主题、工具栏

## Coding Guidelines

- 在已有文件中实现功能，不要创建很多小文件
- 不用 `unwrap()`，用 `?` 或 `.log_err()`
- 不用 `let _ =` 静默丢弃错误
- 不用 `mod.rs`
- 注释只解释"为什么"，不描述代码本身
- 变量名用完整单词
- Entity 闭包内用内部 `cx`，不用外部的
