use gpui::*;
use similar::{ChangeTag, TextDiff};
use util::ResultExt;

use crate::app_theme::colors;
use crate::tab::Tab;
use crate::utils;

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

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum DiffLineKind {
    Normal,
    Added,
    Removed,
}

/// Prompt for a file and show side-by-side diff against the given tab.
pub(crate) fn start_file_comparison(
    active_tab: &Tab,
    cx: &mut Context<crate::workspace::LiteWorkspace>,
) {
    let left_text = active_tab.editor.read(cx).text(cx).to_string();
    let left_title = active_tab.title.clone();

    let receiver = cx.prompt_for_paths(PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some("Compare".into()),
    });

    cx.spawn(async move |this, cx| {
        let paths = match receiver.await {
            Ok(Ok(Some(paths))) => paths,
            _ => return,
        };
        let right_path = match paths.into_iter().next() {
            Some(p) => p,
            None => return,
        };

        let right_text = match std::fs::read_to_string(&right_path) {
            Ok(t) => t,
            Err(err) => {
                eprintln!("failed to read file for comparison: {err:#}");
                return;
            }
        };
        let right_title: SharedString = utils::file_name_from_path(&right_path).into();

        let diff = compute_diff(&left_text, &right_text, left_title, right_title);

        this.update(cx, |this, cx| {
            this.diff_state = Some(diff);
            cx.notify();
        })
        .log_err();
    })
    .detach();
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
                DiffLineKind::Normal => colors::TRANSPARENT,
                DiffLineKind::Removed => colors::DIFF_REMOVED_BG,
                DiffLineKind::Added => colors::TRANSPARENT,
            };
            let right_bg = match right.kind {
                DiffLineKind::Normal => colors::TRANSPARENT,
                DiffLineKind::Added => colors::DIFF_ADDED_BG,
                DiffLineKind::Removed => colors::TRANSPARENT,
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
                                .text_color(colors::TEXT_MUTED)
                                .child(left_num),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(12.0))
                                .text_color(colors::TEXT_BRIGHT)
                                .text_ellipsis()
                                .overflow_hidden()
                                .child(left_text),
                        ),
                )
                .child(div().w(px(1.0)).bg(colors::BG_HOVER))
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
                                .text_color(colors::TEXT_MUTED)
                                .child(right_num),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(12.0))
                                .text_color(colors::TEXT_BRIGHT)
                                .text_ellipsis()
                                .overflow_hidden()
                                .child(right_text),
                        ),
                )
        })
        .collect();

    let left_title = state.left_title.clone();
    let right_title = state.right_title.clone();

    let mut added = 0usize;
    let mut removed = 0usize;
    for (left, right) in &state.lines {
        if left.kind == DiffLineKind::Removed { removed += 1 }
        if right.kind == DiffLineKind::Added { added += 1 }
    }

    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(colors::BG_BASE)
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .h(px(28.0))
                .border_b_1()
                .border_color(colors::BG_BORDER)
                .bg(colors::BG_DEEPEST)
                .child(
                    div()
                        .flex_1()
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(colors::TEXT_DEFAULT)
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
                                .text_color(colors::TEXT_DEFAULT)
                                .child(right_title),
                        ),
                )
                .child(
                    div()
                        .id("close-diff-btn")
                        .cursor_pointer()
                        .px(px(8.0))
                        .text_size(px(14.0))
                        .text_color(colors::TEXT_SECONDARY)
                        .hover(|s| s.bg(colors::BG_HOVER))
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
                .gap(px(12.0))
                .h(px(22.0))
                .px(px(8.0))
                .items_center()
                .border_t_1()
                .border_color(colors::BG_BORDER)
                .bg(colors::BG_DEEPEST)
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(colors::TEXT_DIM)
                        .child(format!("{} lines", state.lines.len())),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(colors::ACCENT)
                        .child(format!("+{}", added)),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(Hsla { h: 0.0, s: 0.6, l: 0.55, a: 1.0 })
                        .child(format!("-{}", removed)),
                ),
        )
}
