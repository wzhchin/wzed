use std::path::PathBuf;

use gpui::*;

use crate::workspace::Tab;

pub(crate) struct FileWatcher {
    watched: Vec<WatchedFile>,
}

struct WatchedFile {
    path: PathBuf,
    last_modified: Option<std::time::SystemTime>,
    last_known_content_hash: u64,
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

            let modified = std::fs::metadata(path)
                .ok()
                .and_then(|m| m.modified().ok());

            let current_hash = simple_hash(&tab.editor.read(cx).text(cx));

            let entry = self.watched.iter().find(|w| w.path == *path);
            let needs_check = match entry {
                Some(w) => {
                    modified != w.last_modified && current_hash == w.last_known_content_hash
                }
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
                entry.last_known_content_hash = simple_hash(&editor_content);
            } else {
                self.watched.push(WatchedFile {
                    path: path.clone(),
                    last_modified: modified,
                    last_known_content_hash: simple_hash(&editor_content),
                });
            }
        }

        changed
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

fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    hash
}
