use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context as _, Result, bail};
use editor::{Editor, EditorMode, MultiBuffer};
use gpui::*;
use language::{Buffer, LanguageRegistry, LoadedLanguage};
use settings::{KeymapFile, KeybindSource, DEFAULT_KEYMAP_PATH};
use theme::{LoadThemes, ThemeSettingsProvider, UiDensity};

fn main() {
    let file_args: Vec<PathBuf> = std::env::args()
        .skip(1)
        .filter(|arg| !arg.starts_with('-'))
        .map(PathBuf::from)
        .collect();

    let app =
        Application::with_platform(gpui_linux::current_platform(false)).with_assets(assets::Assets);

    app.run(move |cx: &mut App| {
        settings::init(cx);
        theme::init(LoadThemes::JustBase, cx);
        theme::set_theme_settings_provider(Box::new(WzedThemeSettings::new()), cx);

        cx.bind_keys(
            KeymapFile::load_asset_allow_partial_failure(DEFAULT_KEYMAP_PATH, cx).unwrap(),
        );

        let languages = Arc::new(LanguageRegistry::new(cx.background_executor().clone()));
        register_languages(&languages);

        let file_args = file_args.clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1200.0), px(800.0)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("WZed".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                focus: true,
                show: true,
                kind: WindowKind::Normal,
                is_movable: true,
                is_resizable: true,
                is_minimizable: true,
                display_id: None,
                window_background: WindowBackgroundAppearance::default(),
                app_id: Some("dev.wzed.editor".to_string()),
                window_min_size: Some(size(px(400.0), px(300.0))),
                window_decorations: Some(WindowDecorations::Client),
                icon: None,
                tabbing_identifier: None,
            },
            move |window, cx| {
                let workspace = cx.new(|cx| {
                    let mut workspace = LiteWorkspace::new(languages, window, cx);
                    for path in &file_args {
                        if path.exists() {
                            workspace.open_file_path(path.clone(), window, cx).ok();
                        }
                    }
                    workspace
                });
                workspace
            },
        )
        .expect("failed to open window");
    });
}

fn register_languages(languages: &Arc<LanguageRegistry>) {
    languages.register_native_grammars(grammars::native_grammars());

    let language_names = [
        "bash",
        "c",
        "cpp",
        "css",
        "diff",
        "go",
        "json",
        "jsonc",
        "markdown",
        "python",
        "regex",
        "rust",
        "tsx",
        "typescript",
        "yaml",
    ];

    for name in language_names {
        let config = grammars::load_config_for_feature(name, true);
        let grammar_name = config.grammar.clone();
        let matcher = config.matcher.clone();
        let hidden = config.hidden;
        let lang_name = config.name.clone();
        let name_static = name.to_owned();

        languages.register_language(
            lang_name,
            grammar_name,
            matcher,
            hidden,
            None,
            Arc::new(move || {
                Ok(LoadedLanguage {
                    config: grammars::load_config_for_feature(&name_static, true),
                    queries: grammars::load_queries(&name_static),
                    context_provider: None,
                    toolchain_provider: None,
                    manifest_name: None,
                })
            }),
        );
    }
}

struct Tab {
    editor: Entity<Editor>,
    path: Option<PathBuf>,
    title: SharedString,
}

struct LiteWorkspace {
    tabs: Vec<Tab>,
    active: usize,
    languages: Arc<LanguageRegistry>,
    focus_handle: FocusHandle,
}

impl LiteWorkspace {
    fn new(
        languages: Arc<LanguageRegistry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let editor = Self::create_empty_editor(window, cx);

        Self {
            tabs: vec![Tab {
                editor,
                path: None,
                title: "untitled".into(),
            }],
            active: 0,
            languages,
            focus_handle,
        }
    }

    fn create_empty_editor(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Editor> {
        let buffer = cx.new(|cx| Buffer::local("", cx));
        let multibuffer = cx.new(|cx| MultiBuffer::singleton(buffer, cx));
        cx.new(|cx| Editor::new(EditorMode::full(), multibuffer, None, window, cx))
    }

    fn open_file_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read file: {}", path.display()))?;
        let title: SharedString = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned().into())
            .unwrap_or("untitled".into());

        let languages = self.languages.clone();
        let path_for_lang = path.clone();
        let buffer = cx.new(|cx| {
            let buffer = Buffer::local(content, cx);
            buffer.set_language_registry(languages.clone());

            let available = languages.language_for_file_path(&path_for_lang);
            if let Some(available) = available {
                cx.spawn(async move |buffer: WeakEntity<Buffer>, cx| {
                    let lang = languages.load_language(&available).await??;
                    buffer.update(cx, |buf, cx| buf.set_language(Some(lang), cx))?;
                    Result::<()>::Ok(())
                })
                .detach_and_log_err(cx);
            }
            buffer
        });

        let multibuffer = cx.new(|cx| MultiBuffer::singleton(buffer, cx));
        let editor = cx.new(|cx| Editor::new(EditorMode::full(), multibuffer, None, window, cx));

        self.tabs.push(Tab {
            editor,
            path: Some(path),
            title,
        });
        self.active = self.tabs.len() - 1;
        cx.notify();
        Ok(())
    }

    fn save_active_tab(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let tab = &self.tabs[self.active];
        let path = match &tab.path {
            Some(p) => p.clone(),
            None => bail!("no file path for this tab, save-as not yet implemented"),
        };

        let content = tab.editor.read(cx).text(cx);
        std::fs::write(&path, &content)
            .with_context(|| format!("failed to write file: {}", path.display()))?;
        Ok(())
    }

    fn handle_open(
        &mut self,
        _action: &OpenFile,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        eprintln!("open file: file dialog not yet implemented, pass files as CLI args");
    }

    fn handle_save(
        &mut self,
        _action: &SaveFile,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(err) = self.save_active_tab(cx) {
            eprintln!("failed to save: {err:#}");
        }
    }

    fn handle_new(
        &mut self,
        _action: &NewFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editor = Self::create_empty_editor(window, cx);
        self.tabs.push(Tab {
            editor,
            path: None,
            title: "untitled".into(),
        });
        self.active = self.tabs.len() - 1;
        cx.notify();
    }

    fn handle_close_tab(
        &mut self,
        _action: &CloseTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.remove(self.active);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
        cx.notify();
    }
}

impl Render for LiteWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = &self.tabs[self.active];

        let side_tabs = div()
            .flex()
            .flex_col()
            .w(px(180.0))
            .h_full()
            .bg(gpui::hsla(0.0, 0.0, 0.1, 1.0))
            .border_r_1()
            .border_color(gpui::hsla(0.0, 0.0, 0.15, 1.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .children(self.tabs.iter().enumerate().map(|(index, tab)| {
                        let is_active = index == self.active;
                        let title = tab.title.clone();
                        let mut tab_el = div()
                            .id(ElementId::Name(format!("tab-{index}").into()))
                            .flex()
                            .items_center()
                            .px(px(10.0))
                            .py(px(6.0))
                            .w_full()
                            .cursor_pointer()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(if is_active {
                                        gpui::hsla(0.0, 0.0, 0.9, 1.0)
                                    } else {
                                        gpui::hsla(0.0, 0.0, 0.6, 1.0)
                                    })
                                    .text_ellipsis()
                                    .child(title),
                            )
                            .on_click(cx.listener(move |workspace, _, _window, cx| {
                                workspace.active = index;
                                cx.notify();
                            }));

                        if is_active {
                            tab_el = tab_el
                                .bg(gpui::hsla(0.0, 0.0, 0.18, 1.0))
                                .border_l_2()
                                .border_color(gpui::hsla(220.0, 0.8, 0.6, 1.0));
                        } else {
                            tab_el = tab_el.hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.13, 1.0)));
                        }
                        tab_el
                    })),
            )
            .child(
                div()
                    .id("new-tab-btn")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .h(px(32.0))
                    .cursor_pointer()
                    .border_t_1()
                    .border_color(gpui::hsla(0.0, 0.0, 0.15, 1.0))
                    .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.15, 1.0)))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .text_color(gpui::hsla(0.0, 0.0, 0.6, 1.0))
                            .child("+"),
                    )
                    .on_click(cx.listener(|workspace, _, window, cx| {
                        workspace.handle_new(&NewFile, window, cx);
                    })),
            );

        let status_bar = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .h(px(24.0))
            .px(px(12.0))
            .bg(gpui::hsla(0.0, 0.0, 0.08, 1.0))
            .border_t_1()
            .border_color(gpui::hsla(0.0, 0.0, 0.15, 1.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(gpui::hsla(0.0, 0.0, 0.6, 1.0))
                    .child(active_tab.title.clone()),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(gpui::hsla(0.0, 0.0, 0.5, 1.0))
                    .child(format!(
                        "Tab {} of {}",
                        self.active + 1,
                        self.tabs.len()
                    )),
            );

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(gpui::hsla(0.0, 0.0, 0.1, 1.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .overflow_hidden()
                    .child(side_tabs)
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .child(active_tab.editor.clone()),
                    ),
            )
            .child(status_bar)
            .key_context("LiteWorkspace")
            .on_action(cx.listener(Self::handle_open))
            .on_action(cx.listener(Self::handle_save))
            .on_action(cx.listener(Self::handle_new))
            .on_action(cx.listener(Self::handle_close_tab))
    }
}

struct WzedThemeSettings {
    ui_font: Font,
    buffer_font: Font,
}

impl WzedThemeSettings {
    fn new() -> Self {
        Self {
            ui_font: Font {
                family: "Helvetica".into(),
                weight: gpui::FontWeight::NORMAL,
                style: gpui::FontStyle::Normal,
                features: FontFeatures::default(),
                fallbacks: None,
            },
            buffer_font: Font {
                family: "Monospace".into(),
                weight: gpui::FontWeight::NORMAL,
                style: gpui::FontStyle::Normal,
                features: FontFeatures::default(),
                fallbacks: None,
            },
        }
    }
}

impl ThemeSettingsProvider for WzedThemeSettings {
    fn ui_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        &self.ui_font
    }

    fn buffer_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        &self.buffer_font
    }

    fn ui_font_size(&self, _cx: &App) -> Pixels {
        px(14.0)
    }

    fn buffer_font_size(&self, _cx: &App) -> Pixels {
        px(14.0)
    }

    fn ui_density(&self, _cx: &App) -> UiDensity {
        UiDensity::Default
    }
}

actions!(
    lite_editor,
    [
        /// Open a file.
        OpenFile,
        /// Save the current file.
        SaveFile,
        /// Create a new file.
        NewFile,
        /// Close the current tab.
        CloseTab,
    ]
);
