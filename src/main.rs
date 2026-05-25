use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context as _, Result, bail};
use editor::{
    Anchor, Editor, EditorMode, HighlightKey, MultiBuffer, MultiBufferOffset, SelectionEffects,
    scroll::Autoscroll,
};
use gpui::*;
use language::{Buffer, LanguageRegistry, LoadedLanguage};
use serde::{Deserialize, Serialize};
use settings::{KeymapFile, DEFAULT_KEYMAP_PATH};
use theme::{LoadThemes, ThemeSettingsProvider, UiDensity};

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

// --- Single instance IPC ---

struct SharedState {
    sender: std::sync::mpsc::Sender<Vec<PathBuf>>,
    workspace_handle: std::sync::Mutex<Option<WindowHandle<LiteWorkspace>>>,
}

struct OpenListener(Arc<SharedState>);

impl Global for OpenListener {}

impl OpenListener {
    fn new(sender: std::sync::mpsc::Sender<Vec<PathBuf>>) -> Self {
        Self(Arc::new(SharedState {
            sender,
            workspace_handle: std::sync::Mutex::new(None),
        }))
    }

    fn shared(&self) -> Arc<SharedState> {
        self.0.clone()
    }

    fn set_workspace(&self, handle: WindowHandle<LiteWorkspace>) {
        *self.0.workspace_handle.lock().unwrap() = Some(handle);
    }

    fn workspace_handle(&self) -> Option<WindowHandle<LiteWorkspace>> {
        self.0.workspace_handle.lock().unwrap().clone()
    }

    fn sender(&self) -> std::sync::mpsc::Sender<Vec<PathBuf>> {
        self.0.sender.clone()
    }
}

#[cfg(unix)]
fn ipc_socket_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("wzed.sock")
}

#[cfg(unix)]
fn try_send_to_existing_instance(paths: &[PathBuf]) -> bool {
    use std::os::unix::net::UnixDatagram;

    let sock_path = ipc_socket_path();
    let sock = match UnixDatagram::unbound() {
        Ok(s) => s,
        Err(_) => return false,
    };
    if sock.connect(&sock_path).is_err() {
        return false;
    }

    let msg = paths
        .iter()
        .filter_map(|p| p.to_str().map(|s| s.to_owned()))
        .collect::<Vec<_>>()
        .join("\n");
    sock.send(msg.as_bytes()).is_ok()
}

#[cfg(unix)]
fn listen_for_instances(sender: std::sync::mpsc::Sender<Vec<PathBuf>>) -> Result<()> {
    use std::os::unix::net::UnixDatagram;
    use std::thread;

    let sock_path = ipc_socket_path();

    if let Err(e) = UnixDatagram::unbound().and_then(|s| {
        s.connect(&sock_path)?;
        s.send(&[])
    }) {
        if e.kind() == std::io::ErrorKind::ConnectionRefused {
            let _ = std::fs::remove_file(&sock_path);
        }
    }

    let listener = UnixDatagram::bind(&sock_path)
        .with_context(|| format!("failed to bind IPC socket at {}", sock_path.display()))?;

    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            if let Ok(len) = listener.recv(&mut buf) {
                let text = String::from_utf8_lossy(&buf[..len]);
                let paths: Vec<PathBuf> = text
                    .split('\n')
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
                    .collect();
                if !paths.is_empty() {
                    let _ = sender.send(paths);
                }
            }
        }
    });
    Ok(())
}

#[cfg(windows)]
fn try_send_to_existing_instance(paths: &[PathBuf]) -> bool {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let stream = match TcpStream::connect("127.0.0.1:0") {
        Ok(s) => s,
        Err(_) => return false,
    };
    // Named pipes require windows-specific API; fall back to a localhost socket approach
    // using a lock file to store the port.
    let lock_path = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("wzed.port");
    let port_str = match std::fs::read_to_string(&lock_path) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let port: u16 = match port_str.trim().parse() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let mut stream = match TcpStream::connect(format!("127.0.0.1:{port}")) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let msg = paths
        .iter()
        .filter_map(|p| p.to_str().map(|s| s.to_owned()))
        .collect::<Vec<_>>()
        .join("\n");
    stream.write_all(msg.as_bytes()).is_ok()
}

#[cfg(windows)]
fn listen_for_instances(sender: std::sync::mpsc::Sender<Vec<PathBuf>>) -> Result<()> {
    use std::io::Read as _;
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0")
        .context("failed to bind IPC listener")?;
    let port = listener.local_addr()?.port();

    let lock_path = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("wzed.port");
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&lock_path, port.to_string())?;

    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            if let Ok((mut stream, _)) = listener.accept() {
                if let Ok(len) = stream.read(&mut buf) {
                    let text = String::from_utf8_lossy(&buf[..len]);
                    let paths: Vec<PathBuf> = text
                        .split('\n')
                        .filter(|s| !s.is_empty())
                        .map(PathBuf::from)
                        .collect();
                    if !paths.is_empty() {
                        let _ = sender.send(paths);
                    }
                }
            }
        }
    });
    Ok(())
}

// --- Main ---

fn main() {
    let file_args: Vec<PathBuf> = std::env::args()
        .skip(1)
        .filter(|arg| !arg.starts_with('-'))
        .map(PathBuf::from)
        .collect();

    if !file_args.is_empty() && try_send_to_existing_instance(&file_args) {
        return;
    }

    let (ipc_sender, ipc_receiver) = std::sync::mpsc::channel::<Vec<PathBuf>>();

    let app =
        Application::with_platform(gpui_linux::current_platform(false)).with_assets(assets::Assets);

    app.run(move |cx: &mut App| {
        settings::init(cx);
        theme::init(LoadThemes::JustBase, cx);
        theme::set_theme_settings_provider(Box::new(WzedThemeSettings::new()), cx);

        cx.bind_keys(
            KeymapFile::load_asset_allow_partial_failure(DEFAULT_KEYMAP_PATH, cx).unwrap(),
        );

        cx.bind_keys(vec![
            KeyBinding::new("ctrl-f", ToggleFind, Some("LiteWorkspace")),
            KeyBinding::new("f3", FindNext, Some("LiteWorkspace")),
            KeyBinding::new("shift-f3", FindPrevious, Some("LiteWorkspace")),
            KeyBinding::new("escape", ToggleFind, Some("LiteWorkspace")),
        ]);

        let languages = Arc::new(LanguageRegistry::new(cx.background_executor().clone()));
        register_languages(&languages);

        cx.set_global(OpenListener::new(ipc_sender));
        if let Err(err) = listen_for_instances(
            cx.global::<OpenListener>().sender(),
        ) {
            eprintln!("IPC listener failed: {err:#}");
        }

        let shared_state = cx.global::<OpenListener>().shared();
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(200)).await;
                let Ok(paths) = ipc_receiver.try_recv() else {
                    continue;
                };
                let Some(handle) = shared_state.workspace_handle.lock().unwrap().clone() else {
                    continue;
                };
                handle.update(cx, |workspace, window, cx| {
                    for path in &paths {
                        if path.exists() {
                            workspace.open_file_path(path.clone(), window, cx).ok();
                        }
                    }
                }).ok();
            }
        }).detach();

        let file_args = file_args.clone();
        let window_handle = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1200.0), px(800.0)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("WZed".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                focus: true,
                show: true,
                kind: WindowKind::Normal,
                is_movable: true,
                is_resizable: true,
                is_minimizable: true,
                display_id: None,
                window_background: WindowBackgroundAppearance::default(),
                app_id: Some("dev.wzed.editor".to_string()),
                window_min_size: Some(size(px(400.0), px(300.0))),
                window_decorations: Some(WindowDecorations::Client),
                icon: None,
                tabbing_identifier: None,
            },
            move |window, cx| {
                let workspace = cx.new(|cx| {
                    let mut workspace = LiteWorkspace::new(languages.clone(), window, cx);

                    workspace.restore_session(window, cx);

                    for path in &file_args {
                        if path.exists() {
                            workspace.open_file_path(path.clone(), window, cx).ok();
                        }
                    }
                    workspace.save_session(cx);
                    workspace
                });

                workspace
            },
        )
        .expect("failed to open window");

        cx.global::<OpenListener>().set_workspace(window_handle);

        let _quit_subscription = cx.on_app_quit(|cx| {
            let listener = cx.global::<OpenListener>();
            if let Some(handle) = listener.workspace_handle() {
                handle.read_with(cx, |workspace, cx| {
                    save_session(workspace, cx);
                }).ok();
            }
            std::future::ready(())
        });
    });
}

fn register_languages(languages: &Arc<LanguageRegistry>) {
    languages.register_native_grammars(grammars::native_grammars());

    let language_names = [
        "bash",
        "c",
        "cpp",
        "css",
        "diff",
        "go",
        "json",
        "jsonc",
        "markdown",
        "python",
        "regex",
        "rust",
        "tsx",
        "typescript",
        "yaml",
    ];

    for name in language_names {
        let config = grammars::load_config_for_feature(name, true);
        let grammar_name = config.grammar.clone();
        let matcher = config.matcher.clone();
        let hidden = config.hidden;
        let lang_name = config.name.clone();
        let name_static = name.to_owned();

        languages.register_language(
            lang_name,
            grammar_name,
            matcher,
            hidden,
            None,
            Arc::new(move || {
                Ok(LoadedLanguage {
                    config: grammars::load_config_for_feature(&name_static, true),
                    queries: grammars::load_queries(&name_static),
                    context_provider: None,
                    toolchain_provider: None,
                    manifest_name: None,
                })
            }),
        );
    }
}

struct Tab {
    editor: Entity<Editor>,
    path: Option<PathBuf>,
    title: SharedString,
}

struct LiteWorkspace {
    tabs: Vec<Tab>,
    active: usize,
    languages: Arc<LanguageRegistry>,
    focus_handle: FocusHandle,
    search: SearchState,
}

impl LiteWorkspace {
    fn new(
        languages: Arc<LanguageRegistry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let query_editor = cx.new(|cx| Editor::single_line(window, cx));

        let this = Self {
            tabs: Vec::new(),
            active: 0,
            languages,
            focus_handle,
            search: SearchState {
                visible: false,
                query_editor,
                matches: Vec::new(),
                current_match: None,
            },
        };

        cx.observe(&this.search.query_editor, move |this, _editor, cx| {
            let query = this.search.query_editor.read(cx).text(cx);
            this.run_search(&query, cx);
        })
        .detach();

        this
    }

    fn restore_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    fn open_file_path(
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
            path: Some(path),
            title,
        });
        self.active = self.tabs.len() - 1;
        self.save_session(cx);
        cx.notify();
        Ok(())
    }

    fn save_active_tab(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let tab = &self.tabs[self.active];
        let path = match &tab.path {
            Some(p) => p.clone(),
            None => bail!("no file path for this tab, save-as not yet implemented"),
        };

        let content = tab.editor.read(cx).text(cx);
        std::fs::write(&path, &content)
            .with_context(|| format!("failed to write file: {}", path.display()))?;
        self.save_session(cx);
        Ok(())
    }

    fn save_session(&self, cx: &App) {
        save_session(self, cx);
    }

    fn handle_open(
        &mut self,
        _action: &OpenFile,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        eprintln!("open file: file dialog not yet implemented, pass files as CLI args");
    }

    fn handle_save(
        &mut self,
        _action: &SaveFile,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(err) = self.save_active_tab(cx) {
            eprintln!("failed to save: {err:#}");
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
            self.clear_search_highlights(cx);
            self.search.matches.clear();
            self.search.current_match = None;
            let active_tab = &self.tabs[self.active];
            active_tab.editor.update(cx, |_editor, cx| cx.notify());
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
        self.navigate_match(1, window, cx);
    }

    fn handle_find_previous(
        &mut self,
        _action: &FindPrevious,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigate_match(-1, window, cx);
    }

    fn run_search(&mut self, query: &str, cx: &mut Context<Self>) {
        let active_tab = &self.tabs[self.active];
        let editor = &active_tab.editor;

        editor.update(cx, |editor, cx| {
            editor.clear_background_highlights(HighlightKey::BufferSearchHighlights, cx);
        });

        if query.is_empty() {
            self.search.matches.clear();
            self.search.current_match = None;
            cx.notify();
            return;
        }

        let text = editor.read(cx).text(cx);
        let mut matches = Vec::new();
        let mut start = 0;
        while let Some(idx) = text[start..].find(query) {
            let abs_start = start + idx;
            let abs_end = abs_start + query.len();
            matches.push(abs_start..abs_end);
            start = abs_end;
            if start >= text.len() {
                break;
            }
        }

        let snapshot = editor.read(cx).buffer().read(cx).snapshot(cx);
        let anchor_ranges: Vec<std::ops::Range<Anchor>> = matches
            .iter()
            .map(|range| {
                snapshot.anchor_before(MultiBufferOffset(range.start))
                    ..snapshot.anchor_before(MultiBufferOffset(range.end))
            })
            .collect();

        editor.update(cx, |editor, cx| {
            editor.highlight_background(
                HighlightKey::BufferSearchHighlights,
                &anchor_ranges,
                |_index, _theme| gpui::hsla(48.0 / 360.0, 1.0, 0.5, 0.4),
                cx,
            );
        });

        self.search.current_match = if matches.is_empty() {
            None
        } else {
            Some(0)
        };
        self.search.matches = matches;
        cx.notify();
    }

    fn clear_search_highlights(&self, cx: &mut Context<Self>) {
        let active_tab = &self.tabs[self.active];
        active_tab.editor.update(cx, |editor, cx| {
            editor.clear_background_highlights(HighlightKey::BufferSearchHighlights, cx);
        });
    }

    fn navigate_match(
        &mut self,
        direction: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let match_count = self.search.matches.len();
        if match_count == 0 {
            return;
        }

        let current = self.search.current_match.unwrap_or(0);
        let new_index = if direction > 0 {
            (current + direction as usize) % match_count
        } else {
            (current + match_count - (-direction as usize) % match_count) % match_count
        };
        self.search.current_match = Some(new_index);

        let range = self.search.matches[new_index].clone();
        let active_tab = &self.tabs[self.active];
        let snapshot = active_tab
            .editor
            .read(cx)
            .buffer()
            .read(cx)
            .snapshot(cx);
        let start_anchor = snapshot.anchor_before(MultiBufferOffset(range.start));
        let end_anchor = snapshot.anchor_before(MultiBufferOffset(range.end));

        active_tab.editor.update(cx, |editor, cx| {
            editor.change_selections(
                SelectionEffects::scroll(Autoscroll::fit()),
                window,
                cx,
                |s| s.select_ranges(vec![start_anchor..end_anchor]),
            );
        });

        cx.notify();
    }
}

impl Render for LiteWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = &self.tabs[self.active];

        let side_tabs = div()
            .flex()
            .flex_col()
            .w(px(180.0))
            .h_full()
            .bg(gpui::hsla(0.0, 0.0, 0.1, 1.0))
            .border_r_1()
            .border_color(gpui::hsla(0.0, 0.0, 0.15, 1.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .children(self.tabs.iter().enumerate().map(|(index, tab)| {
                        let is_active = index == self.active;
                        let title = tab.title.clone();
                        let mut tab_el = div()
                            .id(ElementId::Name(format!("tab-{index}").into()))
                            .flex()
                            .items_center()
                            .px(px(10.0))
                            .py(px(6.0))
                            .w_full()
                            .cursor_pointer()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(if is_active {
                                        gpui::hsla(0.0, 0.0, 0.9, 1.0)
                                    } else {
                                        gpui::hsla(0.0, 0.0, 0.6, 1.0)
                                    })
                                    .text_ellipsis()
                                    .child(title),
                            )
                            .on_click(cx.listener(move |workspace, _, _window, cx| {
                                workspace.active = index;
                                cx.notify();
                            }));

                        if is_active {
                            tab_el = tab_el
                                .bg(gpui::hsla(0.0, 0.0, 0.18, 1.0))
                                .border_l_2()
                                .border_color(gpui::hsla(220.0, 0.8, 0.6, 1.0));
                        } else {
                            tab_el = tab_el.hover(|s| s.bg(gpui::hsla(0.0, 0.0, 0.13, 1.0)));
                        }
                        tab_el
                    })),
            )
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
            div()
                .id("search-bar")
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .h(px(36.0))
                .px(px(8.0))
                .gap(px(6.0))
                .bg(gpui::hsla(0.0, 0.0, 0.13, 1.0))
                .border_b_1()
                .border_color(gpui::hsla(0.0, 0.0, 0.15, 1.0))
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
        });

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(gpui::hsla(0.0, 0.0, 0.1, 1.0))
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
                            ),
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
    }
}

struct WzedThemeSettings {
    ui_font: Font,
    buffer_font: Font,
}

impl WzedThemeSettings {
    fn new() -> Self {
        Self {
            ui_font: Font {
                family: "Helvetica".into(),
                weight: gpui::FontWeight::NORMAL,
                style: gpui::FontStyle::Normal,
                features: FontFeatures::default(),
                fallbacks: None,
            },
            buffer_font: Font {
                family: "Monospace".into(),
                weight: gpui::FontWeight::NORMAL,
                style: gpui::FontStyle::Normal,
                features: FontFeatures::default(),
                fallbacks: None,
            },
        }
    }
}

impl ThemeSettingsProvider for WzedThemeSettings {
    fn ui_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        &self.ui_font
    }

    fn buffer_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        &self.buffer_font
    }

    fn ui_font_size(&self, _cx: &App) -> Pixels {
        px(14.0)
    }

    fn buffer_font_size(&self, _cx: &App) -> Pixels {
        px(14.0)
    }

    fn ui_density(&self, _cx: &App) -> UiDensity {
        UiDensity::Default
    }
}

actions!(
    lite_editor,
    [
        /// Open a file.
        OpenFile,
        /// Save the current file.
        SaveFile,
        /// Create a new file.
        NewFile,
        /// Close the current tab.
        CloseTab,
        /// Toggle the find bar.
        ToggleFind,
        /// Go to next search match.
        FindNext,
        /// Go to previous search match.
        FindPrevious,
    ]
);

struct SearchState {
    visible: bool,
    query_editor: Entity<Editor>,
    matches: Vec<std::ops::Range<usize>>,
    current_match: Option<usize>,
}
