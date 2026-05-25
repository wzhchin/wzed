use gpui::*;

use similar::{ChangeTag, TextDiff};

pub(crate) struct DiffState {
    pub left_title: SharedString,
    pub right_title: SharedString,
    pub lines: Vec<(DiffLineSide, DiffLineSide)>,
}

pub(crate) struct DiffLineSide {
    pub line_number: Option<usize>,
    pub content: String,
    pub kind: DiffLineKind,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DiffLineKind {
    Normal,
    Added,
    Removed,
}

pub(crate) fn compute_diff(
    left_text: &str,
    right_text: &str,
    left_title: SharedString,
    right_title: SharedString,
) -> DiffState {
    let diff = TextDiff::from_lines(left_text, right_text);
    let changes: Vec<_> = diff.iter_all_changes().collect();

    let mut lines: Vec<(DiffLineSide, DiffLineSide)> = Vec::new();
    let mut left_num = 0usize;
    let mut right_num = 0usize;
    let mut i = 0;

    while i < changes.len() {
        match changes[i].tag() {
            ChangeTag::Equal => {
                left_num += 1;
                right_num += 1;
                let content = changes[i].to_string();
                lines.push((
                    DiffLineSide {
                        line_number: Some(left_num),
                        content: changes[i].to_string(),
                        kind: DiffLineKind::Normal,
                    },
                    DiffLineSide {
                        line_number: Some(right_num),
                        content,
                        kind: DiffLineKind::Normal,
                    },
                ));
                i += 1;
            }
            ChangeTag::Delete => {
                let mut deletes = Vec::new();
                while i < changes.len() && changes[i].tag() == ChangeTag::Delete {
                    left_num += 1;
                    deletes.push((left_num, changes[i].to_string()));
                    i += 1;
                }
                let mut inserts = Vec::new();
                while i < changes.len() && changes[i].tag() == ChangeTag::Insert {
                    right_num += 1;
                    inserts.push((right_num, changes[i].to_string()));
                    i += 1;
                }
                let max_len = deletes.len().max(inserts.len());
                for k in 0..max_len {
                    let left = deletes.get(k).map(|(n, c)| DiffLineSide {
                        line_number: Some(*n),
                        content: c.clone(),
                        kind: DiffLineKind::Removed,
                    });
                    let right = inserts.get(k).map(|(n, c)| DiffLineSide {
                        line_number: Some(*n),
                        content: c.clone(),
                        kind: DiffLineKind::Added,
                    });
                    lines.push((
                        left.unwrap_or(DiffLineSide {
                            line_number: None,
                            content: String::new(),
                            kind: DiffLineKind::Normal,
                        }),
                        right.unwrap_or(DiffLineSide {
                            line_number: None,
                            content: String::new(),
                            kind: DiffLineKind::Normal,
                        }),
                    ));
                }
            }
            ChangeTag::Insert => {
                right_num += 1;
                lines.push((
                    DiffLineSide {
                        line_number: None,
                        content: String::new(),
                        kind: DiffLineKind::Normal,
                    },
                    DiffLineSide {
                        line_number: Some(right_num),
                        content: changes[i].to_string(),
                        kind: DiffLineKind::Added,
                    },
                ));
                i += 1;
            }
        }
    }

    DiffState {
        left_title,
        right_title,
        lines,
    }
}

pub(crate) fn render_diff_view(
    state: &DiffState,
    close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let line_height = 18.0f32;

    let rows: Vec<_> = state
        .lines
        .iter()
        .enumerate()
        .map(|(i, (left, right))| {
            let left_bg = match left.kind {
                DiffLineKind::Normal => gpui::hsla(0.0, 0.0, 0.0, 0.0),
                DiffLineKind::Removed => gpui::hsla(0.0, 0.6, 0.15, 0.35),
                DiffLineKind::Added => gpui::hsla(0.0, 0.0, 0.0, 0.0),
            };
            let right_bg = match right.kind {
                DiffLineKind::Normal => gpui::hsla(0.0, 0.0, 0.0, 0.0),
                DiffLineKind::Added => gpui::hsla(120.0 / 360.0, 0.6, 0.2, 0.3),
                DiffLineKind::Removed => gpui::hsla(0.0, 0.0, 0.0, 0.0),
            };

            let left_num = left
                .line_number
                .map(|n| n.to_string())
                .unwrap_or_default();
            let right_num = right
                .line_number
                .map(|n| n.to_string())
                .unwrap_or_default();
            let left_text = left.content.trim_end().to_string();
            let right_text = right.content.trim_end().to_string();

            div()
                .id(ElementId::Name(format!("diff-row-{i}").into()))
                .flex()
                .flex_row()
                .h(px(line_height))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_row()
                        .bg(left_bg)
                        .child(
                            div()
                                .w(px(40.0))
                                .px(px(4.0))
                                .text_size(px(11.0))
                                .text_color(gpui::hsla(0.0, 0.0, 0.35, 1.0))
                                .child(left_num),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(12.0))
                                .text_color(gpui::hsla(0.0, 0.0, 0.85, 1.0))
                                .text_ellipsis()
                                .overflow_hidden()
                                .child(left_text),
                        ),
                )
                .child(div().w(px(1.0)).bg(gpui::hsla(0.0, 0.0, 0.2, 1.0)))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_row()
                        .bg(right_bg)
                        .child(
                            div()
                                .w(px(40.0))
                                .px(px(4.0))
                                .text_size(px(11.0))
                                .text_color(gpui::hsla(0.0, 0.0, 0.35, 1.0))
                                .child(right_num),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(12.0))
                                .text_color(gpui::hsla(0.0, 0.0, 0.85, 1.0))
                                .text_ellipsis()
                                .overflow_hidden()
                                .child(right_text),
                        ),
                )
        })
        .collect();

    let left_title = state.left_title.clone();
    let right_title = state.right_title.clone();

    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(gpui::hsla(0.0, 0.0, 0.1, 1.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .h(px(28.0))
                .border_b_1()
                .border_color(gpui::hsla(0.0, 0.0, 0.15, 1.0))
                .bg(gpui::hsla(0.0, 0.0, 0.08, 1.0))
                .child(
                    div()
                        .flex_1()
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(gpui::hsla(0.0, 0.0, 0.7, 1.0))
                                .child(left_title),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(gpui::hsla(0.0, 0.0, 0.7, 1.0))
                                .child(right_title),
                        ),
                )
                .child(
                    div()
                        .id("close-diff-btn")
                        .cursor_pointer()
                        .px(px(8.0))
                        .text_size(px(14.0))
                        .text_color(gpui::hsla(0.0, 0.0, 0.5, 1.0))
                        .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.2, 1.0)))
                        .child("x")
                        .on_click(close),
                ),
        )
        .child(
            div()
                .id("diff-content")
                .flex_1()
                .overflow_y_scroll()
                .children(rows),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .justify_end()
                .h(px(22.0))
                .px(px(8.0))
                .items_center()
                .border_t_1()
                .border_color(gpui::hsla(0.0, 0.0, 0.15, 1.0))
                .bg(gpui::hsla(0.0, 0.0, 0.08, 1.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(gpui::hsla(0.0, 0.0, 0.4, 1.0))
                        .child(format!("{} lines", state.lines.len())),
                ),
        )
}
