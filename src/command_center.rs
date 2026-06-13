use gpui::*;
use gpui::prelude::FluentBuilder as _;

use crate::app_theme::colors;
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
        self.command_center_editor.update(cx, |editor, cx| {
            editor.set_text("", window, cx);
        });
        self.command_center_selected = 0;
        self.command_submenu = None;
        if self.show_command_center {
            self.command_center_editor.focus_handle(cx).focus(window, cx);
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
            self.command_center_editor.update(cx, |editor, cx| {
                editor.set_text("", window, cx);
            });
            self.command_center_selected = 0;
            cx.notify();
            return;
        }

        self.show_command_center = false;
        self.command_center_editor.update(cx, |editor, cx| {
            editor.set_text("", window, cx);
        });
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
        self.command_center_editor.update(cx, |editor, cx| {
            editor.set_text("", window, cx);
        });
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
                let Some(&label) = encodings.get(index) else {
                    cx.notify();
                    return;
                };
                let Some(enc) = crate::encoding::encoding_from_label(label) else {
                    cx.notify();
                    return;
                };
                let tab = &self.tabs[self.active];
                if tab.path.is_none() {
                    self.show_notification("No file to reload", cx);
                    cx.notify();
                    return;
                }
                if tab.is_dirty(cx) {
                    self.show_notification("Save changes before switching encoding", cx);
                    cx.notify();
                    return;
                }
                let path = tab.path.clone().unwrap_or_default();
                match crate::encoding::read_file_as_encoding(&path, enc) {
                    Ok(content) => {
                        let tab = &mut self.tabs[self.active];
                        tab.encoding = enc;
                        tab.editor.update(cx, |editor, cx| {
                            editor.set_text(content.as_str(), window, cx);
                        });
                    }
                    Err(err) => {
                        self.show_notification(format!("Failed to reload: {err:#}"), cx);
                    }
                }
                cx.notify();
            }
            CommandSubmenu::ChangeFileType => {
                if let Some(&grammar_name) = crate::utils::GRAMMAR_NAMES.get(index) {
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
                    if let Err(err) = self.open_file_path(path, window, cx) {
                        self.show_notification(format!("Failed to open file: {err:#}"), cx);
                    }
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
    let query_text = this.command_center_editor.read(cx).text(cx).to_string();
    let query_lower = query_text.to_lowercase();

    let submenu_items: Vec<String> = match &this.command_submenu {
        Some(CommandSubmenu::SwitchBuffer) => {
            this.tabs.iter().map(|t| t.title.to_string()).collect()
        }
        Some(CommandSubmenu::ChangeEncoding) => {
            crate::encoding::SUPPORTED_ENCODINGS.iter().map(|s| s.to_string()).collect()
        }
        Some(CommandSubmenu::ChangeFileType) => {
            crate::utils::GRAMMAR_DISPLAY_NAMES.iter().map(|s| s.to_string()).collect()
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
    let current_encoding_label = this.tabs.get(this.active).map(|tab| {
        crate::encoding::encoding_label(tab.encoding).to_string()
    });

    let all_commands = collect_commands(cx);

    let filtered: Vec<(usize, String)> = if is_submenu {
        submenu_items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                if query_text.is_empty() {
                    true
                } else {
                    item.to_lowercase().contains(&query_lower)
                }
            })
            .map(|(i, s)| (i, s.clone()))
            .collect()
    } else {
        all_commands
            .iter()
            .enumerate()
            .filter(|(_, cmd)| {
                if query_text.is_empty() {
                    true
                } else {
                    cmd.display_name.to_lowercase().contains(&query_lower)
                }
            })
            .map(|(i, cmd)| (i, cmd.display_name.clone()))
            .collect()
    };
    let selected = this.command_center_selected.min(filtered.len().saturating_sub(1));

    let cc_editor = this.command_center_editor.clone();

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
                .flex()
                .flex_col()
                .w(px(400.0))
                .max_h(px(500.0))
                .bg(colors::BG_PANEL)
                .border_1()
                .border_color(colors::TEXT_DIM)
                .rounded(px(8.0))
                .shadow_lg()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.show_command_center = false;
                    cx.notify();
                }))
                .on_key_down({
                    let filtered_for_keys = filtered.clone();
                    let submenu_for_keys = submenu_clone.clone();
                    let cmds_for_enter = all_commands.clone();
                    cx.listener(
                        move |this, event: &KeyDownEvent, window, cx| {
                        match event.keystroke.key.as_str() {
                            "escape" => {
                                if this.command_submenu.is_some() {
                                    this.command_submenu = None;
                                    this.command_center_editor.update(cx, |editor, cx| {
                                        editor.set_text("", window, cx);
                                    });
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
                                let query = this.command_center_editor.read(cx).text(cx).to_string();
                                let query_lower = query.to_lowercase();
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
                                    let visible: Vec<&CommandEntry> = cmds_for_enter
                                        .iter()
                                        .filter(|cmd| {
                                            if query.is_empty() {
                                                true
                                            } else {
                                                cmd.display_name.to_lowercase().contains(
                                                    &query_lower,
                                                )
                                            }
                                        })
                                        .collect();
                                    if let Some(entry) = visible.get(this.command_center_selected) {
                                        this.execute_command(entry, window, cx);
                                    }
                                }
                            }
                            _ => {}
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
                        .py(px(6.0))
                        .border_b_1()
                        .border_color(colors::BG_HOVER)
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(colors::TEXT_SECONDARY)
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
                                .text_color(colors::TEXT_PRIMARY)
                                .child(cc_editor),
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
                                let is_current = matches!(&this.command_submenu, Some(CommandSubmenu::ChangeEncoding))
                                    && current_encoding_label.as_deref() == Some(cmd.as_str());
                                let display_text = if is_current {
                                    format!("● {cmd_text}")
                                } else {
                                    cmd_text.clone()
                                };
                                div()
                                    .id(ElementId::Name(
                                        format!("cmd-{cmd_idx}").into(),
                                    ))
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .text_size(px(13.0))
                                    .cursor_pointer()
                                    .text_color(if is_selected {
                                        colors::TEXT_SELECTED
                                    } else {
                                        colors::TEXT_BRIGHT
                                    })
                                    .when(is_selected, |el| {
                                        el.bg(colors::ACCENT_SELECTED)
                                    })
                                    .hover(|s| {
                                        s.bg(colors::ACCENT_HOVER)
                                    })
                                    .child(display_text)
                                    .on_click({
                                        let sub = submenu_clone.clone();
                                        let click_cmds = all_commands.clone();
                                        cx.listener(
                                            move |this, _, window, cx| {
                                                if let Some(ref sub) = sub {
                                                    this.execute_submenu_item(sub, click_idx, window, cx);
                                                } else {
                                                    let entry = click_cmds.iter().find(|cmd| cmd.display_name == cmd_text);
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
