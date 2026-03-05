use std::{collections::VecDeque, path::PathBuf};

use serde::{Deserialize, Serialize};

pub const RECENT_FILES_MAX: usize = 10;

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Cache {
    recent_files: VecDeque<PathBuf>,
}

impl Cache {
    pub fn load() -> Self {
        confy::load("mintracker", "cache").unwrap_or_else(|_| Cache::default())
    }

    pub fn save(&self) {
        confy::store("mintracker", "cache", self).unwrap();
    }

    pub fn add_recent_file(&mut self, path: PathBuf) {
        if let Some(pos) = self.recent_files.iter().position(|p| p == &path) {
            self.recent_files.remove(pos);
        }
        self.recent_files.push_front(path);
        if self.recent_files.len() > RECENT_FILES_MAX {
            self.recent_files.pop_back();
        }
    }

    pub fn recent_files(&self) -> &[PathBuf] {
        self.recent_files.as_slices().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recent_files() {
        let mut cache = Cache::default();
        cache.add_recent_file(PathBuf::from("file1"));
        cache.add_recent_file(PathBuf::from("file2"));
        cache.add_recent_file(PathBuf::from("file3"));
        assert_eq!(
            cache.recent_files(),
            &[
                PathBuf::from("file3"),
                PathBuf::from("file2"),
                PathBuf::from("file1")
            ]
        );
        cache.add_recent_file(PathBuf::from("file2"));
        assert_eq!(
            cache.recent_files(),
            &[
                PathBuf::from("file2"),
                PathBuf::from("file3"),
                PathBuf::from("file1")
            ]
        );

        for i in 4..=15 {
            cache.add_recent_file(PathBuf::from(format!("file{}", i)));
        }
        assert_eq!(cache.recent_files().len(), RECENT_FILES_MAX);
        assert_eq!(cache.recent_files()[0], PathBuf::from("file15"));
        assert_eq!(
            cache.recent_files()[RECENT_FILES_MAX - 1],
            PathBuf::from("file6")
        );
    }
}
