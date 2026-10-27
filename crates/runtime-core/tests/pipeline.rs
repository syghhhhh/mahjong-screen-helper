use game_state::{Phase, StateEvent};
use riichi_core::Tile;
use runtime_core::{PipelineOutput, RuntimeEngine};
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
    tiles.sort();
    tiles.dedup();
    let tile_templates = tiles
        .into_iter()
        .enumerate()
        .map(|(i, tile)| (tile, art(i)))
        .collect();
    let action_templates = vec![(ActionLabel::None, art(30)), (ActionLabel::Riichi, art(31))];
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
    assert!(matches!(
        engine.process(2, &image, Some(&actions)),
        PipelineOutput::Ready { .. }
    ));
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
