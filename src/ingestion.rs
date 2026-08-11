use std::cmp::Reverse;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::api::{ApiResult, ChallengerLeague, LeagueEntry, RiotApiClient, TftMatch};
use crate::storage::DataStore;

const BACKFILL_PAGE_SIZE: u8 = 100;
const STANDARD_RANKED_QUEUE_ID: i32 = 1100;

#[derive(Debug, Clone, Copy)]
pub struct IngestionConfig {
    pub player_limit: usize,
    pub matches_per_player: u8,
}

impl Default for IngestionConfig {
    fn default() -> Self {
        Self {
            player_limit: 3,
            matches_per_player: 5,
        }
    }
}

#[derive(Debug)]
pub struct IngestionReport {
    pub ladder_snapshot: PathBuf,
    pub players_considered: usize,
    pub unique_matches: usize,
    pub downloaded_matches: usize,
    pub cached_matches: usize,
    pub observations: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct BackfillConfig {
    pub set_number: i32,
    pub player_limit: usize,
}

#[derive(Debug)]
pub struct BackfillReport {
    pub ladder_snapshot: PathBuf,
    pub players_considered: usize,
    pub pages_requested: usize,
    pub unique_matches: usize,
    pub target_set_ranked_matches: usize,
    pub downloaded_matches: usize,
    pub cached_matches: usize,
}

pub fn ingest(
    client: &RiotApiClient,
    store: &DataStore,
    config: IngestionConfig,
) -> ApiResult<IngestionReport> {
    let ladder_json = client.challenger_league_json()?;
    let mut ladder: ChallengerLeague = serde_json::from_str(&ladder_json)?;
    let ladder_snapshot = store.save_ladder_snapshot(client.platform(), &ladder_json)?;
    let players = select_highest_lp_players(&mut ladder.entries, config.player_limit);
    let players_considered = players.len();

    let mut match_ids = HashSet::new();
    for player in players {
        match_ids.extend(client.match_ids_by_puuid(&player.puuid, 0, config.matches_per_player)?);
    }

    let mut match_ids: Vec<String> = match_ids.into_iter().collect();
    match_ids.sort();

    let mut downloaded_matches = 0;
    let mut cached_matches = 0;
    let mut observations = 0;

    for match_id in &match_ids {
        let (json, downloaded) = load_match_json(client, store, match_id)?;
        if downloaded {
            downloaded_matches += 1;
        } else {
            cached_matches += 1;
        }

        let riot_match: TftMatch = serde_json::from_str(&json)?;
        observations += riot_match.into_observations().len();
    }

    Ok(IngestionReport {
        ladder_snapshot,
        players_considered,
        unique_matches: match_ids.len(),
        downloaded_matches,
        cached_matches,
        observations,
    })
}

pub fn backfill_set(
    client: &RiotApiClient,
    store: &DataStore,
    config: BackfillConfig,
) -> ApiResult<BackfillReport> {
    let ladder_json = client.challenger_league_json()?;
    let mut ladder: ChallengerLeague = serde_json::from_str(&ladder_json)?;
    let ladder_snapshot = store.save_ladder_snapshot(client.platform(), &ladder_json)?;
    let players = select_highest_lp_players(&mut ladder.entries, config.player_limit);

    let mut seen_match_ids = HashSet::new();
    let mut pages_requested = 0;
    let mut target_set_ranked_matches = 0;
    let mut downloaded_matches = 0;
    let mut cached_matches = 0;

    for player in players {
        let mut start = 0;

        loop {
            let match_ids = client.match_ids_by_puuid(&player.puuid, start, BACKFILL_PAGE_SIZE)?;
            pages_requested += 1;

            if match_ids.is_empty() {
                break;
            }

            let page_len = match_ids.len();
            let mut reached_previous_set = false;

            for match_id in match_ids {
                let is_new = seen_match_ids.insert(match_id.clone());
                let (json, downloaded) = if is_new {
                    load_match_json(client, store, &match_id)?
                } else {
                    (store.read_match_json(client.region(), &match_id)?, false)
                };

                if is_new {
                    if downloaded {
                        downloaded_matches += 1;
                    } else {
                        cached_matches += 1;
                    }
                }

                let riot_match: TftMatch = serde_json::from_str(&json)?;
                if is_new
                    && riot_match.set_number() == config.set_number
                    && riot_match.queue_id() == STANDARD_RANKED_QUEUE_ID
                {
                    target_set_ranked_matches += 1;
                }
                if is_previous_standard_set(
                    config.set_number,
                    riot_match.set_number(),
                    riot_match.queue_id(),
                ) {
                    reached_previous_set = true;
                    break;
                }
            }

            if reached_previous_set || page_len < usize::from(BACKFILL_PAGE_SIZE) {
                break;
            }

            start += page_len as u32;
        }
    }

    Ok(BackfillReport {
        ladder_snapshot,
        players_considered: players.len(),
        pages_requested,
        unique_matches: seen_match_ids.len(),
        target_set_ranked_matches,
        downloaded_matches,
        cached_matches,
    })
}

fn is_previous_standard_set(target_set: i32, match_set: i32, queue_id: i32) -> bool {
    queue_id == STANDARD_RANKED_QUEUE_ID && match_set < target_set
}

fn load_match_json(
    client: &RiotApiClient,
    store: &DataStore,
    match_id: &str,
) -> ApiResult<(String, bool)> {
    if store.match_exists(client.region(), match_id) {
        return Ok((store.read_match_json(client.region(), match_id)?, false));
    }

    let json = client.match_json_by_id(match_id)?;
    store.save_match_json(client.region(), match_id, &json)?;
    Ok((json, true))
}

fn select_highest_lp_players(entries: &mut [LeagueEntry], limit: usize) -> &[LeagueEntry] {
    entries.sort_by_key(|entry| Reverse(entry.league_points));
    &entries[..limit.min(entries.len())]
}

#[cfg(test)]
mod tests {
    use crate::api::ChallengerLeague;

    use super::{is_previous_standard_set, select_highest_lp_players};

    #[test]
    fn only_an_older_standard_ranked_match_ends_a_set_backfill() {
        assert!(is_previous_standard_set(17, 16, 1100));
        assert!(!is_previous_standard_set(17, 15, 6100));
        assert!(!is_previous_standard_set(17, 16, 1160));
        assert!(!is_previous_standard_set(17, 17, 1100));
    }

    #[test]
    fn selects_players_with_the_highest_league_points() {
        let json = include_str!("../tests/fixtures/sample_challenger_league.json");
        let mut ladder: ChallengerLeague =
            serde_json::from_str(json).expect("ladder fixture should deserialize");

        let selected = select_highest_lp_players(&mut ladder.entries, 2);
        let selected_ids: Vec<&str> = selected.iter().map(|entry| entry.puuid.as_str()).collect();

        assert_eq!(selected_ids, ["player-b", "player-c"]);
    }
}
