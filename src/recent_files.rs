use std::path::PathBuf;

use gpui::*;

use crate::workspace::LiteWorkspace;

pub(crate) struct RecentFiles {
    pub entries: Vec<PathBuf>,
    max_entries: usize,
}

impl RecentFiles {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 20,
        }
    }

    pub(crate) fn add(&mut self, path: &PathBuf) {
        self.entries.retain(|p| p != path);
        self.entries.insert(0, path.clone());
        self.entries.truncate(self.max_entries);
    }

    pub(crate) fn save_to_disk(&self) {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("wzed");
        if let Err(err) = std::fs::create_dir_all(&dir) {
            eprintln!("failed to create config dir: {err:#}");
            return;
        }
        let paths: Vec<String> = self
            .entries
            .iter()
            .filter_map(|p| p.to_str().map(|s| s.to_owned()))
            .collect();
        if let Err(err) = std::fs::write(
            dir.join("recent.json"),
            serde_json::to_string(&paths).unwrap_or_default(),
        ) {
            eprintln!("failed to save recent files: {err:#}");
        }
    }

    pub(crate) fn load_from_disk() -> Self {
        let path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("wzed")
            .join("recent.json");
        let entries = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
            .map(|v| v.into_iter().map(PathBuf::from).collect())
            .unwrap_or_default();
        Self {
            entries,
            max_entries: 20,
        }
    }

    pub(crate) fn render_list(
        &self,
        cx: &mut Context<LiteWorkspace>,
    ) -> Option<AnyElement> {
        if self.entries.is_empty() {
            return None;
        }

        let items: Vec<AnyElement> = self
            .entries
            .iter()
            .take(10)
            .enumerate()
            .map(|(i, path)| {
                let path = path.clone();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "?".into());
                let dir = path
                    .parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                div()
                    .id(ElementId::Name(format!("recent-{i}").into()))
                    .flex()
                    .flex_col()
                    .px(px(10.0))
                    .py(px(3.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.15, 1.0)))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(gpui::hsla(0.0, 0.0, 0.8, 1.0))
                            .child(name),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(gpui::hsla(0.0, 0.0, 0.4, 1.0))
                            .text_ellipsis()
                            .child(dir),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_file_path(path.clone(), window, cx).ok();
                    }))
                    .into_any_element()
            })
            .collect();

        Some(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(4.0))
                        .text_size(px(10.0))
                        .text_color(gpui::hsla(0.0, 0.0, 0.4, 1.0))
                        .border_t_1()
                        .border_color(gpui::hsla(0.0, 0.0, 0.12, 1.0))
                        .child("Recent"),
                )
                .children(items)
                .into_any_element(),
        )
    }
}
