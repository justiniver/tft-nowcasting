use std::collections::HashMap;

use crate::model::{Composition, MatchObservation};

const MINIMUM_CHAMPION_OVERLAP: f64 = 0.8;

#[derive(Debug, Clone, PartialEq)]
pub struct CompositionSummary {
    pub patch: String,
    pub window: TimeWindow,
    pub composition: Composition,
    pub play_count: usize,
    pub average_placement: f64,
    pub top_four_rate: f64,
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

/// Groups observations into similar composition families and calculates basic
/// statistics for each family. The displayed composition is the family's most
/// common exact board, with alphabetical ordering used to break ties.
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

    for observation in observations {
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
            window: TimeWindow::containing(observation.timestamp, window_size),
            composition: representative,
        };
        let accumulator = grouped.entry(group).or_default();
        accumulator.play_count += 1;
        accumulator.placement_total += u64::from(observation.placement);
        if observation.placement <= 4 {
            accumulator.top_four_count += 1;
        }
    }

    let mut summaries: Vec<CompositionSummary> = grouped
        .into_iter()
        .map(|(group, accumulator)| CompositionSummary {
            patch: group.patch,
            window: group.window,
            composition: group.composition,
            play_count: accumulator.play_count,
            average_placement: accumulator.placement_total as f64 / accumulator.play_count as f64,
            top_four_rate: accumulator.top_four_count as f64 / accumulator.play_count as f64,
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

    summaries
}

fn assign_composition_families(
    observations: &[MatchObservation],
) -> HashMap<PatchComposition, Composition> {
    let mut exact_counts: HashMap<PatchComposition, usize> = HashMap::new();
    for observation in observations {
        let exact_composition = PatchComposition {
            patch: observation.patch.clone(),
            composition: Composition::from_units(&observation.units),
        };
        *exact_counts.entry(exact_composition).or_default() += 1;
    }

    let mut exact_compositions: Vec<_> = exact_counts.into_iter().collect();
    exact_compositions.sort_by(|(left, left_count), (right, right_count)| {
        left.patch
            .cmp(&right.patch)
            .then_with(|| right_count.cmp(left_count))
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
    use super::summarize_compositions;
    use crate::model::{MatchObservation, UnitObservation};

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
        assert_eq!(summaries[2].patch, "14.2");
        assert_eq!(summaries[2].window.start, 0);
        assert_eq!(summaries[2].play_count, 1);
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
    fn empty_input_produces_no_summaries() {
        assert!(summarize_compositions(&[], 100).is_empty());
    }

    #[test]
    #[should_panic(expected = "time window size must be greater than zero")]
    fn rejects_a_zero_sized_time_window() {
        let _ = summarize_compositions(&[], 0);
    }
}
