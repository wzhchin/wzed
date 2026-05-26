use std::path::PathBuf;

use gpui::*;

use crate::tab::Tab;

pub(crate) struct FileWatcher {
    watched: Vec<WatchedFile>,
}

struct WatchedFile {
    path: PathBuf,
    last_modified: Option<std::time::SystemTime>,
}

impl FileWatcher {
    pub(crate) fn new() -> Self {
        Self {
            watched: Vec::new(),
        }
    }

    pub(crate) fn check_for_changes(
        &mut self,
        tabs: &mut [Tab],
        cx: &mut App,
    ) -> Vec<usize> {
        let mut changed = Vec::new();

        for (i, tab) in tabs.iter_mut().enumerate() {
            let Some(path) = &tab.path else { continue };
            if tab.is_dirty(cx) {
                continue;
            }

            let modified = std::fs::metadata(path)
                .ok()
                .and_then(|m| m.modified().ok());

            let entry = self.watched.iter().find(|w| w.path == *path);
            let needs_check = match entry {
                Some(w) => modified != w.last_modified,
                None => true,
            };

            if !needs_check {
                continue;
            }

            let disk_content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let editor_content = tab.editor.read(cx).text(cx);
            if disk_content != editor_content {
                changed.push(i);
            }

            if let Some(entry) = self.watched.iter_mut().find(|w| w.path == *path) {
                entry.last_modified = modified;
            } else {
                self.watched.push(WatchedFile {
                    path: path.clone(),
                    last_modified: modified,
                });
            }
        }

        changed
    }

    pub(crate) fn update_mtime(&mut self, path: &PathBuf) {
        let modified = std::fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok());
        if let Some(entry) = self.watched.iter_mut().find(|w| w.path == *path) {
            entry.last_modified = modified;
        } else {
            self.watched.push(WatchedFile {
                path: path.clone(),
                last_modified: modified,
            });
        }
    }

    pub(crate) fn reload_tab(tab: &mut Tab, window: &mut Window, cx: &mut App) {
        let Some(path) = &tab.path else { return };
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };
        tab.editor.update(cx, |editor, cx| {
            editor.set_text(content.as_str(), window, cx);
        });
    }
}
