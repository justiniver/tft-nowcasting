use std::collections::{HashMap, HashSet};

use crate::model::{Composition, MatchObservation};

const MINIMUM_CHAMPION_OVERLAP: f64 = 0.8;
const MINIMUM_EMERGING_PLAYS: usize = 2;
const MAXIMUM_EMERGING_AVERAGE_PLACEMENT: f64 = 4.5;
const MINIMUM_ESTABLISHED_SIGNALS: usize = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct CompositionSummary {
    pub patch: String,
    pub window: TimeWindow,
    pub composition: Composition,
    pub play_count: usize,
    pub usage_rate: f64,
    /// Percentage-point change from the previous populated window in the patch.
    pub usage_rate_change: Option<f64>,
    pub average_placement: f64,
    pub top_four_rate: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EarlyAdopter {
    pub player_id: String,
    pub patch: String,
    pub window: TimeWindow,
    pub emergence_window: TimeWindow,
    pub composition: Composition,
    pub play_count: usize,
    pub first_played_at: u64,
    pub average_placement: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoutSummary {
    pub player_id: String,
    pub successful_signals: usize,
    pub patches: usize,
    pub early_games: usize,
    pub first_signal_at: u64,
    pub average_placement: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoutForecast {
    pub patch: String,
    pub window: TimeWindow,
    pub composition: Composition,
    pub scout_count: usize,
    pub play_count: usize,
    pub scout_play_rate: f64,
    pub average_placement: f64,
}

/// A fixed, half-open time window: `start <= timestamp < end_exclusive`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimeWindow {
    pub start: u64,
    pub end_exclusive: u64,
}

impl TimeWindow {
    pub fn containing(timestamp: u64, window_size: u64) -> Self {
        assert!(
            window_size > 0,
            "time window size must be greater than zero"
        );

        let start = timestamp - (timestamp % window_size);
        let end_exclusive = start
            .checked_add(window_size)
            .expect("time window end exceeds the largest u64 value");

        Self {
            start,
            end_exclusive,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CompositionGroup {
    patch: String,
    window: TimeWindow,
    composition: Composition,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PatchComposition {
    patch: String,
    composition: Composition,
}

#[derive(Debug, Default)]
struct PlacementAccumulator {
    play_count: usize,
    placement_total: u64,
    top_four_count: usize,
}

struct CandidateWindow {
    patch: String,
    composition: Composition,
    previous_window: TimeWindow,
    emergence_window: TimeWindow,
}

#[derive(PartialEq, Eq, Hash)]
struct AdopterGroup {
    player_id: String,
    patch: String,
    window: TimeWindow,
    emergence_window: TimeWindow,
    composition: Composition,
}

#[derive(Debug, Default)]
struct AdopterAccumulator {
    play_count: usize,
    placement_total: u64,
    first_played_at: Option<u64>,
}

#[derive(Debug, Default)]
struct ScoutAccumulator {
    successful_signals: usize,
    early_games: usize,
    placement_total: f64,
    first_signal_at: Option<u64>,
    patches: HashSet<String>,
}

#[derive(Debug, Default)]
struct ForecastAccumulator {
    play_count: usize,
    placement_total: u64,
    scouts: HashSet<String>,
}

/// Groups observations into similar composition families and calculates basic
/// statistics for each family. The displayed composition is the family's
/// first-seen exact board, with alphabetical ordering used to break ties.
///
/// `&[MatchObservation]` is a borrowed slice: this function can read the
/// observations without taking ownership of them. Time windows are fixed and
/// non-overlapping, with boundaries anchored at timestamp zero.
pub fn summarize_compositions(
    observations: &[MatchObservation],
    window_size: u64,
) -> Vec<CompositionSummary> {
    assert!(
        window_size > 0,
        "time window size must be greater than zero"
    );

    let families = assign_composition_families(observations);
    let mut grouped: HashMap<CompositionGroup, PlacementAccumulator> = HashMap::new();
    let mut window_totals: HashMap<(String, TimeWindow), usize> = HashMap::new();

    for observation in observations {
        let exact_composition = PatchComposition {
            patch: observation.patch.clone(),
            composition: Composition::from_units(&observation.units),
        };
        let representative = families
            .get(&exact_composition)
            .expect("every observed composition should have a family")
            .clone();
        let window = TimeWindow::containing(observation.timestamp, window_size);
        let group = CompositionGroup {
            patch: observation.patch.clone(),
            window,
            composition: representative,
        };
        *window_totals
            .entry((observation.patch.clone(), window))
            .or_default() += 1;
        let accumulator = grouped.entry(group).or_default();
        accumulator.play_count += 1;
        accumulator.placement_total += u64::from(observation.placement);
        if observation.placement <= 4 {
            accumulator.top_four_count += 1;
        }
    }

    let mut summaries: Vec<CompositionSummary> = grouped
        .into_iter()
        .map(|(group, accumulator)| {
            let observations_in_window = window_totals
                .get(&(group.patch.clone(), group.window))
                .expect("every composition group should have a window total");

            CompositionSummary {
                patch: group.patch,
                window: group.window,
                composition: group.composition,
                play_count: accumulator.play_count,
                usage_rate: accumulator.play_count as f64 / *observations_in_window as f64,
                usage_rate_change: None,
                average_placement: accumulator.placement_total as f64
                    / accumulator.play_count as f64,
                top_four_rate: accumulator.top_four_count as f64 / accumulator.play_count as f64,
            }
        })
        .collect();

    // A HashMap has no stable iteration order. Sorting makes CLI output and
    // tests deterministic.
    summaries.sort_by(|left, right| {
        left.patch
            .cmp(&right.patch)
            .then_with(|| left.window.cmp(&right.window))
            .then_with(|| right.play_count.cmp(&left.play_count))
            .then_with(|| left.composition.cmp(&right.composition))
    });
    add_usage_rate_changes(&mut summaries);

    summaries
}

/// Returns growing, successful composition families from the latest window.
/// A candidate needs at least two plays and an average placement of 4.5 or
/// better. This is an intentionally small-sample exploratory filter.
pub fn emerging_candidates(summaries: &[CompositionSummary]) -> Vec<&CompositionSummary> {
    let latest_window = summaries.iter().map(|summary| summary.window).max();
    emerging_events(summaries)
        .into_iter()
        .filter(|summary| Some(summary.window) == latest_window)
        .collect()
}

/// Applies the emerging-candidate rules to every historical window.
pub fn emerging_events(summaries: &[CompositionSummary]) -> Vec<&CompositionSummary> {
    let mut events: Vec<_> = summaries
        .iter()
        .filter(|summary| {
            summary.play_count >= MINIMUM_EMERGING_PLAYS
                && summary.average_placement <= MAXIMUM_EMERGING_AVERAGE_PLACEMENT
                && summary.usage_rate_change.is_some_and(|change| change > 0.0)
        })
        .collect();

    events.sort_by(|left, right| {
        left.window
            .cmp(&right.window)
            .then_with(|| left.patch.cmp(&right.patch))
            .then_with(|| {
                right
                    .usage_rate_change
                    .expect("events should have usage growth")
                    .total_cmp(
                        &left
                            .usage_rate_change
                            .expect("events should have usage growth"),
                    )
            })
            .then_with(|| left.average_placement.total_cmp(&right.average_placement))
            .then_with(|| right.play_count.cmp(&left.play_count))
            .then_with(|| left.composition.cmp(&right.composition))
    });

    events
}

/// Finds players who used an emerging family in its previous populated window.
pub fn early_adopters(
    observations: &[MatchObservation],
    candidates: &[&CompositionSummary],
    window_size: u64,
) -> Vec<EarlyAdopter> {
    assert!(
        window_size > 0,
        "time window size must be greater than zero"
    );

    let candidate_windows: Vec<_> = candidates
        .iter()
        .filter_map(|candidate| {
            let previous_window = observations
                .iter()
                .filter(|observation| observation.patch == candidate.patch)
                .map(|observation| TimeWindow::containing(observation.timestamp, window_size))
                .filter(|window| *window < candidate.window)
                .max()?;

            Some(CandidateWindow {
                patch: candidate.patch.clone(),
                composition: candidate.composition.clone(),
                previous_window,
                emergence_window: candidate.window,
            })
        })
        .collect();

    if candidate_windows.is_empty() {
        return Vec::new();
    }

    let families = assign_composition_families(observations);
    let mut grouped: HashMap<AdopterGroup, AdopterAccumulator> = HashMap::new();

    for observation in observations {
        let window = TimeWindow::containing(observation.timestamp, window_size);
        let exact_composition = PatchComposition {
            patch: observation.patch.clone(),
            composition: Composition::from_units(&observation.units),
        };
        let representative = families
            .get(&exact_composition)
            .expect("every observed composition should have a family");

        for candidate in &candidate_windows {
            if observation.patch != candidate.patch
                || window != candidate.previous_window
                || representative != &candidate.composition
            {
                continue;
            }

            let group = AdopterGroup {
                player_id: observation.player_id.clone(),
                patch: observation.patch.clone(),
                window,
                emergence_window: candidate.emergence_window,
                composition: representative.clone(),
            };
            let accumulator = grouped.entry(group).or_default();
            accumulator.play_count += 1;
            accumulator.placement_total += u64::from(observation.placement);
            accumulator.first_played_at = Some(
                accumulator
                    .first_played_at
                    .map_or(observation.timestamp, |timestamp| {
                        timestamp.min(observation.timestamp)
                    }),
            );
        }
    }

    let mut adopters: Vec<_> = grouped
        .into_iter()
        .map(|(group, accumulator)| EarlyAdopter {
            player_id: group.player_id,
            patch: group.patch,
            window: group.window,
            emergence_window: group.emergence_window,
            composition: group.composition,
            play_count: accumulator.play_count,
            first_played_at: accumulator
                .first_played_at
                .expect("an adopter should have at least one observation"),
            average_placement: accumulator.placement_total as f64 / accumulator.play_count as f64,
        })
        .collect();

    adopters.sort_by(|left, right| {
        left.emergence_window
            .cmp(&right.emergence_window)
            .then_with(|| left.patch.cmp(&right.patch))
            .then_with(|| left.composition.cmp(&right.composition))
            .then_with(|| left.average_placement.total_cmp(&right.average_placement))
            .then_with(|| left.first_played_at.cmp(&right.first_played_at))
            .then_with(|| left.player_id.cmp(&right.player_id))
    });

    adopters
}

/// Ranks players by repeated successful early-adoption signals.
pub fn summarize_scouts(adopters: &[EarlyAdopter]) -> Vec<ScoutSummary> {
    let mut grouped: HashMap<String, ScoutAccumulator> = HashMap::new();

    for adopter in adopters {
        let accumulator = grouped.entry(adopter.player_id.clone()).or_default();
        accumulator.successful_signals += 1;
        accumulator.early_games += adopter.play_count;
        accumulator.placement_total += adopter.average_placement * adopter.play_count as f64;
        accumulator.first_signal_at = Some(
            accumulator
                .first_signal_at
                .map_or(adopter.first_played_at, |timestamp| {
                    timestamp.min(adopter.first_played_at)
                }),
        );
        accumulator.patches.insert(adopter.patch.clone());
    }

    let mut scouts: Vec<_> = grouped
        .into_iter()
        .map(|(player_id, accumulator)| ScoutSummary {
            player_id,
            successful_signals: accumulator.successful_signals,
            patches: accumulator.patches.len(),
            early_games: accumulator.early_games,
            first_signal_at: accumulator
                .first_signal_at
                .expect("a scout should have at least one signal"),
            average_placement: accumulator.placement_total / accumulator.early_games as f64,
        })
        .collect();

    scouts.sort_by(|left, right| {
        right
            .successful_signals
            .cmp(&left.successful_signals)
            .then_with(|| right.patches.cmp(&left.patches))
            .then_with(|| left.average_placement.total_cmp(&right.average_placement))
            .then_with(|| left.first_signal_at.cmp(&right.first_signal_at))
            .then_with(|| left.player_id.cmp(&right.player_id))
    });

    scouts
}

/// Groups the latest boards of players with at least two successful signals.
pub fn forecast_from_scouts(
    observations: &[MatchObservation],
    scouts: &[ScoutSummary],
    window_size: u64,
) -> Vec<ScoutForecast> {
    assert!(
        window_size > 0,
        "time window size must be greater than zero"
    );

    let Some(latest_window) = observations
        .iter()
        .map(|observation| TimeWindow::containing(observation.timestamp, window_size))
        .max()
    else {
        return Vec::new();
    };
    let established_scouts: HashSet<&str> = scouts
        .iter()
        .filter(|scout| scout.successful_signals >= MINIMUM_ESTABLISHED_SIGNALS)
        .map(|scout| scout.player_id.as_str())
        .collect();
    if established_scouts.is_empty() {
        return Vec::new();
    }

    let families = assign_composition_families(observations);
    let mut grouped: HashMap<CompositionGroup, ForecastAccumulator> = HashMap::new();
    let mut total_scout_plays = 0;

    for observation in observations {
        let window = TimeWindow::containing(observation.timestamp, window_size);
        if window != latest_window || !established_scouts.contains(observation.player_id.as_str()) {
            continue;
        }

        let exact_composition = PatchComposition {
            patch: observation.patch.clone(),
            composition: Composition::from_units(&observation.units),
        };
        let representative = families
            .get(&exact_composition)
            .expect("every observed composition should have a family")
            .clone();
        let group = CompositionGroup {
            patch: observation.patch.clone(),
            window,
            composition: representative,
        };
        let accumulator = grouped.entry(group).or_default();
        accumulator.play_count += 1;
        accumulator.placement_total += u64::from(observation.placement);
        accumulator.scouts.insert(observation.player_id.clone());
        total_scout_plays += 1;
    }

    let mut forecasts: Vec<_> = grouped
        .into_iter()
        .map(|(group, accumulator)| ScoutForecast {
            patch: group.patch,
            window: group.window,
            composition: group.composition,
            scout_count: accumulator.scouts.len(),
            play_count: accumulator.play_count,
            scout_play_rate: accumulator.play_count as f64 / total_scout_plays as f64,
            average_placement: accumulator.placement_total as f64 / accumulator.play_count as f64,
        })
        .collect();

    forecasts.sort_by(|left, right| {
        right
            .scout_count
            .cmp(&left.scout_count)
            .then_with(|| right.play_count.cmp(&left.play_count))
            .then_with(|| left.average_placement.total_cmp(&right.average_placement))
            .then_with(|| left.patch.cmp(&right.patch))
            .then_with(|| left.composition.cmp(&right.composition))
    });

    forecasts
}

fn add_usage_rate_changes(summaries: &mut [CompositionSummary]) {
    let mut previous_patch = None;
    let mut previous_rates: HashMap<Composition, f64> = HashMap::new();
    let mut start = 0;

    while start < summaries.len() {
        let patch = summaries[start].patch.clone();
        let window = summaries[start].window;
        let mut end = start + 1;
        while end < summaries.len()
            && summaries[end].patch == patch
            && summaries[end].window == window
        {
            end += 1;
        }

        let summaries_in_window = &mut summaries[start..end];
        if previous_patch.as_deref() == Some(patch.as_str()) {
            for summary in summaries_in_window.iter_mut() {
                let previous_rate = previous_rates
                    .get(&summary.composition)
                    .copied()
                    .unwrap_or(0.0);
                summary.usage_rate_change = Some(summary.usage_rate - previous_rate);
            }
        }

        previous_rates = summaries_in_window
            .iter()
            .map(|summary| (summary.composition.clone(), summary.usage_rate))
            .collect();
        previous_patch = Some(patch);
        start = end;
    }
}

fn assign_composition_families(
    observations: &[MatchObservation],
) -> HashMap<PatchComposition, Composition> {
    let mut first_seen: HashMap<PatchComposition, u64> = HashMap::new();
    for observation in observations {
        let exact_composition = PatchComposition {
            patch: observation.patch.clone(),
            composition: Composition::from_units(&observation.units),
        };
        first_seen
            .entry(exact_composition)
            .and_modify(|timestamp| *timestamp = (*timestamp).min(observation.timestamp))
            .or_insert(observation.timestamp);
    }

    let mut exact_compositions: Vec<_> = first_seen.into_iter().collect();
    exact_compositions.sort_by(|(left, left_timestamp), (right, right_timestamp)| {
        left.patch
            .cmp(&right.patch)
            .then_with(|| left_timestamp.cmp(right_timestamp))
            .then_with(|| left.composition.cmp(&right.composition))
    });

    let mut representatives: Vec<PatchComposition> = Vec::new();
    let mut families = HashMap::new();

    for (exact_composition, _) in exact_compositions {
        let matching_representative = representatives
            .iter()
            .find(|representative| {
                representative.patch == exact_composition.patch
                    && representative
                        .composition
                        .champion_overlap(&exact_composition.composition)
                        >= MINIMUM_CHAMPION_OVERLAP
            })
            .map(|representative| representative.composition.clone());

        let representative = match matching_representative {
            Some(representative) => representative,
            None => {
                representatives.push(exact_composition.clone());
                exact_composition.composition.clone()
            }
        };

        families.insert(exact_composition, representative);
    }

    families
}

#[cfg(test)]
mod tests {
    use super::{
        CompositionSummary, EarlyAdopter, ScoutSummary, TimeWindow, early_adopters,
        emerging_candidates, emerging_events, forecast_from_scouts, summarize_compositions,
        summarize_scouts,
    };
    use crate::model::{Composition, MatchObservation, UnitObservation};

    fn observation(
        player_id: &str,
        patch: &str,
        timestamp: u64,
        placement: u8,
        champions: &[&str],
    ) -> MatchObservation {
        let units = champions
            .iter()
            .map(|champion| UnitObservation::new(champion, 1, vec![]))
            .collect();

        MatchObservation::new(
            player_id,
            patch,
            timestamp,
            placement,
            units,
            vec![],
            vec![],
        )
    }

    #[test]
    fn groups_reordered_boards_and_calculates_average_placement() {
        let observations = vec![
            observation("player-1", "14.1", 100, 2, &["Neeko", "Ahri"]),
            observation("player-2", "14.1", 200, 6, &["Ahri", "Neeko"]),
            observation("player-3", "14.1", 300, 1, &["Jinx", "Vi"]),
        ];

        let summaries = summarize_compositions(&observations, 300);

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].patch, "14.1");
        assert_eq!(summaries[0].window.start, 0);
        assert_eq!(summaries[0].window.end_exclusive, 300);
        assert_eq!(summaries[0].play_count, 2);
        assert_eq!(summaries[0].average_placement, 4.0);
        assert_eq!(summaries[0].composition.to_string(), "Ahri, Neeko");
        assert_eq!(summaries[0].top_four_rate, 0.5);
        assert_eq!(summaries[1].patch, "14.1");
        assert_eq!(summaries[1].window.start, 300);
        assert_eq!(summaries[1].window.end_exclusive, 600);
        assert_eq!(summaries[1].play_count, 1);
        assert_eq!(summaries[1].average_placement, 1.0);
        assert_eq!(summaries[1].composition.to_string(), "Jinx, Vi");
        assert_eq!(summaries[1].top_four_rate, 1.0);
    }

    #[test]
    fn separates_the_same_composition_by_patch_and_time_window() {
        let observations = vec![
            observation("player-1", "14.1", 10, 2, &["Ahri", "Neeko"]),
            observation("player-2", "14.1", 90, 4, &["Neeko", "Ahri"]),
            observation("player-3", "14.1", 100, 6, &["Ahri", "Neeko"]),
            observation("player-4", "14.2", 10, 1, &["Ahri", "Neeko"]),
        ];

        let summaries = summarize_compositions(&observations, 100);

        assert_eq!(summaries.len(), 3);
        assert_eq!(summaries[0].patch, "14.1");
        assert_eq!(summaries[0].window.start, 0);
        assert_eq!(summaries[0].play_count, 2);
        assert_eq!(summaries[1].patch, "14.1");
        assert_eq!(summaries[1].window.start, 100);
        assert_eq!(summaries[1].play_count, 1);
        assert_eq!(summaries[1].usage_rate_change, Some(0.0));
        assert_eq!(summaries[2].patch, "14.2");
        assert_eq!(summaries[2].window.start, 0);
        assert_eq!(summaries[2].play_count, 1);
        assert_eq!(summaries[2].usage_rate_change, None);
    }

    #[test]
    fn current_grouping_ignores_unit_details_traits_and_augments() {
        let observations = vec![
            MatchObservation::new(
                "player-1",
                "14.1",
                10,
                2,
                vec![
                    UnitObservation::new("Ahri", 1, vec![]),
                    UnitObservation::new("Neeko", 2, vec!["Warmog's Armor"]),
                ],
                vec!["Arcanist"],
                vec!["Jeweled Lotus II"],
            ),
            MatchObservation::new(
                "player-2",
                "14.1",
                20,
                4,
                vec![
                    UnitObservation::new("Neeko", 3, vec!["Ionic Spark"]),
                    UnitObservation::new("Ahri", 3, vec!["Rabadon's Deathcap"]),
                ],
                vec!["Different Trait"],
                vec!["Different Augment"],
            ),
        ];

        let summaries = summarize_compositions(&observations, 100);

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].play_count, 2);
    }

    #[test]
    fn groups_boards_with_at_least_eighty_percent_champion_overlap() {
        let observations = vec![
            observation(
                "player-1",
                "14.1",
                10,
                2,
                &["Ahri", "Jinx", "Neeko", "Vi", "Yasuo"],
            ),
            observation(
                "player-2",
                "14.1",
                20,
                6,
                &["Ahri", "Jinx", "Neeko", "Vi", "Zed"],
            ),
        ];

        let summaries = summarize_compositions(&observations, 100);

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].play_count, 2);
        assert_eq!(summaries[0].average_placement, 4.0);
        assert_eq!(summaries[0].top_four_rate, 0.5);
    }

    #[test]
    fn keeps_boards_below_eighty_percent_overlap_separate() {
        let observations = vec![
            observation(
                "player-1",
                "14.1",
                10,
                2,
                &["Ahri", "Jinx", "Neeko", "Vi", "Yasuo"],
            ),
            observation(
                "player-2",
                "14.1",
                20,
                6,
                &["Ahri", "Jinx", "Neeko", "Riven", "Zed"],
            ),
        ];

        let summaries = summarize_compositions(&observations, 100);

        assert_eq!(summaries.len(), 2);
    }

    #[test]
    fn uses_the_same_family_representative_across_time_windows() {
        let common_board = ["Ahri", "Jinx", "Neeko", "Vi", "Yasuo"];
        let observations = vec![
            observation("player-1", "14.1", 10, 2, &common_board),
            observation("player-2", "14.1", 110, 3, &common_board),
            observation(
                "player-3",
                "14.1",
                120,
                4,
                &["Ahri", "Jinx", "Neeko", "Vi", "Zed"],
            ),
        ];

        let summaries = summarize_compositions(&observations, 100);

        assert_eq!(summaries.len(), 2);
        assert_eq!(
            summaries[0].composition.to_string(),
            "Ahri, Jinx, Neeko, Vi, Yasuo"
        );
        assert_eq!(summaries[1].composition, summaries[0].composition);
        assert_eq!(summaries[1].play_count, 2);
    }

    #[test]
    fn later_popularity_does_not_relabel_an_earlier_family() {
        let early_board = ["Ahri", "Jinx", "Neeko", "Vi", "Yasuo"];
        let later_board = ["Ahri", "Jinx", "Neeko", "Vi", "Zed"];
        let observations = vec![
            observation("player-1", "14.1", 10, 2, &early_board),
            observation("player-2", "14.1", 110, 3, &later_board),
            observation("player-3", "14.1", 120, 4, &later_board),
            observation("player-4", "14.1", 130, 5, &later_board),
        ];

        let summaries = summarize_compositions(&observations, 100);

        assert_eq!(
            summaries[0].composition.to_string(),
            "Ahri, Jinx, Neeko, Vi, Yasuo"
        );
        assert_eq!(summaries[1].composition, summaries[0].composition);
    }

    #[test]
    fn calculates_usage_rate_and_change_between_populated_windows() {
        let observations = vec![
            observation("player-1", "14.1", 10, 1, &["Ahri"]),
            observation("player-2", "14.1", 20, 2, &["Ahri"]),
            observation("player-3", "14.1", 30, 3, &["Jinx"]),
            observation("player-4", "14.1", 110, 4, &["Ahri"]),
            observation("player-5", "14.1", 120, 5, &["Jinx"]),
            observation("player-6", "14.1", 130, 6, &["Jinx"]),
            observation("player-7", "14.1", 140, 7, &["Jinx"]),
        ];

        let summaries = summarize_compositions(&observations, 100);

        assert_close(summaries[0].usage_rate, 2.0 / 3.0);
        assert_eq!(summaries[0].usage_rate_change, None);
        assert_close(summaries[1].usage_rate, 1.0 / 3.0);
        assert_eq!(summaries[1].usage_rate_change, None);
        assert_close(summaries[2].usage_rate, 3.0 / 4.0);
        assert_close(
            summaries[2]
                .usage_rate_change
                .expect("the second window should have a comparison"),
            5.0 / 12.0,
        );
        assert_close(summaries[3].usage_rate, 1.0 / 4.0);
        assert_close(
            summaries[3]
                .usage_rate_change
                .expect("the second window should have a comparison"),
            -5.0 / 12.0,
        );
    }

    #[test]
    fn treats_a_family_missing_from_the_previous_window_as_zero_usage() {
        let observations = vec![
            observation("player-1", "14.1", 10, 1, &["Ahri"]),
            observation("player-2", "14.1", 110, 2, &["Jinx"]),
        ];

        let summaries = summarize_compositions(&observations, 100);

        assert_eq!(summaries[0].usage_rate_change, None);
        assert_eq!(summaries[1].usage_rate_change, Some(1.0));
    }

    #[test]
    fn emerging_candidates_require_growth_performance_and_multiple_plays() {
        let observations = vec![
            observation("player-1", "14.1", 10, 1, &["Ahri"]),
            observation("player-2", "14.1", 20, 2, &["Jinx"]),
            observation("player-3", "14.1", 30, 3, &["Jinx"]),
            observation("player-4", "14.1", 40, 4, &["Jinx"]),
            observation("player-5", "14.1", 110, 1, &["Ahri"]),
            observation("player-6", "14.1", 120, 2, &["Ahri"]),
            observation("player-7", "14.1", 130, 3, &["Jinx"]),
            observation("player-8", "14.1", 140, 4, &["Jinx"]),
            observation("player-9", "14.1", 150, 1, &["Neeko"]),
            observation("player-10", "14.1", 160, 7, &["Vi"]),
            observation("player-11", "14.1", 170, 8, &["Vi"]),
        ];
        let summaries = summarize_compositions(&observations, 100);

        let candidates = emerging_candidates(&summaries);
        let adopters = early_adopters(&observations, &candidates, 100);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].composition.to_string(), "Ahri");
        assert_eq!(candidates[0].play_count, 2);
        assert_eq!(candidates[0].average_placement, 1.5);
        assert_eq!(adopters.len(), 1);
        assert_eq!(adopters[0].player_id, "player-1");
        assert_eq!(adopters[0].window.start, 0);
        assert_eq!(adopters[0].emergence_window.start, 100);
        assert_eq!(adopters[0].play_count, 1);
        assert_eq!(adopters[0].first_played_at, 10);
        assert_eq!(adopters[0].average_placement, 1.0);
    }

    #[test]
    fn historical_events_include_windows_before_the_latest_one() {
        let summaries = vec![
            CompositionSummary {
                patch: "14.1".to_owned(),
                window: TimeWindow::containing(110, 100),
                composition: Composition::from_units(&[UnitObservation::new("Ahri", 1, vec![])]),
                play_count: 2,
                usage_rate: 0.25,
                usage_rate_change: Some(0.10),
                average_placement: 3.0,
                top_four_rate: 0.5,
            },
            CompositionSummary {
                patch: "14.1".to_owned(),
                window: TimeWindow::containing(210, 100),
                composition: Composition::from_units(&[UnitObservation::new("Jinx", 1, vec![])]),
                play_count: 2,
                usage_rate: 0.30,
                usage_rate_change: Some(0.15),
                average_placement: 2.0,
                top_four_rate: 1.0,
            },
        ];

        let events = emerging_events(&summaries);
        let latest = emerging_candidates(&summaries);

        assert_eq!(events.len(), 2);
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].composition.to_string(), "Jinx");
    }

    #[test]
    fn ranks_scouts_by_repeated_signals_across_patches() {
        let composition = Composition::from_units(&[UnitObservation::new("Ahri", 1, vec![])]);
        let adopters = vec![
            EarlyAdopter {
                player_id: "player-a".to_owned(),
                patch: "14.1".to_owned(),
                window: TimeWindow::containing(10, 100),
                emergence_window: TimeWindow::containing(110, 100),
                composition: composition.clone(),
                play_count: 1,
                first_played_at: 10,
                average_placement: 2.0,
            },
            EarlyAdopter {
                player_id: "player-a".to_owned(),
                patch: "14.2".to_owned(),
                window: TimeWindow::containing(210, 100),
                emergence_window: TimeWindow::containing(310, 100),
                composition: composition.clone(),
                play_count: 2,
                first_played_at: 210,
                average_placement: 4.0,
            },
            EarlyAdopter {
                player_id: "player-b".to_owned(),
                patch: "14.1".to_owned(),
                window: TimeWindow::containing(10, 100),
                emergence_window: TimeWindow::containing(110, 100),
                composition,
                play_count: 1,
                first_played_at: 20,
                average_placement: 1.0,
            },
        ];

        let scouts = summarize_scouts(&adopters);

        assert_eq!(scouts.len(), 2);
        assert_eq!(scouts[0].player_id, "player-a");
        assert_eq!(scouts[0].successful_signals, 2);
        assert_eq!(scouts[0].patches, 2);
        assert_eq!(scouts[0].early_games, 3);
        assert_close(scouts[0].average_placement, 10.0 / 3.0);
    }

    #[test]
    fn forecasts_from_the_latest_boards_of_established_scouts() {
        let observations = vec![
            observation("scout-a", "14.1", 110, 2, &["Ahri"]),
            observation("scout-a", "14.1", 120, 4, &["Ahri"]),
            observation("scout-b", "14.1", 130, 1, &["Ahri"]),
            observation("scout-b", "14.1", 140, 5, &["Jinx"]),
            observation("new-player", "14.1", 150, 1, &["Ahri"]),
        ];
        let scouts = vec![
            ScoutSummary {
                player_id: "scout-a".to_owned(),
                successful_signals: 2,
                patches: 1,
                early_games: 2,
                first_signal_at: 10,
                average_placement: 2.0,
            },
            ScoutSummary {
                player_id: "scout-b".to_owned(),
                successful_signals: 3,
                patches: 2,
                early_games: 3,
                first_signal_at: 20,
                average_placement: 3.0,
            },
            ScoutSummary {
                player_id: "new-player".to_owned(),
                successful_signals: 1,
                patches: 1,
                early_games: 1,
                first_signal_at: 30,
                average_placement: 1.0,
            },
        ];

        let forecasts = forecast_from_scouts(&observations, &scouts, 100);

        assert_eq!(forecasts.len(), 2);
        assert_eq!(forecasts[0].composition.to_string(), "Ahri");
        assert_eq!(forecasts[0].scout_count, 2);
        assert_eq!(forecasts[0].play_count, 3);
        assert_close(forecasts[0].scout_play_rate, 0.75);
        assert_close(forecasts[0].average_placement, 7.0 / 3.0);
        assert_eq!(forecasts[1].composition.to_string(), "Jinx");
        assert_close(forecasts[1].scout_play_rate, 0.25);
    }

    #[test]
    fn empty_input_produces_no_summaries() {
        assert!(summarize_compositions(&[], 100).is_empty());
    }

    #[test]
    #[should_panic(expected = "time window size must be greater than zero")]
    fn rejects_a_zero_sized_time_window() {
        let _ = summarize_compositions(&[], 0);
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-10);
    }
}
