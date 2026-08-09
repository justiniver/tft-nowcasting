use std::env;
use std::error::Error;
use std::io;

use tft_nowcasting::analysis::summarize_compositions;
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
        Some("ingest") => run_ingestion(),
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
    let mut most_played: Vec<_> = summaries.iter().collect();

    most_played.sort_by(|left, right| {
        right
            .play_count
            .cmp(&left.play_count)
            .then_with(|| left.patch.cmp(&right.patch))
            .then_with(|| left.window.cmp(&right.window))
            .then_with(|| left.composition.cmp(&right.composition))
    });

    println!("Standard ranked analysis ({region})");
    println!("Included matches: {}", dataset.matches);
    println!("Excluded other-mode matches: {}", dataset.excluded_matches);
    println!("Player-match observations: {}", dataset.observations.len());
    println!(
        "Composition families in 24-hour windows: {}",
        summaries.len()
    );
    println!("Most-played composition families:");
    for summary in most_played.into_iter().take(10) {
        println!(
            "- patch {}, window [{}, {}): {} — {} game(s), {:.2} average placement, {:.0}% top four",
            summary.patch,
            summary.window.start,
            summary.window.end_exclusive,
            summary.composition,
            summary.play_count,
            summary.average_placement,
            summary.top_four_rate * 100.0,
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

fn run_ingestion() -> Result<(), Box<dyn Error>> {
    let client = RiotApiClient::from_env()?;
    let store = DataStore::new("data");
    let report = ingest(&client, &store, IngestionConfig::default())?;

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
