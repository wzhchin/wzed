# WZed - 基于 Zed 的轻量级编辑器

基于 Zed 代码库的 `editor` crate，构建一个类似 Notepad++ 的轻量级文本编辑器。

## 核心思路

`Editor` 的 `project` 字段是 `Option<Entity<Project>>`，传入 `None` 时所有 LSP/协作/调试功能全部跳过。直接复用 `editor` crate 的完整编辑能力，只需自建一个轻量 tab 管理器替代 `workspace` crate。

## 依赖图

```
lite_editor (新 crate, ~660行)
├── gpui                    # UI框架 (Apache-2.0)
├── editor                  # 完整编辑器控件 (project=None)
├── multi_buffer            # Buffer管理
├── language                # 语言注册
├── grammars (load-grammars) # 内嵌 tree-sitter 语法
├── theme / theme_settings  # 主题
├── settings                # 设置系统
├── assets                  # 字体资源
└── fs                      # 文件系统抽象
```

**不需要**: `project`, `workspace`, `collab`, `client`, `agent`, `copilot`, `terminal`, `dap`, `git_ui`, `remote`, `extension_host`

## 目录结构

```
crates/lite_editor/
├── Cargo.toml
└── src/
    └── main.rs
```

## 初始化链

```rust
fn main() {
    application().run(|cx: &mut App| {
        settings::init(cx);
        theme_settings::init(theme::LoadThemes::JustBase, cx);

        let languages = Arc::new(LanguageRegistry::new(cx.executor()));
        languages.register_native_grammars(grammars::native_grammars());
        register_languages(&languages);

        cx.open_window(WindowOptions { ... }, |window, cx| {
            cx.new(|cx| LiteWorkspace::new(languages, window, cx))
        });
        cx.activate(true);
    });
}
```

跳过 `editor::init(cx)`，它内部调用 `workspace::register_project_item` 等需要 workspace 已初始化的方法。如需 `GlobalBlameRenderer`，手动设置：

```rust
cx.set_global(editor::GlobalBlameRenderer(Arc::new(())));
```

## 语法高亮注册

利用 `grammars` crate 的 `RustEmbed` 内嵌资源（config.toml + highlights.scm 等）：

```rust
fn register_languages(registry: &Arc<LanguageRegistry>) {
    let language_names = [
        "rust", "python", "typescript", "tsx", "javascript",
        "json", "jsonc", "c", "cpp", "go", "bash", "yaml",
        "css", "markdown", "diff",
    ];
    for name in language_names {
        let config = grammars::load_config(name);
        let queries = grammars::load_queries(name);
        registry.register_language(
            config.name.clone(),
            config.grammar.clone(),
            config.matcher.clone(),
            false,
            None,
            Arc::new(move || {
                let lang = Language::new(config.clone(), /* ts_language 从 grammars 获取 */);
                lang.with_queries(queries.clone())
            }),
        );
    }
}
```

## LiteWorkspace — Tab 管理器

不依赖 `workspace` crate，自建简易管理：

```rust
struct LiteWorkspace {
    tabs: Vec<Tab>,
    active: usize,
    languages: Arc<LanguageRegistry>,
    focus_handle: FocusHandle,
}

struct Tab {
    editor: Entity<Editor>,
    path: Option<PathBuf>,  // None = 未保存新文件
    title: SharedString,
}
```

Render 实现：顶部 tab 栏 + 下方活跃编辑器。

## 文件 I/O

```rust
fn open_file(
    path: PathBuf,
    languages: &Arc<LanguageRegistry>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<Editor> {
    let content = std::fs::read_to_string(&path).unwrap();
    let languages = languages.clone();
    let buffer = cx.new(|cx| {
        let mut buffer = Buffer::local(content, cx);
        buffer.set_language_registry(languages.clone());
        let lang = languages.language_for_file(path.as_path());
        cx.spawn(|buffer, cx| async move {
            let lang = lang.await?;
            buffer.update(cx, |buf, cx| buf.set_language(Some(lang), cx))?;
            anyhow::Ok(())
        }).detach_and_log_err(cx);
        buffer
    });
    let multibuffer = cx.new(|cx| MultiBuffer::singleton(buffer, cx));
    cx.new(|cx| Editor::new(EditorMode::full(), multibuffer, None, window, cx))
}
```

## 实施步骤

### Phase 1: MVP

| 步骤 | 内容 | 预估行数 |
|------|------|----------|
| 1 | 创建 crate，Cargo.toml 依赖配置 | ~30 |
| 2 | main.rs 初始化链（settings/theme/language） | ~80 |
| 3 | register_languages 语法注册 | ~80 |
| 4 | LiteWorkspace + Tab 结构体和 Render | ~300 |
| 5 | 文件打开/保存/新建 | ~100 |
| 6 | 状态栏（文件名、行列号） | ~50 |
| 7 | 键盘快捷键（Ctrl+O/S/N/W） | ~50 |

### Phase 2: 增强功能

- 搜索替换
- 最近打开记录
- 拖拽文件打开
- 编码选择（UTF-8/GBK）
- 简易文件树侧栏
- 自定义设置面板

### Phase 3: 编译优化（可选）

给 `editor` crate 加 feature flag，裁剪不需要的模块以减少编译时间和二进制体积：
- `items.rs`（workspace Item trait impl）
- `completions.rs`、`hover_popover.rs`、`hover_links.rs`（LSP 交互）
- `code_actions.rs`、`semantic_tokens.rs`、`signature_help.rs`（LSP）
- `runnables.rs`（LSP runnable）
- `git.rs`（git blame/diff gutter）

## 注意事项

1. **编译依赖**: `editor` 的 Cargo.toml 仍会拉入 `project`/`workspace`/`client` 等作为编译依赖，MVP 阶段可以接受
2. **editor::init**: 跳过它，内部 `workspace::register_project_item` 在无 workspace 时会 panic
3. **主题绑定**: `theme_settings::init` 将 `Theme` 绑定到 `LanguageRegistry`，语法高亮依赖此步骤
4. **许可证**: GPUI 等基础 crate 是 Apache-2.0；`editor`/`language` 等是 GPL-3.0-or-later，项目整体需遵循 GPL-3.0
