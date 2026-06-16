use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context as _, Result};
use gpui::*;

use crate::workspace::LiteWorkspace;

pub(crate) enum IpcMessage {
    OpenFiles(Vec<PathBuf>),
    ExecuteCommand(String),
    SetText(String),
    SaveAs(PathBuf),
    SwitchTab(usize),
}

pub(crate) struct SharedState {
    pub sender: std::sync::mpsc::Sender<IpcMessage>,
    pub workspace_handle: std::sync::Mutex<Option<WindowHandle<LiteWorkspace>>>,
}

pub(crate) struct OpenListener(Arc<SharedState>);

impl Global for OpenListener {}

impl OpenListener {
    pub(crate) fn new(sender: std::sync::mpsc::Sender<IpcMessage>) -> Self {
        Self(Arc::new(SharedState { sender, workspace_handle: std::sync::Mutex::new(None) }))
    }

    pub(crate) fn shared(&self) -> Arc<SharedState> {
        self.0.clone()
    }

    pub(crate) fn set_workspace(&self, handle: WindowHandle<LiteWorkspace>) {
        match self.0.workspace_handle.lock() {
            Ok(mut guard) => *guard = Some(handle),
            Err(err) => eprintln!("IPC lock poisoned: {err}"),
        }
    }

    pub(crate) fn workspace_handle(&self) -> Option<WindowHandle<LiteWorkspace>> {
        match self.0.workspace_handle.lock() {
            Ok(guard) => *guard,
            Err(err) => {
                eprintln!("IPC lock poisoned: {err}");
                None
            }
        }
    }

    pub(crate) fn sender(&self) -> std::sync::mpsc::Sender<IpcMessage> {
        self.0.sender.clone()
    }
}

pub(crate) fn format_command_message(command: &str) -> String {
    if let Some(content) = command.strip_prefix("set-text:") {
        format!("SET:{content}")
    } else if let Some(path) = command.strip_prefix("save-as:") {
        format!("SAVEAS:{path}")
    } else if let Some(index) = command.strip_prefix("switch-tab:") {
        format!("SWITCHTAB:{index}")
    } else {
        format!("CMD:{command}")
    }
}

pub(crate) fn parse_ipc_message(text: &str) -> Option<IpcMessage> {
    if let Some(cmd) = text.strip_prefix("CMD:") {
        return Some(IpcMessage::ExecuteCommand(cmd.to_string()));
    }
    if let Some(content) = text.strip_prefix("SET:") {
        return Some(IpcMessage::SetText(content.to_string()));
    }
    if let Some(path) = text.strip_prefix("SAVEAS:") {
        return Some(IpcMessage::SaveAs(PathBuf::from(path)));
    }
    if let Some(index) = text.strip_prefix("SWITCHTAB:")
        && let Ok(idx) = index.parse::<usize>()
    {
        return Some(IpcMessage::SwitchTab(idx));
    }
    let paths: Vec<PathBuf> =
        text.split('\n').filter(|s| !s.is_empty()).map(PathBuf::from).collect();
    if !paths.is_empty() {
        return Some(IpcMessage::OpenFiles(paths));
    }
    None
}

#[cfg(unix)]
fn ipc_socket_path() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join("wzed.sock")
}

#[cfg(unix)]
pub(crate) fn try_send_to_existing_instance(paths: &[PathBuf]) -> bool {
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
pub(crate) fn try_send_command_to_existing_instance(command: &str) -> bool {
    use std::os::unix::net::UnixDatagram;

    let sock_path = ipc_socket_path();
    let sock = match UnixDatagram::unbound() {
        Ok(s) => s,
        Err(_) => return false,
    };
    if sock.connect(&sock_path).is_err() {
        return false;
    }

    let msg = format_command_message(command);
    sock.send(msg.as_bytes()).is_ok()
}

#[cfg(unix)]
pub(crate) fn listen_for_instances(sender: std::sync::mpsc::Sender<IpcMessage>) -> Result<()> {
    use std::os::unix::net::UnixDatagram;
    use std::thread;

    let sock_path = ipc_socket_path();

    if let Err(e) = UnixDatagram::unbound().and_then(|s| {
        s.connect(&sock_path)?;
        s.send(&[])
    }) && e.kind() == std::io::ErrorKind::ConnectionRefused
        && let Err(err) = std::fs::remove_file(&sock_path)
    {
        eprintln!("could not remove stale IPC socket: {err}");
    }

    let listener = UnixDatagram::bind(&sock_path)
        .with_context(|| format!("failed to bind IPC socket at {}", sock_path.display()))?;

    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            if let Ok(len) = listener.recv(&mut buf) {
                let text = String::from_utf8_lossy(&buf[..len]);
                if let Some(message) = parse_ipc_message(&text)
                    && let Err(err) = sender.send(message)
                {
                    eprintln!("IPC channel closed, stopping listener: {err}");
                    return;
                }
            }
        }
    });
    Ok(())
}

#[cfg(windows)]
pub(crate) fn try_send_to_existing_instance(paths: &[PathBuf]) -> bool {
    use std::io::Write;
    use std::net::TcpStream;

    let lock_path = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("wzed.port");
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
pub(crate) fn try_send_command_to_existing_instance(command: &str) -> bool {
    use std::io::Write;
    use std::net::TcpStream;

    let lock_path = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("wzed.port");
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

    let msg = format_command_message(command);
    stream.write_all(msg.as_bytes()).is_ok()
}

#[cfg(windows)]
pub(crate) fn listen_for_instances(sender: std::sync::mpsc::Sender<IpcMessage>) -> Result<()> {
    use std::io::Read as _;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    let lock_path = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("wzed.port");

    if let Ok(port_str) = std::fs::read_to_string(&lock_path) {
        if let Ok(port) = port_str.trim().parse::<u16>() {
            // Bounded probe: a plain connect() to 127.0.0.1 refuses fast when no
            // one listens, but if the port is taken by something else (firewall
            // silent-drop, a hung non-wzed process) it can block the UI thread for
            // the OS connect timeout. A 150ms deadline keeps startup snappy.
            let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse()?;
            match std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(150))
            {
                Ok(_) => anyhow::bail!("another WZed instance is already running on port {port}"),
                Err(_) => {
                    if let Err(err) = std::fs::remove_file(&lock_path) {
                        eprintln!("could not remove stale port lock: {err}");
                    }
                }
            }
        }
    }

    let listener = TcpListener::bind("127.0.0.1:0").context("failed to bind IPC listener")?;
    let port = listener.local_addr()?.port();

    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&lock_path, port.to_string())?;

    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            if let Ok((mut stream, _)) = listener.accept() {
                if let Ok(len) = stream.read(&mut buf) {
                    let text = String::from_utf8_lossy(&buf[..len]);
                    if let Some(message) = parse_ipc_message(&text) {
                        if let Err(err) = sender.send(message) {
                            eprintln!("IPC channel closed, stopping listener: {err}");
                            return;
                        }
                    }
                }
            }
        }
    });
    Ok(())
}
