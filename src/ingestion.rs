use std::collections::HashSet;
use std::path::PathBuf;
use std::cmp::Reverse;

use crate::api::{ApiResult, ChallengerLeague, LeagueEntry, RiotApiClient, TftMatch};
use crate::storage::DataStore;

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
        let json = if store.match_exists(client.region(), match_id) {
            cached_matches += 1;
            store.read_match_json(client.region(), match_id)?
        } else {
            downloaded_matches += 1;
            let json = client.match_json_by_id(match_id)?;
            store.save_match_json(client.region(), match_id, &json)?;
            json
        };

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

fn select_highest_lp_players(entries: &mut [LeagueEntry], limit: usize) -> &[LeagueEntry] {
    entries.sort_by_key(|entry| Reverse(entry.league_points));
    &entries[..limit.min(entries.len())]
}

#[cfg(test)]
mod tests {
    use crate::api::ChallengerLeague;

    use super::select_highest_lp_players;

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
