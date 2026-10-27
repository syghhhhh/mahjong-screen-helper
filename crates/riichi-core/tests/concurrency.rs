use riichi_core::{Recommendation, Tile, recommend_discards, shanten};

// Each worker owns its hand and reference results. This exercises the same
// recursive rule code as the image pipeline without vision, state or Win32.
#[test]
fn independently_owned_hands_keep_exact_results_across_ten_workers() {
    std::thread::scope(|scope| {
        for worker in 0..10 {
            scope.spawn(move || {
                let mut references: [Option<Vec<Recommendation>>; 2] = [None, None];
                for iteration in 0..20 {
                    let variant = (worker + iteration) % 2;
                    let mut hand: Vec<_> = [
                        "1m", "2m", "3m", "4m", "5m", "6m", "2p", "3p", "4p", "2s", "3s", "7z",
                        "7z",
                    ]
                    .into_iter()
                    .map(|code| Tile::from_code(code).unwrap())
                    .collect();
                    hand.push(Tile::from_code(if variant == 0 { "9p" } else { "1s" }).unwrap());
                    let mut counts = [0; 34];
                    for tile in &hand {
                        counts[tile.index() as usize] += 1;
                    }
                    // 9p leaves three melds, a pair and 23s. 1s completes
                    // the fourth meld. These expectations do not use a
                    // second invocation of the algorithm as an oracle.
                    assert_eq!(
                        shanten(&counts, 0).unwrap(),
                        if variant == 0 { 0 } else { -1 }
                    );
                    let advice = recommend_discards(&hand, &[0; 34], &[], 0, 1.0).unwrap();
                    assert_eq!(advice.len(), 3);
                    for (index, candidate) in advice.iter().enumerate() {
                        assert!(hand.contains(&candidate.discard));
                        assert_eq!(candidate.rank as usize, index + 1);
                    }
                    if let Some(reference) = &references[variant] {
                        assert_eq!(&advice, reference);
                    } else {
                        references[variant] = Some(advice);
                    }
                }
            });
        }
    });
}
