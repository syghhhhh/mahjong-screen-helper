use game_state::{Phase, StateEvent};
use riichi_core::Tile;
use runtime_core::{PipelineOutput, PublicImages, RuntimeEngine};
use vision_core::{ActionLabel, MatchConfig, RgbImage, TemplateBank};

const W: u32 = 16;
const H: u32 = 20;

fn art(seed: usize) -> RgbImage {
    let mut rgb = vec![240; (W * H * 3) as usize];
    for y in 3..H - 3 {
        for x in 3..W - 3 {
            if ((x * (seed as u32 + 1) + y * 3) % 23) < 11 {
                let at = ((y * W + x) * 3) as usize;
                rgb[at..at + 3].copy_from_slice(&[20, 25, 30]);
            }
        }
    }
    RgbImage::new(W, H, rgb).unwrap()
}

fn hand_tiles() -> Vec<Tile> {
    [
        "1m", "2m", "3m", "4m", "5m", "6m", "2p", "3p", "4p", "2s", "3s", "7z", "7z",
    ]
    .into_iter()
    .map(|code| Tile::from_code(code).unwrap())
    .collect()
}

fn bank() -> TemplateBank {
    let mut tiles: Vec<Tile> = hand_tiles();
    tiles.push(Tile::from_code("9p").unwrap());
    tiles.push(Tile::from_code("1s").unwrap());
    tiles.sort();
    tiles.dedup();
    let tile_templates = tiles
        .into_iter()
        .enumerate()
        .map(|(i, tile)| (tile, art(i)))
        .collect();
    let action_templates = vec![
        (ActionLabel::None, art(30)),
        (ActionLabel::Riichi, art(31)),
        (ActionLabel::Pon, art(32)),
        (ActionLabel::Kan, art(33)),
    ];
    TemplateBank::new(tile_templates, action_templates).unwrap()
}

fn scene(bank: &TemplateBank, hand: &[Tile]) -> RgbImage {
    let width = (W + 2) * hand.len() as u32 + 4;
    let height = H + 4;
    let mut rgb = vec![31; (width * height * 3) as usize];
    for (index, tile) in hand.iter().enumerate() {
        let template = &bank
            .tile_templates()
            .iter()
            .find(|(label, _)| label == tile)
            .unwrap()
            .1;
        for y in 0..H {
            for x in 0..W {
                let src = ((y * W + x) * 3) as usize;
                let dst = (((y + 2) * width + index as u32 * (W + 2) + x + 2) * 3) as usize;
                rgb[dst..dst + 3].copy_from_slice(&template.pixels()[src..src + 3]);
            }
        }
    }
    RgbImage::new(width, height, rgb).unwrap()
}

fn action_scene(bank: &TemplateBank, action: ActionLabel) -> RgbImage {
    bank.action_templates()
        .iter()
        .find(|(label, _)| *label == action)
        .unwrap()
        .1
        .clone()
}

fn public_scene(bank: &TemplateBank, tile: Tile, highlighted: bool) -> RgbImage {
    let mut image = RgbImage::new(100, 100, vec![38; 100 * 100 * 3]).unwrap();
    let source = &bank
        .tile_templates()
        .iter()
        .find(|(label, _)| *label == tile)
        .unwrap()
        .1;
    for y in 33..57 {
        for x in 72..92 {
            let at = ((y * 100 + x) * 3) as usize;
            if (74..90).contains(&x) && (35..55).contains(&y) {
                let from = (((y - 35) * W + x - 74) * 3) as usize;
                image.pixels_mut()[at..at + 3].copy_from_slice(&source.pixels()[from..from + 3]);
            } else if highlighted {
                image.pixels_mut()[at..at + 3].copy_from_slice(&[70, 225, 100]);
            }
        }
    }
    image
}

#[test]
fn stable_public_pixels_feed_offers_and_changes_clear_stale_call_advice() {
    let bank = bank();
    let mut engine = RuntimeEngine::new(bank.clone(), vec![], MatchConfig::default(), 3).unwrap();
    let hand = scene(&bank, &hand_tiles());
    let actions = action_scene(&bank, ActionLabel::Pon);
    let empty = RgbImage::new(60, 60, vec![38; 60 * 60 * 3]).unwrap();
    let river = public_scene(&bank, Tile::from_code("7z").unwrap(), true);
    let images = PublicImages {
        river: &river,
        own_melds: &empty,
        opponent_melds: [&empty; 3],
        dora: &empty,
    };
    for frame in 1..=2 {
        assert!(matches!(
            engine.process_with_public(frame, &hand, Some(&actions), &images),
            PipelineOutput::Waiting
        ));
    }
    match engine.process_with_public(3, &hand, Some(&actions), &images) {
        PipelineOutput::Ready { actions, .. } => assert!(
            actions
                .iter()
                .any(|advice| advice.action == ActionLabel::Pon)
        ),
        output => panic!("expected stable call advice, got {output:?}"),
    }
    let river = public_scene(&bank, Tile::from_code("9p").unwrap(), false);
    let images = PublicImages {
        river: &river,
        own_melds: &empty,
        opponent_melds: [&empty; 3],
        dora: &empty,
    };
    assert!(matches!(
        engine.process_with_public(4, &hand, Some(&actions), &images),
        PipelineOutput::Waiting
    ));
    assert!(matches!(
        engine.process_with_public(5, &hand, Some(&actions), &images),
        PipelineOutput::Waiting
    ));
    match engine.process_with_public(6, &hand, Some(&actions), &images) {
        PipelineOutput::Ready { actions, .. } => assert!(actions.is_empty()),
        output => panic!("expected stable public update, got {output:?}"),
    }
}

#[test]
fn pixels_after_a_pon_reconcile_the_open_hand_and_resume_discard_advice() {
    let bank = bank();
    let mut engine = RuntimeEngine::new(bank.clone(), vec![], MatchConfig::default(), 1).unwrap();
    let hand = hand_tiles();
    let empty = RgbImage::new(60, 60, vec![38; 60 * 60 * 3]).unwrap();
    let river = public_scene(&bank, Tile::from_code("7z").unwrap(), true);
    let initial = PublicImages {
        river: &river,
        own_melds: &empty,
        opponent_melds: [&empty; 3],
        dora: &empty,
    };
    assert!(matches!(
        engine.process_with_public(
            1,
            &scene(&bank, &hand),
            Some(&action_scene(&bank, ActionLabel::Pon)),
            &initial
        ),
        PipelineOutput::Ready { .. }
    ));
    let after: Vec<_> = hand.into_iter().filter(|tile| tile.index() != 33).collect();
    let melds = scene(&bank, &[Tile::from_code("7z").unwrap(); 3]);
    let public = PublicImages {
        river: &empty,
        own_melds: &melds,
        opponent_melds: [&empty; 3],
        dora: &empty,
    };
    match engine.process_with_public(2, &scene(&bank, &after), None, &public) {
        PipelineOutput::Ready {
            phase,
            events,
            discards,
            ..
        } => {
            assert_eq!(phase, Phase::AwaitDiscard);
            assert!(events.contains(&StateEvent::CallDeclared(ActionLabel::Pon)));
            assert_eq!(engine.tracker().open_melds(), 1);
            assert!(!discards.is_empty());
        }
        output => panic!("expected an open-hand recommendation, got {output:?}"),
    }
}

#[test]
fn self_kan_overrides_a_stale_river_highlight_and_preserves_closed_riichi_eligibility() {
    let bank = bank();
    let mut engine = RuntimeEngine::new(bank.clone(), vec![], MatchConfig::default(), 1).unwrap();
    let hand: Vec<_> = [
        "1s", "1s", "1s", "1s", "1m", "2m", "3m", "4m", "5m", "6m", "2p", "3p", "7z", "7z",
    ]
    .into_iter()
    .map(|code| Tile::from_code(code).unwrap())
    .collect();
    let empty = RgbImage::new(60, 60, vec![38; 60 * 60 * 3]).unwrap();
    let river = public_scene(&bank, Tile::from_code("7z").unwrap(), true);
    let public = PublicImages {
        river: &river,
        own_melds: &empty,
        opponent_melds: [&empty; 3],
        dora: &empty,
    };
    match engine.process_with_public(
        1,
        &scene(&bank, &hand),
        Some(&action_scene(&bank, ActionLabel::Kan)),
        &public,
    ) {
        PipelineOutput::Ready { actions, .. } => assert!(
            actions
                .iter()
                .any(|advice| advice.tiles_from_hand.len() == 4 && advice.reason.contains("暗杠"))
        ),
        output => panic!("expected self-kan advice, got {output:?}"),
    }
    let after: Vec<_> = hand.into_iter().filter(|tile| tile.index() != 18).collect();
    let melds = scene(&bank, &[Tile::from_code("1s").unwrap(); 4]);
    let public = PublicImages {
        river: &river,
        own_melds: &melds,
        opponent_melds: [&empty; 3],
        dora: &empty,
    };
    assert!(matches!(
        engine.process_with_public(2, &scene(&bank, &after), None, &public),
        PipelineOutput::Ready {
            phase: Phase::AwaitDraw,
            ..
        }
    ));
    assert_eq!(engine.tracker().concealed_kans(), 1);
    assert!(engine.tracker().is_closed());
    let mut drawn = after;
    drawn.push(Tile::from_code("9p").unwrap());
    match engine.process_with_public(
        3,
        &scene(&bank, &drawn),
        Some(&action_scene(&bank, ActionLabel::Riichi)),
        &public,
    ) {
        PipelineOutput::Ready { actions, .. } => assert!(
            actions
                .iter()
                .any(|advice| advice.action == ActionLabel::Riichi)
        ),
        output => panic!("expected riichi advice after concealed kan, got {output:?}"),
    }
}

#[test]
fn pixels_after_confirmed_riichi_keep_legal_kan_waits_and_suppress_discards() {
    let bank = bank();
    let mut engine = RuntimeEngine::new(bank.clone(), vec![], MatchConfig::default(), 1).unwrap();
    let base: Vec<_> = [
        "1m", "1m", "1m", "2m", "3m", "4m", "2p", "3p", "4p", "2s", "3s", "7z", "7z",
    ]
    .into_iter()
    .map(|code| Tile::from_code(code).unwrap())
    .collect();
    let empty = RgbImage::new(60, 60, vec![38; 60 * 60 * 3]).unwrap();
    let public = PublicImages {
        river: &empty,
        own_melds: &empty,
        opponent_melds: [&empty; 3],
        dora: &empty,
    };
    let mut declaration = base.clone();
    declaration.push(Tile::from_code("9p").unwrap());
    assert!(matches!(
        engine.process_with_public(
            1,
            &scene(&bank, &declaration),
            Some(&action_scene(&bank, ActionLabel::Riichi)),
            &public
        ),
        PipelineOutput::Ready { .. }
    ));
    engine.tracker_mut().confirm_riichi().unwrap();
    engine.process_with_public(2, &scene(&bank, &base), None, &public);
    let mut drawn = base;
    drawn.push(Tile::from_code("1m").unwrap());
    match engine.process_with_public(
        3,
        &scene(&bank, &drawn),
        Some(&action_scene(&bank, ActionLabel::Kan)),
        &public,
    ) {
        PipelineOutput::Ready {
            actions, discards, ..
        } => {
            assert!(discards.is_empty());
            assert_eq!(actions.len(), 1);
            assert!(actions[0].reason.contains("立直后待牌不变"));
        }
        other => panic!("expected legal post-riichi kan, got {other:?}"),
    }
    let melds = scene(&bank, &[Tile::from_code("1m").unwrap(); 4]);
    let public = PublicImages {
        river: &empty,
        own_melds: &melds,
        opponent_melds: [&empty; 3],
        dora: &empty,
    };
    let mut remaining: Vec<_> = drawn.into_iter().filter(|tile| tile.index() != 0).collect();
    assert!(matches!(
        engine.process_with_public(4, &scene(&bank, &remaining), None, &public),
        PipelineOutput::Ready {
            phase: Phase::AwaitDraw,
            ..
        }
    ));
    remaining.push(Tile::from_code("9p").unwrap());
    match engine.process_with_public(5, &scene(&bank, &remaining), None, &public) {
        PipelineOutput::Ready { discards, .. } => assert!(discards.is_empty()),
        other => panic!("expected replacement draw after kan, got {other:?}"),
    }
    assert!(engine.tracker().riichi_declared());
    assert_eq!(engine.tracker().concealed_kans(), 1);
}

#[test]
fn public_tiles_reduce_ukeire_and_impossible_combined_counts_stop_output() {
    let bank = bank();
    let mut hand = hand_tiles();
    hand.push(Tile::from_code("9p").unwrap());
    let image = scene(&bank, &hand);
    let mut engine = RuntimeEngine::new(bank.clone(), vec![], MatchConfig::default(), 1).unwrap();
    let base = match engine.process(1, &image, None) {
        PipelineOutput::Ready { discards, .. } => discards,
        other => panic!("{other:?}"),
    };
    let (discard, improving) = base
        .iter()
        .find_map(|advice| {
            advice
                .improving_tiles
                .iter()
                .find(|tile| {
                    bank.tile_templates()
                        .iter()
                        .any(|(label, _)| label == *tile)
                })
                .map(|tile| (advice.clone(), *tile))
        })
        .expect("fixture has a represented improving tile");
    let river = public_scene(&bank, improving, false);
    let empty = RgbImage::new(60, 60, vec![38; 60 * 60 * 3]).unwrap();
    let images = PublicImages {
        river: &river,
        own_melds: &empty,
        opponent_melds: [&empty; 3],
        dora: &empty,
    };
    match engine.process_with_public(2, &image, None, &images) {
        PipelineOutput::Ready { discards, .. } => assert_eq!(
            discards
                .iter()
                .find(|advice| advice.discard == discard.discard)
                .unwrap()
                .ukeire,
            discard.ukeire - 1
        ),
        output => panic!("expected a public-adjusted recommendation, got {output:?}"),
    }
    let red = public_scene(&bank, Tile::from_code("7z").unwrap(), false);
    let impossible = PublicImages {
        river: &red,
        own_melds: &empty,
        opponent_melds: [&red; 3],
        dora: &red,
    };
    assert!(matches!(
        engine.process_with_public(3, &image, None, &impossible),
        PipelineOutput::Uncertain
    ));
}

#[test]
fn requires_stable_frames_before_recommending_and_clears_on_uncertainty() {
    let bank = bank();
    let mut engine = RuntimeEngine::new(bank.clone(), vec![], MatchConfig::default(), 3).unwrap();
    let base = hand_tiles();
    for frame in 1..=2 {
        assert!(matches!(
            engine.process(
                frame,
                &scene(&bank, &base),
                Some(&action_scene(&bank, ActionLabel::None))
            ),
            PipelineOutput::Waiting
        ));
    }
    let ready = engine.process(
        3,
        &scene(&bank, &base),
        Some(&action_scene(&bank, ActionLabel::None)),
    );
    assert!(matches!(
        ready,
        PipelineOutput::Ready {
            phase: Phase::AwaitDraw,
            ..
        }
    ));

    let mut drawn = base.clone();
    drawn.push(Tile::from_code("9p").unwrap());
    for frame in 4..=5 {
        assert!(matches!(
            engine.process(
                frame,
                &scene(&bank, &drawn),
                Some(&action_scene(&bank, ActionLabel::Riichi))
            ),
            PipelineOutput::Waiting
        ));
    }
    let ready = engine.process(
        6,
        &scene(&bank, &drawn),
        Some(&action_scene(&bank, ActionLabel::Riichi)),
    );
    match ready {
        PipelineOutput::Ready {
            phase,
            events,
            discards,
            actions,
            ..
        } => {
            assert_eq!(phase, Phase::AwaitDiscard);
            assert!(events.contains(&StateEvent::Draw(Tile::from_code("9p").unwrap())));
            assert!(!discards.is_empty());
            assert!(
                actions
                    .iter()
                    .any(|advice| advice.action == ActionLabel::Riichi)
            );
        }
        _ => panic!("expected a ready recommendation"),
    }

    engine.set_paused(true);
    assert!(matches!(
        engine.process(
            7,
            &scene(&bank, &drawn),
            Some(&action_scene(&bank, ActionLabel::Riichi))
        ),
        PipelineOutput::Paused
    ));
    engine.set_paused(false);
    assert!(matches!(
        engine.process(
            8,
            &scene(&bank, &drawn),
            Some(&action_scene(&bank, ActionLabel::Riichi))
        ),
        PipelineOutput::Waiting
    ));
    assert!(matches!(
        engine.process(
            9,
            &scene(&bank, &drawn),
            Some(&action_scene(&bank, ActionLabel::Riichi))
        ),
        PipelineOutput::Waiting
    ));
    assert!(matches!(
        engine.process(
            10,
            &scene(&bank, &drawn),
            Some(&action_scene(&bank, ActionLabel::Riichi))
        ),
        PipelineOutput::Ready { .. }
    ));

    let mut obscured = scene(&bank, &drawn);
    let last_x = 2 + 13 * (W + 2);
    for y in 2..H + 2 {
        for x in last_x..last_x + W {
            let i = ((y * obscured.width() + x) * 3) as usize;
            obscured.pixels_mut()[i..i + 3].fill(245);
        }
    }
    assert!(matches!(
        engine.process(
            11,
            &obscured,
            Some(&action_scene(&bank, ActionLabel::Riichi))
        ),
        PipelineOutput::Uncertain
    ));
}

#[test]
fn unchanged_frames_reuse_discard_advice_until_inputs_change() {
    let bank = bank();
    let mut engine = RuntimeEngine::new(bank.clone(), vec![], MatchConfig::default(), 1).unwrap();
    let mut hand = hand_tiles();
    hand.push(Tile::from_code("9p").unwrap());
    let image = scene(&bank, &hand);
    let actions = action_scene(&bank, ActionLabel::None);

    assert!(matches!(
        engine.process(1, &image, Some(&actions)),
        PipelineOutput::Ready { .. }
    ));
    assert_eq!(engine.recommendation_calculations(), 1);
    let mut slightly_changed = image.clone();
    let altered_pixel = (((7 + 2) * image.width() + (3 + 2)) * 3) as usize;
    slightly_changed.pixels_mut()[altered_pixel..altered_pixel + 3].fill(240);
    match engine.process(2, &slightly_changed, Some(&actions)) {
        PipelineOutput::Ready {
            confidence,
            discards,
            ..
        } => {
            assert!(confidence < 1.0);
            assert!(
                discards
                    .iter()
                    .all(|advice| (advice.confidence - confidence).abs() < f32::EPSILON)
            );
        }
        _ => panic!("expected a ready recommendation"),
    }
    assert_eq!(engine.recommendation_calculations(), 1);

    engine
        .set_public_tiles([0; 34], vec![Tile::from_code("1z").unwrap()])
        .unwrap();
    assert!(matches!(
        engine.process(3, &image, Some(&actions)),
        PipelineOutput::Ready { .. }
    ));
    assert_eq!(engine.recommendation_calculations(), 2);

    engine.tracker_mut().set_known_yaku(true);
    assert!(matches!(
        engine.process(4, &image, Some(&actions)),
        PipelineOutput::Ready { .. }
    ));
    assert_eq!(engine.recommendation_calculations(), 3);

    engine.reset_recognition();
    assert!(matches!(
        engine.process(5, &image, Some(&actions)),
        PipelineOutput::Ready { .. }
    ));
    assert_eq!(engine.recommendation_calculations(), 4);
}
