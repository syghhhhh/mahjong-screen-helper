use crate::counts;
use riichi_core::{Tile, shanten};

/// Mahjong Soul FAQ: unchanged waits and no kan unrelated to the current draw.
/// Unlike stricter tournament rules, other decomposition changes are allowed.
pub(super) fn riichi_kan_allowed(
    hand: &[Tile],
    own_meld_tiles: &[Tile],
    fixed: u8,
    concealed: u8,
    drawn: Option<Tile>,
    candidate: Tile,
) -> bool {
    if fixed >= 4
        || fixed != concealed
        || hand.len() != 14 - 3 * fixed as usize
        || own_meld_tiles.len() != 4 * fixed as usize
    {
        return false;
    }
    let Some(drawn) = drawn else { return false };
    let index = candidate.index() as usize;
    if drawn.index() != candidate.index() || !hand.contains(&drawn) {
        return false;
    }
    let mut before = counts(hand);
    let mut after_own = counts(own_meld_tiles);
    if before[index] != 4
        || after_own.iter().any(|count| !matches!(count, 0 | 4))
        || (0..34).any(|i| before[i] as u16 + after_own[i] as u16 > 4)
    {
        return false;
    }
    before[index] -= 1;
    if shanten(&before, fixed) != Ok(0) {
        return false;
    }
    let old_waits = waits(&before, &after_own, fixed);
    let mut after = before;
    after[index] -= 3;
    after_own[index] += 4;
    !old_waits.is_empty() && old_waits == waits(&after, &after_own, fixed + 1)
}

/// All structurally winning tile types and physically possible copies, including
/// dead waits in the rivers: external discards cannot change kan legality.
fn waits(hand: &[u8; 34], own: &[u8; 34], fixed: u8) -> Vec<(usize, u8)> {
    let mut result = Vec::new();
    for index in 0..34 {
        let held = hand[index] + own[index];
        if held >= 4 {
            continue;
        }
        let mut completed = *hand;
        completed[index] += 1;
        if shanten(&completed, fixed) == Ok(-1) {
            result.push((index, 4 - held));
        }
    }
    result
}
