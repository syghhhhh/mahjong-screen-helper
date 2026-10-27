//! Deterministic, offline four-player riichi hand-efficiency calculations.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tile {
    index: u8,
    red: bool,
}

impl Tile {
    pub fn new(index: u8, red: bool) -> Result<Self, CoreError> {
        if index >= 34 || (red && !matches!(index, 4 | 13 | 22)) {
            return Err(CoreError::InvalidTile);
        }
        Ok(Self { index, red })
    }

    pub fn from_code(code: &str) -> Result<Self, CoreError> {
        let bytes = code.as_bytes();
        if bytes.len() != 2 {
            return Err(CoreError::InvalidTile);
        }
        let red = bytes[0] == b'0';
        let number = if red { 5 } else { bytes[0].wrapping_sub(b'0') };
        let index = match bytes[1] {
            b'm' if (1..=9).contains(&number) => number - 1,
            b'p' if (1..=9).contains(&number) => number + 8,
            b's' if (1..=9).contains(&number) => number + 17,
            b'z' if !red && (1..=7).contains(&number) => number + 26,
            _ => return Err(CoreError::InvalidTile),
        };
        Self::new(index, red)
    }

    pub fn index(self) -> u8 {
        self.index
    }
    pub fn is_red(self) -> bool {
        self.red
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreError {
    InvalidTile,
    InvalidTileCount,
    InvalidHandSize,
    InvalidOpenMelds,
    InvalidConfidence,
    InvalidVisibleCount,
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for CoreError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ukeire {
    pub shanten: i8,
    pub count: u16,
    pub tiles: Vec<Tile>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recommendation {
    pub discard: Tile,
    pub rank: u8,
    pub shanten: i8,
    pub ukeire: u16,
    pub improving_tiles: Vec<Tile>,
    pub confidence: f32,
    pub reason: String,
}

fn validate_counts(counts: &[u8; 34], open_melds: u8) -> Result<usize, CoreError> {
    if open_melds > 4 {
        return Err(CoreError::InvalidOpenMelds);
    }
    if counts.iter().any(|&count| count > 4) {
        return Err(CoreError::InvalidTileCount);
    }
    let total: usize = counts.iter().map(|&count| count as usize).sum();
    let base = 13 - 3 * open_melds as usize;
    if total != base && total != base + 1 {
        return Err(CoreError::InvalidHandSize);
    }
    Ok(total)
}

/// Exact minimum of normal, seven-pairs, and thirteen-orphans shanten.
/// `counts` contains concealed tiles; each open meld counts as three effective tiles.
pub fn shanten(counts: &[u8; 34], open_melds: u8) -> Result<i8, CoreError> {
    validate_counts(counts, open_melds)?;
    let mut working = *counts;
    let mut best = 8_i8;
    search_normal(&mut working, 0, open_melds, 0, 0, false, &mut best);
    if open_melds == 0 {
        best = best.min(seven_pairs_shanten(counts));
        best = best.min(thirteen_orphans_shanten(counts));
    }
    Ok(best)
}

fn search_normal(
    counts: &mut [u8; 34],
    start: usize,
    open_melds: u8,
    melds: u8,
    taatsu: u8,
    pair: bool,
    best: &mut i8,
) {
    let Some(i) = (start..34).find(|&i| counts[i] != 0) else {
        let melds = melds + open_melds;
        let useful_taatsu = taatsu.min(4 - melds);
        let value = 8 - 2 * melds as i8 - useful_taatsu as i8 - i8::from(pair);
        *best = (*best).min(value);
        return;
    };

    if melds + open_melds < 4 {
        if counts[i] >= 3 {
            counts[i] -= 3;
            search_normal(counts, i, open_melds, melds + 1, taatsu, pair, best);
            counts[i] += 3;
        }
        if i < 27 && i % 9 <= 6 && counts[i + 1] > 0 && counts[i + 2] > 0 {
            counts[i] -= 1;
            counts[i + 1] -= 1;
            counts[i + 2] -= 1;
            search_normal(counts, i, open_melds, melds + 1, taatsu, pair, best);
            counts[i] += 1;
            counts[i + 1] += 1;
            counts[i + 2] += 1;
        }
    }
    if counts[i] >= 2 {
        counts[i] -= 2;
        if !pair {
            search_normal(counts, i, open_melds, melds, taatsu, true, best);
        }
        if taatsu < 4 {
            search_normal(counts, i, open_melds, melds, taatsu + 1, pair, best);
        }
        counts[i] += 2;
    }
    if taatsu < 4 && i < 27 {
        if i % 9 <= 7 && counts[i + 1] > 0 {
            counts[i] -= 1;
            counts[i + 1] -= 1;
            search_normal(counts, i, open_melds, melds, taatsu + 1, pair, best);
            counts[i] += 1;
            counts[i + 1] += 1;
        }
        if i % 9 <= 6 && counts[i + 2] > 0 {
            counts[i] -= 1;
            counts[i + 2] -= 1;
            search_normal(counts, i, open_melds, melds, taatsu + 1, pair, best);
            counts[i] += 1;
            counts[i + 2] += 1;
        }
    }
    counts[i] -= 1;
    search_normal(counts, i, open_melds, melds, taatsu, pair, best);
    counts[i] += 1;
}

fn seven_pairs_shanten(counts: &[u8; 34]) -> i8 {
    let pair_types = counts.iter().filter(|&&count| count >= 2).count() as i8;
    let distinct = counts.iter().filter(|&&count| count > 0).count() as i8;
    6 - pair_types + (7 - distinct).max(0)
}

fn thirteen_orphans_shanten(counts: &[u8; 34]) -> i8 {
    const ORPHANS: [usize; 13] = [0, 8, 9, 17, 18, 26, 27, 28, 29, 30, 31, 32, 33];
    let distinct = ORPHANS.iter().filter(|&&index| counts[index] > 0).count() as i8;
    let has_pair = ORPHANS.iter().any(|&index| counts[index] >= 2);
    13 - distinct - i8::from(has_pair)
}

/// Count physically remaining tiles that reduce shanten after a draw.
/// `visible` excludes the concealed hand and includes all visible discards/melds/indicators.
pub fn ukeire(counts: &[u8; 34], visible: &[u8; 34], open_melds: u8) -> Result<Ukeire, CoreError> {
    let total = validate_counts(counts, open_melds)?;
    if total != 13 - 3 * open_melds as usize {
        return Err(CoreError::InvalidHandSize);
    }
    if (0..34).any(|i| counts[i] as u16 + visible[i] as u16 > 4) {
        return Err(CoreError::InvalidVisibleCount);
    }
    let baseline = shanten(counts, open_melds)?;
    let mut improving = Vec::new();
    let mut remaining = 0_u16;
    let mut drawn = *counts;
    for i in 0..34 {
        let copies = 4 - counts[i] - visible[i];
        if copies == 0 {
            continue;
        }
        drawn[i] += 1;
        let next = shanten(&drawn, open_melds)?;
        drawn[i] -= 1;
        if next < baseline {
            remaining += copies as u16;
            improving.push(Tile::new(i as u8, false).expect("valid tile index"));
        }
    }
    Ok(Ukeire {
        shanten: baseline,
        count: remaining,
        tiles: improving,
    })
}

fn dora_from_indicator(indicator: Tile) -> u8 {
    let index = indicator.index;
    if index < 27 {
        index / 9 * 9 + (index % 9 + 1) % 9
    } else if index <= 30 {
        27 + (index - 27 + 1) % 4
    } else {
        31 + (index - 31 + 1) % 3
    }
}

/// Rank up to three distinct discard choices by shanten, remaining ukeire,
/// and preservation of red/dora tiles when the first two metrics tie.
pub fn recommend_discards(
    hand: &[Tile],
    visible: &[u8; 34],
    dora_indicators: &[Tile],
    open_melds: u8,
    confidence: f32,
) -> Result<Vec<Recommendation>, CoreError> {
    if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
        return Err(CoreError::InvalidConfidence);
    }
    if open_melds > 4 {
        return Err(CoreError::InvalidOpenMelds);
    }
    if hand.len() != 14 - 3 * open_melds as usize {
        return Err(CoreError::InvalidHandSize);
    }
    let mut counts = [0_u8; 34];
    for tile in hand {
        counts[tile.index as usize] += 1;
    }
    validate_counts(&counts, open_melds)?;
    if (0..34).any(|i| counts[i] as u16 + visible[i] as u16 > 4) {
        return Err(CoreError::InvalidVisibleCount);
    }
    let mut candidates = Vec::new();
    let mut choices = hand.to_vec();
    choices.sort();
    choices.dedup();
    for tile in choices {
        counts[tile.index as usize] -= 1;
        let result = ukeire(&counts, visible, open_melds)?;
        counts[tile.index as usize] += 1;
        candidates.push(Recommendation {
            discard: tile,
            rank: 0,
            shanten: result.shanten,
            ukeire: result.count,
            improving_tiles: result.tiles,
            confidence,
            reason: format!("{} 向听，{} 张有效牌", result.shanten, result.count),
        });
    }
    let dora: Vec<u8> = dora_indicators
        .iter()
        .copied()
        .map(dora_from_indicator)
        .collect();
    candidates.sort_by(|a, b| {
        let value = |tile: Tile| {
            u8::from(tile.red) + dora.iter().filter(|&&d| d == tile.index).count() as u8
        };
        a.shanten
            .cmp(&b.shanten)
            .then_with(|| b.ukeire.cmp(&a.ukeire))
            .then_with(|| value(a.discard).cmp(&value(b.discard)))
            .then_with(|| a.discard.cmp(&b.discard))
    });
    candidates.truncate(3);
    for (i, candidate) in candidates.iter_mut().enumerate() {
        candidate.rank = (i + 1) as u8;
    }
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn red_five_and_dora_cycle() {
        assert_eq!(Tile::from_code("0m").unwrap(), Tile::new(4, true).unwrap());
        assert_eq!(dora_from_indicator(Tile::from_code("9m").unwrap()), 0);
        assert_eq!(dora_from_indicator(Tile::from_code("4z").unwrap()), 27);
        assert_eq!(dora_from_indicator(Tile::from_code("7z").unwrap()), 31);
        assert!(Tile::from_code("0z").is_err());
    }

    #[test]
    fn open_meld_disables_special_hands() {
        let mut counts = [0; 34];
        for code in ["1m", "2m", "3m", "4p", "5p", "6p", "7s", "8s", "9s", "1z"] {
            counts[Tile::from_code(code).unwrap().index() as usize] += 1;
        }
        assert_eq!(shanten(&counts, 1).unwrap(), 0);
    }
}
