mod ipc;
mod search;
mod app_theme;
mod workspace;

use std::path::PathBuf;
use std::sync::Arc;

use gpui::*;
use language::{LanguageRegistry, LoadedLanguage};
use settings::{KeymapFile, DEFAULT_KEYMAP_PATH};
use app_theme::WzedThemeSettings;
use workspace::LiteWorkspace;

use ipc::{listen_for_instances, try_send_to_existing_instance, OpenListener};

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
        /// Toggle the find bar.
        ToggleFind,
        /// Toggle the replace bar.
        ToggleReplace,
        /// Go to next search match.
        FindNext,
        /// Go to previous search match.
        FindPrevious,
        /// Replace current match.
        ReplaceNext,
        /// Replace all matches.
        ReplaceAll,
        /// Toggle regex mode.
        ToggleRegex,
        /// Search all open tabs.
        SearchAllTabs,
        /// Save all open tabs.
        SaveAll,
    ]
);

fn main() {
    let file_args: Vec<PathBuf> = std::env::args()
        .skip(1)
        .filter(|arg| !arg.starts_with('-'))
        .map(PathBuf::from)
        .collect();

    if !file_args.is_empty() && try_send_to_existing_instance(&file_args) {
        return;
    }

    let (ipc_sender, ipc_receiver) = std::sync::mpsc::channel::<Vec<PathBuf>>();

    let app =
        Application::with_platform(gpui_linux::current_platform(false)).with_assets(assets::Assets);

    app.run(move |cx: &mut App| {
        settings::init(cx);
        theme::init(theme::LoadThemes::JustBase, cx);
        theme::set_theme_settings_provider(Box::new(WzedThemeSettings::new()), cx);

        cx.bind_keys(
            KeymapFile::load_asset_allow_partial_failure(DEFAULT_KEYMAP_PATH, cx).unwrap(),
        );

        cx.bind_keys(vec![
            KeyBinding::new("ctrl-f", ToggleFind, Some("LiteWorkspace")),
            KeyBinding::new("ctrl-h", ToggleReplace, Some("LiteWorkspace")),
            KeyBinding::new("f3", FindNext, Some("LiteWorkspace")),
            KeyBinding::new("shift-f3", FindPrevious, Some("LiteWorkspace")),
            KeyBinding::new("escape", ToggleFind, Some("LiteWorkspace")),
            KeyBinding::new("alt-r", ToggleRegex, Some("LiteWorkspace")),
            KeyBinding::new("ctrl-shift-f", SearchAllTabs, Some("LiteWorkspace")),
            KeyBinding::new("ctrl-shift-s", SaveAll, Some("LiteWorkspace")),
        ]);

        let languages = Arc::new(LanguageRegistry::new(cx.background_executor().clone()));
        register_languages(&languages);

        cx.set_global(OpenListener::new(ipc_sender));
        if let Err(err) = listen_for_instances(cx.global::<OpenListener>().sender()) {
            eprintln!("IPC listener failed: {err:#}");
        }

        let shared_state = cx.global::<OpenListener>().shared();
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(200)).await;
                let Ok(paths) = ipc_receiver.try_recv() else {
                    continue;
                };
                let Some(handle) = shared_state.workspace_handle.lock().unwrap().clone() else {
                    continue;
                };
                handle.update(cx, |workspace, window, cx| {
                    for path in &paths {
                        if path.exists() {
                            workspace.open_file_path(path.clone(), window, cx).ok();
                        }
                    }
                }).ok();
            }
        }).detach();

        let file_args = file_args.clone();
        let window_handle = cx.open_window(
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
                    let mut workspace = LiteWorkspace::new(languages.clone(), window, cx);

                    workspace.restore_session(window, cx);

                    for path in &file_args {
                        if path.exists() {
                            workspace.open_file_path(path.clone(), window, cx).ok();
                        }
                    }
                    workspace.save_session(cx);
                    workspace
                });

                workspace
            },
        )
        .expect("failed to open window");

        cx.global::<OpenListener>().set_workspace(window_handle);

        let _quit_subscription = cx.on_app_quit(|cx| {
            let listener = cx.global::<OpenListener>();
            if let Some(handle) = listener.workspace_handle() {
                handle.read_with(cx, |workspace, cx| {
                    workspace::save_session_from_outside(workspace, cx);
                }).ok();
            }
            std::future::ready(())
        });
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
