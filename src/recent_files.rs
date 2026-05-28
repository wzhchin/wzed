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
        let dir = match crate::utils::ensure_config_dir() {
            Ok(d) => d,
            Err(err) => {
                eprintln!("{err:#}");
                return;
            }
        };
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
        let path = crate::utils::config_dir().join("recent.json");
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make() -> RecentFiles {
        RecentFiles {
            entries: Vec::new(),
            max_entries: 3,
        }
    }

    #[test]
    fn test_add_pushes_to_front() {
        let mut r = make();
        r.add(&PathBuf::from("/a"));
        r.add(&PathBuf::from("/b"));
        assert_eq!(r.entries, vec![PathBuf::from("/b"), PathBuf::from("/a")]);
    }

    #[test]
    fn test_add_deduplicates() {
        let mut r = make();
        r.add(&PathBuf::from("/a"));
        r.add(&PathBuf::from("/b"));
        r.add(&PathBuf::from("/a"));
        assert_eq!(r.entries, vec![PathBuf::from("/a"), PathBuf::from("/b")]);
    }

    #[test]
    fn test_add_truncates_at_max() {
        let mut r = make();
        r.add(&PathBuf::from("/a"));
        r.add(&PathBuf::from("/b"));
        r.add(&PathBuf::from("/c"));
        r.add(&PathBuf::from("/d"));
        assert_eq!(r.entries.len(), 3);
        assert_eq!(r.entries[0], PathBuf::from("/d"));
    }

    #[test]
    fn test_add_same_path_moves_to_front() {
        let mut r = make();
        r.add(&PathBuf::from("/a"));
        r.add(&PathBuf::from("/b"));
        r.add(&PathBuf::from("/c"));
        r.add(&PathBuf::from("/a"));
        assert_eq!(r.entries.len(), 3);
        assert_eq!(r.entries[0], PathBuf::from("/a"));
    }
}
