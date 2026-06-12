mod command_center;
mod ipc;
mod search;
mod app_theme;
mod diff_view;
mod encoding;
mod file_watcher;
mod recent_files;
mod tab;
mod topbar;
mod utils;
mod workspace;

use std::path::PathBuf;
#[cfg(windows)]
use std::rc::Rc;
use std::sync::Arc;

use gpui::*;
use util::ResultExt;
use theme::ActiveTheme;
use language::{LanguageRegistry, LoadedLanguage};
use editor::EditorSettings;
use settings::{KeymapFile, KeymapFileLoadResult, Settings, DEFAULT_KEYMAP_PATH};
use app_theme::WzedThemeSettings;
use workspace::LiteWorkspace;

use ipc::{listen_for_instances, try_send_to_existing_instance, try_send_command_to_existing_instance, IpcMessage, OpenListener};

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
        /// Autosave timer fired.
        AutosaveTimer,
        /// Toggle toolbar visibility.
        ToggleToolbar,
        /// Move current tab to a new group.
        MoveToGroup,
        /// Reload file with a different encoding.
        ReloadWithEncoding,
        /// Compare current file with another file.
        CompareFiles,
        /// Toggle the command center.
        ToggleCommandCenter,
        /// Dismiss search bar or diff view.
        Dismiss,
    ]
);

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut file_args: Vec<PathBuf> = Vec::new();
    let mut command_arg: Option<String> = None;
    let mut list_commands = false;

    let mut iter = args.iter().skip(1).peekable();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--command" | "-c" => {
                command_arg = iter.next().cloned();
            }
            "--list-commands" => {
                list_commands = true;
            }
            s if !s.starts_with('-') => {
                file_args.push(PathBuf::from(s));
            }
            _ => {
                eprintln!("unknown argument: {arg}");
            }
        }
    }

    if list_commands {
        list_all_commands();
        return;
    }

    if let Some(ref command) = command_arg {
        let qualified = if command.starts_with("set-text:")
            || command.starts_with("save-as:")
            || command.starts_with("switch-tab:")
        {
            command.clone()
        } else {
            friendly_name_to_qualified(command)
        };
        if try_send_command_to_existing_instance(&qualified) {
            return;
        }
        eprintln!("no running wzed instance found");
        std::process::exit(1);
    }

    if !file_args.is_empty() && try_send_to_existing_instance(&file_args) {
        return;
    }

    let (ipc_sender, ipc_receiver) = std::sync::mpsc::channel::<IpcMessage>();
    let ipc_receiver = std::sync::Arc::new(std::sync::Mutex::new(ipc_receiver));

    let platform = {
        #[cfg(unix)]
        {
            gpui_linux::current_platform(false)
        }
        #[cfg(windows)]
        {
            Rc::new(
                gpui_windows::WindowsPlatform::new(false)
                    .expect("failed to initialize Windows platform"),
            ) as Rc<dyn gpui::Platform>
        }
    };
    let app = Application::with_platform(platform).with_assets(assets::Assets);

    app.run(move |cx: &mut App| {
        settings::init(cx);
        {
            let mut editor_settings = EditorSettings::get_global(cx).clone();
            editor_settings.gutter.runnables = false;
            editor_settings.gutter.folds = false;
            EditorSettings::override_global(editor_settings, cx);
        }
        theme::init(theme::LoadThemes::JustBase, cx);
        theme::set_theme_settings_provider(Box::new(WzedThemeSettings::new()), cx);

        cx.bind_keys(
            KeymapFile::load_asset_allow_partial_failure(DEFAULT_KEYMAP_PATH, cx)
                .log_err()
                .unwrap_or_default(),
        );

        cx.bind_keys(vec![
            // Workspace actions
            KeyBinding::new("ctrl-n", NewFile, Some("LiteWorkspace")),
            KeyBinding::new("ctrl-o", OpenFile, Some("LiteWorkspace")),
            KeyBinding::new("ctrl-s", SaveFile, Some("LiteWorkspace")),
            KeyBinding::new("ctrl-w", CloseTab, Some("LiteWorkspace")),
            KeyBinding::new("ctrl-f", ToggleFind, Some("LiteWorkspace")),
            KeyBinding::new("ctrl-h", ToggleReplace, Some("LiteWorkspace")),
            KeyBinding::new("f3", FindNext, Some("LiteWorkspace")),
            KeyBinding::new("shift-f3", FindPrevious, Some("LiteWorkspace")),
            KeyBinding::new("escape", Dismiss, Some("LiteWorkspace")),
            KeyBinding::new("alt-r", ToggleRegex, Some("LiteWorkspace")),
            KeyBinding::new("ctrl-shift-f", SearchAllTabs, Some("LiteWorkspace")),
            KeyBinding::new("ctrl-shift-s", SaveAll, Some("LiteWorkspace")),
            KeyBinding::new("ctrl-g", MoveToGroup, Some("LiteWorkspace")),
            KeyBinding::new("ctrl-alt-d", CompareFiles, Some("LiteWorkspace")),
            KeyBinding::new("ctrl-shift-e", ReloadWithEncoding, Some("LiteWorkspace")),
            KeyBinding::new("alt-x", ToggleCommandCenter, Some("LiteWorkspace")),
            // Editor actions — these use Zed's built-in editor actions
            KeyBinding::new("ctrl-d", editor::actions::SelectNext::default(), Some("Editor")),
            KeyBinding::new("ctrl-shift-d", editor::actions::DuplicateLineDown, Some("Editor")),
            KeyBinding::new("ctrl-shift-k", editor::actions::DeleteLine, Some("Editor")),
            KeyBinding::new("alt-up", editor::actions::MoveLineUp, Some("Editor")),
            KeyBinding::new("alt-down", editor::actions::MoveLineDown, Some("Editor")),
            KeyBinding::new("ctrl-/", editor::actions::ToggleComments::default(), Some("Editor")),
        ]);

        let user_keymap_path = utils::config_dir().join("keymap.json");
        if let Ok(content) = std::fs::read_to_string(&user_keymap_path) {
            match KeymapFile::load(&content, cx) {
                KeymapFileLoadResult::Success { key_bindings } => {
                    cx.bind_keys(key_bindings);
                }
                KeymapFileLoadResult::SomeFailedToLoad { key_bindings, error_message } => {
                    eprintln!("user keymap partially loaded: {error_message}");
                    cx.bind_keys(key_bindings);
                }
                KeymapFileLoadResult::JsonParseFailure { error } => {
                    eprintln!("user keymap parse error: {error:#}");
                }
            }
        }

        let languages = Arc::new(LanguageRegistry::new(cx.background_executor().clone()));
        register_languages(&languages);
        languages.set_theme(cx.theme().clone());
        cx.observe_global::<theme::GlobalTheme>({
            let languages = languages.clone();
            move |cx| {
                languages.set_theme(cx.theme().clone());
            }
        })
        .detach();

        cx.set_global(OpenListener::new(ipc_sender));
        if let Err(err) = listen_for_instances(cx.global::<OpenListener>().sender()) {
            eprintln!("IPC listener failed: {err:#}");
        }

        let shared_state = cx.global::<OpenListener>().shared();
        cx.spawn(async move |cx| {
            loop {
                let receiver = ipc_receiver.clone();
                let message = cx
                    .background_executor()
                    .spawn(async move {
                        match receiver.lock() {
                            Ok(guard) => guard.recv(),
                            Err(err) => {
                                eprintln!("IPC lock poisoned: {err}");
                                Err(std::sync::mpsc::RecvError)
                            }
                        }
                    })
                    .await;
                let Ok(message) = message else {
                    break;
                };
                let handle = match shared_state.workspace_handle.lock() {
                    Ok(guard) => *guard,
                    Err(err) => {
                        eprintln!("IPC lock poisoned: {err}");
                        continue;
                    }
                };
                let Some(handle) = handle else {
                    continue;
                };
                handle.update(cx, |_workspace, window, cx| {
                    match message {
                        IpcMessage::OpenFiles(paths) => {
                            for path in &paths {
                                if path.exists() {
                                    _workspace.open_file_path(path.clone(), window, cx).ok();
                                }
                            }
                        }
                        IpcMessage::ExecuteCommand(command) => {
                            match command.as_str() {
                                "lite_editor::NewFile" => {
                                    _workspace.handle_new(&NewFile, window, cx);
                                }
                                "lite_editor::OpenFile" => {
                                    _workspace.handle_open(&OpenFile, window, cx);
                                }
                                "lite_editor::SaveFile" => {
                                    _workspace.handle_save(&SaveFile, window, cx);
                                }
                                "lite_editor::SaveAll" => {
                                    _workspace.handle_save_all(&SaveAll, window, cx);
                                }
                                "lite_editor::CloseTab" => {
                                    _workspace.handle_close_tab(&CloseTab, window, cx);
                                }
                                "lite_editor::ToggleFind" => {
                                    _workspace.handle_toggle_find(&ToggleFind, window, cx);
                                }
                                "lite_editor::Dismiss" => {
                                    _workspace.handle_dismiss(&Dismiss, window, cx);
                                }
                                "lite_editor::FindNext" => {
                                    _workspace.handle_find_next(&FindNext, window, cx);
                                }
                                "lite_editor::FindPrevious" => {
                                    _workspace.handle_find_previous(&FindPrevious, window, cx);
                                }
                                "lite_editor::ToggleReplace" => {
                                    _workspace.handle_toggle_replace(&ToggleReplace, window, cx);
                                }
                                "lite_editor::ReplaceNext" => {
                                    _workspace.handle_replace_next(&ReplaceNext, window, cx);
                                }
                                "lite_editor::ReplaceAll" => {
                                    _workspace.handle_replace_all(&ReplaceAll, window, cx);
                                }
                                "lite_editor::ToggleRegex" => {
                                    _workspace.handle_toggle_regex(&ToggleRegex, window, cx);
                                }
                                "lite_editor::ToggleToolbar" => {
                                    _workspace.handle_toggle_toolbar(&ToggleToolbar, window, cx);
                                }
                                "lite_editor::MoveToGroup" => {
                                    _workspace.handle_move_to_group(&MoveToGroup, window, cx);
                                }
                                "lite_editor::ToggleCommandCenter" => {
                                    _workspace.handle_toggle_command_center(
                                        &ToggleCommandCenter, window, cx,
                                    );
                                }
                                "lite_editor::ReloadWithEncoding" => {
                                    _workspace.handle_reload_encoding(
                                        &ReloadWithEncoding, window, cx,
                                    );
                                }
                                "lite_editor::CompareFiles" => {
                                    _workspace.handle_compare_files(&CompareFiles, window, cx);
                                }
                                "lite_editor::SearchAllTabs" => {
                                    _workspace.handle_search_all_tabs(&SearchAllTabs, window, cx);
                                }
                                other => {
                                    eprintln!("[IPC] unknown command: {other}");
                                }
                            }
                        }
                        IpcMessage::SetText(content) => {
                            let tab = &_workspace.tabs[_workspace.active];
                            tab.editor.update(cx, |editor, cx| {
                                editor.set_text(content.as_str(), window, cx);
                            });
                            _workspace.save_session(cx);
                        }
                        IpcMessage::SaveAs(path) => {
                            if let Err(err) = _workspace.save_active_tab_as(path, cx) {
                                eprintln!("save-as failed: {err:#}");
                            }
                        }
                        IpcMessage::SwitchTab(index) => {
                            if index < _workspace.tabs.len() {
                                _workspace.active = index;
                                _workspace.save_session(cx);
                                cx.notify();
                            }
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

                let workspace_close = workspace.clone();
                window.on_window_should_close(cx, move |_window, cx| {
                    workspace_close.read_with(cx, |workspace, cx| {
                        workspace::save_session_from_outside(workspace, cx);
                    });
                    true
                });

                workspace
            },
        )
        .expect("failed to open window");

        cx.global::<OpenListener>().set_workspace(window_handle);

        cx.on_app_quit(|cx| {
            let listener = cx.global::<OpenListener>();
            if let Some(handle) = listener.workspace_handle() {
                handle.read_with(cx, |workspace, cx| {
                    workspace::save_session_from_outside(workspace, cx);
                }).ok();
            }
            std::future::ready(())
        })
        .detach();
    });
}

fn list_all_commands() {
    let mut entries: Vec<(String, &'static str)> = Vec::new();
    for action_data in generate_list_of_all_registered_actions() {
        if action_data.name.starts_with("lite_editor::") {
            let display = command_center::format_action_name(action_data.name);
            entries.push((display, action_data.name));
        }
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    for (display, qualified) in &entries {
        println!("{:<40} {}", display, qualified);
    }
}

fn friendly_name_to_qualified(input: &str) -> String {
    if input.contains("::") {
        return input.to_string();
    }
    let mut fallback: Option<&'static str> = None;
    for action_data in generate_list_of_all_registered_actions() {
        let display = command_center::format_action_name(action_data.name);
        if display.eq_ignore_ascii_case(input) {
            if action_data.name.starts_with("lite_editor::") {
                return action_data.name.to_string();
            }
            fallback = Some(action_data.name);
        }
    }
    if let Some(name) = fallback {
        return name.to_string();
    }
    format!("lite_editor::{input}")
}

fn register_languages(languages: &Arc<LanguageRegistry>) {
    languages.register_native_grammars(grammars::native_grammars());

    for name in crate::utils::GRAMMAR_NAMES {
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
