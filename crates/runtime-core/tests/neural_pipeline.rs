use riichi_core::Tile;
use runtime_core::{PipelineOutput, PublicImages, RuntimeEngine};
use std::{cell::Cell, rc::Rc};
use vision_core::{
    ActionLabel, MatchConfig, RgbImage, TemplateBank,
    classifier::{
        ClassifierDecision, ClassifierFailure, FaceClassifier, FaceObservation, RejectReason,
        ScoreSummary,
    },
};

fn art(seed: u32) -> RgbImage {
    let mut rgb = vec![240; 18 * 26 * 3];
    for y in 5..22 {
        for x in 3..15 {
            if (x * (seed + 1) + y * 7) % 23 < 9 {
                let i = ((y * 18 + x) * 3) as usize;
                rgb[i..i + 3].copy_from_slice(&[20, 30, 40]);
            }
        }
    }
    RgbImage::new(18, 26, rgb).unwrap()
}
fn tile(code: &str) -> Tile {
    Tile::from_code(code).unwrap()
}
fn tiles() -> Vec<Tile> {
    "1m 2m 3m 4m 5m 6m 2p 3p 4p 2s 3s 7z 7z 9p"
        .split_whitespace()
        .map(tile)
        .collect()
}
fn picture(hand: &[Tile], dictionary: &[(Tile, RgbImage)]) -> RgbImage {
    let width = 4 + 22 * hand.len() as u32;
    let mut image = RgbImage::new(width, 30, vec![30; width as usize * 30 * 3]).unwrap();
    for (i, tile) in hand.iter().enumerate() {
        paste(
            &mut image,
            &dictionary.iter().find(|(t, _)| t == tile).unwrap().1,
            2 + i as u32 * 22,
            2,
        );
    }
    image
}
fn paste(image: &mut RgbImage, source: &RgbImage, left: u32, top: u32) {
    for y in 0..source.height() {
        for x in 0..source.width() {
            let s = ((y * source.width() + x) * 3) as usize;
            let d = (((y + top) * image.width() + x + left) * 3) as usize;
            image.pixels_mut()[d..d + 3].copy_from_slice(&source.pixels()[s..s + 3]);
        }
    }
}
struct Classifier {
    samples: Vec<(Tile, Vec<f32>)>,
    fail: Rc<Cell<bool>>,
    calls: Rc<Cell<usize>>,
}
impl FaceClassifier for Classifier {
    fn classify(&self, face: &FaceObservation) -> Result<ClassifierDecision, ClassifierFailure> {
        self.calls.set(self.calls.get() + 1);
        if self.fail.get() {
            return Err(ClassifierFailure::Unavailable);
        }
        if let Some((tile, _)) = self
            .samples
            .iter()
            .find(|(_, pixels)| pixels.as_slice() == face.input())
        {
            Ok(ClassifierDecision::Accepted {
                tile: *tile,
                scores: ScoreSummary {
                    probability: 0.96,
                    margin: 0.8,
                    normalized_entropy: 0.1,
                },
            })
        } else {
            Ok(ClassifierDecision::Rejected {
                reason: RejectReason::LowProbability,
                scores: None,
            })
        }
    }
}
type Setup = (
    RuntimeEngine,
    Vec<(Tile, RgbImage)>,
    Rc<Cell<bool>>,
    Rc<Cell<usize>>,
);
fn setup() -> Setup {
    let mut labels = tiles();
    labels.push(tile("1p"));
    labels.sort();
    labels.dedup();
    let dictionary: Vec<_> = labels
        .into_iter()
        .enumerate()
        .map(|(i, t)| (t, art(i as u32)))
        .collect();
    let fail = Rc::new(Cell::new(false));
    let calls = Rc::new(Cell::new(0));
    let classifier = Classifier {
        samples: dictionary
            .iter()
            .map(|(t, i)| (*t, FaceObservation::isolated(i).unwrap().input().to_vec()))
            .collect(),
        fail: fail.clone(),
        calls: calls.clone(),
    };
    let bank = TemplateBank::actions_only(vec![(ActionLabel::None, art(30))]).unwrap();
    let mut engine = RuntimeEngine::new(bank, vec![], MatchConfig::default(), 3).unwrap();
    engine.set_tile_classifier(Some(Box::new(classifier)));
    (engine, dictionary, fail, calls)
}

fn selected_classifier(
    dictionary: &[(Tile, RgbImage)],
    selected: &[Tile],
    fail: Rc<Cell<bool>>,
    calls: Rc<Cell<usize>>,
) -> Box<dyn FaceClassifier> {
    Box::new(Classifier {
        samples: dictionary
            .iter()
            .filter(|(tile, _)| selected.contains(tile))
            .map(|(tile, image)| {
                (
                    *tile,
                    FaceObservation::isolated(image).unwrap().input().to_vec(),
                )
            })
            .collect(),
        fail,
        calls,
    })
}

fn public_test_images(dictionary: &[(Tile, RgbImage)]) -> (RgbImage, RgbImage) {
    let empty = RgbImage::new(80, 80, vec![38; 80 * 80 * 3]).unwrap();
    let mut dora = empty.clone();
    paste(
        &mut dora,
        &dictionary.iter().find(|(t, _)| *t == tile("1p")).unwrap().1,
        30,
        25,
    );
    (empty, dora)
}
fn public_images<'a>(empty: &'a RgbImage, dora: &'a RgbImage) -> PublicImages<'a> {
    PublicImages {
        river: empty,
        own_melds: empty,
        opponent_melds: [empty; 3],
        dora,
    }
}

#[test]
fn distinct_models_route_hand_and_public_faces_without_cross_fallback() {
    // Each public region must route to the override. The hand classifier has
    // no 1p sample, so accidentally using it in any branch prevents Ready.
    for region in 0..6 {
        let (mut engine, dictionary, _, _) = setup();
        let hand_calls = Rc::new(Cell::new(0));
        let public_calls = Rc::new(Cell::new(0));
        engine.set_tile_classifier(Some(selected_classifier(
            &dictionary,
            &tiles(),
            Rc::new(Cell::new(false)),
            hand_calls.clone(),
        )));
        engine.set_public_tile_classifier(Some(selected_classifier(
            &dictionary,
            &[tile("1p")],
            Rc::new(Cell::new(false)),
            public_calls.clone(),
        )));
        let concealed = if region == 1 {
            tiles()[3..].to_vec()
        } else {
            tiles()
        };
        let hand = picture(&concealed, &dictionary);
        let (empty, _) = public_test_images(&dictionary);
        let face = picture(&[tile("1p")], &dictionary);
        let pon = picture(&[tile("1p"); 3], &dictionary);
        let public =
            PublicImages {
                river: if region == 0 { &face } else { &empty },
                own_melds: if region == 1 { &pon } else { &empty },
                opponent_melds: std::array::from_fn(|index| {
                    if region == index + 2 { &face } else { &empty }
                }),
                dora: if region == 5 { &face } else { &empty },
            };
        for frame in 1..=2 {
            assert_eq!(
                engine.process_with_public(frame, &hand, None, &public),
                PipelineOutput::Waiting,
                "public region {region}"
            );
        }
        assert!(
            matches!(
                engine.process_with_public(3, &hand, None, &public),
                PipelineOutput::Ready { .. }
            ),
            "public region {region}"
        );
        assert_eq!(hand_calls.get(), 3 * concealed.len());
        assert!(public_calls.get() > 0);
    }
}

#[test]
fn public_classifier_is_not_used_when_public_images_are_absent() {
    let (mut engine, dictionary, _, _) = setup();
    let hand_calls = Rc::new(Cell::new(0));
    let public_calls = Rc::new(Cell::new(0));
    engine.set_tile_classifier(Some(selected_classifier(
        &dictionary,
        &tiles(),
        Rc::new(Cell::new(false)),
        hand_calls.clone(),
    )));
    engine.set_public_tile_classifier(Some(selected_classifier(
        &dictionary,
        &[tile("1p")],
        Rc::new(Cell::new(false)),
        public_calls.clone(),
    )));
    let hand = picture(&tiles(), &dictionary);
    let (empty, dora) = public_test_images(&dictionary);
    let public = public_images(&empty, &dora);
    for frame in 1..=2 {
        assert_eq!(
            engine.process_with_public(frame, &hand, None, &public),
            PipelineOutput::Waiting
        );
    }
    assert!(matches!(
        engine.process_with_public(3, &hand, None, &public),
        PipelineOutput::Ready { .. }
    ));
    assert_eq!(hand_calls.get(), 42);
    assert!(public_calls.get() > 0);
    let previous = public_calls.get();
    engine.process(4, &hand, None);
    assert_eq!(hand_calls.get(), 56);
    assert_eq!(public_calls.get(), previous);
}

#[test]
fn failing_public_override_clears_advice_even_when_shared_model_could_classify() {
    let (mut engine, dictionary, _, _) = setup();
    let fail = Rc::new(Cell::new(false));
    engine.set_public_tile_classifier(Some(selected_classifier(
        &dictionary,
        &[tile("1p")],
        fail.clone(),
        Rc::new(Cell::new(0)),
    )));
    let hand = picture(&tiles(), &dictionary);
    let (empty, dora) = public_test_images(&dictionary);
    let public = public_images(&empty, &dora);
    for frame in 1..=3 {
        engine.process_with_public(frame, &hand, None, &public);
    }
    assert!(!engine.tracker().discard_recommendations().is_empty());
    fail.set(true);
    assert_eq!(
        engine.process_with_public(4, &hand, None, &public),
        PipelineOutput::Uncertain
    );
    assert!(engine.tracker().discard_recommendations().is_empty());
    fail.set(false);
    for frame in 5..=6 {
        assert_eq!(
            engine.process_with_public(frame, &hand, None, &public),
            PipelineOutput::Waiting
        );
    }
    assert!(matches!(
        engine.process_with_public(7, &hand, None, &public),
        PipelineOutput::Ready { .. }
    ));
}

#[test]
fn replacing_and_removing_public_model_requires_fresh_frames_and_restores_shared_route() {
    let (mut engine, dictionary, _, shared_calls) = setup();
    let hand = picture(&tiles(), &dictionary);
    let (empty, dora) = public_test_images(&dictionary);
    let public = public_images(&empty, &dora);
    for frame in 1..=3 {
        engine.process_with_public(frame, &hand, None, &public);
    }
    engine.set_public_tile_classifier(Some(selected_classifier(
        &dictionary,
        &[tile("1p")],
        Rc::new(Cell::new(false)),
        Rc::new(Cell::new(0)),
    )));
    assert!(engine.tracker().discard_recommendations().is_empty());
    for frame in 4..=5 {
        assert_eq!(
            engine.process_with_public(frame, &hand, None, &public),
            PipelineOutput::Waiting
        );
    }
    assert!(matches!(
        engine.process_with_public(6, &hand, None, &public),
        PipelineOutput::Ready { .. }
    ));
    engine.set_public_tile_classifier(None);
    let before = shared_calls.get();
    for frame in 7..=8 {
        assert_eq!(
            engine.process_with_public(frame, &hand, None, &public),
            PipelineOutput::Waiting
        );
    }
    assert!(matches!(
        engine.process_with_public(9, &hand, None, &public),
        PipelineOutput::Ready { .. }
    ));
    assert!(shared_calls.get() > before + 42);
}

#[test]
fn model_pixels_require_three_frames_and_backend_failure_erases_cached_advice() {
    let (mut engine, dictionary, fail, _) = setup();
    let hand = picture(&tiles(), &dictionary);
    assert!(matches!(
        engine.process(1, &hand, None),
        PipelineOutput::Waiting
    ));
    assert!(matches!(
        engine.process(2, &hand, None),
        PipelineOutput::Waiting
    ));
    match engine.process(3, &hand, None) {
        PipelineOutput::Ready {
            hand: found,
            discards,
            ..
        } => {
            assert_eq!(found, tiles());
            assert_eq!(discards.len(), 3);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(engine.recommendation_calculations(), 1);
    fail.set(true);
    assert!(matches!(
        engine.process(4, &hand, None),
        PipelineOutput::Uncertain
    ));
    assert!(engine.tracker().discard_recommendations().is_empty());
    fail.set(false);
    for frame in 5..=6 {
        assert!(matches!(
            engine.process(frame, &hand, None),
            PipelineOutput::Waiting
        ));
    }
    assert!(matches!(
        engine.process(7, &hand, None),
        PipelineOutput::Ready { .. }
    ));
    assert_eq!(engine.recommendation_calculations(), 2);
}

#[test]
fn public_neural_reads_feed_the_same_pipeline_and_rejection_clears_old_state() {
    let (mut engine, dictionary, _, _) = setup();
    let hand = picture(&tiles(), &dictionary);
    let empty = RgbImage::new(80, 80, vec![38; 80 * 80 * 3]).unwrap();
    let mut dora = empty.clone();
    paste(
        &mut dora,
        &dictionary.iter().find(|(t, _)| *t == tile("1p")).unwrap().1,
        30,
        25,
    );
    let public = PublicImages {
        river: &empty,
        own_melds: &empty,
        opponent_melds: [&empty; 3],
        dora: &dora,
    };
    for frame in 1..=2 {
        assert!(matches!(
            engine.process_with_public(frame, &hand, None, &public),
            PipelineOutput::Waiting
        ));
    }
    assert!(matches!(
        engine.process_with_public(3, &hand, None, &public),
        PipelineOutput::Ready { .. }
    ));
    let mut unknown = empty.clone();
    paste(&mut unknown, &art(22), 30, 25);
    let public = PublicImages {
        river: &empty,
        own_melds: &empty,
        opponent_melds: [&empty; 3],
        dora: &unknown,
    };
    assert!(matches!(
        engine.process_with_public(4, &hand, None, &public),
        PipelineOutput::Uncertain
    ));
    assert!(engine.tracker().discard_recommendations().is_empty());
}

#[test]
fn a_shadowed_unknown_public_face_erases_advice_and_requires_three_clean_frames() {
    let (mut engine, dictionary, _, _) = setup();
    let hand = picture(&tiles(), &dictionary);
    let empty = RgbImage::new(80, 80, vec![38; 80 * 80 * 3]).unwrap();
    let mut dora = empty.clone();
    paste(
        &mut dora,
        &dictionary.iter().find(|(t, _)| *t == tile("1p")).unwrap().1,
        30,
        25,
    );
    let mut public = PublicImages {
        river: &empty,
        own_melds: &empty,
        opponent_melds: [&empty; 3],
        dora: &dora,
    };
    for frame in 1..=2 {
        engine.process_with_public(frame, &hand, None, &public);
    }
    assert!(matches!(
        engine.process_with_public(3, &hand, None, &public),
        PipelineOutput::Ready { .. }
    ));
    let mut shadow = empty.clone();
    let washed = RgbImage::new(18, 26, [165, 210, 180].repeat(18 * 26)).unwrap();
    paste(&mut shadow, &washed, 30, 25);
    public.river = &shadow;
    assert!(matches!(
        engine.process_with_public(4, &hand, None, &public),
        PipelineOutput::Uncertain
    ));
    assert!(engine.tracker().discard_recommendations().is_empty());
    public.river = &empty;
    for frame in 5..=6 {
        assert!(matches!(
            engine.process_with_public(frame, &hand, None, &public),
            PipelineOutput::Waiting
        ));
    }
    assert!(matches!(
        engine.process_with_public(7, &hand, None, &public),
        PipelineOutput::Ready { .. }
    ));
    assert_eq!(engine.recommendation_calculations(), 2);
}

#[test]
fn pausing_and_removing_a_classifier_never_reuse_prior_stable_results() {
    let (mut engine, dictionary, _, calls) = setup();
    let hand = picture(&tiles(), &dictionary);
    for frame in 1..=3 {
        engine.process(frame, &hand, None);
    }
    let prior = calls.get();
    engine.set_paused(true);
    assert!(matches!(
        engine.process(4, &hand, None),
        PipelineOutput::Paused
    ));
    assert_eq!(calls.get(), prior);
    engine.set_paused(false);
    assert!(matches!(
        engine.process(5, &hand, None),
        PipelineOutput::Waiting
    ));
    engine.set_tile_classifier(None);
    assert!(matches!(
        engine.process(6, &hand, None),
        PipelineOutput::Uncertain
    ));
    assert!(engine.tracker().discard_recommendations().is_empty());
}

#[test]
fn changing_classifiers_cannot_turn_cross_model_evidence_into_a_new_round() {
    check_cross_model_round(false);
}

#[test]
fn changing_public_classifiers_cannot_turn_cross_model_evidence_into_a_new_round() {
    check_cross_model_round(true);
}

fn check_cross_model_round(public_override: bool) {
    let (mut engine, dictionary, _, _) = setup();
    if public_override {
        let all: Vec<_> = dictionary.iter().map(|(tile, _)| *tile).collect();
        engine.set_public_tile_classifier(Some(selected_classifier(
            &dictionary,
            &all,
            Rc::new(Cell::new(false)),
            Rc::new(Cell::new(0)),
        )));
    }
    let original = tiles();
    let hand = picture(&original, &dictionary);
    let empty = RgbImage::new(80, 80, vec![38; 80 * 80 * 3]).unwrap();
    let mut first_indicator = empty.clone();
    paste(
        &mut first_indicator,
        &dictionary.iter().find(|(t, _)| *t == tile("1p")).unwrap().1,
        30,
        25,
    );
    let public = PublicImages {
        river: &first_indicator,
        own_melds: &empty,
        opponent_melds: [&empty; 3],
        dora: &first_indicator,
    };
    for frame in 1..=2 {
        engine.process_with_public(frame, &hand, None, &public);
    }
    assert!(matches!(
        engine.process_with_public(3, &hand, None, &public),
        PipelineOutput::Ready { .. }
    ));
    let replacement = Classifier {
        samples: dictionary
            .iter()
            .map(|(t, i)| (*t, FaceObservation::isolated(i).unwrap().input().to_vec()))
            .collect(),
        fail: Rc::new(Cell::new(false)),
        calls: Rc::new(Cell::new(0)),
    };
    if public_override {
        engine.set_public_tile_classifier(Some(Box::new(replacement)));
    } else {
        engine.set_tile_classifier(Some(Box::new(replacement)));
    }
    let mut changed = original;
    changed[0] = tile("2m");
    changed[4] = tile("6m");
    let hand = picture(&changed, &dictionary);
    let mut new_indicator = empty.clone();
    paste(
        &mut new_indicator,
        &dictionary.iter().find(|(t, _)| *t == tile("2m")).unwrap().1,
        30,
        25,
    );
    let public = PublicImages {
        river: &empty,
        own_melds: &empty,
        opponent_melds: [&empty; 3],
        dora: &new_indicator,
    };
    for frame in 4..=5 {
        engine.process_with_public(frame, &hand, None, &public);
    }
    match engine.process_with_public(6, &hand, None, &public) {
        PipelineOutput::Ready { events, .. } => assert!(
            !events.contains(&game_state::StateEvent::RoundEnd),
            "different classifier interpretations cannot prove a round transition"
        ),
        PipelineOutput::Uncertain => {}
        other => panic!("expected a decided third frame, got {other:?}"),
    }
}
