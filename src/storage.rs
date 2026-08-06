use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct DataStore {
    root: PathBuf,
}

impl DataStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn save_ladder_snapshot(&self, platform: &str, json: &str) -> io::Result<PathBuf> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let path = self
            .root
            .join("raw/ladders")
            .join(platform)
            .join(format!("{timestamp}.json"));

        self.write_json(&path, json)?;
        Ok(path)
    }

    pub fn match_exists(&self, region: &str, match_id: &str) -> bool {
        self.match_path(region, match_id).is_file()
    }

    pub fn read_match_json(&self, region: &str, match_id: &str) -> io::Result<String> {
        fs::read_to_string(self.match_path(region, match_id))
    }

    pub fn save_match_json(&self, region: &str, match_id: &str, json: &str) -> io::Result<PathBuf> {
        let path = self.match_path(region, match_id);
        self.write_json(&path, json)?;
        Ok(path)
    }

    fn match_path(&self, region: &str, match_id: &str) -> PathBuf {
        self.root
            .join("raw/matches")
            .join(region)
            .join(format!("{match_id}.json"))
    }

    fn write_json(&self, path: &Path, json: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path.with_extension("tmp"), json)?;
        fs::rename(path.with_extension("tmp"), path)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::DataStore;

    #[test]
    fn saves_and_reads_match_json() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "tft-nowcasting-storage-test-{}-{unique}",
            std::process::id()
        ));
        let store = DataStore::new(&root);

        let path = store
            .save_match_json("asia", "JP1_123", "{\"match\":true}")
            .expect("fixture should be saved");

        assert!(path.is_file());
        assert!(store.match_exists("asia", "JP1_123"));
        assert_eq!(
            store
                .read_match_json("asia", "JP1_123")
                .expect("fixture should be readable"),
            "{\"match\":true}"
        );

        fs::remove_dir_all(root).expect("temporary test directory should be removable");
    }
}
