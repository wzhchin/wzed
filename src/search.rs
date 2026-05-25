use editor::{
    Anchor, Editor, HighlightKey, MultiBufferOffset, SelectionEffects,
    scroll::Autoscroll,
};
use gpui::*;

pub(crate) struct SearchState {
    pub visible: bool,
    pub show_replace: bool,
    pub use_regex: bool,
    pub query_editor: Entity<Editor>,
    pub replace_editor: Entity<Editor>,
    pub matches: Vec<std::ops::Range<usize>>,
    pub current_match: Option<usize>,
}

impl SearchState {
    pub(crate) fn new(window: &mut Window, cx: &mut App) -> Self {
        let query_editor = cx.new(|cx| Editor::single_line(window, cx));
        let replace_editor = cx.new(|cx| Editor::single_line(window, cx));
        Self {
            visible: false,
            show_replace: false,
            use_regex: false,
            query_editor,
            replace_editor,
            matches: Vec::new(),
            current_match: None,
        }
    }

    pub(crate) fn run_search(
        &mut self,
        active_editor: &Entity<Editor>,
        cx: &mut App,
    ) {
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
                Ok(re) => re
                    .find_iter(&text)
                    .map(|m| m.start()..m.end())
                    .collect(),
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

        active_editor.update(cx, |editor, cx| {
            editor.highlight_background(
                HighlightKey::BufferSearchHighlights,
                &anchor_ranges,
                |_, _| gpui::hsla(48.0 / 360.0, 1.0, 0.5, 0.4),
                cx,
            );
        });

        self.current_match = if matches.is_empty() {
            None
        } else {
            Some(0)
        };
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
        let snapshot = active_editor
            .read(cx)
            .buffer()
            .read(cx)
            .snapshot(cx);
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

    pub(crate) fn replace_all(
        &mut self,
        active_editor: &Entity<Editor>,
        cx: &mut App,
    ) {
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
        self.run_search(active_editor, cx);
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
}
