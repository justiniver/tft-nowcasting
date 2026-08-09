use std::env;
use std::error::Error;
use std::io;

use tft_nowcasting::analysis::{
    early_adopters, emerging_candidates, emerging_events, forecast_from_scouts,
    summarize_compositions, summarize_scouts,
};
use tft_nowcasting::api::RiotApiClient;
use tft_nowcasting::audit::audit_cached_matches;
use tft_nowcasting::dataset::load_standard_ranked_dataset;
use tft_nowcasting::ingestion::{IngestionConfig, ingest};
use tft_nowcasting::model::{MatchObservation, UnitObservation};
use tft_nowcasting::storage::DataStore;

const SAMPLE_WINDOW_SIZE: u64 = 300;
const ANALYSIS_WINDOW_SIZE_MS: u64 = 24 * 60 * 60 * 1_000;

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    match env::args().nth(1).as_deref() {
        None => {
            run_local_demo();
            Ok(())
        }
        Some("api-smoke") => run_api_smoke_test(),
        Some("analyze") => run_cached_analysis(),
        Some("audit") => run_dataset_audit(),
        Some("ingest") => run_ingestion(ingestion_config_from_args(env::args().skip(2))?),
        Some(command) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown command {command:?}; try `cargo run -- ingest`"),
        )
        .into()),
    }
}

fn run_cached_analysis() -> Result<(), Box<dyn Error>> {
    let _ = dotenvy::dotenv();
    let region = env::var("RIOT_REGION").unwrap_or_else(|_| "asia".to_owned());
    let store = DataStore::new("data");
    let dataset = load_standard_ranked_dataset(&store, &region)?;
    let summaries = summarize_compositions(&dataset.observations, ANALYSIS_WINDOW_SIZE_MS);
    let historical_events = emerging_events(&summaries);
    let adopters = early_adopters(
        &dataset.observations,
        &historical_events,
        ANALYSIS_WINDOW_SIZE_MS,
    );
    let scouts = summarize_scouts(&adopters);
    let forecasts = forecast_from_scouts(&dataset.observations, &scouts, ANALYSIS_WINDOW_SIZE_MS);
    let candidates = emerging_candidates(&summaries);

    println!("Standard ranked analysis ({region})");
    println!("Included matches: {}", dataset.matches);
    println!("Excluded other-mode matches: {}", dataset.excluded_matches);
    println!("Player-match observations: {}", dataset.observations.len());
    println!(
        "Composition families in 24-hour windows: {}",
        summaries.len()
    );
    println!("Emerging candidates in the latest window:");
    if candidates.is_empty() {
        println!("- None met the growth, performance, and minimum-play filters");
    }
    for summary in candidates.into_iter().take(10) {
        let usage_change = summary
            .usage_rate_change
            .map(|change| format!("{:+.1} pp", change * 100.0))
            .unwrap_or_else(|| "no prior window".to_owned());
        println!(
            "- patch {}, window [{}, {}): {} — {} game(s), {:.1}% usage ({usage_change}), {:.2} average placement, {:.0}% top four",
            summary.patch,
            summary.window.start,
            summary.window.end_exclusive,
            summary.composition,
            summary.play_count,
            summary.usage_rate * 100.0,
            summary.average_placement,
            summary.top_four_rate * 100.0,
        );

        let mut matching_adopters = adopters
            .iter()
            .filter(|adopter| {
                adopter.patch == summary.patch
                    && adopter.emergence_window == summary.window
                    && adopter.composition == summary.composition
            })
            .take(5)
            .peekable();
        if matching_adopters.peek().is_none() {
            println!("  Previous-window adopters: none");
        }
        for adopter in matching_adopters {
            println!(
                "  Previous-window adopter {} — {} game(s), {:.2} average placement, first played at {}",
                adopter.player_id,
                adopter.play_count,
                adopter.average_placement,
                adopter.first_played_at,
            );
        }
    }

    println!(
        "Historical replay: {} successful growth event(s), {} early-adopter signal(s)",
        historical_events.len(),
        adopters.len()
    );
    println!("Scout leaderboard:");
    if scouts.is_empty() {
        println!("- No historical scout signals yet");
    }
    for scout in scouts.into_iter().take(10) {
        println!(
            "- {} — {} successful signal(s) across {} patch(es), {} early game(s), {:.2} average placement",
            scout.player_id,
            scout.successful_signals,
            scout.patches,
            scout.early_games,
            scout.average_placement,
        );
    }

    println!("Next-window forecast from established scouts' latest boards:");
    if forecasts.is_empty() {
        println!("- No established scouts played in the latest window");
    }
    for forecast in forecasts.into_iter().take(10) {
        println!(
            "- patch {}, window [{}, {}): {} — {} scout(s), {} play(s), {:.1}% of scout plays, {:.2} average placement",
            forecast.patch,
            forecast.window.start,
            forecast.window.end_exclusive,
            forecast.composition,
            forecast.scout_count,
            forecast.play_count,
            forecast.scout_play_rate * 100.0,
            forecast.average_placement,
        );
    }

    Ok(())
}

fn run_dataset_audit() -> Result<(), Box<dyn Error>> {
    let _ = dotenvy::dotenv();
    let region = env::var("RIOT_REGION").unwrap_or_else(|_| "asia".to_owned());
    let store = DataStore::new("data");
    let audit = audit_cached_matches(&store, &region)?;

    println!("Cached dataset audit ({region})");
    println!("Matches: {}", audit.matches);
    println!("Player-match observations: {}", audit.observations);
    println!("Unique players: {}", audit.unique_players);
    println!(
        "Exact champion-set compositions: {}",
        audit.unique_compositions
    );
    println!(
        "Average final-board units: {:.2}",
        audit.average_units_per_observation()
    );
    println!(
        "Observations missing units/augments: {}/{}",
        audit.observations_without_units, audit.observations_without_augments
    );
    println!(
        "Timestamp range: {:?} to {:?}",
        audit.earliest_timestamp, audit.latest_timestamp
    );
    println!("Observations by patch: {:?}", audit.observations_by_patch);
    println!("Matches by queue: {:?}", audit.matches_by_queue);
    println!("Matches by game type: {:?}", audit.matches_by_game_type);
    println!("Matches by set: {:?}", audit.matches_by_set);
    println!("Most common exact compositions:");
    for (composition, count) in audit.most_common_compositions() {
        println!("- {composition}: {count} observation(s)");
    }

    Ok(())
}

fn run_ingestion(config: IngestionConfig) -> Result<(), Box<dyn Error>> {
    let client = RiotApiClient::from_env()?;
    let store = DataStore::new("data");
    let report = ingest(&client, &store, config)?;

    println!(
        "Requested up to {} player(s) and {} match(es) per player",
        config.player_limit, config.matches_per_player
    );
    println!(
        "Saved ladder snapshot: {}",
        report.ladder_snapshot.display()
    );
    println!("Players considered: {}", report.players_considered);
    println!("Unique matches: {}", report.unique_matches);
    println!("Downloaded matches: {}", report.downloaded_matches);
    println!("Cache hits: {}", report.cached_matches);
    println!("Player-match observations: {}", report.observations);

    Ok(())
}

fn ingestion_config_from_args(
    mut args: impl Iterator<Item = String>,
) -> Result<IngestionConfig, io::Error> {
    let defaults = IngestionConfig::default();
    let player_limit = match args.next() {
        Some(value) => value.parse::<usize>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("player limit must be a positive integer, got {value:?}"),
            )
        })?,
        None => defaults.player_limit,
    };
    let matches_per_player = match args.next() {
        Some(value) => value.parse::<u8>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("matches per player must be a positive integer, got {value:?}"),
            )
        })?,
        None => defaults.matches_per_player,
    };

    if player_limit == 0 || matches_per_player == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ingestion limits must be greater than zero",
        ));
    }
    if let Some(value) = args.next() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unexpected ingestion argument {value:?}"),
        ));
    }

    Ok(IngestionConfig {
        player_limit,
        matches_per_player,
    })
}

fn run_local_demo() {
    let observations = sample_observations();
    let summaries = summarize_compositions(&observations, SAMPLE_WINDOW_SIZE);

    println!("Composition summary");
    for summary in summaries {
        let game_label = if summary.play_count == 1 {
            "game"
        } else {
            "games"
        };
        println!(
            "- patch {}, window [{}, {}): {} — {} {}, {:.2} average placement, {:.0}% top four",
            summary.patch,
            summary.window.start,
            summary.window.end_exclusive,
            summary.composition,
            summary.play_count,
            game_label,
            summary.average_placement,
            summary.top_four_rate * 100.0,
        );
    }
}

fn run_api_smoke_test() -> Result<(), Box<dyn Error>> {
    let client = RiotApiClient::from_env()?;

    let status = client.platform_status()?;
    println!(
        "Authenticated with Riot: {} ({}) has {} maintenance notice(s) and {} incident(s)",
        status.name,
        status.id,
        status.maintenances.len(),
        status.incidents.len(),
    );

    let challenger = client.challenger_league()?;
    println!(
        "Fetched the Challenger ladder: {} player(s)",
        challenger.entries.len()
    );

    let player = challenger
        .entries
        .first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Challenger ladder was empty"))?;
    let match_ids = client.match_ids_by_puuid(&player.puuid, 0, 1)?;
    let match_id = match_ids.first().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "the selected Challenger player had no recent TFT matches",
        )
    })?;

    let riot_match = client.match_by_id(match_id)?;
    let fetched_match_id = riot_match.id().to_owned();
    let observations = riot_match.into_observations();
    let unit_count: usize = observations
        .iter()
        .map(|observation| observation.units.len())
        .sum();

    println!(
        "Fetched match {fetched_match_id}: {} participant(s), {unit_count} final-board unit(s)",
        observations.len()
    );

    Ok(())
}

fn sample_observations() -> Vec<MatchObservation> {
    vec![
        MatchObservation::new(
            "player-1",
            "14.1",
            100,
            2,
            vec![
                UnitObservation::new("Neeko", 2, vec!["Warmog's Armor"]),
                UnitObservation::new("Ahri", 2, vec!["Spear of Shojin"]),
            ],
            vec!["Arcanist"],
            vec!["Jeweled Lotus II"],
        ),
        MatchObservation::new(
            "player-2",
            "14.1",
            200,
            4,
            vec![
                UnitObservation::new("Ahri", 3, vec!["Rabadon's Deathcap"]),
                UnitObservation::new("Neeko", 2, vec!["Ionic Spark"]),
            ],
            vec!["Arcanist"],
            vec!["Pandora's Items"],
        ),
        MatchObservation::new(
            "player-3",
            "14.1",
            300,
            1,
            vec![
                UnitObservation::new("Jinx", 2, vec!["Infinity Edge"]),
                UnitObservation::new("Vi", 2, vec!["Warmog's Armor"]),
            ],
            vec!["Punk"],
            vec!["Harmacist II"],
        ),
        MatchObservation::new(
            "player-4",
            "14.1",
            400,
            5,
            vec![
                UnitObservation::new("Vi", 3, vec!["Sunfire Cape"]),
                UnitObservation::new("Jinx", 3, vec!["Last Whisper"]),
            ],
            vec!["Punk"],
            vec!["Team Building"],
        ),
        MatchObservation::new(
            "player-5",
            "14.1",
            500,
            3,
            vec![
                UnitObservation::new("Ahri", 2, vec!["Blue Buff"]),
                UnitObservation::new("Neeko", 2, vec!["Dragon's Claw"]),
            ],
            vec!["Arcanist"],
            vec!["Magic Wand"],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::ingestion_config_from_args;

    #[test]
    fn ingestion_arguments_override_the_defaults() {
        let config = ingestion_config_from_args(["10".to_owned(), "20".to_owned()].into_iter())
            .expect("valid limits should parse");

        assert_eq!(config.player_limit, 10);
        assert_eq!(config.matches_per_player, 20);
    }

    #[test]
    fn ingestion_arguments_reject_zero_limits() {
        let error = ingestion_config_from_args(["0".to_owned()].into_iter())
            .expect_err("zero should be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
}
