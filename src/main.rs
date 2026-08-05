use tft_nowcasting::analysis::summarize_compositions;
use tft_nowcasting::model::{MatchObservation, UnitObservation};

const SAMPLE_WINDOW_SIZE: u64 = 300;

fn main() {
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
