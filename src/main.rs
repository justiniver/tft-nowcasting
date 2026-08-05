use std::env;
use std::error::Error;
use std::io;

use tft_nowcasting::analysis::summarize_compositions;
use tft_nowcasting::api::RiotApiClient;
use tft_nowcasting::model::{MatchObservation, UnitObservation};

const SAMPLE_WINDOW_SIZE: u64 = 300;

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
        Some(command) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown command {command:?}; try `cargo run -- api-smoke`"),
        )
        .into()),
    }
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
    let platform = env::var("RIOT_PLATFORM").unwrap_or_else(|_| "jp1".to_owned());
    let region = env::var("RIOT_REGION").unwrap_or_else(|_| "asia".to_owned());

    let status = client.platform_status(&platform)?;
    println!(
        "Authenticated with Riot: {} ({}) has {} maintenance notice(s) and {} incident(s)",
        status.name,
        status.id,
        status.maintenances.len(),
        status.incidents.len(),
    );

    let challenger = client.challenger_league(&platform)?;
    println!(
        "Fetched the {platform} Challenger ladder: {} player(s)",
        challenger.entries.len()
    );

    let player = challenger
        .entries
        .first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Challenger ladder was empty"))?;
    let match_ids = client.match_ids_by_puuid(&region, &player.puuid, 0, 1)?;
    let match_id = match_ids.first().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "the selected Challenger player had no recent TFT matches",
        )
    })?;

    let riot_match = client.match_by_id(&region, match_id)?;
    let fetched_match_id = riot_match.metadata.match_id.clone();
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
