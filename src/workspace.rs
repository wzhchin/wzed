use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context as _, Result, bail};
use editor::{Editor, EditorMode, MultiBuffer};
use gpui::{self, *};
use gpui::ExternalPaths;
use gpui::prelude::FluentBuilder as _;
use language::{Buffer, LanguageRegistry};
use serde::{Deserialize, Serialize};

use crate::tab_groups;
use crate::file_watcher::FileWatcher;
use crate::recent_files::RecentFiles;

use crate::search::SearchState;
use crate::{
    AutosaveTimer, CloseTab, FindNext, FindPrevious, MoveToGroup, NewFile, OpenFile, ReplaceAll,
    ReplaceNext, SaveAll, SaveFile, SearchAllTabs, ToggleFind, ToggleRegex, ToggleReplace,
    ToggleToolbar,
};

// --- Session persistence ---

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
            let unsaved_content = if tab.path.is_none() {
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
    save_session(workspace, cx);
}

fn chrono_like_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", now.as_secs())
}

// --- Tab and Workspace ---

pub(crate) struct Tab {
    pub editor: Entity<Editor>,
    pub path: Option<PathBuf>,
    pub title: SharedString,
    pub group: Option<SharedString>,
}

impl Tab {
    fn is_dirty(&self, cx: &App) -> bool {
        self.editor.read(cx).buffer().read(cx).is_dirty(cx)
    }
}

pub(crate) struct LiteWorkspace {
    tabs: Vec<Tab>,
    pub active: usize,
    languages: Arc<LanguageRegistry>,
    focus_handle: FocusHandle,
    pub search: SearchState,
    show_toolbar: bool,
    file_watcher: FileWatcher,
    recent_files: RecentFiles,
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
        let content = std::fs::read_to_string(&path)
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

        self.tabs.push(Tab {
            editor,
            path: Some(path.clone()),
            title,
            group: None,
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
        self.save_session(cx);
        cx.notify();
        Ok(())
    }

    pub(crate) fn save_session(&self, cx: &App) {
        save_session(self, cx);
    }

    fn handle_open(
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

    fn handle_save(
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

    fn handle_save_all(
        &mut self,
        _action: &SaveAll,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for i in 0..self.tabs.len() {
            let tab = &self.tabs[i];
            let Some(path) = &tab.path else { continue };
            let content = tab.editor.read(cx).text(cx);
            if let Err(err) = std::fs::write(path, &content) {
                eprintln!("failed to save {}: {err:#}", path.display());
            }
        }
        self.save_session(cx);
        cx.notify();
    }

    fn handle_autosave(
        &mut self,
        _action: &AutosaveTimer,
        cx: &mut Context<Self>,
    ) {
        let snapshots_dir = config_dir().join("snapshots");
        if let Err(err) = std::fs::create_dir_all(&snapshots_dir) {
            eprintln!("failed to create snapshots dir: {err:#}");
            return;
        }

        for (i, tab) in self.tabs.iter().enumerate() {
            if !tab.is_dirty(cx) {
                continue;
            }

            if let Some(path) = &tab.path {
                let content = tab.editor.read(cx).text(cx);
                if let Err(err) = std::fs::write(path, &content) {
                    eprintln!("autosave failed for {}: {err:#}", path.display());
                }
            }

            let content = tab.editor.read(cx).text(cx);
            let timestamp = chrono_like_timestamp();
            let snapshot_name = format!("tab-{i}-{timestamp}.txt");
            let _ = std::fs::write(snapshots_dir.join(&snapshot_name), &content);
        }
    }

    fn handle_new(
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
        });
        self.active = self.tabs.len() - 1;
        self.save_session(cx);
        cx.notify();
    }

    fn handle_close_tab(
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

    fn handle_toggle_find(
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

    fn handle_find_next(
        &mut self,
        _action: &FindNext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active_editor = self.tabs[self.active].editor.clone();
        self.search.navigate_match(1, &active_editor, window, cx);
        cx.notify();
    }

    fn handle_find_previous(
        &mut self,
        _action: &FindPrevious,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active_editor = self.tabs[self.active].editor.clone();
        self.search.navigate_match(-1, &active_editor, window, cx);
        cx.notify();
    }

    fn handle_toggle_replace(
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

    fn handle_replace_next(
        &mut self,
        _action: &ReplaceNext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active_editor = self.tabs[self.active].editor.clone();
        self.search.replace_current(&active_editor, window, cx);
        cx.notify();
    }

    fn handle_replace_all(
        &mut self,
        _action: &ReplaceAll,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let active_editor = self.tabs[self.active].editor.clone();
        self.search.replace_all(&active_editor, cx);
        cx.notify();
    }

    fn handle_toggle_regex(
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

    fn handle_search_all_tabs(
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

    fn handle_toggle_toolbar(
        &mut self,
        _action: &ToggleToolbar,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_toolbar = !self.show_toolbar;
        cx.notify();
    }

    fn handle_move_to_group(
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
}

impl Render for LiteWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = &self.tabs[self.active];

        let toolbar = self.show_toolbar.then(|| {
            div()
                .id("toolbar")
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .h(px(32.0))
                .px(px(8.0))
                .gap(px(2.0))
                .bg(gpui::hsla(0.0, 0.0, 0.12, 1.0))
                .border_b_1()
                .border_color(gpui::hsla(0.0, 0.0, 0.15, 1.0))
                .child(toolbar_btn("New", cx.listener(|this, _, window, cx| {
                    this.handle_new(&NewFile, window, cx);
                })))
                .child(toolbar_btn("Open", cx.listener(|this, _, window, cx| {
                    this.handle_open(&OpenFile, window, cx);
                })))
                .child(toolbar_btn("Save", cx.listener(|this, _, window, cx| {
                    this.handle_save(&SaveFile, window, cx);
                })))
                .child(toolbar_separator())
                .child(toolbar_btn("Find", cx.listener(|this, _, window, cx| {
                    this.handle_toggle_find(&ToggleFind, window, cx);
                })))
                .child(toolbar_btn("Replace", cx.listener(|this, _, window, cx| {
                    this.handle_toggle_replace(&ToggleReplace, window, cx);
                })))
        });

        let tab_infos: Vec<tab_groups::TabInfo> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| tab_groups::TabInfo {
                index,
                title: tab.title.clone(),
                is_active: index == self.active,
                is_dirty: tab.is_dirty(cx),
                group: tab.group.clone(),
            })
            .collect();
        let recent_list = self.recent_files.render_list(cx);
        let tab_list = tab_groups::render_tab_list(&tab_infos, cx);

        let side_tabs = div()
            .flex()
            .flex_col()
            .w(px(180.0))
            .h_full()
            .bg(gpui::hsla(0.0, 0.0, 0.1, 1.0))
            .border_r_1()
            .border_color(gpui::hsla(0.0, 0.0, 0.15, 1.0))
            .child(tab_list)
            .children(recent_list)
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
                    .text_size(px(12.0))
                    .text_color(gpui::hsla(0.0, 0.0, 0.5, 1.0))
                    .child(format!(
                        "Tab {} of {}",
                        self.active + 1,
                        self.tabs.len()
                    )),
            );

        let match_info = if self.search.visible {
            let total = self.search.matches.len();
            let current = self.search.current_match.map(|i| i + 1).unwrap_or(0);
            format!("{current}/{total}")
        } else {
            String::new()
        };

        let search_bar = self.search.visible.then(|| {
            let find_row = div()
                .id("find-row")
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .h(px(36.0))
                .px(px(8.0))
                .gap(px(6.0))
                .child(self.search.query_editor.clone())
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(gpui::hsla(0.0, 0.0, 0.6, 1.0))
                        .child(match_info),
                )
                .child(
                    div()
                        .id("find-prev-btn")
                        .cursor_pointer()
                        .px(px(6.0))
                        .py(px(2.0))
                        .text_size(px(14.0))
                        .text_color(gpui::hsla(0.0, 0.0, 0.7, 1.0))
                        .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.2, 1.0)))
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
                        .text_color(gpui::hsla(0.0, 0.0, 0.7, 1.0))
                        .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.2, 1.0)))
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
                        .text_color(gpui::hsla(0.0, 0.0, 0.5, 1.0))
                        .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.2, 1.0)))
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
                        .text_color(if self.search.use_regex {
                            gpui::hsla(48.0 / 360.0, 1.0, 0.6, 1.0)
                        } else {
                            gpui::hsla(0.0, 0.0, 0.5, 1.0)
                        })
                        .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.2, 1.0)))
                        .child(".*")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.handle_toggle_regex(&ToggleRegex, window, cx);
                        })),
                );

            let replace_row = self.search.show_replace.then(|| {
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
                    .border_color(gpui::hsla(0.0, 0.0, 0.12, 1.0))
                    .child(self.search.replace_editor.clone())
                    .child(
                        div()
                            .id("replace-btn")
                            .cursor_pointer()
                            .px(px(6.0))
                            .py(px(2.0))
                            .text_size(px(12.0))
                            .text_color(gpui::hsla(0.0, 0.0, 0.7, 1.0))
                            .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.2, 1.0)))
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
                            .text_color(gpui::hsla(0.0, 0.0, 0.7, 1.0))
                            .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.2, 1.0)))
                            .child("All")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.handle_replace_all(&ReplaceAll, window, cx);
                            })),
                    )
            });

            div()
                .id("search-bar")
                .flex()
                .flex_col()
                .w_full()
                .bg(gpui::hsla(0.0, 0.0, 0.13, 1.0))
                .border_b_1()
                .border_color(gpui::hsla(0.0, 0.0, 0.15, 1.0))
                .child(find_row)
                .children(replace_row)
        });

        div()
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
                            .children(search_bar)
                            .child(
                                div().flex_1().overflow_hidden().child(active_tab.editor.clone()),
                            )
                            .children(self.search.search_all_tabs.then(|| {
                                let results = &self.search.tab_results;
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
                                        .border_color(gpui::hsla(0.0, 0.0, 0.15, 1.0))
                                        .bg(gpui::hsla(0.0, 0.0, 0.08, 1.0))
                                        .children(results.iter().map(|result| {
                                            let tab_index = result.tab_index;
                                            div()
                                                .id(ElementId::Name(format!("search-result-{tab_index}").into()))
                                                .flex()
                                                .flex_col()
                                                .px(px(10.0))
                                                .py(px(4.0))
                                                .cursor_pointer()
                                                .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.15, 1.0)))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_row()
                                                        .gap(px(8.0))
                                                        .child(
                                                            div()
                                                                .text_size(px(12.0))
                                                                .text_color(gpui::hsla(0.0, 0.0, 0.9, 1.0))
                                                                .child(result.title.clone()),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_size(px(11.0))
                                                                .text_color(gpui::hsla(0.0, 0.0, 0.5, 1.0))
                                                                .child(format!("{} matches", result.match_count)),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(11.0))
                                                        .text_color(gpui::hsla(0.0, 0.0, 0.4, 1.0))
                                                        .text_ellipsis()
                                                        .child(result.first_line.clone()),
                                                )
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.active = tab_index;
                                                    cx.notify();
                                                }))
                                        }))
                                )
                            }).flatten()),
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
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                for path in paths.paths() {
                    if path.is_file() {
                        this.open_file_path(path.clone(), window, cx).ok();
                    }
                }
            }))
    }
}

fn toolbar_btn(
    label: &'static str,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(format!("tb-{label}").into()))
        .cursor_pointer()
        .px(px(8.0))
        .py(px(4.0))
        .text_size(px(12.0))
        .text_color(gpui::hsla(0.0, 0.0, 0.7, 1.0))
        .hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.2, 1.0)).text_color(gpui::hsla(0.0, 0.0, 0.9, 1.0)))
        .child(label)
        .on_click(handler)
}

fn toolbar_separator() -> Div {
    div()
        .w(px(1.0))
        .h(px(16.0))
        .mx(px(4.0))
        .bg(gpui::hsla(0.0, 0.0, 0.2, 1.0))
}
