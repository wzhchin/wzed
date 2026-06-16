use std::path::PathBuf;

use gpui::prelude::FluentBuilder as _;
use gpui::*;

use crate::app_theme::colors;
use crate::tab::Tab;
use crate::workspace::LiteWorkspace;
use crate::{CompareFiles, NewFile, OpenFile, SaveFile, ToggleFind, ToggleReplace};

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
        .child(toolbar_btn(
            "New",
            cx.listener(|this, _, window, cx| {
                this.handle_new(&NewFile, window, cx);
            }),
        ))
        .child(toolbar_btn(
            "Open",
            cx.listener(|this, _, window, cx| {
                this.handle_open(&OpenFile, window, cx);
            }),
        ))
        .child(toolbar_btn(
            "Save",
            cx.listener(|this, _, window, cx| {
                this.handle_save(&SaveFile, window, cx);
            }),
        ))
        .child(toolbar_separator())
        .child(toolbar_btn(
            "Find",
            cx.listener(|this, _, window, cx| {
                this.handle_toggle_find(&ToggleFind, window, cx);
            }),
        ))
        .child(toolbar_btn(
            "Replace",
            cx.listener(|this, _, window, cx| {
                this.handle_toggle_replace(&ToggleReplace, window, cx);
            }),
        ))
        .child(toolbar_separator())
        .child(toolbar_btn(
            "Compare",
            cx.listener(|this, _, window, cx| {
                this.handle_compare_files(&CompareFiles, window, cx);
            }),
        ))
        .child(toolbar_separator())
        .child(render_recent_menu(this, cx))
}

fn render_recent_menu(this: &LiteWorkspace, cx: &Context<LiteWorkspace>) -> impl IntoElement {
    let is_open = this.show_recent_menu;
    let entries: Vec<(PathBuf, String, String)> = this
        .recent_files
        .entries
        .iter()
        .take(15)
        .map(|path| {
            let name = crate::utils::file_name_from_path(path);
            let dir = path.parent().map(|p| p.display().to_string()).unwrap_or_default();
            (path.clone(), name, dir)
        })
        .collect();

    div()
        .id(ElementId::Name("tb-recent".into()))
        .cursor_pointer()
        .px(px(8.0))
        .py(px(4.0))
        .text_size(px(12.0))
        .text_color(if is_open { colors::TEXT_PRIMARY } else { colors::TEXT_DEFAULT })
        .hover(|s| s.bg(colors::BG_HOVER).text_color(colors::TEXT_PRIMARY))
        .child("Recent")
        .on_click(cx.listener(|this, _, _, cx| {
            this.show_recent_menu = !this.show_recent_menu;
            cx.notify();
        }))
        .when(is_open, |el| {
            el.child(
                deferred(
                    anchored().anchor(Anchor::TopLeft).snap_to_window_with_margin(px(8.)).child(
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
                            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                this.show_recent_menu = false;
                                cx.notify();
                            }))
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
                            .children(entries.into_iter().enumerate().map(
                                |(i, (path, name, dir))| {
                                    div()
                                        .id(ElementId::Name(format!("recent-menu-{i}").into()))
                                        .flex()
                                        .flex_col()
                                        .px(px(12.0))
                                        .py(px(6.0))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(colors::BG_HOVER_DEEP))
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .text_color(colors::TEXT_PRIMARY)
                                                .child(name),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.0))
                                                .text_color(colors::TEXT_SECONDARY)
                                                .text_ellipsis()
                                                .child(dir),
                                        )
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.show_recent_menu = false;
                                            if let Err(err) =
                                                this.open_file_path(path.clone(), window, cx)
                                            {
                                                this.show_notification(
                                                    format!("Failed to open file: {err:#}"),
                                                    cx,
                                                );
                                            }
                                        }))
                                },
                            )),
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
    div().w(px(1.0)).h(px(16.0)).mx(px(4.0)).bg(colors::BG_HOVER)
}

// On Windows the OS title bar is hidden (TitlebarOptions.appears_transparent),
// so GPUI expects the app to draw its own drag region and caption buttons.
// Without this the window has no title bar at all. This bar only renders on
// Windows; macOS/Linux keep their native chrome.
#[cfg(target_os = "windows")]
pub(crate) fn render_title_bar(window: &mut Window) -> Div {
    let is_maximized = window.is_maximized();
    div()
        .id("title-bar")
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(32.0))
        .bg(colors::BG_DEEPEST)
        // The leftover space (everything not a caption button) doubles as the
        // drag handle that lets the user move the window.
        .child(
            div()
                .id("title-drag")
                .flex_1()
                .h_full()
                .window_control_area(WindowControlArea::Drag)
                .child(
                    div()
                        .px(px(12.0))
                        .h_full()
                        .flex()
                        .items_center()
                        .text_size(px(12.0))
                        .text_color(colors::TEXT_SECONDARY)
                        .child("WZed"),
                ),
        )
        .child(caption_button("tb-win-min", is_maximized, CaptionIcon::Minimize))
        .child(caption_button("tb-win-max", is_maximized, CaptionIcon::Maximize))
        .child(caption_button("tb-win-close", is_maximized, CaptionIcon::Close))
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
enum CaptionIcon {
    Minimize,
    Maximize,
    Close,
}

#[cfg(target_os = "windows")]
fn caption_button(id: &'static str, is_maximized: bool, icon: CaptionIcon) -> Div {
    // Each caption button registers its WindowControlArea so the Windows
    // platform (WM_NCHITTEST) routes the click to the OS minimize/maximize/
    // close behavior, and also paints a hover background like the native bar.
    let area = match icon {
        CaptionIcon::Minimize => WindowControlArea::Min,
        CaptionIcon::Maximize => WindowControlArea::Max,
        CaptionIcon::Close => WindowControlArea::Close,
    };
    let hover_bg = match icon {
        CaptionIcon::Close => {
            gpui::Rgba { r: 232.0 / 255.0, g: 17.0 / 255.0, b: 32.0 / 255.0, a: 1.0 }.into()
        }
        _ => colors::BG_HOVER,
    };
    div()
        .id(ElementId::Name(id.into()))
        .flex()
        .items_center()
        .justify_center()
        .w(px(46.0))
        .h_full()
        .window_control_area(area)
        .hover(|style| style.bg(hover_bg))
        .child(canvas(
            move |_bounds, _window, _cx| {},
            move |bounds, _state, window, _cx| {
                paint_caption_icon(window, bounds, icon, is_maximized);
            },
        ))
}

// Draw the caption glyphs with the low-level path painter so the bar has no
// dependency on an icon font or asset file.
#[cfg(target_os = "windows")]
fn paint_caption_icon(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    icon: CaptionIcon,
    is_maximized: bool,
) {
    let center = bounds.center();
    let stroke_width = px(1.0);
    let half = px(5.0);
    let color: gpui::Hsla = colors::TEXT_DEFAULT;
    let mut builder = |path: Result<gpui::Path<Pixels>, _>| {
        if let Ok(path) = path {
            window.paint_path(path, color);
        }
    };
    match icon {
        CaptionIcon::Minimize => {
            let mut path = gpui::PathBuilder::stroke(stroke_width);
            path.move_to(point(center.x - half, center.y + half - px(1.0)));
            path.line_to(point(center.x + half, center.y + half - px(1.0)));
            builder(path.build());
        }
        CaptionIcon::Maximize if is_maximized => {
            // Restore glyph: a front square plus a notched back square.
            let mut front = gpui::PathBuilder::stroke(stroke_width);
            front.move_to(point(center.x - px(3.0), center.y));
            front.line_to(point(center.x + half, center.y));
            front.line_to(point(center.x + half, center.y + half));
            front.line_to(point(center.x - px(3.0), center.y + half));
            front.close();
            builder(front.build());
            let mut back = gpui::PathBuilder::stroke(stroke_width);
            back.move_to(point(center.x - px(1.0), center.y + px(2.0)));
            back.line_to(point(center.x - px(1.0), center.y - half));
            back.line_to(point(center.x + half, center.y - half));
            builder(back.build());
        }
        CaptionIcon::Maximize => {
            let mut path = gpui::PathBuilder::stroke(stroke_width);
            path.move_to(point(center.x - half, center.y - half));
            path.line_to(point(center.x + half, center.y - half));
            path.line_to(point(center.x + half, center.y + half));
            path.line_to(point(center.x - half, center.y + half));
            path.close();
            builder(path.build());
        }
        CaptionIcon::Close => {
            let mut path = gpui::PathBuilder::stroke(stroke_width);
            path.move_to(point(center.x - half, center.y - half));
            path.line_to(point(center.x + half, center.y + half));
            builder(path.build());
            let mut path = gpui::PathBuilder::stroke(stroke_width);
            path.move_to(point(center.x + half, center.y - half));
            path.line_to(point(center.x - half, center.y + half));
            builder(path.build());
        }
    }
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
        .child(div().text_size(px(12.0)).text_color(colors::TEXT_MUTED).child(tab.title.clone()))
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
                    div().text_size(px(12.0)).text_color(colors::TEXT_SECONDARY).child(format!(
                        "Tab {} of {}",
                        active + 1,
                        tab_count
                    )),
                ),
        )
}
