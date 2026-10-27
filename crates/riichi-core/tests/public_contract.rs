use riichi_core::{Tile, recommend_discards, shanten, ukeire};

fn tiles(codes: &[&str]) -> Vec<Tile> {
    codes
        .iter()
        .map(|code| Tile::from_code(code).unwrap())
        .collect()
}

fn counts(hand: &[Tile]) -> [u8; 34] {
    let mut counts = [0; 34];
    for tile in hand {
        counts[tile.index() as usize] += 1;
    }
    counts
}

#[test]
fn completed_standard_hand_is_minus_one_shanten() {
    let hand = tiles(&[
        "1m", "2m", "3m", "1p", "2p", "3p", "1s", "2s", "3s", "1z", "1z", "1z", "5m", "5m",
    ]);
    assert_eq!(shanten(&counts(&hand), 0).unwrap(), -1);
}

#[test]
fn special_hands_are_supported() {
    let seven_pairs = tiles(&[
        "1m", "1m", "2m", "2m", "3m", "3m", "4p", "4p", "5p", "5p", "6p", "6p", "7s",
    ]);
    assert_eq!(shanten(&counts(&seven_pairs), 0).unwrap(), 0);
    let kokushi = tiles(&[
        "1m", "9m", "1p", "9p", "1s", "9s", "1z", "2z", "3z", "4z", "5z", "6z", "7z",
    ]);
    assert_eq!(shanten(&counts(&kokushi), 0).unwrap(), 0);
    let mut won = kokushi;
    won.push(Tile::from_code("1m").unwrap());
    assert_eq!(shanten(&counts(&won), 0).unwrap(), -1);
}

#[test]
fn ukeire_subtracts_visible_tiles() {
    let hand = tiles(&[
        "1m", "2m", "3m", "1p", "2p", "3p", "1s", "2s", "3s", "1z", "1z", "1z", "5m",
    ]);
    let mut visible = [0; 34];
    visible[Tile::from_code("5m").unwrap().index() as usize] = 1;
    let result = ukeire(&counts(&hand), &visible, 0).unwrap();
    assert_eq!(result.count, 2);
    assert_eq!(result.tiles, vec![Tile::from_code("5m").unwrap()]);
}

#[test]
fn recommendations_are_legal_and_deterministic() {
    let hand = tiles(&[
        "1m", "2m", "3m", "4m", "5m", "6m", "1p", "2p", "3p", "4s", "5s", "6s", "7z", "9m",
    ]);
    let a = recommend_discards(&hand, &[0; 34], &[], 0, 0.95).unwrap();
    let b = recommend_discards(&hand, &[0; 34], &[], 0, 0.95).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.len(), 3);
    for candidate in a {
        assert!(hand.contains(&candidate.discard));
        assert!(candidate.ukeire <= 136);
        assert_eq!(candidate.confidence, 0.95);
    }
}

#[test]
fn top_discard_uses_remaining_tiles() {
    let hand = tiles(&[
        "1m", "2m", "3m", "1p", "2p", "3p", "1s", "2s", "3s", "1z", "1z", "1z", "5m", "9m",
    ]);
    let mut visible = [0; 34];
    visible[Tile::from_code("9m").unwrap().index() as usize] = 2;
    let result = recommend_discards(&hand, &visible, &[], 0, 1.0).unwrap();
    assert_eq!(result[0].discard, Tile::from_code("9m").unwrap());
    assert_eq!(result[0].ukeire, 3);
}

#[test]
fn illegal_five_of_a_kind_is_rejected() {
    let mut hand = tiles(&[
        "1m", "1m", "1m", "1m", "1m", "2m", "3m", "4m", "5m", "6m", "7m", "8m", "9m",
    ]);
    assert!(shanten(&counts(&hand), 0).is_err());
    hand.push(Tile::from_code("9m").unwrap());
    assert!(recommend_discards(&hand, &[0; 34], &[], 0, 1.0).is_err());
}
