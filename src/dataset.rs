use std::error::Error;
use std::fs;
use std::io;

use crate::api::TftMatch;
use crate::model::MatchObservation;
use crate::storage::DataStore;

pub type DatasetResult<T> = Result<T, Box<dyn Error>>;

const STANDARD_RANKED_QUEUE_ID: i32 = 1100;

#[derive(Debug, Default)]
pub struct StandardRankedDataset {
    pub matches: usize,
    pub excluded_matches: usize,
    pub observations: Vec<MatchObservation>,
}

pub fn load_standard_ranked_dataset(
    store: &DataStore,
    region: &str,
    set_number: i32,
) -> DatasetResult<StandardRankedDataset> {
    let mut dataset = StandardRankedDataset::default();

    for path in store.cached_match_paths(region)? {
        let json = fs::read_to_string(&path)?;
        let riot_match: TftMatch = serde_json::from_str(&json).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("could not parse {}: {error}", path.display()),
            )
        })?;

        if riot_match.queue_id() != STANDARD_RANKED_QUEUE_ID
            || riot_match.set_number() != set_number
        {
            dataset.excluded_matches += 1;
            continue;
        }

        dataset.matches += 1;
        dataset.observations.extend(riot_match.into_observations());
    }

    Ok(dataset)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::storage::DataStore;

    use super::load_standard_ranked_dataset;

    #[test]
    fn loads_ranked_matches_from_the_requested_set() {
        let root = temporary_data_root();
        let store = DataStore::new(&root);
        let standard_match = include_str!("../tests/fixtures/sample_match.json");
        let double_up_match = standard_match
            .replace("JP1_123", "JP1_456")
            .replace("\"queue_id\": 1100", "\"queue_id\": 1160")
            .replace(
                "\"tft_game_type\": \"standard\"",
                "\"tft_game_type\": \"pairs\"",
            );
        let previous_set_match = standard_match
            .replace("JP1_123", "JP1_789")
            .replace("\"tft_set_number\": 17", "\"tft_set_number\": 16");

        store
            .save_match_json("asia", "JP1_123", standard_match)
            .expect("standard fixture should be saved");
        store
            .save_match_json("asia", "JP1_456", &double_up_match)
            .expect("Double Up fixture should be saved");
        store
            .save_match_json("asia", "JP1_789", &previous_set_match)
            .expect("previous-set fixture should be saved");

        let dataset =
            load_standard_ranked_dataset(&store, "asia", 17).expect("dataset should load");

        assert_eq!(dataset.matches, 1);
        assert_eq!(dataset.excluded_matches, 2);
        assert_eq!(dataset.observations.len(), 1);
        assert_eq!(dataset.observations[0].player_id, "player-1");

        fs::remove_dir_all(root).expect("temporary test directory should be removable");
    }

    #[test]
    fn missing_cache_produces_an_empty_dataset() {
        let root = temporary_data_root();
        let dataset = load_standard_ranked_dataset(&DataStore::new(root), "asia", 17)
            .expect("a missing cache should be valid");

        assert_eq!(dataset.matches, 0);
        assert_eq!(dataset.excluded_matches, 0);
        assert!(dataset.observations.is_empty());
    }

    fn temporary_data_root() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "tft-nowcasting-dataset-test-{}-{unique}",
            std::process::id()
        ))
    }
}
