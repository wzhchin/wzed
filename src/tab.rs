use std::path::PathBuf;

use editor::Editor;
use gpui::*;
use gpui::prelude::FluentBuilder as _;

pub(crate) struct Tab {
    pub editor: Entity<Editor>,
    pub path: Option<PathBuf>,
    pub title: SharedString,
    pub group: Option<SharedString>,
    pub encoding: &'static encoding_rs::Encoding,
    pub pinned: bool,
}

impl Tab {
    pub(crate) fn is_dirty(&self, cx: &App) -> bool {
        self.editor.read(cx).buffer().read(cx).is_dirty(cx)
    }
}

pub(crate) struct TabInfo {
    pub index: usize,
    pub title: SharedString,
    pub is_active: bool,
    pub is_dirty: bool,
    pub is_pinned: bool,
    pub group: Option<SharedString>,
    pub file_extension: Option<String>,
}

fn icon_path_for_extension(ext: Option<&str>) -> &'static str {
    match ext {
        Some("rs") => "icons/file_rust.svg",
        Some("md") | Some("mdx") => "icons/file_markdown.svg",
        Some("toml") => "icons/file_toml.svg",
        Some("js") | Some("ts") | Some("jsx") | Some("tsx") | Some("py") | Some("go")
        | Some("c") | Some("h") | Some("cpp") | Some("java") | Some("rb") => {
            "icons/file_code.svg"
        }
        Some("json") | Some("yaml") | Some("yml") | Some("xml") | Some("ini") | Some("cfg") => {
            "icons/file_generic.svg"
        }
        Some("txt") | Some("log") => "icons/file_text_outlined.svg",
        Some("diff") | Some("patch") => "icons/file_diff.svg",
        _ => "icons/file.svg",
    }
}

#[derive(Clone)]
pub(crate) struct DraggedTab {
    pub index: usize,
    pub title: SharedString,
}

impl Render for DraggedTab {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.0))
            .py(px(6.0))
            .bg(gpui::hsla(0.0, 0.0, 0.2, 0.9))
            .border_1()
            .border_color(gpui::hsla(220.0, 0.8, 0.6, 1.0))
            .rounded(px(4.0))
            .text_size(px(13.0))
            .text_color(gpui::hsla(0.0, 0.0, 0.9, 1.0))
            .child(self.title.clone())
    }
}

pub(crate) fn render_tab_list(
    tabs: &[TabInfo],
    scroll_handle: &ScrollHandle,
    last_scrolled_active: usize,
    cx: &Context<crate::workspace::LiteWorkspace>,
) -> impl IntoElement {
    let mut children: Vec<AnyElement> = Vec::new();
    let mut last_group: Option<&SharedString> = None;
    let mut active_child_index = 0usize;
    let mut active_tab_index = 0usize;
    let mut child_index = 0usize;

    for tab in tabs {
        if let Some(ref group) = tab.group {
            if last_group != Some(group) {
                children.push(
                    div()
                        .px(px(10.0))
                        .py(px(3.0))
                        .text_size(px(10.0))
                        .text_color(gpui::hsla(0.0, 0.0, 0.4, 1.0))
                        .border_t_1()
                        .border_color(gpui::hsla(0.0, 0.0, 0.12, 1.0))
                        .child(group.clone())
                        .into_any_element(),
                );
                child_index += 1;
                last_group = Some(group);
            }
        } else {
            last_group = None;
        }

        if tab.is_active {
            active_child_index = child_index;
            active_tab_index = tab.index;
        }

        let idx = tab.index;
        let active = tab.is_active;
        let dirty = tab.is_dirty;
        let pinned = tab.is_pinned;
        let title = tab.title.clone();
        let icon_path = icon_path_for_extension(tab.file_extension.as_deref());

        let dragged = DraggedTab {
            index: idx,
            title: title.clone(),
        };

        let icon_color = if active {
            gpui::hsla(0.0, 0.0, 0.7, 1.0)
        } else {
            gpui::hsla(0.0, 0.0, 0.45, 1.0)
        };

        let mut tab_el = div()
            .id(ElementId::Name(format!("tab-{idx}").into()))
            .flex()
            .items_center()
            .px(px(10.0))
            .py(px(6.0))
            .w_full()
            .cursor_pointer()
            .child(
                svg()
                    .path(icon_path)
                    .size(px(14.0))
                    .text_color(icon_color)
                    .mr(px(6.0))
                    .flex_shrink_0(),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(if active {
                                gpui::hsla(0.0, 0.0, 0.9, 1.0)
                            } else {
                                gpui::hsla(0.0, 0.0, 0.6, 1.0)
                            })
                            .text_ellipsis()
                            .child(title),
                    )
                    .when(dirty, |el| {
                        el.child(
                            div()
                                .ml(px(4.0))
                                .size(px(6.0))
                                .rounded_full()
                                .bg(gpui::hsla(220.0, 0.8, 0.6, 1.0)),
                        )
                    }),
            )
            .child(
                div()
                    .id(ElementId::Name(format!("tab-close-{idx}").into()))
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(16.0))
                    .rounded(px(3.0))
                    .text_color(gpui::hsla(0.0, 0.0, 0.5, 1.0))
                    .hover(|s| {
                        s.bg(gpui::hsla(0.0, 0.0, 0.25, 1.0))
                            .text_color(gpui::hsla(0.0, 0.0, 0.9, 1.0))
                    })
                    .child(
                        svg()
                            .path("icons/close.svg")
                            .size(px(12.0))
                            .flex_shrink_0(),
                    )
                    .on_click(cx.listener(move |workspace, _event, _window, cx| {
                        if workspace.tabs.len() <= 1 {
                            return;
                        }
                        let closed_idx = idx;
                        workspace.tabs.remove(closed_idx);
                        if workspace.tabs.is_empty() {
                            return;
                        }
                        if workspace.active >= workspace.tabs.len() {
                            workspace.active = workspace.tabs.len() - 1;
                        } else if closed_idx <= workspace.active && workspace.active > 0 {
                            workspace.active -= 1;
                        }
                        workspace.save_session(cx);
                        cx.notify();
                    })),
            )
            .on_click(cx.listener(move |workspace, _, _window, cx| {
                workspace.active = idx;
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |workspace, _event, _window, cx| {
                    let is_pinned = workspace.tabs.get(idx).map_or(false, |t| t.pinned);
                    workspace.context_menu_tab = Some(idx);
                    workspace.show_tab_context_menu = true;
                    workspace.tab_context_menu_is_pinned = is_pinned;
                    cx.notify();
                }),
            )
            .on_drag(dragged, |drag: &DraggedTab, _position, _window, cx| {
                cx.new(|_| drag.clone())
            })
            .on_drop(cx.listener(move |this, dragged: &DraggedTab, _window, cx| {
                let from = dragged.index;
                let to = idx;
                if from == to || from >= this.tabs.len() || to >= this.tabs.len() {
                    return;
                }
                let active_id = this.tabs[this.active].editor.entity_id();
                let tab = this.tabs.remove(from);
                this.tabs.insert(to, tab);
                this.active = this
                    .tabs
                    .iter()
                    .position(|t| t.editor.entity_id() == active_id)
                    .unwrap_or(0);
                cx.notify();
            }));

        if active {
            tab_el = tab_el
                .bg(gpui::hsla(0.0, 0.0, 0.18, 1.0))
                .border_l_2()
                .border_color(if pinned {
                    gpui::hsla(40.0, 0.8, 0.55, 1.0)
                } else {
                    gpui::hsla(220.0, 0.8, 0.6, 1.0)
                });
        } else if pinned {
            tab_el = tab_el
                .bg(gpui::hsla(0.0, 0.0, 0.13, 1.0))
                .border_l_2()
                .border_color(gpui::hsla(40.0, 0.6, 0.4, 1.0));
        } else {
            tab_el = tab_el.hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.13, 1.0)));
        }

        children.push(tab_el.into_any_element());
        child_index += 1;
    }

    if active_tab_index != last_scrolled_active {
        scroll_handle.scroll_to_item(active_child_index);
    }
    div()
        .id("tab-list")
        .track_scroll(scroll_handle)
        .flex()
        .flex_col()
        .flex_1()
        .overflow_y_scroll()
        .children(children)
}
