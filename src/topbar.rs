use std::path::PathBuf;

use gpui::*;
use gpui::prelude::FluentBuilder as _;

use crate::app_theme::colors;
use crate::workspace::LiteWorkspace;
use crate::tab::Tab;
use crate::{
    CompareFiles, OpenFile, NewFile, SaveFile, ToggleFind, ToggleReplace,
};

pub(crate) fn render_toolbar(
    this: &LiteWorkspace,
    cx: &Context<LiteWorkspace>,
) -> impl IntoElement {
    div()
        .id("toolbar")
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(32.0))
        .px(px(8.0))
        .gap(px(2.0))
        .bg(colors::BG_RAISED)
        .border_b_1()
        .border_color(colors::BG_BORDER)
        .child(toolbar_btn("New", cx.listener(|this, _, window, cx| {
            this.handle_new(&NewFile, window, cx);
        })))
        .child(toolbar_btn("Open", cx.listener(|this, _, window, cx| {
            this.handle_open(&OpenFile, window, cx);
        })))
        .child(toolbar_btn("Save", cx.listener(|this, _, window, cx| {
            this.handle_save(&SaveFile, window, cx);
        })))
        .child(toolbar_separator())
        .child(toolbar_btn("Find", cx.listener(|this, _, window, cx| {
            this.handle_toggle_find(&ToggleFind, window, cx);
        })))
        .child(toolbar_btn("Replace", cx.listener(|this, _, window, cx| {
            this.handle_toggle_replace(&ToggleReplace, window, cx);
        })))
        .child(toolbar_separator())
        .child(toolbar_btn("Compare", cx.listener(|this, _, window, cx| {
            this.handle_compare_files(&CompareFiles, window, cx);
        })))
        .child(toolbar_separator())
        .child(render_recent_menu(this, cx))
}

fn render_recent_menu(
    this: &LiteWorkspace,
    cx: &Context<LiteWorkspace>,
) -> impl IntoElement {
    let is_open = this.show_recent_menu;
    let entries: Vec<(PathBuf, String, String)> = this
        .recent_files
        .entries
        .iter()
        .take(15)
        .map(|path| {
            let name = crate::utils::file_name_from_path(path);
            let dir = path
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            (path.clone(), name, dir)
        })
        .collect();

    div()
        .id(ElementId::Name("tb-recent".into()))
        .cursor_pointer()
        .px(px(8.0))
        .py(px(4.0))
        .text_size(px(12.0))
        .text_color(if is_open {
            colors::TEXT_PRIMARY
        } else {
            colors::TEXT_DEFAULT
        })
        .hover(|s| {
            s.bg(colors::BG_HOVER)
                .text_color(colors::TEXT_PRIMARY)
        })
        .child("Recent")
        .on_click(cx.listener(|this, _, _, cx| {
            this.show_recent_menu = !this.show_recent_menu;
            cx.notify();
        }))
        .when(is_open, |el| {
            el.child(
                deferred(
                    anchored()
                        .anchor(Anchor::TopLeft)
                        .snap_to_window_with_margin(px(8.))
                        .child(
                            div()
                                .id("recent-dropdown")
                                .occlude()
                                .flex()
                                .flex_col()
                                .w(px(280.0))
                                .max_h(px(400.0))
                                .overflow_y_scroll()
                                .bg(colors::BG_PANEL)
                                .border_1()
                                .border_color(colors::BG_BORDER_STRONG)
                                .rounded(px(4.0))
                                .shadow_lg()
                                .on_mouse_down_out(cx.listener(
                                    |this, _, _, cx| {
                                        this.show_recent_menu = false;
                                        cx.notify();
                                    },
                                ))
                                .when(entries.is_empty(), |el| {
                                    el.child(
                                        div()
                                            .px(px(12.0))
                                            .py(px(8.0))
                                            .text_size(px(12.0))
                                            .text_color(colors::TEXT_SECONDARY)
                                            .child("No recent files"),
                                    )
                                })
                                .children(
                                    entries.into_iter().enumerate().map(
                                        |(i, (path, name, dir))| {
                                            div()
                                                .id(ElementId::Name(
                                                    format!("recent-menu-{i}")
                                                        .into(),
                                                ))
                                                .flex()
                                                .flex_col()
                                                .px(px(12.0))
                                                .py(px(6.0))
                                                .cursor_pointer()
                                                .hover(|s| {
                                                    s.bg(colors::BG_HOVER_DEEP)
                                                })
                                                .child(
                                                    div()
                                                        .text_size(px(13.0))
                                                        .text_color(
                                                            colors::TEXT_PRIMARY,
                                                        )
                                                        .child(name),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(10.0))
                                                        .text_color(
                                                            colors::TEXT_SECONDARY,
                                                        )
                                                        .text_ellipsis()
                                                        .child(dir),
                                                )
                                                .on_click(cx.listener(
                                                    move |this,
                                                          _,
                                                          window,
                                                          cx| {
                                                        this.show_recent_menu =
                                                            false;
                                                        if let Err(err) = this.open_file_path(
                                                            path.clone(),
                                                            window,
                                                            cx,
                                                        ) {
                                                        this.show_notification(
                                                            format!("Failed to open file: {err:#}"),
                                                            cx,
                                                        );
                                                    }
                                                    },
                                                ))
                                        },
                                    ),
                                ),
                        ),
                )
                .priority(1),
            )
        })
}

fn toolbar_btn(
    label: &'static str,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(format!("tb-{label}").into()))
        .cursor_pointer()
        .px(px(8.0))
        .py(px(4.0))
        .text_size(px(12.0))
        .text_color(colors::TEXT_DEFAULT)
        .hover(|s| s.bg(colors::BG_HOVER).text_color(colors::TEXT_PRIMARY))
        .child(label)
        .on_click(handler)
}

fn toolbar_separator() -> Div {
    div()
        .w(px(1.0))
        .h(px(16.0))
        .mx(px(4.0))
        .bg(colors::BG_HOVER)
}

pub(crate) fn render_status_bar(tab: &Tab, active: usize, tab_count: usize) -> Div {
    let encoding_label = crate::encoding::encoding_label(tab.encoding);
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .w_full()
        .h(px(24.0))
        .px(px(12.0))
        .bg(colors::BG_DEEPEST)
        .border_t_1()
        .border_color(colors::BG_BORDER)
        .child(
            div()
                .text_size(px(12.0))
                .text_color(colors::TEXT_MUTED)
                .child(tab.title.clone()),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(colors::TEXT_SECONDARY)
                        .child(encoding_label),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(colors::TEXT_SECONDARY)
                        .child(format!("Tab {} of {}", active + 1, tab_count)),
                ),
        )
}
