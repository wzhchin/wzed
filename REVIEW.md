# wzed 代码锐评

> 审查日期：2026-06-13 | 基于 main 分支 ca742ee | 4,319 行 / 12 个 Rust 源文件

4,300 行、12 个文件，做一个基于 Zed GPUI 的极简编辑器——定位清晰，野心克制。但看完全部代码，问题不少。

---

## 🔴 必须修的

### 1. Cargo.toml 声明了 GPL-3.0-or-later，但没有 LICENSE 文件

这是开源项目的基本功。License 不落地 = 法律上等于"保留所有权利"。别人不敢 fork，不敢用。

### 2. clippy.toml 禁了 `unwrap()`/`expect()`，代码里照样写

`main.rs` 有 `expect()`（L137, L402），`workspace.rs` 有 `unwrap()`（L1143），规则形同虚设。要么改代码，要么改规则，别假装有 lint。

### 3. `.ok()` 滥用——到处吞错误

`main.rs` 的 IPC（L253, L345, L384, L411）、`workspace.rs` 的文件操作（L105, L223, L495, L497）、`file_watcher.rs` 的元数据读取（L37, L38, L75, L76），全部 `.ok()` 一放了之。用户打开文件失败？没反应。IPC 断了？静默。这是编辑器，不是一次性脚本——每一个操作失败都应该有反馈。

### 4. `workspace.rs` 1171 行，上帝类

一个 struct 持有 tab、搜索、diff、文件监视、会话持久化、命令处理、UI 渲染。这不是"精简"，这是"没拆"。任何一个功能出 bug 都要在这 1171 行里大海捞针。

---

## 🟡 应该做的

### 5. 测试覆盖率 ~5%，核心 UI 模块零测试

33 个测试全在 `utils`/`encoding`/`recent_files` 三个辅助模块。`workspace.rs`、`search.rs`、`command_center.rs` 这些核心逻辑一行测试没有。修 bug 基本靠手动跑 `test-step.md`。

### 6. 无用户可见的错误通知

大量 `eprintln!()` 把错误扔到 stderr（`workspace.rs` L50, L85, L510, L544, L556, L578；`main.rs` L198, L215, L229, L241, L276）。GUI 应用里用户根本看不到 stderr。需要一个 toast/notification 机制把错误反馈到 UI。

### 7. 文件监视用 5 秒轮询

`file_watcher.rs` 用 `std::fs::metadata` 轮询。Zed 自己有 `notify` crate 的封装，可以直接用。5 秒延迟在编辑器里体感很差。

### 8. 大量硬编码魔法数字

轮询 5 秒、快照保留 7 天、最近文件 20 个、通知 4 秒、IPC buffer 8192 字节——全是散落在代码里的字面量。至少该统一到一个 config struct。

### 9. 无 CHANGELOG、无贡献指南、无架构文档

README 写了"what"但没写"how to contribute"。新贡献者看到 1171 行的 workspace.rs 会直接放弃。

---

## 🟢 做得不错的

- **定位准确**：说好不做 debug/terminal，就真的没做。克制力难得。
- **单实例 IPC 设计**：Unix socket + Windows TCP，有端口冲突处理，思路正确。
- **会话持久化**：`session.json` 保存/恢复 tab 状态，用户体验完整。
- **编码检测**：用 `chardetng` 做自动编码识别，对中文用户很实用。
- **编译通过，只有 1 个 dead_code 警告**：代码质量底线在。

---

## 一句话总结

> 代码能跑，但经不起推敲。作为个人工具够用，作为开源项目还差 LICENSE、测试、错误处理这三块板子。`workspace.rs` 是最大的技术债——现在 1171 行还能硬撑，加两个功能就到"改不动"的临界点了。

---

## 建议优先级

| 优先级 | 任务 | 预估工作量 |
|--------|------|-----------|
| P0 | 添加 LICENSE 文件 | 5 分钟 |
| P0 | 消灭 `.ok()` 静默吞错误 | 2-3 小时 |
| P1 | 消除 `unwrap()`/`expect()` 违规 | 1 小时 |
| P1 | 拆分 `workspace.rs` | 1-2 天 |
| P2 | 核心模块测试 | 持续 |
| P2 | 用户可见的错误通知机制 | 半天 |
| P3 | 文件监视改用 native notify | 半天 |
| P3 | 魔法数字提取为配置 | 1 小时 |
