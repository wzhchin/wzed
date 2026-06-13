use editor::{
    Anchor, Editor, HighlightKey, MultiBufferOffset, SelectionEffects, scroll::Autoscroll,
};
use gpui::*;

use crate::app_theme::colors;
use crate::workspace::LiteWorkspace;
use crate::{FindNext, FindPrevious, ReplaceAll, ReplaceNext, ToggleFind, ToggleRegex};

pub(crate) struct TabSearchResult {
    pub tab_index: usize,
    pub title: SharedString,
    pub match_count: usize,
    pub first_line: String,
}

pub(crate) struct SearchState {
    pub visible: bool,
    pub show_replace: bool,
    pub use_regex: bool,
    pub search_all_tabs: bool,
    pub query_editor: Entity<Editor>,
    pub replace_editor: Entity<Editor>,
    pub matches: Vec<std::ops::Range<usize>>,
    pub current_match: Option<usize>,
    pub tab_results: Vec<TabSearchResult>,
    pub last_error: Option<String>,
}

impl SearchState {
    pub(crate) fn new(window: &mut Window, cx: &mut App) -> Self {
        let query_editor = cx.new(|cx| Editor::single_line(window, cx));
        let replace_editor = cx.new(|cx| Editor::single_line(window, cx));
        Self {
            visible: false,
            show_replace: false,
            use_regex: false,
            search_all_tabs: false,
            query_editor,
            replace_editor,
            matches: Vec::new(),
            current_match: None,
            tab_results: Vec::new(),
            last_error: None,
        }
    }

    pub(crate) fn run_search(&mut self, active_editor: &Entity<Editor>, cx: &mut App) {
        let query = self.query_editor.read(cx).text(cx);

        active_editor.update(cx, |editor, cx| {
            editor.clear_background_highlights(HighlightKey::BufferSearchHighlights, cx);
        });

        if query.is_empty() {
            self.matches.clear();
            self.current_match = None;
            return;
        }

        let text = active_editor.read(cx).text(cx);
        let matches = if self.use_regex {
            match regex::Regex::new(&query) {
                Ok(re) => re.find_iter(&text).map(|m| m.start()..m.end()).collect(),
                Err(_) => Vec::new(),
            }
        } else {
            let mut matches = Vec::new();
            let mut start = 0;
            while let Some(idx) = text[start..].find(&query) {
                let abs_start = start + idx;
                let abs_end = abs_start + query.len();
                matches.push(abs_start..abs_end);
                start = abs_end;
                if start >= text.len() {
                    break;
                }
            }
            matches
        };

        let snapshot = active_editor.read(cx).buffer().read(cx).snapshot(cx);
        let anchor_ranges: Vec<std::ops::Range<Anchor>> = matches
            .iter()
            .map(|range| {
                snapshot.anchor_before(MultiBufferOffset(range.start))
                    ..snapshot.anchor_before(MultiBufferOffset(range.end))
            })
            .collect();

        let highlight_color = colors::SEARCH_CURRENT;
        active_editor.update(cx, |editor, cx| {
            editor.highlight_background(
                HighlightKey::BufferSearchHighlights,
                &anchor_ranges,
                move |_, _| highlight_color,
                cx,
            );
        });

        self.current_match = if matches.is_empty() { None } else { Some(0) };
        self.matches = matches;
    }

    pub(crate) fn clear_highlights(&self, active_editor: &Entity<Editor>, cx: &mut App) {
        active_editor.update(cx, |editor, cx| {
            editor.clear_background_highlights(HighlightKey::BufferSearchHighlights, cx);
        });
    }

    pub(crate) fn navigate_match(
        &mut self,
        direction: isize,
        active_editor: &Entity<Editor>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let match_count = self.matches.len();
        if match_count == 0 {
            return;
        }

        let current = self.current_match.unwrap_or(0);
        let new_index = if direction > 0 {
            (current + direction as usize) % match_count
        } else {
            (current + match_count - (-direction as usize) % match_count) % match_count
        };
        self.current_match = Some(new_index);

        let range = self.matches[new_index].clone();
        let snapshot = active_editor.read(cx).buffer().read(cx).snapshot(cx);
        let start_anchor = snapshot.anchor_before(MultiBufferOffset(range.start));
        let end_anchor = snapshot.anchor_before(MultiBufferOffset(range.end));

        active_editor.update(cx, |editor, cx| {
            editor.change_selections(
                SelectionEffects::scroll(Autoscroll::fit()),
                window,
                cx,
                |s| s.select_ranges(vec![start_anchor..end_anchor]),
            );
        });
    }

    pub(crate) fn replace_current(
        &mut self,
        active_editor: &Entity<Editor>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let current = match self.current_match {
            Some(i) if i < self.matches.len() => i,
            _ => return,
        };
        let range = self.matches[current].clone();
        let replacement = self.replace_editor.read(cx).text(cx);
        self.replace_range(&range, &replacement, active_editor, window, cx);
        self.navigate_match(1, active_editor, window, cx);
        self.run_search(active_editor, cx);
    }

    pub(crate) fn replace_all(&mut self, active_editor: &Entity<Editor>, cx: &mut App) {
        if self.matches.is_empty() {
            return;
        }
        let replacement = self.replace_editor.read(cx).text(cx);
        let matches: Vec<std::ops::Range<usize>> = self.matches.drain(..).collect();
        active_editor.update(cx, |editor, cx| {
            let snapshot = editor.buffer().read(cx).snapshot(cx);
            let edits: Vec<(std::ops::Range<Anchor>, String)> = matches
                .iter()
                .map(|range| {
                    let start = snapshot.anchor_before(MultiBufferOffset(range.start));
                    let end = snapshot.anchor_before(MultiBufferOffset(range.end));
                    (start..end, replacement.clone())
                })
                .collect();
            editor.edit(edits, cx);
        });
        self.current_match = None;
        self.matches.clear();
    }

    fn replace_range(
        &self,
        range: &std::ops::Range<usize>,
        replacement: &str,
        active_editor: &Entity<Editor>,
        _window: &mut Window,
        cx: &mut App,
    ) {
        active_editor.update(cx, |editor, cx| {
            let snapshot = editor.buffer().read(cx).snapshot(cx);
            let start = snapshot.anchor_before(MultiBufferOffset(range.start));
            let end = snapshot.anchor_before(MultiBufferOffset(range.end));
            editor.edit(vec![(start..end, replacement.to_owned())], cx);
        });
    }

    pub(crate) fn run_multi_tab_search(
        &mut self,
        tabs: &[(Entity<Editor>, SharedString)],
        cx: &App,
    ) {
        let query = self.query_editor.read(cx).text(cx);
        if query.is_empty() {
            self.tab_results.clear();
            return;
        }

        let regex = if self.use_regex {
            match regex::Regex::new(&query) {
                Ok(re) => Some(re),
                Err(err) => {
                    self.last_error = Some(format!("Invalid regex: {err}"));
                    return;
                }
            }
        } else {
            None
        };

        self.tab_results = tabs
            .iter()
            .enumerate()
            .filter_map(|(i, (editor, title))| {
                let text = editor.read(cx).text(cx);
                let count = if self.use_regex {
                    regex.as_ref().map(|re| re.find_iter(&text).count()).unwrap_or(0)
                } else {
                    text.matches::<&str>(&query).count()
                };

                if count == 0 {
                    return None;
                }

                let first_line = if self.use_regex {
                    regex
                        .as_ref()
                        .and_then(|re| re.find(&text).map(|m| m.start()))
                        .and_then(|pos| text[..pos].rfind('\n').map(|nl| nl + 1).or(Some(0)))
                        .and_then(|line_start| {
                            text[line_start..]
                                .find('\n')
                                .map(|end| text[line_start..line_start + end].to_string())
                                .or_else(|| Some(text[line_start..].to_string()))
                        })
                        .unwrap_or_default()
                        .chars()
                        .take(80)
                        .collect()
                } else {
                    text.lines()
                        .find(|line| line.contains(&query[..]))
                        .unwrap_or("")
                        .chars()
                        .take(80)
                        .collect()
                };

                Some(TabSearchResult {
                    tab_index: i,
                    title: title.clone(),
                    match_count: count,
                    first_line,
                })
            })
            .collect();
    }
}

pub(crate) fn render_search_bar(
    this: &LiteWorkspace,
    cx: &Context<LiteWorkspace>,
) -> Option<impl IntoElement> {
    if !this.search.visible {
        return None;
    }

    let total = this.search.matches.len();
    let current = this.search.current_match.map(|i| i + 1).unwrap_or(0);
    let match_info = format!("{current}/{total}");

    let find_row = div()
        .id("find-row")
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(36.0))
        .px(px(8.0))
        .gap(px(6.0))
        .child(this.search.query_editor.clone())
        .child(div().text_size(px(12.0)).text_color(colors::TEXT_MUTED).child(match_info))
        .child(
            div()
                .id("find-prev-btn")
                .cursor_pointer()
                .px(px(6.0))
                .py(px(2.0))
                .text_size(px(14.0))
                .text_color(colors::TEXT_DEFAULT)
                .hover(|s| s.bg(colors::BG_HOVER))
                .child("^")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.handle_find_previous(&FindPrevious, window, cx);
                })),
        )
        .child(
            div()
                .id("find-next-btn")
                .cursor_pointer()
                .px(px(6.0))
                .py(px(2.0))
                .text_size(px(14.0))
                .text_color(colors::TEXT_DEFAULT)
                .hover(|s| s.bg(colors::BG_HOVER))
                .child("v")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.handle_find_next(&FindNext, window, cx);
                })),
        )
        .child(
            div()
                .id("find-close-btn")
                .cursor_pointer()
                .px(px(6.0))
                .py(px(2.0))
                .text_size(px(14.0))
                .text_color(colors::TEXT_SECONDARY)
                .hover(|s| s.bg(colors::BG_HOVER))
                .child("x")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.handle_toggle_find(&ToggleFind, window, cx);
                })),
        )
        .child(
            div()
                .id("regex-toggle-btn")
                .cursor_pointer()
                .px(px(6.0))
                .py(px(2.0))
                .text_size(px(12.0))
                .text_color(if this.search.use_regex {
                    colors::SEARCH_MATCH
                } else {
                    colors::TEXT_SECONDARY
                })
                .hover(|s| s.bg(colors::BG_HOVER))
                .child(".*")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.handle_toggle_regex(&ToggleRegex, window, cx);
                })),
        );

    let replace_row = this.search.show_replace.then(|| {
        div()
            .id("replace-row")
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(36.0))
            .px(px(8.0))
            .gap(px(6.0))
            .border_t_1()
            .border_color(colors::BG_RAISED)
            .child(this.search.replace_editor.clone())
            .child(
                div()
                    .id("replace-btn")
                    .cursor_pointer()
                    .px(px(6.0))
                    .py(px(2.0))
                    .text_size(px(12.0))
                    .text_color(colors::TEXT_DEFAULT)
                    .hover(|s| s.bg(colors::BG_HOVER))
                    .child("Replace")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.handle_replace_next(&ReplaceNext, window, cx);
                    })),
            )
            .child(
                div()
                    .id("replace-all-btn")
                    .cursor_pointer()
                    .px(px(6.0))
                    .py(px(2.0))
                    .text_size(px(12.0))
                    .text_color(colors::TEXT_DEFAULT)
                    .hover(|s| s.bg(colors::BG_HOVER))
                    .child("All")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.handle_replace_all(&ReplaceAll, window, cx);
                    })),
            )
    });

    Some(
        div()
            .id("search-bar")
            .flex()
            .flex_col()
            .w_full()
            .bg(colors::BG_PANEL)
            .border_b_1()
            .border_color(colors::BG_BORDER)
            .child(find_row)
            .children(replace_row),
    )
}

pub(crate) fn render_multi_tab_results(
    this: &LiteWorkspace,
    cx: &Context<LiteWorkspace>,
) -> Option<impl IntoElement> {
    if !this.search.search_all_tabs {
        return None;
    }
    let results = &this.search.tab_results;
    if results.is_empty() {
        return None;
    }

    Some(
        div()
            .id("multi-tab-results")
            .flex()
            .flex_col()
            .w_full()
            .max_h(px(200.0))
            .overflow_y_scroll()
            .border_t_1()
            .border_color(colors::BG_BORDER)
            .bg(colors::BG_DEEPEST)
            .children(results.iter().map(|result| {
                let tab_index = result.tab_index;
                div()
                    .id(ElementId::Name(format!("search-result-{tab_index}").into()))
                    .flex()
                    .flex_col()
                    .px(px(10.0))
                    .py(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(colors::BG_BORDER))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(colors::TEXT_PRIMARY)
                                    .child(result.title.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(colors::TEXT_SECONDARY)
                                    .child(format!("{} matches", result.match_count)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(colors::TEXT_DIM)
                            .text_ellipsis()
                            .child(result.first_line.clone()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.active = tab_index;
                        cx.notify();
                    }))
            })),
    )
}
