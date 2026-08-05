use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitObservation {
    pub champion: String,
    pub star_level: u8,
    pub items: Vec<String>,
}

impl UnitObservation {
    pub fn new(champion: &str, star_level: u8, items: Vec<&str>) -> Self {
        Self {
            champion: champion.to_owned(),
            star_level,
            items: items.into_iter().map(str::to_owned).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchObservation {
    pub player_id: String,
    pub patch: String,
    pub timestamp: u64,
    pub placement: u8,
    pub units: Vec<UnitObservation>,
    pub traits: Vec<String>,
    pub augments: Vec<String>,
}

impl MatchObservation {
    pub fn new(
        player_id: &str,
        patch: &str,
        timestamp: u64,
        placement: u8,
        units: Vec<UnitObservation>,
        traits: Vec<&str>,
        augments: Vec<&str>,
    ) -> Self {
        assert!(
            (1..=8).contains(&placement),
            "placement must be between 1 and 8"
        );

        Self {
            player_id: player_id.to_owned(),
            patch: patch.to_owned(),
            timestamp,
            placement,
            units,
            traits: traits.into_iter().map(str::to_owned).collect(),
            augments: augments.into_iter().map(str::to_owned).collect(),
        }
    }
}

/// The current, deliberately simple identity for a composition.
///
/// Champion names are sorted and deduplicated, so unit order, star levels,
/// and items do not affect identity yet.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Composition {
    champions: Vec<String>,
}

impl Composition {
    pub fn from_units(units: &[UnitObservation]) -> Self {
        let mut champions: Vec<String> = units.iter().map(|unit| unit.champion.clone()).collect();
        champions.sort();
        champions.dedup();

        // TODO: Introduce similarity-based composition grouping and decide how
        // strongly three-star units should influence that similarity score.
        Self { champions }
    }

    pub fn champions(&self) -> &[String] {
        &self.champions
    }
}

impl fmt::Display for Composition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.champions.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::{Composition, UnitObservation};

    #[test]
    fn composition_identity_is_independent_of_unit_order() {
        let first = vec![
            UnitObservation::new("Neeko", 2, vec!["Warmog's Armor"]),
            UnitObservation::new("Ahri", 2, vec!["Spear of Shojin"]),
        ];
        let second = vec![
            UnitObservation::new("Ahri", 2, vec!["Spear of Shojin"]),
            UnitObservation::new("Neeko", 2, vec!["Warmog's Armor"]),
        ];

        assert_eq!(
            Composition::from_units(&first),
            Composition::from_units(&second)
        );
    }

    #[test]
    fn composition_identity_deduplicates_champions() {
        let units = vec![
            UnitObservation::new("Ahri", 2, vec![]),
            UnitObservation::new("Neeko", 2, vec![]),
            UnitObservation::new("Ahri", 1, vec![]),
        ];

        let composition = Composition::from_units(&units);

        assert_eq!(
            composition.champions(),
            &["Ahri".to_owned(), "Neeko".to_owned()]
        );
    }

    #[test]
    fn current_identity_ignores_star_levels_and_items() {
        let ordinary_board = vec![
            UnitObservation::new("Ahri", 1, vec![]),
            UnitObservation::new("Neeko", 2, vec!["Warmog's Armor"]),
        ];
        let upgraded_board = vec![
            UnitObservation::new("Ahri", 3, vec!["Rabadon's Deathcap"]),
            UnitObservation::new("Neeko", 3, vec!["Ionic Spark"]),
        ];

        assert_eq!(
            Composition::from_units(&ordinary_board),
            Composition::from_units(&upgraded_board)
        );
    }
}
