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
use crate::encoding;
use crate::diff_view;

use crate::search::SearchState;
use crate::{
    AutosaveTimer, CloseTab, CompareFiles, FindNext, FindPrevious, MoveToGroup, NewFile, OpenFile, ReplaceAll,
    ReplaceNext, ReloadWithEncoding, SaveAll, SaveFile, SearchAllTabs, ToggleCommandCenter, ToggleFind,
    ToggleRegex, ToggleReplace, ToggleToolbar,
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

#[derive(Clone)]
enum CommandSubmenu {
    SwitchBuffer,
    ChangeEncoding,
    ChangeFileType,
    RecentFiles,
}

// --- Tab and Workspace ---

pub(crate) struct Tab {
    pub editor: Entity<Editor>,
    pub path: Option<PathBuf>,
    pub title: SharedString,
    pub group: Option<SharedString>,
    pub encoding: &'static encoding_rs::Encoding,
}

impl Tab {
    fn is_dirty(&self, cx: &App) -> bool {
        self.editor.read(cx).buffer().read(cx).is_dirty(cx)
    }
}

pub(crate) struct LiteWorkspace {
    pub(crate) tabs: Vec<Tab>,
    pub active: usize,
    languages: Arc<LanguageRegistry>,
    focus_handle: FocusHandle,
    pub search: SearchState,
    show_toolbar: bool,
    file_watcher: FileWatcher,
    recent_files: RecentFiles,
    show_recent_menu: bool,
    tab_scroll_handle: ScrollHandle,
    show_command_center: bool,
    command_center_query: String,
    command_center_selected: usize,
    command_submenu: Option<CommandSubmenu>,
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
            encoding: encoding_rs::UTF_8,
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

    fn handle_toggle_command_center(
        &mut self,
        _action: &ToggleCommandCenter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_command_center = !self.show_command_center;
        self.command_center_query.clear();
        self.command_center_selected = 0;
        self.command_submenu = None;
        if self.show_command_center {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    fn execute_command(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        match name {
            "Switch Buffer" => {
                self.command_submenu = Some(CommandSubmenu::SwitchBuffer);
                self.command_center_query.clear();
                self.command_center_selected = 0;
                cx.notify();
                return;
            }
            "Switch Encoding" => {
                self.command_submenu = Some(CommandSubmenu::ChangeEncoding);
                self.command_center_query.clear();
                self.command_center_selected = 0;
                cx.notify();
                return;
            }
            "Change File Type" => {
                self.command_submenu = Some(CommandSubmenu::ChangeFileType);
                self.command_center_query.clear();
                self.command_center_selected = 0;
                cx.notify();
                return;
            }
            "Recent Files" => {
                self.command_submenu = Some(CommandSubmenu::RecentFiles);
                self.command_center_query.clear();
                self.command_center_selected = 0;
                cx.notify();
                return;
            }
            _ => {}
        }
        self.show_command_center = false;
        self.command_center_query.clear();
        self.command_submenu = None;
        match name {
            "New File" => self.handle_new(&NewFile, window, cx),
            "Open File" => self.handle_open(&OpenFile, window, cx),
            "Save File" => self.handle_save(&SaveFile, window, cx),
            "Save All" => self.handle_save_all(&SaveAll, window, cx),
            "Close Tab" => self.handle_close_tab(&CloseTab, window, cx),
            "Find" => self.handle_toggle_find(&ToggleFind, window, cx),
            "Find Next" => self.handle_find_next(&FindNext, window, cx),
            "Find Previous" => self.handle_find_previous(&FindPrevious, window, cx),
            "Replace" => self.handle_toggle_replace(&ToggleReplace, window, cx),
            "Replace Next" => self.handle_replace_next(&ReplaceNext, window, cx),
            "Replace All" => self.handle_replace_all(&ReplaceAll, window, cx),
            "Toggle Regex" => self.handle_toggle_regex(&ToggleRegex, window, cx),
            "Search All Tabs" => self.handle_search_all_tabs(&SearchAllTabs, window, cx),
            "Toggle Toolbar" => self.handle_toggle_toolbar(&ToggleToolbar, window, cx),
            "Move to Group" => self.handle_move_to_group(&MoveToGroup, window, cx),
            "Compare Files" => self.handle_compare_files(&CompareFiles, window, cx),
            _ => {}
        }
    }

    fn execute_submenu_item(
        &mut self,
        submenu: &CommandSubmenu,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_command_center = false;
        self.command_center_query.clear();
        self.command_submenu = None;
        match submenu {
            CommandSubmenu::SwitchBuffer => {
                if index < self.tabs.len() {
                    self.active = index;
                    cx.notify();
                }
            }
            CommandSubmenu::ChangeEncoding => {
                let encodings = encoding::SUPPORTED_ENCODINGS;
                if let Some(&label) = encodings.get(index) {
                    if let Some(enc) = encoding::encoding_from_label(label) {
                        let tab = &self.tabs[self.active];
                        if let Some(path) = tab.path.clone() {
                            if let Ok(content) = encoding::read_file_as_encoding(&path, enc) {
                                let tab = &mut self.tabs[self.active];
                                tab.encoding = enc;
                                tab.editor.update(cx, |editor, cx| {
                                    editor.set_text(content.as_str(), window, cx);
                                });
                            }
                        }
                    }
                }
                cx.notify();
            }
            CommandSubmenu::ChangeFileType => {
                let grammar_names = [
                    "bash", "c", "cpp", "css", "diff", "go", "json", "jsonc", "markdown",
                    "python", "regex", "rust", "tsx", "typescript", "yaml",
                ];
                if let Some(&grammar_name) = grammar_names.get(index) {
                    let languages = self.languages.clone();
                    let buffer = {
                        let tab = &self.tabs[self.active];
                        tab.editor.read(cx).buffer().read(cx).as_singleton().map(|b| b.clone())
                    };
                    if let Some(buffer) = buffer {
                        cx.spawn(async move |_, cx| {
                            let lang = languages.language_for_name(grammar_name).await?;
                            buffer.update(cx, |buf, cx| {
                                buf.set_language(Some(lang), cx);
                            });
                            Result::<()>::Ok(())
                        })
                        .detach_and_log_err(cx);
                    }
                }
                cx.notify();
            }
            CommandSubmenu::RecentFiles => {
                if let Some(path) = self.recent_files.entries.get(index).cloned() {
                    self.open_file_path(path, window, cx).ok();
                }
            }
        }
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

    fn handle_reload_encoding(
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

    fn handle_compare_files(
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
                .child(toolbar_separator())
                .child(toolbar_btn("Compare", cx.listener(|this, _, window, cx| {
                    this.handle_compare_files(&CompareFiles, window, cx);
                })))
                .child(toolbar_separator())
                .child({
                    let is_open = self.show_recent_menu;
                    let entries: Vec<(PathBuf, String, String)> = self
                        .recent_files
                        .entries
                        .iter()
                        .take(15)
                        .map(|path| {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "?".into());
                            let dir = path
                                .parent()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default();
                            (path.clone(), name, dir)
                        })
                        .collect();

                    div()
                        .id(ElementId::Name("tb-recent".into()))
                        .cursor_pointer()
                        .px(px(8.0))
                        .py(px(4.0))
                        .text_size(px(12.0))
                        .text_color(if is_open {
                            gpui::hsla(0.0, 0.0, 0.9, 1.0)
                        } else {
                            gpui::hsla(0.0, 0.0, 0.7, 1.0)
                        })
                        .hover(|s| {
                            s.bg(gpui::hsla(0.0, 0.0, 0.2, 1.0))
                                .text_color(gpui::hsla(0.0, 0.0, 0.9, 1.0))
                        })
                        .child("Recent")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_recent_menu = !this.show_recent_menu;
                            cx.notify();
                        }))
                        .when(is_open, |el| {
                            el.child(
                                deferred(
                                    anchored()
                                        .anchor(Anchor::TopLeft)
                                        .snap_to_window_with_margin(px(8.))
                                        .child(
                                            div()
                                                .id("recent-dropdown")
                                                .occlude()
                                                .flex()
                                                .flex_col()
                                                .w(px(280.0))
                                                .max_h(px(400.0))
                                                .overflow_y_scroll()
                                                .bg(gpui::hsla(0.0, 0.0, 0.15, 1.0))
                                                .border_1()
                                                .border_color(
                                                    gpui::hsla(0.0, 0.0, 0.25, 1.0),
                                                )
                                                .rounded(px(4.0))
                                                .shadow_lg()
                                                .on_mouse_down_out(cx.listener(
                                                    |this, _, _, cx| {
                                                        this.show_recent_menu = false;
                                                        cx.notify();
                                                    },
                                                ))
                                                .when(entries.is_empty(), |el| {
                                                    el.child(
                                                        div()
                                                            .px(px(12.0))
                                                            .py(px(8.0))
                                                            .text_size(px(12.0))
                                                            .text_color(gpui::hsla(
                                                                0.0, 0.0, 0.5, 1.0,
                                                            ))
                                                            .child("No recent files"),
                                                    )
                                                })
                                                .children(
                                                    entries.into_iter().enumerate().map(
                                                        |(i, (path, name, dir))| {
                                                            div()
                                                                .id(ElementId::Name(
                                                                    format!("recent-menu-{i}")
                                                                        .into(),
                                                                ))
                                                                .flex()
                                                                .flex_col()
                                                                .px(px(12.0))
                                                                .py(px(6.0))
                                                                .cursor_pointer()
                                                                .hover(|s| {
                                                                    s.bg(gpui::hsla(
                                                                        0.0, 0.0, 0.22, 1.0,
                                                                    ))
                                                                })
                                                                .child(
                                                                    div()
                                                                        .text_size(px(13.0))
                                                                        .text_color(
                                                                            gpui::hsla(
                                                                                0.0, 0.0, 0.9,
                                                                                1.0,
                                                                            ),
                                                                        )
                                                                        .child(name),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_size(px(10.0))
                                                                        .text_color(
                                                                            gpui::hsla(
                                                                                0.0, 0.0, 0.5,
                                                                                1.0,
                                                                            ),
                                                                        )
                                                                        .text_ellipsis()
                                                                        .child(dir),
                                                                )
                                                                .on_click(cx.listener(
                                                                    move |this,
                                                                          _,
                                                                          window,
                                                                          cx| {
                                                                        this.show_recent_menu =
                                                                            false;
                                                                        this.open_file_path(
                                                                            path.clone(),
                                                                            window,
                                                                            cx,
                                                                        )
                                                                        .ok();
                                                                    },
                                                                ))
                                                        },
                                                    ),
                                                ),
                                        ),
                                )
                                .priority(1),
                            )
                        })
                })
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
        let tab_list = tab_groups::render_tab_list(&tab_infos, &self.tab_scroll_handle, cx);

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
            .on_action(cx.listener(Self::handle_reload_encoding))
            .on_action(cx.listener(Self::handle_compare_files))
            .on_action(cx.listener(Self::handle_toggle_command_center))
            .when(self.show_command_center, |el| {
                let main_commands = [
                    "New File",
                    "Open File",
                    "Recent Files",
                    "Save File",
                    "Save All",
                    "Close Tab",
                    "Switch Buffer",
                    "Find",
                    "Find Next",
                    "Find Previous",
                    "Replace",
                    "Replace Next",
                    "Replace All",
                    "Toggle Regex",
                    "Search All Tabs",
                    "Toggle Toolbar",
                    "Move to Group",
                    "Switch Encoding",
                    "Change File Type",
                    "Compare Files",
                ];
                let submenu_items: Vec<String> = match &self.command_submenu {
                    Some(CommandSubmenu::SwitchBuffer) => {
                        self.tabs.iter().map(|t| t.title.to_string()).collect()
                    }
                    Some(CommandSubmenu::ChangeEncoding) => {
                        encoding::SUPPORTED_ENCODINGS.iter().map(|s| s.to_string()).collect()
                    }
                    Some(CommandSubmenu::ChangeFileType) => {
                        ["Bash", "C", "C++", "CSS", "Diff", "Go", "JSON", "JSONC", "Markdown",
                         "Python", "Regex", "Rust", "TSX", "TypeScript", "YAML",
                        ].iter().map(|s| s.to_string()).collect()
                    }
                    Some(CommandSubmenu::RecentFiles) => {
                        self.recent_files.entries.iter().take(20)
                            .filter_map(|p| p.to_str().map(|s| s.to_string()))
                            .collect()
                    }
                    None => Vec::new(),
                };
                let submenu_title: Option<&str> = match &self.command_submenu {
                    Some(CommandSubmenu::SwitchBuffer) => Some("Switch Buffer"),
                    Some(CommandSubmenu::ChangeEncoding) => Some("Switch Encoding"),
                    Some(CommandSubmenu::ChangeFileType) => Some("Change File Type"),
                    Some(CommandSubmenu::RecentFiles) => Some("Recent Files"),
                    None => None,
                };
                let submenu_clone = self.command_submenu.clone();
                let is_submenu = self.command_submenu.is_some();
                let filtered: Vec<(usize, String)> = if is_submenu {
                    submenu_items
                        .iter()
                        .enumerate()
                        .filter(|(_, item)| {
                            if self.command_center_query.is_empty() {
                                true
                            } else {
                                item.to_lowercase()
                                    .contains(&self.command_center_query.to_lowercase())
                            }
                        })
                        .map(|(i, s)| (i, s.clone()))
                        .collect()
                } else {
                    main_commands
                        .iter()
                        .enumerate()
                        .filter(|(_, cmd)| {
                            if self.command_center_query.is_empty() {
                                true
                            } else {
                                cmd.to_lowercase()
                                    .contains(&self.command_center_query.to_lowercase())
                            }
                        })
                        .map(|(i, s)| (i, s.to_string()))
                        .collect()
                };
                let selected = self.command_center_selected.min(filtered.len().saturating_sub(1));
                let selected_cmd = filtered
                    .get(selected)
                    .map(|(_, c)| c.clone())
                    .unwrap_or_default();

                el.child(
                    div()
                        .id("command-center-overlay")
                        .absolute()
                        .top(px(0.0))
                        .left(px(0.0))
                        .size_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .pt(px(150.0))
                        .child(
                            div()
                                .id("command-center")
                                .occlude()
                                .focusable()
                                .track_focus(&self.focus_handle)
                                .flex()
                                .flex_col()
                                .w(px(400.0))
                                .max_h(px(500.0))
                                .bg(gpui::hsla(0.0, 0.0, 0.13, 1.0))
                                .border_1()
                                .border_color(gpui::hsla(0.0, 0.0, 0.3, 1.0))
                                .rounded(px(8.0))
                                .shadow_lg()
                                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                    this.show_command_center = false;
                                    cx.notify();
                                }))
                                .on_key_down({
                                    let filtered_for_keys = filtered.clone();
                                    let submenu_for_keys = submenu_clone.clone();
                                    cx.listener(
                                        move |this, event: &KeyDownEvent, window, cx| {
                                        match event.keystroke.key.as_str() {
                                            "escape" => {
                                                if this.command_submenu.is_some() {
                                                    this.command_submenu = None;
                                                    this.command_center_query.clear();
                                                    this.command_center_selected = 0;
                                                } else {
                                                    this.show_command_center = false;
                                                }
                                                cx.notify();
                                            }
                                            "up" => {
                                                this.command_center_selected =
                                                    this.command_center_selected.saturating_sub(1);
                                                cx.notify();
                                            }
                                            "down" => {
                                                this.command_center_selected += 1;
                                                cx.notify();
                                            }
                                            "enter" => {
                                                if is_submenu {
                                                    let selected_idx = filtered_for_keys
                                                        .get(this.command_center_selected)
                                                        .map(|(i, _)| *i)
                                                        .unwrap_or(0);
                                                    if let Some(ref sub) = submenu_for_keys {
                                                        this.execute_submenu_item(
                                                            sub,
                                                            selected_idx,
                                                            window,
                                                            cx,
                                                        );
                                                    }
                                                } else {
                                                    this.execute_command(
                                                        &selected_cmd,
                                                        window,
                                                        cx,
                                                    );
                                                }
                                            }
                                            "backspace" => {
                                                this.command_center_query.pop();
                                                this.command_center_selected = 0;
                                                cx.notify();
                                            }
                                            _ => {
                                                if let Some(ch) = event.keystroke.key.chars().next()
                                                {
                                                    if ch.is_alphanumeric()
                                                        || ch == ' '
                                                        || ch == '-'
                                                    {
                                                        this.command_center_query.push(ch);
                                                        this.command_center_selected = 0;
                                                        cx.notify();
                                                    }
                                                }
                                            }
                                        }
                                    },
                                )
                                })
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .px(px(12.0))
                                        .py(px(10.0))
                                        .border_b_1()
                                        .border_color(gpui::hsla(0.0, 0.0, 0.2, 1.0))
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .text_color(gpui::hsla(0.0, 0.0, 0.5, 1.0))
                                                .child(if is_submenu {
                                                    submenu_title.unwrap_or("M-x")
                                                } else {
                                                    "M-x "
                                                }),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .text_size(px(14.0))
                                                .text_color(gpui::hsla(0.0, 0.0, 0.9, 1.0))
                                                .child(if self.command_center_query.is_empty() {
                                                    "Type a command...".into()
                                                } else {
                                                    SharedString::from(
                                                        self.command_center_query.clone(),
                                                    )
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("command-list")
                                        .flex()
                                        .flex_col()
                                        .overflow_y_scroll()
                                        .children(filtered.iter().enumerate().map(
                                            |(i, (cmd_idx, cmd))| {
                                                let is_selected = i == selected;
                                                let cmd_text = cmd.clone();
                                                let click_idx = *cmd_idx;
                                                div()
                                                    .id(ElementId::Name(
                                                        format!("cmd-{cmd_idx}").into(),
                                                    ))
                                                    .px(px(12.0))
                                                    .py(px(6.0))
                                                    .text_size(px(13.0))
                                                    .cursor_pointer()
                                                    .text_color(if is_selected {
                                                        gpui::hsla(0.0, 0.0, 1.0, 1.0)
                                                    } else {
                                                        gpui::hsla(0.0, 0.0, 0.8, 1.0)
                                                    })
                                                    .when(is_selected, |el| {
                                                        el.bg(gpui::hsla(220.0, 0.6, 0.4, 1.0))
                                                    })
                                                    .hover(|s| {
                                                        s.bg(gpui::hsla(220.0, 0.5, 0.35, 1.0))
                                                    })
                                                    .child(cmd_text.clone())
                                                    .on_click({
                                                        let sub = submenu_clone.clone();
                                                        cx.listener(
                                                            move |this, _, window, cx| {
                                                                if let Some(ref sub) = sub {
                                                                    this.execute_submenu_item(sub, click_idx, window, cx);
                                                                } else {
                                                                    this.execute_command(&cmd_text, window, cx);
                                                                }
                                                            },
                                                        )
                                                    })
                                            },
                                        )),
                                ),
                        ),
                )
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
