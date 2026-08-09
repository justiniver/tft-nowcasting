use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::api::TftMatch;
use crate::model::Composition;
use crate::storage::DataStore;

pub type AuditResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Default, PartialEq)]
pub struct DatasetAudit {
    pub matches: usize,
    pub observations: usize,
    pub unique_players: usize,
    pub unique_compositions: usize,
    pub total_units: usize,
    pub observations_without_units: usize,
    pub observations_without_augments: usize,
    pub earliest_timestamp: Option<u64>,
    pub latest_timestamp: Option<u64>,
    pub observations_by_patch: BTreeMap<String, usize>,
    pub matches_by_queue: BTreeMap<i32, usize>,
    pub matches_by_game_type: BTreeMap<String, usize>,
    pub matches_by_set: BTreeMap<i32, usize>,
    composition_counts: HashMap<Composition, usize>,
}

impl DatasetAudit {
    pub fn average_units_per_observation(&self) -> f64 {
        if self.observations == 0 {
            0.0
        } else {
            self.total_units as f64 / self.observations as f64
        }
    }

    pub fn most_common_compositions(&self) -> Vec<(&Composition, usize)> {
        let mut counts: Vec<(&Composition, usize)> = self
            .composition_counts
            .iter()
            .map(|(composition, count)| (composition, *count))
            .collect();

        counts.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
        counts.truncate(5);
        counts
    }
}

pub fn audit_cached_matches(store: &DataStore, region: &str) -> AuditResult<DatasetAudit> {
    let paths = store.cached_match_paths(region)?;
    audit_match_paths(&paths)
}

fn audit_match_paths(paths: &[PathBuf]) -> AuditResult<DatasetAudit> {
    let mut audit = DatasetAudit::default();
    let mut players = HashSet::new();

    for path in paths {
        let json = fs::read_to_string(path)?;
        let riot_match: TftMatch = serde_json::from_str(&json).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("could not parse {}: {error}", path.display()),
            )
        })?;

        audit.matches += 1;
        *audit
            .matches_by_queue
            .entry(riot_match.queue_id())
            .or_default() += 1;
        *audit
            .matches_by_game_type
            .entry(riot_match.game_type().to_owned())
            .or_default() += 1;
        *audit
            .matches_by_set
            .entry(riot_match.set_number())
            .or_default() += 1;

        for observation in riot_match.into_observations() {
            audit.observations += 1;
            audit.total_units += observation.units.len();
            audit.earliest_timestamp = Some(
                audit
                    .earliest_timestamp
                    .map_or(observation.timestamp, |current| {
                        current.min(observation.timestamp)
                    }),
            );
            audit.latest_timestamp = Some(
                audit
                    .latest_timestamp
                    .map_or(observation.timestamp, |current| {
                        current.max(observation.timestamp)
                    }),
            );

            if observation.units.is_empty() {
                audit.observations_without_units += 1;
            }
            if observation.augments.is_empty() {
                audit.observations_without_augments += 1;
            }

            *audit
                .observations_by_patch
                .entry(observation.patch.clone())
                .or_default() += 1;
            players.insert(observation.player_id.clone());
            *audit
                .composition_counts
                .entry(Composition::from_units(&observation.units))
                .or_default() += 1;
        }
    }

    audit.unique_players = players.len();
    audit.unique_compositions = audit.composition_counts.len();

    Ok(audit)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::model::{Composition, UnitObservation};

    use super::{DatasetAudit, audit_match_paths};

    #[test]
    fn audits_a_cached_match_fixture() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample_match.json");

        let audit = audit_match_paths(&[path]).expect("fixture should be auditable");

        assert_eq!(audit.matches, 1);
        assert_eq!(audit.observations, 1);
        assert_eq!(audit.unique_players, 1);
        assert_eq!(audit.unique_compositions, 1);
        assert_eq!(audit.total_units, 1);
        assert_eq!(audit.average_units_per_observation(), 1.0);
        assert_eq!(audit.observations_without_units, 0);
        assert_eq!(audit.observations_without_augments, 0);
        assert_eq!(audit.earliest_timestamp, Some(1_785_900_000_000));
        assert_eq!(audit.latest_timestamp, Some(1_785_900_000_000));
        assert_eq!(audit.observations_by_patch["16.15"], 1);
        assert_eq!(audit.matches_by_queue[&1100], 1);
        assert_eq!(audit.matches_by_game_type["standard"], 1);
        assert_eq!(audit.matches_by_set[&17], 1);
        let most_common = audit.most_common_compositions();
        assert_eq!(most_common.len(), 1);
        assert_eq!(most_common[0].0.to_string(), "TFT_Champion_Test");
        assert_eq!(most_common[0].1, 1);
    }

    #[test]
    fn empty_dataset_produces_an_empty_audit() {
        let audit = audit_match_paths(&[]).expect("empty input should be valid");

        assert_eq!(audit.matches, 0);
        assert_eq!(audit.observations, 0);
        assert_eq!(audit.average_units_per_observation(), 0.0);
        assert_eq!(audit.earliest_timestamp, None);
        assert_eq!(audit.latest_timestamp, None);
    }

    #[test]
    fn returns_five_most_common_compositions_with_deterministic_ties() {
        let mut audit = DatasetAudit::default();
        for (champion, count) in [
            ("Zed", 3),
            ("Ahri", 3),
            ("Neeko", 2),
            ("Yasuo", 1),
            ("Vi", 1),
            ("Jinx", 1),
        ] {
            let composition = Composition::from_units(&[UnitObservation::new(champion, 1, vec![])]);
            audit.composition_counts.insert(composition, count);
        }

        let actual: Vec<(String, usize)> = audit
            .most_common_compositions()
            .into_iter()
            .map(|(composition, count)| (composition.to_string(), count))
            .collect();

        assert_eq!(
            actual,
            [
                ("Ahri".to_owned(), 3),
                ("Zed".to_owned(), 3),
                ("Neeko".to_owned(), 2),
                ("Jinx".to_owned(), 1),
                ("Vi".to_owned(), 1),
            ]
        );
    }
}
