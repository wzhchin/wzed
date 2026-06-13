use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context as _, Result, bail};
use editor::{Editor, EditorEvent, EditorMode, MultiBuffer};
use language::language_settings::SoftWrap;
use gpui::{self, *};
use gpui::ExternalPaths;
use gpui::prelude::FluentBuilder as _;
use language::{Buffer, LanguageRegistry};
use serde::{Deserialize, Serialize};
use util::ResultExt;

use crate::tab::{self, Tab};
use crate::file_watcher::FileWatcher;
use crate::recent_files::RecentFiles;
use crate::encoding;
use crate::diff_view;
use crate::search::SearchState;
use crate::command_center;
use crate::topbar;
use crate::utils;
use crate::app_theme::colors;

use crate::{
    AutosaveTimer, CloseTab, CompareFiles, Dismiss, FindNext, FindPrevious, MoveToGroup,
    NewFile, OpenFile, ReloadWithEncoding, ReplaceAll, ReplaceNext, SaveAll,
    SaveFile, SearchAllTabs, ToggleFind, ToggleRegex,
    ToggleReplace, ToggleToolbar,
};

fn session_path() -> PathBuf {
    utils::config_dir().join("session.json")
}

#[derive(Serialize, Deserialize)]
struct SessionState {
    tabs: Vec<SessionTab>,
    active: usize,
}

#[derive(Serialize, Deserialize)]
struct SessionTab {
    path: Option<String>,
    unsaved_content: Option<String>,
    pinned: bool,
}

fn save_session(workspace: &LiteWorkspace, cx: &App) {
    if let Err(err) = utils::ensure_config_dir() {
        eprintln!("{err:#}");
        return;
    }

    let tabs: Vec<SessionTab> = workspace
        .tabs
        .iter()
        .map(|tab| {
            let unsaved_content = if tab.path.is_none() || tab.is_dirty(cx) {
                Some(tab.editor.read(cx).text(cx).to_string())
            } else {
                None
            };
            SessionTab {
                path: tab.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                unsaved_content,
                pinned: tab.pinned,
            }
        })
        .collect();

    let state = SessionState {
        tabs,
        active: workspace.active,
    };

    let path = session_path();
    if let Err(err) = (|| -> Result<()> {
        let json = serde_json::to_string_pretty(&state)?;
        let mut tmp = path.clone();
        tmp.set_extension("json.tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    })() {
        eprintln!("failed to save session: {err:#}");
    }
}

pub(crate) fn save_session_from_outside(workspace: &LiteWorkspace, cx: &App) {
    workspace.save_dirty_snapshots(cx);
    save_session(workspace, cx);
}

/// Remove snapshot files older than 7 days to prevent unbounded disk growth.
fn prune_old_snapshots() {
    let snapshots_dir = utils::config_dir().join("snapshots");
    let Ok(entries) = std::fs::read_dir(&snapshots_dir) else {
        return;
    };
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(utils::AppConfig::SNAPSHOT_RETENTION_DAYS * 24 * 3600);
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else { continue };
        if let Ok(modified) = metadata.modified() {
            if modified < cutoff {
                if let Err(err) = std::fs::remove_file(entry.path()) {
                    eprintln!("failed to remove old snapshot: {err:#}");
                }
            }
        }
    }
}

pub(crate) struct LiteWorkspace {
    pub(crate) tabs: Vec<Tab>,
    pub active: usize,
    pub(crate) languages: Arc<LanguageRegistry>,
    pub(crate) focus_handle: FocusHandle,
    pub search: SearchState,
    show_toolbar: bool,
    file_watcher: FileWatcher,
    pub(crate) recent_files: RecentFiles,
    pub(crate) show_recent_menu: bool,
    tab_scroll_handle: ScrollHandle,
    last_scrolled_active: usize,
    pub(crate) show_command_center: bool,
    pub(crate) command_center_editor: Entity<Editor>,
    pub(crate) command_center_selected: usize,
    pub(crate) command_submenu: Option<command_center::CommandSubmenu>,
    pub(crate) diff_state: Option<diff_view::DiffState>,
    pub(crate) show_tab_context_menu: bool,
    pub(crate) context_menu_tab: Option<usize>,
    pub(crate) tab_context_menu_is_pinned: bool,
    pub(crate) context_menu_position: gpui::Point<Pixels>,
    pub(crate) notification: Option<(String, std::time::Instant)>,
}

impl LiteWorkspace {
    pub(crate) fn new(
        languages: Arc<LanguageRegistry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let search = SearchState::new(window, cx);

        let this = Self {
            tabs: Vec::new(),
            active: 0,
            languages,
            focus_handle,
            search,
            show_toolbar: true,
            file_watcher: FileWatcher::new(),
            recent_files: RecentFiles::load_from_disk(),
            show_recent_menu: false,
            tab_scroll_handle: ScrollHandle::new(),
            last_scrolled_active: 0,
            show_command_center: false,
            command_center_editor: cx.new(|cx| Editor::single_line(window, cx)),
            command_center_selected: 0,
            command_submenu: None,
            diff_state: None,
            show_tab_context_menu: false,
            context_menu_tab: None,
            tab_context_menu_is_pinned: false,
            context_menu_position: gpui::Point::new(px(0.0), px(0.0)),
            notification: None,
        };

        let query_editor = this.search.query_editor.clone();
        cx.observe(&query_editor, move |this, _editor, cx| {
            let active_editor = this.tabs[this.active].editor.clone();
            this.search.run_search(&active_editor, cx);
            cx.notify();
        })
        .detach();

        let cc_editor = this.command_center_editor.clone();
        cx.subscribe_in(&cc_editor, window, move |this, _editor, event: &EditorEvent, _window, cx| {
            if matches!(event, EditorEvent::BufferEdited | EditorEvent::Edited { .. }) {
                this.command_center_selected = 0;
                cx.notify();
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_secs(utils::AppConfig::AUTOSAVE_INTERVAL_SECS)).await;
                let Ok(()) = this.update(cx, |this, cx| {
                    this.handle_autosave(&AutosaveTimer, cx);
                }) else {
                    return;
                };
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_secs(utils::AppConfig::FILE_WATCHER_POLL_SECS)).await;
                let Ok(()) = this.update_in(cx, |this, window, cx| {
                    let changed = this.file_watcher.check_for_changes(&mut this.tabs, cx);
                    for idx in changed {
                        FileWatcher::reload_tab(&mut this.tabs[idx], window, cx);
                    }
                }) else {
                    return;
                };
            }
        })
        .detach();

        this
    }

    pub(crate) fn show_notification(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.notification = Some((message.into(), std::time::Instant::now()));
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_secs(utils::AppConfig::NOTIFICATION_DISPLAY_SECS)).await;
            this.update(cx, |this, cx| {
                if this.notification.as_ref().is_some_and(|(_, t)| t.elapsed().as_secs() >= utils::AppConfig::NOTIFICATION_DISPLAY_SECS) {
                    this.notification = None;
                    cx.notify();
                }
            }).log_err();
        }).detach();
    }

    pub(crate) fn restore_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = session_path();
        let data = match std::fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => {
                self.tabs.push(Self::create_tab(
                    None,
                    "untitled".into(),
                    String::new(),
                    None,
                    window,
                    cx,
                ));
                return;
            }
        };

        let state: SessionState = match serde_json::from_str(&data) {
            Ok(s) => s,
            Err(_) => {
                self.tabs.push(Self::create_tab(
                    None,
                    "untitled".into(),
                    String::new(),
                    None,
                    window,
                    cx,
                ));
                return;
            }
        };

        if state.tabs.is_empty() {
            self.tabs.push(Self::create_tab(
                None,
                "untitled".into(),
                String::new(),
                None,
                window,
                cx,
            ));
            return;
        }

        for (i, tab) in state.tabs.into_iter().enumerate() {
            let was_pinned = tab.pinned;
            match tab.path {
                Some(path_str) => {
                    let path = PathBuf::from(&path_str);
                    if path.exists() {
                        if self.open_file_path(path.clone(), window, cx).is_err() {
                            continue;
                        }
                        if let Some(content) = tab.unsaved_content {
                            let last_idx = self.tabs.len() - 1;
                            self.tabs[last_idx].editor.update(cx, |editor, cx| {
                                editor.set_text(content.as_str(), window, cx);
                            });
                        }
                    } else if let Some(content) = tab.unsaved_content {
                        let title = utils::file_name_from_path(&path);
                        self.tabs.push(Self::create_tab(
                            Some(path),
                            title.into(),
                            content,
                            Some(&self.languages),
                            window,
                            cx,
                        ));
                    }
                }
                None => {
                    let content = tab.unsaved_content.unwrap_or_default();
                    self.tabs.push(Self::create_tab(
                        None,
                        "untitled".into(),
                        content,
                        None,
                        window,
                        cx,
                    ));
                }
            }
            if was_pinned
                && let Some(last_tab) = self.tabs.last_mut()
            {
                last_tab.pinned = true;
            }
            if i == state.active {
                self.active = self.tabs.len() - 1;
            }
        }

        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }
        self.sort_tabs_pinned_first();
    }

    fn create_tab(
        path: Option<PathBuf>,
        title: SharedString,
        content: String,
        languages: Option<&Arc<LanguageRegistry>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Tab {
        let buffer = match languages {
            Some(registry) => {
                let path_for_lang = path.clone();
                let registry = registry.clone();
                cx.new(|cx| {
                    let buffer = Buffer::local(content, cx);
                    buffer.set_language_registry(registry.clone());

                    if let Some(available) = path_for_lang.as_ref().and_then(|p| registry.language_for_file_path(p)) {
                        cx.spawn(async move |buffer: WeakEntity<Buffer>, cx| {
                            let lang = registry.load_language(&available).await??;
                            buffer.update(cx, |buf, cx| buf.set_language(Some(lang), cx))?;
                            Result::<()>::Ok(())
                        })
                        .detach_and_log_err(cx);
                    }
                    buffer
                })
            }
            None => cx.new(|cx| Buffer::local(content, cx)),
        };
        let multibuffer = cx.new(|cx| MultiBuffer::singleton(buffer, cx));
        let editor = cx.new(|cx| Editor::new(EditorMode::full(), multibuffer, None, window, cx));
        editor.update(cx, |e, cx| e.set_soft_wrap_mode(SoftWrap::EditorWidth, cx));
        Tab {
            editor,
            path,
            title,
            group: None,
            encoding: encoding_rs::UTF_8,
            pinned: false,
        }
    }

    pub(crate) fn open_file_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        if let Some(idx) = self.tabs.iter().position(|tab| tab.path.as_ref() == Some(&path)) {
            self.active = idx;
            self.recent_files.add(&path);
            self.recent_files.save_to_disk();
            cx.notify();
            return Ok(());
        }

        let (content, detected_encoding) = encoding::read_file_with_detection(&path)
            .with_context(|| format!("failed to read file: {}", path.display()))?;
        let title: SharedString = utils::file_name_from_path(&path).into();

        let languages = self.languages.clone();
        let path_for_lang = path.clone();
        let buffer = cx.new(|cx| {
            let buffer = Buffer::local(content, cx);
            buffer.set_language_registry(languages.clone());

            let available = languages.language_for_file_path(&path_for_lang);
            if let Some(available) = available {
                let lang_name = available.name().to_string();
                cx.spawn(async move |buffer: WeakEntity<Buffer>, cx| {
                    let lang = languages.load_language(&available).await??;
                    eprintln!("loaded language: {lang_name}");
                    buffer.update(cx, |buf, cx| buf.set_language(Some(lang), cx))?;
                    Result::<()>::Ok(())
                })
                .detach_and_log_err(cx);
            } else {
                eprintln!("no language found for: {}", path_for_lang.display());
            }
            buffer
        });

        let multibuffer = cx.new(|cx| MultiBuffer::singleton(buffer, cx));
        let editor = cx.new(|cx| Editor::new(EditorMode::full(), multibuffer, None, window, cx));
        editor.update(cx, |e, cx| e.set_soft_wrap_mode(SoftWrap::EditorWidth, cx));

        self.tabs.push(Tab {
            editor,
            path: Some(path.clone()),
            title,
            group: None,
            encoding: detected_encoding,
            pinned: false,
        });
        self.active = self.tabs.len() - 1;
        self.recent_files.add(&path);
        self.recent_files.save_to_disk();
        self.save_session(cx);
        cx.notify();
        Ok(())
    }

    fn write_editor_to_file(
        editor: &Entity<Editor>, path: &Path, cx: &mut Context<Self>,
    ) -> Result<()> {
        let content = editor.read(cx).text(cx);
        std::fs::write(path, &content)
            .with_context(|| format!("failed to write file: {}", path.display()))?;
        let buffer = editor
            .read(cx)
            .buffer()
            .read(cx)
            .as_singleton()
            .context("expected singleton buffer")?
            .clone();
        let version = buffer.read(cx).snapshot().text.version().clone();
        buffer.update(cx, |buf, cx| {
            buf.did_save(version, None, cx);
        });
        Ok(())
    }

    fn save_active_tab(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let tab = &self.tabs[self.active];
        let path = match &tab.path {
            Some(p) => p.clone(),
            None => bail!("no file path for this tab"),
        };
        Self::write_editor_to_file(&tab.editor, &path, cx)?;
        self.file_watcher.update_mtime(&path);
        self.save_session(cx);
        Ok(())
    }

    pub(crate) fn save_active_tab_as(&mut self, path: PathBuf, cx: &mut Context<Self>) -> Result<()> {
        let tab = &mut self.tabs[self.active];
        Self::write_editor_to_file(&tab.editor, &path, cx)?;
        tab.path = Some(path.clone());
        tab.title = utils::file_name_from_path(&path).into();
        self.recent_files.add(&path);
        self.recent_files.save_to_disk();
        self.file_watcher.update_mtime(&path);
        self.save_session(cx);
        cx.notify();
        Ok(())
    }

    pub(crate) fn save_session(&self, cx: &App) {
        save_session(self, cx);
    }

    pub(crate) fn handle_open(
        &mut self,
        _action: &OpenFile,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Open".into()),
        });
        cx.spawn(async move |this, cx| {
            let result = match receiver.await {
                Ok(Ok(Some(paths))) => paths,
                _ => return,
            };
            this.update_in(cx, |this, window, cx| {
                for path in result {
                    if let Err(err) = this.open_file_path(path, window, cx) {
                        this.show_notification(format!("Failed to open file: {err:#}"), cx);
                    }
                }
            }).log_err();
        }).detach();
    }

    pub(crate) fn handle_save(
        &mut self,
        _action: &SaveFile,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tab = &self.tabs[self.active];
        if tab.path.is_some() {
            if let Err(err) = self.save_active_tab(cx) {
                eprintln!("failed to save: {err:#}");
                self.show_notification(format!("Save failed: {err:#}"), cx);
            }
            return;
        }

        let receiver = cx.prompt_for_new_path(
            Path::new("."),
            Some("untitled"),
        );
        cx.spawn(async move |this, cx| {
            let path = match receiver.await {
                Ok(Ok(Some(path))) => path,
                _ => return,
            };
            this.update(cx, |this, cx| {
                if let Err(err) = this.save_active_tab_as(path, cx) {
                    eprintln!("failed to save as: {err:#}");
                    this.show_notification(format!("Save failed: {err:#}"), cx);
                }
            }).log_err();
        }).detach();
    }

    pub(crate) fn handle_save_all(
        &mut self,
        _action: &SaveAll,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for i in 0..self.tabs.len() {
            let tab = &self.tabs[i];
            let Some(path) = tab.path.clone() else { continue };
            if let Err(err) = Self::write_editor_to_file(&tab.editor, &path, cx) {
                eprintln!("failed to save {}: {err:#}", path.display());
                continue;
            }
            self.file_watcher.update_mtime(&path);
        }
        self.save_session(cx);
        cx.notify();
    }

    fn save_dirty_snapshots(&self, cx: &App) {
        let snapshots_dir = utils::config_dir().join("snapshots");
        if let Err(err) = std::fs::create_dir_all(&snapshots_dir) {
            eprintln!("failed to create snapshots dir: {err:#}");
            return;
        }

        for (i, tab) in self.tabs.iter().enumerate() {
            if !tab.is_dirty(cx) {
                continue;
            }
            self.save_snapshot_for_tab(i, tab, cx);
        }
    }

    fn save_snapshot_for_tab(&self, index: usize, tab: &Tab, cx: &App) {
        let snapshots_dir = utils::config_dir().join("snapshots");
        if let Err(err) = std::fs::create_dir_all(&snapshots_dir) {
            eprintln!("failed to create snapshots dir: {err:#}");
            return;
        }
        let content = tab.editor.read(cx).text(cx);
        let timestamp = utils::timestamp_secs();
        let snapshot_name = format!("tab-{index}-{timestamp}.txt");
        if let Err(err) = std::fs::write(snapshots_dir.join(&snapshot_name), &content) {
            eprintln!("autosave snapshot failed: {err:#}");
        }
    }

    fn handle_autosave(
        &mut self,
        _action: &AutosaveTimer,
        cx: &mut Context<Self>,
    ) {
        self.save_dirty_snapshots(cx);
        self.save_session(cx);
        prune_old_snapshots();
    }

    pub(crate) fn handle_new(
        &mut self,
        _action: &NewFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.tabs.push(Self::create_tab(
            None,
            "untitled".into(),
            String::new(),
            None,
            window,
            cx,
        ));
        self.active = self.tabs.len() - 1;
        self.save_session(cx);
        cx.notify();
    }

    pub(crate) fn handle_close_tab(
        &mut self,
        _action: &CloseTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_tab_at(self.active, cx);
    }

    /// Reorder tabs so all pinned tabs come before unpinned tabs, preserving
    /// relative order within each group. Recalculates `self.active` to track
    /// the same editor after reorder.
    fn sort_tabs_pinned_first(&mut self) {
        let active_id = self.tabs[self.active].editor.entity_id();
        let (pinned, mut unpinned): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.tabs).into_iter().partition(|t| t.pinned);
        self.tabs = pinned;
        self.tabs.append(&mut unpinned);
        self.active = self
            .tabs
            .iter()
            .position(|t| t.editor.entity_id() == active_id)
            .unwrap_or(0);
    }

    pub(crate) fn close_tab_at(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            return;
        }
        if index >= self.tabs.len() {
            return;
        }
        if self.tabs[index].pinned {
            return;
        }
        if self.tabs[index].is_dirty(cx) {
            self.save_snapshot_for_tab(index, &self.tabs[index], cx);
        }
        self.tabs.remove(index);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        } else if index <= self.active && self.active > 0 {
            self.active -= 1;
        }
        self.save_session(cx);
        cx.notify();
    }

    pub(crate) fn handle_toggle_find(
        &mut self,
        _action: &ToggleFind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.search.visible {
            self.search.visible = false;
            let active_editor = &self.tabs[self.active].editor;
            self.search.clear_highlights(active_editor, cx);
            self.search.matches.clear();
            self.search.current_match = None;
            active_editor.update(cx, |_, cx| cx.notify());
        } else {
            self.search.visible = true;
            self.search
                .query_editor
                .update(cx, |editor, cx| {
                    editor.select_all(&editor::actions::SelectAll, window, cx);
                });
            self.search.query_editor.focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn handle_dismiss(
        &mut self,
        _action: &Dismiss,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.diff_state.is_some() {
            self.diff_state = None;
            cx.notify();
            return;
        }
        if self.search.visible {
            self.search.visible = false;
            let active_editor = &self.tabs[self.active].editor;
            self.search.clear_highlights(active_editor, cx);
            self.search.matches.clear();
            self.search.current_match = None;
            active_editor.update(cx, |_, cx| cx.notify());
            cx.notify();
        }
    }

    pub(crate) fn handle_find_next(
        &mut self,
        _action: &FindNext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active_editor = self.tabs[self.active].editor.clone();
        self.search.navigate_match(1, &active_editor, window, cx);
        cx.notify();
    }

    pub(crate) fn handle_find_previous(
        &mut self,
        _action: &FindPrevious,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active_editor = self.tabs[self.active].editor.clone();
        self.search.navigate_match(-1, &active_editor, window, cx);
        cx.notify();
    }

    pub(crate) fn handle_toggle_replace(
        &mut self,
        _action: &ToggleReplace,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.search.visible {
            self.search.visible = true;
        }
        self.search.show_replace = !self.search.show_replace;
        cx.notify();
    }

    pub(crate) fn handle_replace_next(
        &mut self,
        _action: &ReplaceNext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active_editor = self.tabs[self.active].editor.clone();
        self.search.replace_current(&active_editor, window, cx);
        cx.notify();
    }

    pub(crate) fn handle_replace_all(
        &mut self,
        _action: &ReplaceAll,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active_editor = self.tabs[self.active].editor.clone();
        self.search.replace_all(&active_editor, cx);
        cx.notify();
    }

    pub(crate) fn handle_toggle_regex(
        &mut self,
        _action: &ToggleRegex,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.search.use_regex = !self.search.use_regex;
        let active_editor = self.tabs[self.active].editor.clone();
        self.search.run_search(&active_editor, cx);
        cx.notify();
    }

    pub(crate) fn handle_search_all_tabs(
        &mut self,
        _action: &SearchAllTabs,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.search.search_all_tabs = !self.search.search_all_tabs;
        if self.search.search_all_tabs {
            if !self.search.visible {
                self.search.visible = true;
            }
            let tab_info: Vec<(Entity<Editor>, SharedString)> = self
                .tabs
                .iter()
                .map(|t| (t.editor.clone(), t.title.clone()))
                .collect();
            self.search.run_multi_tab_search(&tab_info, cx);
        } else {
            self.search.tab_results.clear();
        }
        cx.notify();
    }

    pub(crate) fn handle_toggle_toolbar(
        &mut self,
        _action: &ToggleToolbar,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_toolbar = !self.show_toolbar;
        cx.notify();
    }

    pub(crate) fn handle_move_to_group(
        &mut self,
        _action: &MoveToGroup,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tab = &mut self.tabs[self.active];
        let groups = [None, Some("A"), Some("B"), Some("C")];
        let current = tab.group.as_deref();
        let next = groups
            .iter()
            .find(|g| g.map(|s| s > current.unwrap_or("")) != Some(true))
            .or_else(|| groups.first())
            .copied()
            .flatten();
        tab.group = next.map(SharedString::from);
        cx.notify();
    }

    pub(crate) fn handle_reload_encoding(
        &mut self,
        _action: &ReloadWithEncoding,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tab = &self.tabs[self.active];
        let Some(path) = tab.path.clone() else { return };
        let current = tab.encoding;
        let current_idx = encoding::SUPPORTED_ENCODINGS
            .iter()
            .position(|e| *e == encoding::encoding_label(current))
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % encoding::SUPPORTED_ENCODINGS.len();
        let label = encoding::SUPPORTED_ENCODINGS[next_idx];
        let Some(enc) = encoding::encoding_from_label(label) else { return };
        let content = match encoding::read_file_as_encoding(&path, enc) {
            Ok(c) => c,
            Err(err) => {
                eprintln!("failed to reload with encoding {label}: {err:#}");
                self.show_notification(format!("Failed to reload: {err:#}"), cx);
                return;
            }
        };
        let tab = &mut self.tabs[self.active];
        tab.encoding = enc;
        tab.editor.update(cx, |editor, cx| {
            editor.set_text(content.as_str(), window, cx);
        });
        cx.notify();
    }

    pub(crate) fn handle_compare_files(
        &mut self,
        _action: &CompareFiles,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        diff_view::start_file_comparison(
            &self.tabs[self.active],
            cx,
        );
    }
}

impl Render for LiteWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }
        let active_tab = &self.tabs[self.active];

        self.last_scrolled_active = self.active;
        let toolbar = self.show_toolbar.then(|| topbar::render_toolbar(self, cx));

        let tab_infos: Vec<tab::TabInfo> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| tab::TabInfo {
                index,
                title: tab.title.clone(),
                is_active: index == self.active,
                is_dirty: tab.is_dirty(cx),
                is_pinned: tab.pinned,
                group: tab.group.clone(),
                file_extension: tab.path.as_ref().and_then(|p| {
                    p.extension().map(|e| e.to_string_lossy().to_lowercase())
                }),
            })
            .collect();
        let tab_list = tab::render_tab_list(
            &tab_infos,
            &self.tab_scroll_handle,
            self.last_scrolled_active,
            cx,
        );

        let show_tab_context_menu = self.show_tab_context_menu;
        let context_menu_tab = self.context_menu_tab;
        let tab_context_menu_is_pinned = self.tab_context_menu_is_pinned;
        let context_menu_y = self.context_menu_position.y;

        let side_tabs = div()
            .id("side-tabs")
            .flex()
            .flex_col()
            .w(px(180.0))
            .h_full()
            .bg(colors::BG_BASE)
            .border_r_1()
            .border_color(colors::BG_BORDER)
            .child(tab_list)
            .when(show_tab_context_menu, |el| {
                el.child(
                    div()
                        .absolute()
                        .left(px(10.0))
                        .top(context_menu_y)
                        .bg(colors::BG_PANEL)
                        .border_1()
                        .border_color(colors::BG_BORDER_STRONG)
                        .rounded(px(6.0))
                        .shadow(vec![gpui::BoxShadow {
                            color: colors::SHADOW,
                            offset: point(px(0.0), px(4.0)),
                            blur_radius: px(12.0),
                            spread_radius: px(0.0),
                        }])
                        .py(px(4.0))
                        .min_w(px(140.0))
                        .child(
                            div()
                                .id("ctx-pin")
                                .flex()
                                .items_center()
                                .px(px(10.0))
                                .py(px(6.0))
                                .cursor_pointer()
                                .text_size(px(13.0))
                                .text_color(colors::TEXT_BRIGHT)
                                .hover(|s| s.bg(colors::BG_HOVER_DEEP))
                                .child(if tab_context_menu_is_pinned {
                                    "Unpin Tab"
                                } else {
                                    "Pin Tab"
                                })
                                .on_click(cx.listener(move |workspace, _, _window, cx| {
                                    let Some(tab_idx) = context_menu_tab else {
                                        return;
                                    };
                                    let Some(tab) = workspace.tabs.get_mut(tab_idx) else {
                                        return;
                                    };
                                    tab.pinned = !tab.pinned;
                                    workspace.sort_tabs_pinned_first();
                                    workspace.show_tab_context_menu = false;
                                    workspace.save_session(cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .id("ctx-close")
                                .flex()
                                .items_center()
                                .px(px(10.0))
                                .py(px(6.0))
                                .cursor_pointer()
                                .text_size(px(13.0))
                                .text_color(colors::TEXT_BRIGHT)
                                .hover(|s| s.bg(colors::BG_HOVER_DEEP))
                                .child("Close Tab")
                                .on_click(cx.listener(move |workspace, _, _window, cx| {
                                    let Some(tab_idx) = context_menu_tab else {
                                        return;
                                    };
                                    workspace.show_tab_context_menu = false;
                                    workspace.close_tab_at(tab_idx, cx);
                                })),
                        ),
                )
            })
            .child(
                div()
                    .id("new-tab-btn")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .h(px(32.0))
                    .cursor_pointer()
                    .border_t_1()
                    .border_color(colors::BG_BORDER)
                    .hover(|s| s.bg(colors::BG_BORDER))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .text_color(colors::TEXT_MUTED)
                            .child("+"),
                    )
                    .on_click(cx.listener(|workspace, _, window, cx| {
                        workspace.handle_new(&NewFile, window, cx);
                    })),
            );

        let status_bar = topbar::render_status_bar(active_tab, self.active, self.tabs.len());

        let search_bar = crate::search::render_search_bar(self, cx).map(|el| el.into_any_element());
        let multi_tab_results = crate::search::render_multi_tab_results(self, cx);

        div()
            .id("workspace-root")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::BG_BASE)
            .on_click(cx.listener(|workspace, _event, _window, cx| {
                if workspace.show_tab_context_menu {
                    workspace.show_tab_context_menu = false;
                    cx.notify();
                }
            }))
            .children(toolbar)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .overflow_hidden()
                    .child(side_tabs)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .overflow_hidden()
                            .children(if self.diff_state.is_some() { None } else { search_bar })
                            .child(
                                if let Some(ref diff) = self.diff_state {
                                    diff_view::render_diff_view(
                                        diff,
                                        cx.listener(|this, _event, _window, cx| {
                                            this.diff_state = None;
                                            cx.notify();
                                        }),
                                    )
                                    .into_any_element()
                                } else {
                                    div().flex_1().overflow_hidden().child(active_tab.editor.clone()).into_any_element()
                                }
                            )
                            .children(multi_tab_results),
                    ),
            )
            .child(status_bar)
            .key_context("LiteWorkspace")
            .on_action(cx.listener(Self::handle_open))
            .on_action(cx.listener(Self::handle_save))
            .on_action(cx.listener(Self::handle_new))
            .on_action(cx.listener(Self::handle_close_tab))
            .on_action(cx.listener(Self::handle_toggle_find))
            .on_action(cx.listener(Self::handle_dismiss))
            .on_action(cx.listener(Self::handle_find_next))
            .on_action(cx.listener(Self::handle_find_previous))
            .on_action(cx.listener(Self::handle_toggle_replace))
            .on_action(cx.listener(Self::handle_replace_next))
            .on_action(cx.listener(Self::handle_replace_all))
            .on_action(cx.listener(Self::handle_toggle_regex))
            .on_action(cx.listener(Self::handle_search_all_tabs))
            .on_action(cx.listener(Self::handle_save_all))
            .on_action(cx.listener(Self::handle_toggle_toolbar))
            .on_action(cx.listener(Self::handle_move_to_group))
            .on_action(cx.listener(Self::handle_reload_encoding))
            .on_action(cx.listener(Self::handle_compare_files))
            .on_action(cx.listener(Self::handle_toggle_command_center))
            .when(self.show_command_center, |el| {
                el.child(command_center::render_command_center(self, _window, cx))
            })
            .when(
                self.notification.as_ref().is_some_and(|(_, instant)| instant.elapsed().as_secs() < utils::AppConfig::NOTIFICATION_DISPLAY_SECS),
                |el| {
                    let msg = self.notification.as_ref().map(|(m, _)| m.clone()).unwrap_or_default();
                    el.child(
                        div()
                            .absolute()
                            .bottom(px(36.0))
                            .right(px(12.0))
                            .px(px(12.0))
                            .py(px(8.0))
                            .bg(colors::BG_PANEL)
                            .border_1()
                            .border_color(colors::ACCENT)
                            .rounded(px(6.0))
                            .shadow_lg()
                            .text_size(px(13.0))
                            .text_color(colors::TEXT_PRIMARY)
                            .max_w(px(400.0))
                            .child(msg),
                    )
                },
            )
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                for path in paths.paths() {
                    if path.is_file() {
                        if let Err(err) = this.open_file_path(path.clone(), window, cx) {
                            this.show_notification(format!("Failed to open file: {err:#}"), cx);
                        }
                    }
                }
            }))
    }
}
