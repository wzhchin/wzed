use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gpui::*;

use crate::tab::Tab;

pub(crate) struct FileWatcher {
    // The on-disk mtime we last wrote ourselves (or last observed). An external
    // change shows up as a different mtime; matching this entry means the event
    // was caused by our own save and must be ignored.
    known_mtimes: HashMap<PathBuf, std::time::SystemTime>,
}

impl FileWatcher {
    pub(crate) fn new() -> Self {
        Self {
            known_mtimes: HashMap::new(),
        }
    }

    // Record that we just wrote `path` so the watcher can suppress the change
    // event our own save produces (no false "file changed externally" reload).
    pub(crate) fn update_mtime(&mut self, path: &Path) {
        if let Ok(modified) = std::fs::metadata(path).and_then(|m| m.modified()) {
            self.known_mtimes.insert(path.to_path_buf(), modified);
        }
    }

    // Record the current mtime when we start watching a path, so the first event
    // after open isn't mistaken for an external change.
    pub(crate) fn note_current_mtime(&mut self, path: &Path) {
        if let Ok(modified) = std::fs::metadata(path).and_then(|m| m.modified()) {
            self.known_mtimes.entry(path.to_path_buf()).or_insert(modified);
        }
    }

    // Returns true if `path` changed on disk relative to what we last knew — i.e.
    // this is a genuine external change, not our own save echoing back.
    pub(crate) fn is_external_change(&self, path: &Path) -> bool {
        let modified = match std::fs::metadata(path).and_then(|m| m.modified()) {
            Ok(m) => m,
            Err(_) => return false,
        };
        match self.known_mtimes.get(path) {
            Some(known) => *known != modified,
            None => true,
        }
    }

    // Mark the current on-disk mtime as seen so we don't keep re-flagging the
    // same external change.
    pub(crate) fn mark_seen(&mut self, path: &Path) {
        if let Ok(modified) = std::fs::metadata(path).and_then(|m| m.modified()) {
            self.known_mtimes.insert(path.to_path_buf(), modified);
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
