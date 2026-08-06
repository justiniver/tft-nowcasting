use std::env;
use std::error::Error;
use std::time::Duration;

use reqwest::blocking::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::model::{MatchObservation, UnitObservation};

pub type ApiResult<T> = Result<T, Box<dyn Error>>;

pub struct RiotApiClient {
    http: Client,
    api_key: String,
    platform: String,
    region: String,
}

impl RiotApiClient {
    pub fn from_env() -> ApiResult<Self> {
        let _ = dotenvy::dotenv();

        Ok(Self {
            http: Client::builder().timeout(Duration::from_secs(15)).build()?,
            api_key: env::var("RIOT_KEY")?,
            platform: env::var("RIOT_PLATFORM").unwrap_or_else(|_| "jp1".to_owned()),
            region: env::var("RIOT_REGION").unwrap_or_else(|_| "asia".to_owned()),
        })
    }

    pub fn platform_status(&self) -> ApiResult<PlatformStatus> {
        self.get(&format!(
            "https://{}.api.riotgames.com/tft/status/v1/platform-data",
            self.platform
        ))
    }

    pub fn challenger_league(&self) -> ApiResult<ChallengerLeague> {
        Ok(serde_json::from_str(&self.challenger_league_json()?)?)
    }

    pub fn challenger_league_json(&self) -> ApiResult<String> {
        self.get_text(&format!(
            "https://{}.api.riotgames.com/tft/league/v1/challenger",
            self.platform
        ))
    }

    pub fn match_ids_by_puuid(&self, puuid: &str, start: u32, count: u8) -> ApiResult<Vec<String>> {
        self.get(&format!(
            "https://{}.api.riotgames.com/tft/match/v1/matches/by-puuid/{puuid}/ids?start={start}&count={count}",
            self.region
        ))
    }

    pub fn match_by_id(&self, match_id: &str) -> ApiResult<TftMatch> {
        Ok(serde_json::from_str(&self.match_json_by_id(match_id)?)?)
    }

    pub fn match_json_by_id(&self, match_id: &str) -> ApiResult<String> {
        self.get_text(&format!(
            "https://{}.api.riotgames.com/tft/match/v1/matches/{match_id}",
            self.region
        ))
    }

    pub fn platform(&self) -> &str {
        &self.platform
    }

    pub fn region(&self) -> &str {
        &self.region
    }

    fn get<T: DeserializeOwned>(&self, url: &str) -> ApiResult<T> {
        Ok(serde_json::from_str(&self.get_text(url)?)?)
    }

    fn get_text(&self, url: &str) -> ApiResult<String> {
        Ok(self
            .http
            .get(url)
            .header("X-Riot-Token", &self.api_key)
            .send()?
            .error_for_status()?
            .text()?)
    }
}

#[derive(Debug, Deserialize)]
pub struct PlatformStatus {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub maintenances: Vec<serde_json::Value>,
    #[serde(default)]
    pub incidents: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ChallengerLeague {
    pub entries: Vec<LeagueEntry>,
}

#[derive(Debug, Deserialize)]
pub struct LeagueEntry {
    pub puuid: String,
    #[serde(rename = "leaguePoints")]
    pub league_points: i32,
}

#[derive(Debug, Deserialize)]
pub struct TftMatch {
    metadata: MatchMetadata,
    info: MatchInfo,
}

impl TftMatch {
    pub fn id(&self) -> &str {
        &self.metadata.match_id
    }

    pub fn into_observations(self) -> Vec<MatchObservation> {
        let patch = patch_from_game_version(&self.info.game_version);
        let timestamp = self.info.game_datetime;

        self.info
            .participants
            .into_iter()
            .map(|participant| MatchObservation {
                player_id: participant.puuid,
                patch: patch.clone(),
                timestamp,
                placement: participant.placement,
                units: participant
                    .units
                    .into_iter()
                    .map(|unit| UnitObservation {
                        champion: unit.character_id,
                        star_level: unit.tier,
                        items: unit.item_names,
                    })
                    .collect(),
                traits: participant
                    .traits
                    .into_iter()
                    .filter(|trait_state| trait_state.tier_current > 0)
                    .map(|trait_state| trait_state.name)
                    .collect(),
                augments: participant.augments,
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct MatchMetadata {
    match_id: String,
}

#[derive(Debug, Deserialize)]
struct MatchInfo {
    game_datetime: u64,
    game_version: String,
    participants: Vec<Participant>,
}

#[derive(Debug, Deserialize)]
struct Participant {
    #[serde(default)]
    augments: Vec<String>,
    placement: u8,
    puuid: String,
    #[serde(default)]
    traits: Vec<TraitState>,
    #[serde(default)]
    units: Vec<Unit>,
}

#[derive(Debug, Deserialize)]
struct TraitState {
    name: String,
    #[serde(default)]
    tier_current: i32,
}

#[derive(Debug, Deserialize)]
struct Unit {
    character_id: String,
    #[serde(rename = "itemNames", default)]
    item_names: Vec<String>,
    tier: u8,
}

fn patch_from_game_version(game_version: &str) -> String {
    let Some(version) = game_version.split_whitespace().find(|part| {
        part.contains('.')
            && part
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
    }) else {
        return game_version.to_owned();
    };

    version.split('.').take(2).collect::<Vec<_>>().join(".")
}

#[cfg(test)]
mod tests {
    use super::{ChallengerLeague, TftMatch, patch_from_game_version};

    #[test]
    fn extracts_patch_from_riot_game_version() {
        assert_eq!(
            patch_from_game_version("Linux Version 16.15.693.1856 (Jul 29 2026)"),
            "16.15"
        );
    }

    #[test]
    fn deserializes_league_points() {
        let json = include_str!("../tests/fixtures/sample_challenger_league.json");
        let ladder: ChallengerLeague =
            serde_json::from_str(json).expect("ladder fixture should deserialize");

        assert_eq!(ladder.entries[0].puuid, "player-a");
        assert_eq!(ladder.entries[0].league_points, 100);
    }

    #[test]
    fn deserializes_and_converts_a_match_fixture() {
        let json = include_str!("../tests/fixtures/sample_match.json");

        let riot_match: TftMatch = serde_json::from_str(json).expect("fixture should deserialize");
        assert_eq!(riot_match.id(), "JP1_123");

        let observations = riot_match.into_observations();
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].patch, "16.15");
        assert_eq!(observations[0].placement, 1);
        assert_eq!(observations[0].units[0].star_level, 3);
        assert_eq!(observations[0].traits, ["TFT_Trait_Active"]);
    }
}
