use std::env;
use std::error::Error;
use std::fmt;
use std::time::Duration;

use reqwest::Url;
use reqwest::blocking::{Client, Response};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::model::{MatchObservation, UnitObservation};

const PLATFORM_ROUTES: &[&str] = &[
    "br1", "eun1", "euw1", "jp1", "kr", "la1", "la2", "na1", "oc1", "tr1", "ru", "ph2", "sg2",
    "th2", "tw2", "vn2",
];
const REGIONAL_ROUTES: &[&str] = &["americas", "asia", "europe", "sea"];

#[derive(Debug)]
pub enum RiotApiError {
    MissingApiKey,
    InvalidRoute { route: String, kind: &'static str },
    InvalidMatchCount(u8),
    Http(reqwest::Error),
    Api { status: u16, message: String },
}

impl fmt::Display for RiotApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey => write!(formatter, "RIOT_KEY is missing or empty"),
            Self::InvalidRoute { route, kind } => {
                write!(formatter, "{route:?} is not a supported Riot {kind} route")
            }
            Self::InvalidMatchCount(count) => {
                write!(
                    formatter,
                    "match count must be between 1 and 100, got {count}"
                )
            }
            Self::Http(error) => write!(formatter, "Riot API request failed: {error}"),
            Self::Api { status, message } => {
                write!(formatter, "Riot API returned HTTP {status}: {message}")
            }
        }
    }
}

impl Error for RiotApiError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Http(error) => Some(error),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for RiotApiError {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error)
    }
}

pub struct RiotApiClient {
    http: Client,
    api_key: String,
}

impl RiotApiClient {
    pub fn from_env() -> Result<Self, RiotApiError> {
        // A missing `.env` file is fine when RIOT_KEY is already exported by
        // the shell or provided by a deployment environment.
        let _ = dotenvy::dotenv();

        let api_key = env::var("RIOT_KEY").map_err(|_| RiotApiError::MissingApiKey)?;
        if api_key.trim().is_empty() {
            return Err(RiotApiError::MissingApiKey);
        }

        let http = Client::builder()
            .user_agent(concat!("tft-nowcasting/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(15))
            .build()?;

        Ok(Self { http, api_key })
    }

    pub fn platform_status(&self, platform: &str) -> Result<PlatformStatus, RiotApiError> {
        let mut url = route_url(platform, PLATFORM_ROUTES, "platform")?;
        url.path_segments_mut()
            .expect("Riot API base URL supports path segments")
            .extend(["tft", "status", "v1", "platform-data"]);

        self.get_json(url)
    }

    pub fn challenger_league(&self, platform: &str) -> Result<ChallengerLeague, RiotApiError> {
        let mut url = route_url(platform, PLATFORM_ROUTES, "platform")?;
        url.path_segments_mut()
            .expect("Riot API base URL supports path segments")
            .extend(["tft", "league", "v1", "challenger"]);

        self.get_json(url)
    }

    pub fn match_ids_by_puuid(
        &self,
        region: &str,
        puuid: &str,
        start: u32,
        count: u8,
    ) -> Result<Vec<String>, RiotApiError> {
        if !(1..=100).contains(&count) {
            return Err(RiotApiError::InvalidMatchCount(count));
        }

        let mut url = route_url(region, REGIONAL_ROUTES, "regional")?;
        url.path_segments_mut()
            .expect("Riot API base URL supports path segments")
            .extend(["tft", "match", "v1", "matches", "by-puuid"])
            .push(puuid)
            .push("ids");
        url.query_pairs_mut()
            .append_pair("start", &start.to_string())
            .append_pair("count", &count.to_string());

        self.get_json(url)
    }

    pub fn match_by_id(&self, region: &str, match_id: &str) -> Result<TftMatch, RiotApiError> {
        let mut url = route_url(region, REGIONAL_ROUTES, "regional")?;
        url.path_segments_mut()
            .expect("Riot API base URL supports path segments")
            .extend(["tft", "match", "v1", "matches"])
            .push(match_id);

        self.get_json(url)
    }

    fn get_json<T: DeserializeOwned>(&self, url: Url) -> Result<T, RiotApiError> {
        let response = self
            .http
            .get(url)
            .header("X-Riot-Token", &self.api_key)
            .send()?;

        parse_response(response)
    }
}

fn route_url(route: &str, allowed: &[&str], kind: &'static str) -> Result<Url, RiotApiError> {
    let route = route.to_ascii_lowercase();
    if !allowed.contains(&route.as_str()) {
        return Err(RiotApiError::InvalidRoute { route, kind });
    }

    Ok(Url::parse(&format!("https://{route}.api.riotgames.com/"))
        .expect("a validated Riot route produces a valid URL"))
}

fn parse_response<T: DeserializeOwned>(response: Response) -> Result<T, RiotApiError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response.json()?);
    }

    let status_code = status.as_u16();
    let body = response.text().unwrap_or_default();
    let message = serde_json::from_str::<RiotErrorEnvelope>(&body)
        .map(|error| error.status.message)
        .unwrap_or_else(|_| "request was rejected without a JSON error message".to_owned());

    Err(RiotApiError::Api {
        status: status_code,
        message,
    })
}

#[derive(Debug, Deserialize)]
struct RiotErrorEnvelope {
    status: RiotErrorStatus,
}

#[derive(Debug, Deserialize)]
struct RiotErrorStatus {
    message: String,
}

#[derive(Debug, Deserialize)]
pub struct PlatformStatus {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub maintenances: Vec<StatusNotice>,
    #[serde(default)]
    pub incidents: Vec<StatusNotice>,
}

#[derive(Debug, Deserialize)]
pub struct StatusNotice {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengerLeague {
    pub tier: String,
    pub queue: String,
    pub entries: Vec<LeagueEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeagueEntry {
    pub puuid: String,
    pub league_points: i32,
    pub rank: String,
}

#[derive(Debug, Deserialize)]
pub struct TftMatch {
    pub metadata: MatchMetadata,
    pub info: MatchInfo,
}

impl TftMatch {
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
pub struct MatchMetadata {
    pub data_version: String,
    pub match_id: String,
    pub participants: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct MatchInfo {
    pub game_datetime: u64,
    pub game_version: String,
    pub participants: Vec<Participant>,
}

#[derive(Debug, Deserialize)]
pub struct Participant {
    #[serde(default)]
    pub augments: Vec<String>,
    pub placement: u8,
    pub puuid: String,
    #[serde(default)]
    pub traits: Vec<TraitState>,
    #[serde(default)]
    pub units: Vec<Unit>,
}

#[derive(Debug, Deserialize)]
pub struct TraitState {
    pub name: String,
    #[serde(default)]
    pub tier_current: i32,
}

#[derive(Debug, Deserialize)]
pub struct Unit {
    pub character_id: String,
    #[serde(rename = "itemNames", default)]
    pub item_names: Vec<String>,
    pub tier: u8,
}

fn patch_from_game_version(game_version: &str) -> String {
    game_version
        .split_whitespace()
        .find_map(|part| {
            let version = part
                .trim_matches(|character: char| !character.is_ascii_digit() && character != '.');
            let mut components = version.split('.');
            let major = components.next()?;
            let minor = components.next()?;

            if major.chars().all(|character| character.is_ascii_digit())
                && minor.chars().all(|character| character.is_ascii_digit())
                && !major.is_empty()
                && !minor.is_empty()
            {
                Some(format!("{major}.{minor}"))
            } else {
                None
            }
        })
        .unwrap_or_else(|| game_version.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{RiotApiClient, RiotApiError, TftMatch, patch_from_game_version};

    #[test]
    fn extracts_patch_from_riot_game_version() {
        assert_eq!(
            patch_from_game_version("Linux Version 16.15.693.1856 (Jul 29 2026)"),
            "16.15"
        );
    }

    #[test]
    fn rejects_an_unknown_route_before_sending_a_request() {
        let client = RiotApiClient {
            http: reqwest::blocking::Client::new(),
            api_key: "test-only-key".to_owned(),
        };

        let error = client
            .platform_status("not-a-route")
            .expect_err("an unknown route should fail");

        assert!(matches!(error, RiotApiError::InvalidRoute { .. }));
    }

    #[test]
    fn deserializes_and_converts_a_match_fixture() {
        let json = r#"
        {
          "metadata": {
            "data_version": "5",
            "match_id": "JP1_123",
            "participants": ["player-1"]
          },
          "info": {
            "game_datetime": 1785900000000,
            "game_version": "Linux Version 16.15.693.1856",
            "participants": [{
              "augments": ["TFT_Augment_Test"],
              "placement": 1,
              "puuid": "player-1",
              "traits": [
                {"name": "TFT_Trait_Active", "tier_current": 1},
                {"name": "TFT_Trait_Inactive", "tier_current": 0}
              ],
              "units": [{
                "character_id": "TFT_Champion_Test",
                "itemNames": ["TFT_Item_Test"],
                "tier": 3
              }]
            }]
          }
        }
        "#;

        let riot_match: TftMatch = serde_json::from_str(json).expect("fixture should deserialize");
        let observations = riot_match.into_observations();

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].patch, "16.15");
        assert_eq!(observations[0].placement, 1);
        assert_eq!(observations[0].units[0].star_level, 3);
        assert_eq!(observations[0].traits, ["TFT_Trait_Active"]);
    }
}
