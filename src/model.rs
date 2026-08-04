use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchObservation {
    pub player_id: String,
    pub patch: String,
    pub timestamp: u64,
    pub placement: u8,
    pub champions: Vec<String>,
}

impl MatchObservation {
    pub fn new(
        player_id: &str,
        patch: &str,
        timestamp: u64,
        placement: u8,
        champions: Vec<&str>,
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
            champions: champions.into_iter().map(str::to_owned).collect(),
        }
    }
}

/// A normalized identity for a composition.
///
/// Champion names are sorted and deduplicated, so `["Ahri", "Neeko"]` and
/// `["Neeko", "Ahri"]` produce the same key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Composition {
    champions: Vec<String>,
}

impl Composition {
    pub fn from_champions(champions: &[String]) -> Self {
        // `to_vec` clones the strings because we only borrowed the input slice.
        let mut normalized = champions.to_vec();
        normalized.sort();
        normalized.dedup();

        // TODO: Include items, star levels, traits, and augments once the
        // champion-set baseline is working end to end.
        Self {
            champions: normalized,
        }
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
    use super::Composition;

    #[test]
    fn composition_identity_is_independent_of_champion_order() {
        let first = vec!["Neeko".to_owned(), "Ahri".to_owned()];
        let second = vec!["Ahri".to_owned(), "Neeko".to_owned()];

        assert_eq!(
            Composition::from_champions(&first),
            Composition::from_champions(&second)
        );
    }

    #[test]
    fn composition_identity_deduplicates_champions() {
        let champions = vec!["Ahri".to_owned(), "Neeko".to_owned(), "Ahri".to_owned()];

        let composition = Composition::from_champions(&champions);

        assert_eq!(
            composition.champions(),
            &["Ahri".to_owned(), "Neeko".to_owned()]
        );
    }
}
