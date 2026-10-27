use riichi_core::Tile;
use vision_core::PublicSnapshot;

/// A limited visual transition, evaluated only after the full stability filter:
/// a previously played table is cleared, and its indicator row resets to one
/// different first indicator (or shrinks from a kan-expanded row).
/// The concealed hand must change beyond a single draw/discard as well.
/// An unchanged single indicator alone cannot establish a new round.
pub(super) fn fresh_round(
    previous: &PublicSnapshot,
    current: &PublicSnapshot,
    previous_hand: &[Tile],
    current_hand: &[Tile],
) -> bool {
    matches!(current_hand.len(), 13 | 14)
        && !previous.dora_indicators.is_empty()
        && previous.visible_tiles.len() > previous.dora_indicators.len()
        && current.dora_indicators.len() == 1
        && current.visible_tiles == current.dora_indicators
        && current.own_meld_tiles.is_empty()
        && current.offered.is_none()
        && (previous.dora_indicators.len() > 1
            || previous.dora_indicators.first() != current.dora_indicators.first())
        && incompatible_hand(previous_hand, current_hand)
}

fn incompatible_hand(previous: &[Tile], current: &[Tile]) -> bool {
    let mut before = [0u8; 34];
    let mut after = [0u8; 34];
    for tile in previous {
        before[tile.index() as usize] += 1;
    }
    for tile in current {
        after[tile.index() as usize] += 1;
    }
    let removed: u8 = (0..34)
        .map(|index| before[index].saturating_sub(after[index]))
        .sum();
    let added: u8 = (0..34)
        .map(|index| after[index].saturating_sub(before[index]))
        .sum();
    removed >= 2 && added >= 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(dora: &[&str], public: &[&str]) -> PublicSnapshot {
        let dora: Vec<_> = dora
            .iter()
            .map(|code| Tile::from_code(code).unwrap())
            .collect();
        let mut visible = dora.clone();
        visible.extend(public.iter().map(|code| Tile::from_code(code).unwrap()));
        visible.sort();
        PublicSnapshot {
            visible_tiles: visible,
            own_meld_tiles: vec![],
            dora_indicators: dora,
            offered: None,
        }
    }

    #[test]
    fn requires_a_played_table_clear_and_indicator_reset_together() {
        let previous = table(&["1m"], &["2p", "3s"]);
        assert!(fresh(&previous, &table(&["2m"], &[]), 14));
        assert!(!fresh(&previous, &table(&["2m"], &["2p"]), 14));
        assert!(!fresh(&previous, &table(&[], &[]), 14));
        assert!(!fresh(&previous, &table(&["1m"], &[]), 14));
        assert!(!fresh(&table(&["1m"], &[]), &table(&["2m"], &[]), 14));
        assert!(!fresh(&previous, &table(&["2m"], &[]), 11));
    }

    #[test]
    fn a_new_kan_indicator_does_not_reset_the_round() {
        let previous = table(&["1m"], &["2p"]);
        assert!(!fresh(&previous, &table(&["1m", "2m"], &["2p"]), 14));
        let expanded = table(&["1m", "2m"], &["2p"]);
        assert!(fresh(&expanded, &table(&["1m"], &[]), 13));
    }

    fn fresh(previous: &PublicSnapshot, current: &PublicSnapshot, size: usize) -> bool {
        let old: Vec<_> = [
            "1m", "1m", "1m", "2m", "3m", "4m", "5p", "6p", "7p", "2s", "3s", "5z", "5z",
        ]
        .into_iter()
        .map(|code| Tile::from_code(code).unwrap())
        .collect();
        let new: Vec<_> = [
            "9m", "9m", "9m", "6m", "7m", "8m", "1p", "2p", "3p", "5s", "6s", "7s", "4z", "4z",
        ]
        .into_iter()
        .map(|code| Tile::from_code(code).unwrap())
        .collect();
        fresh_round(previous, current, &old, &new[..size])
    }

    #[test]
    fn an_unchanged_hand_or_one_draw_discard_is_not_enough() {
        let old = table(&["1m", "2m"], &["2p"]);
        let new = table(&["1m"], &[]);
        let hand: Vec<_> = [
            "1m", "2m", "3m", "4m", "5m", "6m", "2p", "3p", "4p", "2s", "3s", "7z", "7z",
        ]
        .into_iter()
        .map(|code| Tile::from_code(code).unwrap())
        .collect();
        assert!(!fresh_round(&old, &new, &hand, &hand));
        let mut drawn = hand.clone();
        drawn.push(Tile::from_code("9p").unwrap());
        assert!(!fresh_round(&old, &new, &hand, &drawn));
        let mut swapped = hand.clone();
        swapped[0] = Tile::from_code("9p").unwrap();
        assert!(!fresh_round(&old, &new, &hand, &swapped));
    }
}
