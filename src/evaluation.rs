use std::collections::{HashMap, HashSet};

use crate::analysis::{
    CompositionAnalysis, CompositionSummary, TimeWindow, emerging_events, summarize_scouts,
};
use crate::model::{Composition, MatchObservation};

const MINIMUM_BASELINE_PLAYS: usize = 2;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MethodEvaluation {
    pub predictions: usize,
    pub hits: usize,
}

impl MethodEvaluation {
    pub fn hit_rate(&self) -> f64 {
        if self.predictions == 0 {
            0.0
        } else {
            self.hits as f64 / self.predictions as f64
        }
    }

    pub fn coverage(&self, event_windows: usize) -> f64 {
        if event_windows == 0 {
            0.0
        } else {
            self.predictions as f64 / event_windows as f64
        }
    }

    fn record(&mut self, prediction: Option<&Composition>, targets: &HashSet<Composition>) {
        let Some(prediction) = prediction else {
            return;
        };

        self.predictions += 1;
        if targets.contains(prediction) {
            self.hits += 1;
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct HistoricalForecastEvaluation {
    pub event_windows: usize,
    pub scout: MethodEvaluation,
    pub popularity: MethodEvaluation,
    pub performance: MethodEvaluation,
}

/// Rebuilds each forecast using only data available through the prior window.
/// A hit means the top prediction became a successful growth event in the next
/// populated window of the same patch.
pub fn evaluate_historical_forecasts(
    observations: &[MatchObservation],
    window_size: u64,
) -> HistoricalForecastEvaluation {
    let analysis = CompositionAnalysis::new(observations, window_size);
    evaluate_historical_forecasts_with_analysis(&analysis)
}

pub fn evaluate_historical_forecasts_with_analysis(
    analysis: &CompositionAnalysis<'_>,
) -> HistoricalForecastEvaluation {
    let observations = analysis.observations();
    let window_size = analysis.window_size();
    let full_summaries = analysis.summarize_compositions();
    let target_events = emerging_events(&full_summaries);
    let all_adopters = analysis.early_adopters(&target_events);
    let mut windows_by_patch: HashMap<String, HashSet<TimeWindow>> = HashMap::new();
    for observation in observations {
        windows_by_patch
            .entry(observation.patch.clone())
            .or_default()
            .insert(TimeWindow::containing(observation.timestamp, window_size));
    }

    let mut evaluation = HistoricalForecastEvaluation::default();
    for (patch, windows) in windows_by_patch {
        let mut windows: Vec<_> = windows.into_iter().collect();
        windows.sort();

        for adjacent_windows in windows.windows(2) {
            let forecast_window = adjacent_windows[0];
            let target_window = adjacent_windows[1];
            let targets: HashSet<_> = target_events
                .iter()
                .filter(|event| event.patch == patch && event.window == target_window)
                .map(|event| event.composition.clone())
                .collect();
            if targets.is_empty() {
                continue;
            }

            evaluation.event_windows += 1;
            let historical_adopters: Vec<_> = all_adopters
                .iter()
                .filter(|adopter| {
                    adopter.emergence_window.end_exclusive <= forecast_window.end_exclusive
                })
                .cloned()
                .collect();
            let scouts = summarize_scouts(&historical_adopters);
            let forecasts = analysis.forecast_from_scouts_in_window(&scouts, forecast_window);
            let current_summaries: Vec<_> = full_summaries
                .iter()
                .filter(|summary| summary.patch == patch && summary.window == forecast_window)
                .collect();

            evaluation.scout.record(
                forecasts
                    .iter()
                    .find(|forecast| forecast.patch == patch)
                    .map(|forecast| &forecast.composition),
                &targets,
            );
            evaluation
                .popularity
                .record(popularity_baseline(&current_summaries), &targets);
            evaluation
                .performance
                .record(performance_baseline(&current_summaries), &targets);
        }
    }

    evaluation
}

fn popularity_baseline<'a>(summaries: &[&'a CompositionSummary]) -> Option<&'a Composition> {
    summaries
        .iter()
        .copied()
        .min_by(|left, right| {
            right
                .usage_rate
                .total_cmp(&left.usage_rate)
                .then_with(|| left.composition.cmp(&right.composition))
        })
        .map(|summary| &summary.composition)
}

fn performance_baseline<'a>(summaries: &[&'a CompositionSummary]) -> Option<&'a Composition> {
    summaries
        .iter()
        .copied()
        .filter(|summary| summary.play_count >= MINIMUM_BASELINE_PLAYS)
        .min_by(|left, right| {
            left.average_placement
                .total_cmp(&right.average_placement)
                .then_with(|| right.play_count.cmp(&left.play_count))
                .then_with(|| left.composition.cmp(&right.composition))
        })
        .map(|summary| &summary.composition)
}

#[cfg(test)]
mod tests {
    use crate::analysis::CompositionAnalysis;
    use crate::model::{MatchObservation, UnitObservation};

    use super::{evaluate_historical_forecasts, evaluate_historical_forecasts_with_analysis};

    #[test]
    fn empty_dataset_produces_an_empty_evaluation() {
        let evaluation = evaluate_historical_forecasts(&[], 100);

        assert_eq!(evaluation.event_windows, 0);
        assert_eq!(evaluation.scout.hit_rate(), 0.0);
        assert_eq!(evaluation.scout.coverage(evaluation.event_windows), 0.0);
    }

    #[test]
    fn replays_scout_forecasts_without_using_the_target_window() {
        let observations = vec![
            observation("scout", 10, 2, "Ahri"),
            observation("p-1", 20, 8, "Vi"),
            observation("p-2", 30, 7, "Vi"),
            observation("p-3", 40, 6, "Vi"),
            observation("p-4", 110, 1, "Ahri"),
            observation("p-5", 120, 2, "Ahri"),
            observation("scout", 130, 3, "Jinx"),
            observation("p-6", 140, 8, "Vi"),
            observation("p-7", 210, 1, "Jinx"),
            observation("p-8", 220, 2, "Jinx"),
            observation("scout", 230, 3, "Neeko"),
            observation("p-9", 240, 8, "Vi"),
            observation("p-10", 310, 1, "Neeko"),
            observation("p-11", 320, 2, "Neeko"),
            observation("p-12", 330, 7, "Vi"),
            observation("p-13", 340, 8, "Vi"),
        ];

        let evaluation = evaluate_historical_forecasts(&observations, 100);
        let analysis = CompositionAnalysis::new(&observations, 100);
        assert_eq!(
            evaluation,
            evaluate_historical_forecasts_with_analysis(&analysis)
        );

        assert_eq!(evaluation.event_windows, 3);
        assert_eq!(evaluation.scout.predictions, 1);
        assert_eq!(evaluation.scout.hits, 1);
        assert_eq!(evaluation.scout.hit_rate(), 1.0);
        assert_eq!(
            evaluation.scout.coverage(evaluation.event_windows),
            1.0 / 3.0
        );
        assert_eq!(evaluation.popularity.predictions, 3);
        assert_eq!(evaluation.popularity.hits, 0);
        assert_eq!(evaluation.performance.predictions, 3);
        assert_eq!(evaluation.performance.hits, 0);
    }

    fn observation(
        player_id: &str,
        timestamp: u64,
        placement: u8,
        champion: &str,
    ) -> MatchObservation {
        MatchObservation::new(
            player_id,
            "14.1",
            timestamp,
            placement,
            vec![UnitObservation::new(champion, 1, vec![])],
            vec![],
            vec![],
        )
    }
}
