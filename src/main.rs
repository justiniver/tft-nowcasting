use tft_nowcasting::analysis::summarize_compositions;
use tft_nowcasting::model::MatchObservation;

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
        MatchObservation::new("player-1", "14.1", 100, 2, vec!["Neeko", "Ahri"]),
        MatchObservation::new("player-2", "14.1", 200, 4, vec!["Ahri", "Neeko"]),
        MatchObservation::new("player-3", "14.1", 300, 1, vec!["Jinx", "Vi"]),
        MatchObservation::new("player-4", "14.1", 400, 5, vec!["Vi", "Jinx"]),
        MatchObservation::new("player-5", "14.1", 500, 3, vec!["Ahri", "Neeko"]),
    ]
}
