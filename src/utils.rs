use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

pub(crate) fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("wzed")
}

pub(crate) fn ensure_config_dir() -> Result<PathBuf> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).with_context(|| format!("failed to create config dir: {}", dir.display()))?;
    Ok(dir)
}

pub(crate) fn file_name_from_path(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "untitled".into())
}

// 用本地墙钟时间，方便人眼看懂新建 tab 的创建时刻
pub(crate) fn untitled_name() -> String {
    format!("untitled-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"))
}

pub(crate) fn format_action_name(name: &str) -> String {
    let unqualified = name.rsplit("::").next().unwrap_or(name);
    let mut result = String::new();
    for (i, ch) in unqualified.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('-');
        }
        result.push(ch.to_ascii_lowercase());
    }
    result
}

/// Grammar identifiers for language registration, command center, and IPC.
/// Add new languages here — all consumers reference this single list.
pub(crate) const GRAMMAR_NAMES: &[&str] = &[
    "bash", "c", "cpp", "css", "diff", "go", "json", "jsonc", "markdown",
    "python", "regex", "rust", "tsx", "typescript", "yaml",
];

/// Centralized tunable constants. Single source of truth for all magic numbers.
pub(crate) struct AppConfig;

impl AppConfig {
    pub const AUTOSAVE_INTERVAL_SECS: u64 = 30;
    pub const NOTIFICATION_DISPLAY_SECS: u64 = 4;
    pub const SNAPSHOT_RETENTION_DAYS: u64 = 7;
    pub const MAX_RECENT_FILES: usize = 20;
}

/// Human-readable display names matching [`GRAMMAR_NAMES`] 1:1.
pub(crate) const GRAMMAR_DISPLAY_NAMES: &[&str] = &[
    "Bash", "C", "C++", "CSS", "Diff", "Go", "JSON", "JSONC", "Markdown",
    "Python", "Regex", "Rust", "TSX", "TypeScript", "YAML",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_action_simple() {
        assert_eq!(format_action_name("lite_editor::SaveFile"), "save-file");
    }

    #[test]
    fn test_format_action_single_word() {
        assert_eq!(format_action_name("lite_editor::Save"), "save");
    }

    #[test]
    fn test_format_action_no_namespace() {
        assert_eq!(format_action_name("SaveFile"), "save-file");
    }

    #[test]
    fn test_format_action_all_caps() {
        assert_eq!(format_action_name("UTF8"), "u-t-f8");
    }

    #[test]
    fn test_format_action_consecutive_caps() {
        assert_eq!(format_action_name("OpenURL"), "open-u-r-l");
    }

    #[test]
    fn test_format_action_deeply_nested() {
        assert_eq!(format_action_name("a::b::c::DoThing"), "do-thing");
    }

    #[test]
    fn test_file_name_from_path() {
        assert_eq!(file_name_from_path(std::path::Path::new("/tmp/hello.rs")), "hello.rs");
    }

    #[test]
    fn test_file_name_from_path_no_extension() {
        assert_eq!(file_name_from_path(std::path::Path::new("/tmp/Makefile")), "Makefile");
    }

    #[test]
    fn test_file_name_from_path_empty() {
        assert_eq!(file_name_from_path(std::path::Path::new("/")), "untitled");
    }

    #[test]
    fn test_untitled_name_format() {
        let name = untitled_name();
        assert!(name.starts_with("untitled-"), "got: {name}");
        // untitled-YYYYMMDD-HHMMSS
        assert_eq!(name.len(), "untitled-20260613-120000".len());
        assert_eq!(name.as_bytes()[17], b'-');
    }

    #[test]
    fn test_ipc_parse_open_files() {
        let msg = crate::ipc::parse_ipc_message("/tmp/a.rs\n/tmp/b.rs").unwrap();
        match msg {
            crate::ipc::IpcMessage::OpenFiles(paths) => {
                assert_eq!(paths.len(), 2);
                assert_eq!(paths[0], std::path::PathBuf::from("/tmp/a.rs"));
            }
            _ => panic!("expected OpenFiles"),
        }
    }

    #[test]
    fn test_ipc_parse_execute_command() {
        let msg = crate::ipc::parse_ipc_message("CMD:lite_editor::SaveFile").unwrap();
        match msg {
            crate::ipc::IpcMessage::ExecuteCommand(cmd) => {
                assert_eq!(cmd, "lite_editor::SaveFile");
            }
            _ => panic!("expected ExecuteCommand"),
        }
    }

    #[test]
    fn test_ipc_parse_set_text() {
        let msg = crate::ipc::parse_ipc_message("SET:hello world").unwrap();
        match msg {
            crate::ipc::IpcMessage::SetText(content) => assert_eq!(content, "hello world"),
            _ => panic!("expected SetText"),
        }
    }

    #[test]
    fn test_ipc_parse_save_as() {
        let msg = crate::ipc::parse_ipc_message("SAVEAS:/tmp/out.rs").unwrap();
        match msg {
            crate::ipc::IpcMessage::SaveAs(path) => {
                assert_eq!(path, std::path::PathBuf::from("/tmp/out.rs"));
            }
            _ => panic!("expected SaveAs"),
        }
    }

    #[test]
    fn test_ipc_parse_switch_tab() {
        let msg = crate::ipc::parse_ipc_message("SWITCHTAB:3").unwrap();
        match msg {
            crate::ipc::IpcMessage::SwitchTab(idx) => assert_eq!(idx, 3),
            _ => panic!("expected SwitchTab"),
        }
    }

    #[test]
    fn test_ipc_parse_empty() {
        assert!(crate::ipc::parse_ipc_message("").is_none());
    }

    #[test]
    fn test_ipc_format_command_regular() {
        assert_eq!(
            crate::ipc::format_command_message("lite_editor::SaveFile"),
            "CMD:lite_editor::SaveFile"
        );
    }

    #[test]
    fn test_ipc_format_command_set_text() {
        assert_eq!(crate::ipc::format_command_message("set-text:hello"), "SET:hello");
    }

    #[test]
    fn test_ipc_format_command_save_as() {
        assert_eq!(
            crate::ipc::format_command_message("save-as:/tmp/out.rs"),
            "SAVEAS:/tmp/out.rs"
        );
    }

    #[test]
    fn test_ipc_format_command_switch_tab() {
        assert_eq!(crate::ipc::format_command_message("switch-tab:2"), "SWITCHTAB:2");
    }

    #[test]
    fn test_diff_identical() {
        let state = crate::diff_view::compute_diff(
            "hello\nworld\n", "hello\nworld\n", "a".into(), "b".into(),
        );
        assert_eq!(state.lines.len(), 2);
    }

    #[test]
    fn test_diff_empty() {
        let state = crate::diff_view::compute_diff("", "", "a".into(), "b".into());
        assert!(state.lines.is_empty());
    }

    #[test]
    fn test_diff_added_line() {
        let state = crate::diff_view::compute_diff(
            "a\n", "a\nb\n", "left".into(), "right".into(),
        );
        assert_eq!(state.lines.len(), 2);
        assert_eq!(state.lines[1].1.kind, crate::diff_view::DiffLineKind::Added);
    }

    #[test]
    fn test_diff_removed_line() {
        let state = crate::diff_view::compute_diff(
            "a\nb\n", "a\n", "left".into(), "right".into(),
        );
        assert_eq!(state.lines.len(), 2);
        assert_eq!(state.lines[1].0.kind, crate::diff_view::DiffLineKind::Removed);
    }

    #[test]
    fn test_diff_all_changed() {
        let state = crate::diff_view::compute_diff(
            "old\n", "new\n", "l".into(), "r".into(),
        );
        assert_eq!(state.lines.len(), 1);
        assert_eq!(state.lines[0].0.kind, crate::diff_view::DiffLineKind::Removed);
        assert_eq!(state.lines[0].1.kind, crate::diff_view::DiffLineKind::Added);
    }
}
