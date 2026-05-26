use gpui::*;
use gpui::prelude::FluentBuilder as _;

use crate::encoding;
use crate::workspace::LiteWorkspace;
use crate::{
    CloseTab, CompareFiles, FindNext, FindPrevious, MoveToGroup, NewFile, OpenFile,
    ReplaceAll, ReplaceNext, SaveAll, SaveFile, SearchAllTabs, ToggleCommandCenter,
    ToggleFind, ToggleRegex, ToggleReplace, ToggleToolbar,
};

#[derive(Clone)]
pub(crate) enum CommandSubmenu {
    SwitchBuffer,
    ChangeEncoding,
    ChangeFileType,
    RecentFiles,
}

impl LiteWorkspace {
    pub(crate) fn handle_toggle_command_center(
        &mut self,
        _action: &ToggleCommandCenter,
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

    pub(crate) fn execute_command(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        match name {
            "Switch Buffer" => {
                self.command_submenu = Some(CommandSubmenu::SwitchBuffer);
                self.command_center_query.clear();
                self.command_center_selected = 0;
                cx.notify();
                return;
            }
            "Switch Encoding" => {
                self.command_submenu = Some(CommandSubmenu::ChangeEncoding);
                self.command_center_query.clear();
                self.command_center_selected = 0;
                cx.notify();
                return;
            }
            "Change File Type" => {
                self.command_submenu = Some(CommandSubmenu::ChangeFileType);
                self.command_center_query.clear();
                self.command_center_selected = 0;
                cx.notify();
                return;
            }
            "Recent Files" => {
                self.command_submenu = Some(CommandSubmenu::RecentFiles);
                self.command_center_query.clear();
                self.command_center_selected = 0;
                cx.notify();
                return;
            }
            _ => {}
        }
        self.show_command_center = false;
        self.command_center_query.clear();
        self.command_submenu = None;
        match name {
            "New File" => self.handle_new(&NewFile, window, cx),
            "Open File" => self.handle_open(&OpenFile, window, cx),
            "Save File" => self.handle_save(&SaveFile, window, cx),
            "Save All" => self.handle_save_all(&SaveAll, window, cx),
            "Close Tab" => self.handle_close_tab(&CloseTab, window, cx),
            "Find" => self.handle_toggle_find(&ToggleFind, window, cx),
            "Find Next" => self.handle_find_next(&FindNext, window, cx),
            "Find Previous" => self.handle_find_previous(&FindPrevious, window, cx),
            "Replace" => self.handle_toggle_replace(&ToggleReplace, window, cx),
            "Replace Next" => self.handle_replace_next(&ReplaceNext, window, cx),
            "Replace All" => self.handle_replace_all(&ReplaceAll, window, cx),
            "Toggle Regex" => self.handle_toggle_regex(&ToggleRegex, window, cx),
            "Search All Tabs" => self.handle_search_all_tabs(&SearchAllTabs, window, cx),
            "Toggle Toolbar" => self.handle_toggle_toolbar(&ToggleToolbar, window, cx),
            "Move to Group" => self.handle_move_to_group(&MoveToGroup, window, cx),
            "Compare Files" => self.handle_compare_files(&CompareFiles, window, cx),
            _ => {}
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
                let encodings = encoding::SUPPORTED_ENCODINGS;
                if let Some(&label) = encodings.get(index) {
                    if let Some(enc) = encoding::encoding_from_label(label) {
                        let tab = &self.tabs[self.active];
                        if let Some(path) = tab.path.clone() {
                            if let Ok(content) = encoding::read_file_as_encoding(&path, enc) {
                                let tab = &mut self.tabs[self.active];
                                tab.encoding = enc;
                                tab.editor.update(cx, |editor, cx| {
                                    editor.set_text(content.as_str(), window, cx);
                                });
                            }
                        }
                    }
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
    cx: &Context<LiteWorkspace>,
) -> impl IntoElement {
    let main_commands = [
        "New File",
        "Open File",
        "Recent Files",
        "Save File",
        "Save All",
        "Close Tab",
        "Switch Buffer",
        "Find",
        "Find Next",
        "Find Previous",
        "Replace",
        "Replace Next",
        "Replace All",
        "Toggle Regex",
        "Search All Tabs",
        "Toggle Toolbar",
        "Move to Group",
        "Switch Encoding",
        "Change File Type",
        "Compare Files",
    ];
    let submenu_items: Vec<String> = match &this.command_submenu {
        Some(CommandSubmenu::SwitchBuffer) => {
            this.tabs.iter().map(|t| t.title.to_string()).collect()
        }
        Some(CommandSubmenu::ChangeEncoding) => {
            encoding::SUPPORTED_ENCODINGS.iter().map(|s| s.to_string()).collect()
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
        Some(CommandSubmenu::SwitchBuffer) => Some("Switch Buffer"),
        Some(CommandSubmenu::ChangeEncoding) => Some("Switch Encoding"),
        Some(CommandSubmenu::ChangeFileType) => Some("Change File Type"),
        Some(CommandSubmenu::RecentFiles) => Some("Recent Files"),
        None => None,
    };
    let submenu_clone = this.command_submenu.clone();
    let is_submenu = this.command_submenu.is_some();
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
        main_commands
            .iter()
            .enumerate()
            .filter(|(_, cmd)| {
                if this.command_center_query.is_empty() {
                    true
                } else {
                    cmd.to_lowercase()
                        .contains(&this.command_center_query.to_lowercase())
                }
            })
            .map(|(i, s)| (i, s.to_string()))
            .collect()
    };
    let selected = this.command_center_selected.min(filtered.len().saturating_sub(1));
    let selected_cmd = filtered
        .get(selected)
        .map(|(_, c)| c.clone())
        .unwrap_or_default();

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
                                    this.execute_command(
                                        &selected_cmd,
                                        window,
                                        cx,
                                    );
                                }
                            }
                            "backspace" => {
                                this.command_center_query.pop();
                                this.command_center_selected = 0;
                                cx.notify();
                            }
                            _ => {
                                if let Some(ch) = event.keystroke.key.chars().next()
                                {
                                    if ch.is_alphanumeric()
                                        || ch == ' '
                                        || ch == '-'
                                    {
                                        this.command_center_query.push(ch);
                                        this.command_center_selected = 0;
                                        cx.notify();
                                    }
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
                                                    this.execute_command(&cmd_text, window, cx);
                                                }
                                            },
                                        )
                                    })
                            },
                        )),
                ),
        )
}
