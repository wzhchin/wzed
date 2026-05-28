use gpui::*;
use gpui::prelude::FluentBuilder as _;

use crate::workspace::LiteWorkspace;

#[derive(Clone)]
pub(crate) enum CommandSubmenu {
    SwitchBuffer,
    ChangeEncoding,
    ChangeFileType,
    RecentFiles,
}

#[derive(Clone)]
pub(crate) struct CommandEntry {
    pub action_name: &'static str,
    pub display_name: String,
    pub submenu_kind: Option<CommandSubmenu>,
}

pub(crate) fn format_action_name(name: &str) -> String {
    crate::utils::format_action_name(name)
}

pub(crate) fn collect_commands(cx: &App) -> Vec<CommandEntry> {
    let mut commands: Vec<CommandEntry> = cx
        .all_action_names()
        .iter()
        .filter(|name| name.starts_with("lite_editor::"))
        .map(|name| CommandEntry {
            action_name: name,
            display_name: format_action_name(name),
            submenu_kind: None,
        })
        .collect();

    commands.push(CommandEntry {
        action_name: "",
        display_name: "switch-buffer".into(),
        submenu_kind: Some(CommandSubmenu::SwitchBuffer),
    });
    commands.push(CommandEntry {
        action_name: "",
        display_name: "switch-encoding".into(),
        submenu_kind: Some(CommandSubmenu::ChangeEncoding),
    });
    commands.push(CommandEntry {
        action_name: "",
        display_name: "change-file-type".into(),
        submenu_kind: Some(CommandSubmenu::ChangeFileType),
    });
    commands.push(CommandEntry {
        action_name: "",
        display_name: "recent-files".into(),
        submenu_kind: Some(CommandSubmenu::RecentFiles),
    });

    commands.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    commands
}

impl LiteWorkspace {
    pub(crate) fn handle_toggle_command_center(
        &mut self,
        _action: &crate::ToggleCommandCenter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_command_center = !self.show_command_center;
        self.command_center_query.clear();
        self.command_center_selected = 0;
        self.command_submenu = None;
        if self.show_command_center {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn execute_command(
        &mut self,
        entry: &CommandEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(submenu) = &entry.submenu_kind {
            self.command_submenu = Some(submenu.clone());
            self.command_center_query.clear();
            self.command_center_selected = 0;
            cx.notify();
            return;
        }

        self.show_command_center = false;
        self.command_center_query.clear();
        self.command_submenu = None;

        if let Ok(action) = cx.build_action(entry.action_name, None) {
            window.dispatch_action(action, cx);
        }
    }

    pub(crate) fn execute_submenu_item(
        &mut self,
        submenu: &CommandSubmenu,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_command_center = false;
        self.command_center_query.clear();
        self.command_submenu = None;
        match submenu {
            CommandSubmenu::SwitchBuffer => {
                if index < self.tabs.len() {
                    self.active = index;
                    cx.notify();
                }
            }
            CommandSubmenu::ChangeEncoding => {
                let encodings = crate::encoding::SUPPORTED_ENCODINGS;
                if let Some(&label) = encodings.get(index)
                    && let Some(enc) = crate::encoding::encoding_from_label(label)
                    && let Some(path) = self.tabs[self.active].path.clone()
                    && let Ok(content) = crate::encoding::read_file_as_encoding(&path, enc)
                {
                    let tab = &mut self.tabs[self.active];
                    tab.encoding = enc;
                    tab.editor.update(cx, |editor, cx| {
                        editor.set_text(content.as_str(), window, cx);
                    });
                }
                cx.notify();
            }
            CommandSubmenu::ChangeFileType => {
                let grammar_names = [
                    "bash", "c", "cpp", "css", "diff", "go", "json", "jsonc", "markdown",
                    "python", "regex", "rust", "tsx", "typescript", "yaml",
                ];
                if let Some(&grammar_name) = grammar_names.get(index) {
                    let languages = self.languages.clone();
                    let buffer = {
                        let tab = &self.tabs[self.active];
                        #[allow(clippy::map_clone)]
                        tab.editor.read(cx).buffer().read(cx).as_singleton().map(|b| b.clone())
                    };
                    if let Some(buffer) = buffer {
                        cx.spawn(async move |_, cx| {
                            let lang = languages.language_for_name(grammar_name).await?;
                            buffer.update(cx, |buf, cx| {
                                buf.set_language(Some(lang), cx);
                            });
                            Result::<()>::Ok(())
                        })
                        .detach_and_log_err(cx);
                    }
                }
                cx.notify();
            }
            CommandSubmenu::RecentFiles => {
                if let Some(path) = self.recent_files.entries.get(index).cloned() {
                    self.open_file_path(path, window, cx).ok();
                }
            }
        }
    }
}

pub(crate) fn render_command_center(
    this: &LiteWorkspace,
    _window: &mut Window,
    cx: &mut Context<LiteWorkspace>,
) -> impl IntoElement {
    let all_commands = collect_commands(cx);
    let submenu_items: Vec<String> = match &this.command_submenu {
        Some(CommandSubmenu::SwitchBuffer) => {
            this.tabs.iter().map(|t| t.title.to_string()).collect()
        }
        Some(CommandSubmenu::ChangeEncoding) => {
            crate::encoding::SUPPORTED_ENCODINGS.iter().map(|s| s.to_string()).collect()
        }
        Some(CommandSubmenu::ChangeFileType) => {
            ["Bash", "C", "C++", "CSS", "Diff", "Go", "JSON", "JSONC", "Markdown",
             "Python", "Regex", "Rust", "TSX", "TypeScript", "YAML",
            ].iter().map(|s| s.to_string()).collect()
        }
        Some(CommandSubmenu::RecentFiles) => {
            this.recent_files.entries.iter().take(20)
                .filter_map(|p| p.to_str().map(|s| s.to_string()))
                .collect()
        }
        None => Vec::new(),
    };
    let submenu_title: Option<&str> = match &this.command_submenu {
        Some(CommandSubmenu::SwitchBuffer) => Some("switch-buffer"),
        Some(CommandSubmenu::ChangeEncoding) => Some("switch-encoding"),
        Some(CommandSubmenu::ChangeFileType) => Some("change-file-type"),
        Some(CommandSubmenu::RecentFiles) => Some("recent-files"),
        None => None,
    };
    let submenu_clone = this.command_submenu.clone();
    let is_submenu = this.command_submenu.is_some();

    let filtered_commands: Vec<CommandEntry> = if is_submenu {
        Vec::new()
    } else {
        all_commands
    };

    let filtered: Vec<(usize, String)> = if is_submenu {
        submenu_items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                if this.command_center_query.is_empty() {
                    true
                } else {
                    item.to_lowercase()
                        .contains(&this.command_center_query.to_lowercase())
                }
            })
            .map(|(i, s)| (i, s.clone()))
            .collect()
    } else {
        filtered_commands
            .iter()
            .enumerate()
            .filter(|(_, cmd)| {
                if this.command_center_query.is_empty() {
                    true
                } else {
                    cmd.display_name
                        .to_lowercase()
                        .contains(&this.command_center_query.to_lowercase())
                }
            })
            .map(|(i, cmd)| (i, cmd.display_name.clone()))
            .collect()
    };
    let selected = this.command_center_selected.min(filtered.len().saturating_sub(1));

    div()
        .id("command-center-overlay")
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .pt(px(150.0))
        .child(
            div()
                .id("command-center")
                .occlude()
                .focusable()
                .track_focus(&this.focus_handle)
                .flex()
                .flex_col()
                .w(px(400.0))
                .max_h(px(500.0))
                .bg(gpui::hsla(0.0, 0.0, 0.13, 1.0))
                .border_1()
                .border_color(gpui::hsla(0.0, 0.0, 0.3, 1.0))
                .rounded(px(8.0))
                .shadow_lg()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.show_command_center = false;
                    cx.notify();
                }))
                .on_key_down({
                    let filtered_for_keys = filtered.clone();
                    let submenu_for_keys = submenu_clone.clone();
                    cx.listener(
                        move |this, event: &KeyDownEvent, window, cx| {
                        match event.keystroke.key.as_str() {
                            "escape" => {
                                if this.command_submenu.is_some() {
                                    this.command_submenu = None;
                                    this.command_center_query.clear();
                                    this.command_center_selected = 0;
                                } else {
                                    this.show_command_center = false;
                                }
                                cx.notify();
                            }
                            "up" => {
                                this.command_center_selected =
                                    this.command_center_selected.saturating_sub(1);
                                cx.notify();
                            }
                            "down" => {
                                this.command_center_selected += 1;
                                cx.notify();
                            }
                            "enter" => {
                                if is_submenu {
                                    let selected_idx = filtered_for_keys
                                        .get(this.command_center_selected)
                                        .map(|(i, _)| *i)
                                        .unwrap_or(0);
                                    if let Some(ref sub) = submenu_for_keys {
                                        this.execute_submenu_item(
                                            sub,
                                            selected_idx,
                                            window,
                                            cx,
                                        );
                                    }
                                } else {
                                    let all_cmds = collect_commands(cx);
                                    let visible: Vec<&CommandEntry> = all_cmds
                                        .iter()
                                        .filter(|cmd| {
                                            if this.command_center_query.is_empty() {
                                                true
                                            } else {
                                                cmd.display_name.to_lowercase().contains(
                                                    &this.command_center_query.to_lowercase(),
                                                )
                                            }
                                        })
                                        .collect();
                                    if let Some(entry) = visible.get(this.command_center_selected) {
                                        this.execute_command(entry, window, cx);
                                    }
                                }
                            }
                            "backspace" => {
                                this.command_center_query.pop();
                                this.command_center_selected = 0;
                                cx.notify();
                            }
                            _ => {
                                if let Some(ch) = event.keystroke.key.chars().next()
                                    && (ch.is_alphanumeric() || ch == ' ' || ch == '-')
                                {
                                    this.command_center_query.push(ch);
                                    this.command_center_selected = 0;
                                    cx.notify();
                                }
                            }
                        }
                    },
                    )
                })
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .px(px(12.0))
                        .py(px(10.0))
                        .border_b_1()
                        .border_color(gpui::hsla(0.0, 0.0, 0.2, 1.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(gpui::hsla(0.0, 0.0, 0.5, 1.0))
                                .child(if is_submenu {
                                    submenu_title.unwrap_or("M-x")
                                } else {
                                    "M-x "
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(14.0))
                                .text_color(gpui::hsla(0.0, 0.0, 0.9, 1.0))
                                .child(if this.command_center_query.is_empty() {
                                    "Type a command...".into()
                                } else {
                                    SharedString::from(
                                        this.command_center_query.clone(),
                                    )
                                }),
                        ),
                )
                .child(
                    div()
                        .id("command-list")
                        .flex()
                        .flex_col()
                        .overflow_y_scroll()
                        .children(filtered.iter().enumerate().map(
                            |(i, (cmd_idx, cmd))| {
                                let is_selected = i == selected;
                                let cmd_text = cmd.clone();
                                let click_idx = *cmd_idx;
                                div()
                                    .id(ElementId::Name(
                                        format!("cmd-{cmd_idx}").into(),
                                    ))
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .text_size(px(13.0))
                                    .cursor_pointer()
                                    .text_color(if is_selected {
                                        gpui::hsla(0.0, 0.0, 1.0, 1.0)
                                    } else {
                                        gpui::hsla(0.0, 0.0, 0.8, 1.0)
                                    })
                                    .when(is_selected, |el| {
                                        el.bg(gpui::hsla(220.0, 0.6, 0.4, 1.0))
                                    })
                                    .hover(|s| {
                                        s.bg(gpui::hsla(220.0, 0.5, 0.35, 1.0))
                                    })
                                    .child(cmd_text.clone())
                                    .on_click({
                                        let sub = submenu_clone.clone();
                                        cx.listener(
                                            move |this, _, window, cx| {
                                                if let Some(ref sub) = sub {
                                                    this.execute_submenu_item(sub, click_idx, window, cx);
                                                } else {
                                                    let all_cmds = collect_commands(cx);
                                                    let entry = all_cmds.iter().find(|cmd| cmd.display_name == cmd_text);
                                                    if let Some(entry) = entry {
                                                        this.execute_command(entry, window, cx);
                                                    }
                                                }
                                            },
                                        )
                                    })
                            },
                        )),
                ),
        )
}
