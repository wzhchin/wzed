use std::path::PathBuf;

pub(crate) struct RecentFiles {
    pub entries: Vec<PathBuf>,
    max_entries: usize,
}

impl RecentFiles {
    pub(crate) fn add(&mut self, path: &PathBuf) {
        self.entries.retain(|p| p != path);
        self.entries.insert(0, path.clone());
        self.entries.truncate(self.max_entries);
    }

    pub(crate) fn save_to_disk(&self) {
        let dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("wzed");
        if let Err(err) = std::fs::create_dir_all(&dir) {
            eprintln!("failed to create config dir: {err:#}");
            return;
        }
        let paths: Vec<String> = self
            .entries
            .iter()
            .filter_map(|p| p.to_str().map(|s| s.to_owned()))
            .collect();
        if let Err(err) = std::fs::write(
            dir.join("recent.json"),
            serde_json::to_string(&paths).unwrap_or_default(),
        ) {
            eprintln!("failed to save recent files: {err:#}");
        }
    }

    pub(crate) fn load_from_disk() -> Self {
        let path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("wzed")
            .join("recent.json");
        let entries = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
            .map(|v| v.into_iter().map(PathBuf::from).collect())
            .unwrap_or_default();
        Self {
            entries,
            max_entries: 20,
        }
    }
}
