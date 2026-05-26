use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context as _, Result, bail};
use editor::{Editor, EditorMode, MultiBuffer};
use gpui::{self, *};
use gpui::ExternalPaths;
use gpui::prelude::FluentBuilder as _;
use language::{Buffer, LanguageRegistry};
use serde::{Deserialize, Serialize};

use crate::tab::{self, Tab};
use crate::file_watcher::FileWatcher;
use crate::recent_files::RecentFiles;
use crate::encoding;
use crate::diff_view;
use crate::search::SearchState;
use crate::command_center;
use crate::toolbar;

use crate::{
    AutosaveTimer, CloseTab, CompareFiles, FindNext, FindPrevious, MoveToGroup, NewFile, OpenFile, ReplaceAll,
    ReplaceNext, ReloadWithEncoding, SaveAll, SaveFile, SearchAllTabs, ToggleFind,
    ToggleRegex, ToggleReplace, ToggleToolbar,
};

fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("wzed")
}

fn session_path() -> PathBuf {
    config_dir().join("session.json")
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
}

fn save_session(workspace: &LiteWorkspace, cx: &App) {
    let dir = config_dir();
    if let Err(err) = std::fs::create_dir_all(&dir) {
        eprintln!("failed to create config dir: {err:#}");
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

fn chrono_like_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", now.as_secs())
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
    pub(crate) command_center_query: String,
    pub(crate) command_center_selected: usize,
    pub(crate) command_submenu: Option<command_center::CommandSubmenu>,
    diff_state: Option<diff_view::DiffState>,
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
            command_center_query: String::new(),
            command_center_selected: 0,
            command_submenu: None,
            diff_state: None,
        };

        let query_editor = this.search.query_editor.clone();
        cx.observe(&query_editor, move |this, _editor, cx| {
            let active_editor = this.tabs[this.active].editor.clone();
            this.search.run_search(&active_editor, cx);
            cx.notify();
        })
        .detach();

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_secs(30)).await;
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
                cx.background_executor().timer(std::time::Duration::from_secs(5)).await;
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

    pub(crate) fn restore_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = session_path();
        let data = match std::fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => {
                self.tabs.push(Self::create_tab(
                    None,
                    "untitled".into(),
                    String::new(),
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
                window,
                cx,
            ));
            return;
        }

        for (i, tab) in state.tabs.into_iter().enumerate() {
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
                        let title = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "untitled".into());
                        self.tabs.push(Self::create_tab_from_content(
                            Some(path),
                            title.into(),
                            content,
                            &self.languages,
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
                        window,
                        cx,
                    ));
                }
            }
            if i == state.active {
                self.active = self.tabs.len() - 1;
            }
        }

        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }
    }

    fn create_tab(
        path: Option<PathBuf>,
        title: SharedString,
        content: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Tab {
        let buffer = cx.new(|cx| Buffer::local(content, cx));
        let multibuffer = cx.new(|cx| MultiBuffer::singleton(buffer, cx));
        let editor = cx.new(|cx| Editor::new(EditorMode::full(), multibuffer, None, window, cx));
        Tab {
            editor,
            path,
            title,
            group: None,
            encoding: encoding_rs::UTF_8,
        }
    }

    fn create_tab_from_content(
        path: Option<PathBuf>,
        title: SharedString,
        content: String,
        languages: &Arc<LanguageRegistry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Tab {
        let path_for_lang = path.clone();
        let languages = languages.clone();
        let buffer = cx.new(|cx| {
            let buffer = Buffer::local(content, cx);
            buffer.set_language_registry(languages.clone());

            let available = path_for_lang.as_ref().and_then(|p| languages.language_for_file_path(p));
            if let Some(available) = available {
                cx.spawn(async move |buffer: WeakEntity<Buffer>, cx| {
                    let lang = languages.load_language(&available).await??;
                    buffer.update(cx, |buf, cx| buf.set_language(Some(lang), cx))?;
                    Result::<()>::Ok(())
                })
                .detach_and_log_err(cx);
            }
            buffer
        });
        let multibuffer = cx.new(|cx| MultiBuffer::singleton(buffer, cx));
        let editor = cx.new(|cx| Editor::new(EditorMode::full(), multibuffer, None, window, cx));
        Tab {
            editor,
            path,
            title,
            group: None,
            encoding: encoding_rs::UTF_8,
        }
    }

    fn create_empty_editor(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Editor> {
        let buffer = cx.new(|cx| Buffer::local("", cx));
        let multibuffer = cx.new(|cx| MultiBuffer::singleton(buffer, cx));
        cx.new(|cx| Editor::new(EditorMode::full(), multibuffer, None, window, cx))
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
        let title: SharedString = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned().into())
            .unwrap_or("untitled".into());

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

        self.tabs.push(Tab {
            editor,
            path: Some(path.clone()),
            title,
            group: None,
            encoding: detected_encoding,
        });
        self.active = self.tabs.len() - 1;
        self.recent_files.add(&path);
        self.recent_files.save_to_disk();
        self.save_session(cx);
        cx.notify();
        Ok(())
    }

    fn save_active_tab(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let tab = &self.tabs[self.active];
        let path = match &tab.path {
            Some(p) => p.clone(),
            None => bail!("no file path for this tab"),
        };

        let content = tab.editor.read(cx).text(cx);
        std::fs::write(&path, &content)
            .with_context(|| format!("failed to write file: {}", path.display()))?;
        self.file_watcher.update_mtime(&path);
        self.save_session(cx);
        Ok(())
    }

    fn save_active_tab_as(&mut self, path: PathBuf, cx: &mut Context<Self>) -> Result<()> {
        let tab = &mut self.tabs[self.active];
        let content = tab.editor.read(cx).text(cx);
        std::fs::write(&path, &content)
            .with_context(|| format!("failed to write file: {}", path.display()))?;
        tab.path = Some(path.clone());
        tab.title = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned().into())
            .unwrap_or("untitled".into());
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
                    this.open_file_path(path, window, cx).ok();
                }
            }).ok();
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
                }
            }).ok();
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
            let content = tab.editor.read(cx).text(cx);
            if let Err(err) = std::fs::write(&path, &content) {
                eprintln!("failed to save {}: {err:#}", path.display());
            } else {
                self.file_watcher.update_mtime(&path);
            }
        }
        self.save_session(cx);
        cx.notify();
    }

    fn save_dirty_snapshots(&self, cx: &App) {
        let snapshots_dir = config_dir().join("snapshots");
        if let Err(err) = std::fs::create_dir_all(&snapshots_dir) {
            eprintln!("failed to create snapshots dir: {err:#}");
            return;
        }

        for (i, tab) in self.tabs.iter().enumerate() {
            if !tab.is_dirty(cx) {
                continue;
            }

            let content = tab.editor.read(cx).text(cx);
            let timestamp = chrono_like_timestamp();
            let snapshot_name = format!("tab-{i}-{timestamp}.txt");
            if let Err(err) = std::fs::write(snapshots_dir.join(&snapshot_name), &content) {
                eprintln!("autosave snapshot failed: {err:#}");
            }
        }
    }

    fn handle_autosave(
        &mut self,
        _action: &AutosaveTimer,
        cx: &mut Context<Self>,
    ) {
        self.save_dirty_snapshots(cx);
        self.save_session(cx);
    }

    pub(crate) fn handle_new(
        &mut self,
        _action: &NewFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editor = Self::create_empty_editor(window, cx);
        self.tabs.push(Tab {
            editor,
            path: None,
            title: "untitled".into(),
            group: None,
            encoding: encoding_rs::UTF_8,
        });
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
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.remove(self.active);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
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
        tab.group = next.map(|s| SharedString::from(s));
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
        let left_text = self.tabs[self.active].editor.read(cx).text(cx).to_string();
        let left_title = self.tabs[self.active].title.clone();

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
            let right_title: SharedString = right_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned().into())
                .unwrap_or("unknown".into());

            let diff =
                diff_view::compute_diff(&left_text, &right_text, left_title, right_title);

            this.update(cx, |this, cx| {
                this.diff_state = Some(diff);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for LiteWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = &self.tabs[self.active];

        self.last_scrolled_active = self.active;
        let toolbar = self.show_toolbar.then(|| toolbar::render_toolbar(self, cx));

        let tab_infos: Vec<tab::TabInfo> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| tab::TabInfo {
                index,
                title: tab.title.clone(),
                is_active: index == self.active,
                is_dirty: tab.is_dirty(cx),
                group: tab.group.clone(),
            })
            .collect();
        let tab_list = tab::render_tab_list(
            &tab_infos,
            &self.tab_scroll_handle,
            self.last_scrolled_active,
            cx,
        );

        let side_tabs = div()
            .id("side-tabs")
            .flex()
            .flex_col()
            .w(px(180.0))
            .h_full()
            .bg(gpui::hsla(0.0, 0.0, 0.1, 1.0))
            .border_r_1()
            .border_color(gpui::hsla(0.0, 0.0, 0.15, 1.0))
            .child(tab_list)
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
                    .border_color(gpui::hsla(0.0, 0.0, 0.15, 1.0))
                    .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.15, 1.0)))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .text_color(gpui::hsla(0.0, 0.0, 0.6, 1.0))
                            .child("+"),
                    )
                    .on_click(cx.listener(|workspace, _, window, cx| {
                        workspace.handle_new(&NewFile, window, cx);
                    })),
            );

        let encoding_label = encoding::encoding_label(active_tab.encoding);
        let status_bar = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .h(px(24.0))
            .px(px(12.0))
            .bg(gpui::hsla(0.0, 0.0, 0.08, 1.0))
            .border_t_1()
            .border_color(gpui::hsla(0.0, 0.0, 0.15, 1.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(gpui::hsla(0.0, 0.0, 0.6, 1.0))
                    .child(active_tab.title.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(gpui::hsla(0.0, 0.0, 0.5, 1.0))
                            .child(encoding_label),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(gpui::hsla(0.0, 0.0, 0.5, 1.0))
                            .child(format!(
                                "Tab {} of {}",
                                self.active + 1,
                                self.tabs.len()
                            )),
                    ),
            );

        let search_bar = crate::search::render_search_bar(self, cx);
        let multi_tab_results = crate::search::render_multi_tab_results(self, cx);

        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(gpui::hsla(0.0, 0.0, 0.1, 1.0))
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
                el.child(command_center::render_command_center(self, cx))
            })
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                for path in paths.paths() {
                    if path.is_file() {
                        this.open_file_path(path.clone(), window, cx).ok();
                    }
                }
            }))
    }
}
