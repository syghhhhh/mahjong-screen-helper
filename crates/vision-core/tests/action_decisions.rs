use vision_core::{
    Rect, RgbImage,
    action_decision::{
        ActionWord, ActionWordDecision, ActionWordPolicy, CLASS_ORDER, RejectReason,
    },
    action_input::{
        ActionWordInput, INPUT_FLOATS, PreparedActionWord, prepare_complete_action_word,
    },
};

fn observation() -> ActionWordInput {
    let mut image = RgbImage::new(128, 64, vec![0; 128 * 64 * 3]).unwrap();
    for y in 12..52 {
        for x in (20..30).chain(60..70) {
            let at = ((y * 128 + x) * 3) as usize;
            image.pixels_mut()[at..at + 3].copy_from_slice(&[240; 3]);
        }
    }
    prepare_complete_action_word(&image).unwrap()
}
fn peak(index: usize) -> [f32; 7] {
    let mut scores = [-20.0; 7];
    scores[index] = 20.0;
    scores
}
fn reason(decision: ActionWordDecision) -> RejectReason {
    match decision {
        ActionWordDecision::Rejected { reason, .. } => reason,
        other => panic!("expected rejection, got {other:?}"),
    }
}

#[test]
fn fixed_classes_produce_only_five_word_candidates_and_pass_stays_distinct() {
    assert_eq!(
        CLASS_ORDER,
        [
            "Chi",
            "Pon",
            "Kan",
            "Riichi",
            "Pass",
            "Unsupported",
            "Unknown"
        ]
    );
    for (index, word) in [
        ActionWord::Chi,
        ActionWord::Pon,
        ActionWord::Kan,
        ActionWord::Riichi,
        ActionWord::Pass,
    ]
    .into_iter()
    .enumerate()
    {
        let ActionWordDecision::Candidate {
            word: actual,
            scores,
        } = ActionWordPolicy::default().decide(&observation(), &peak(index))
        else {
            panic!("known word rejected")
        };
        assert_eq!(actual, word);
        assert!(scores.probability > 0.999);
    }
    for (index, expected) in [
        (5, RejectReason::UnsupportedWord),
        (6, RejectReason::UnknownWord),
    ] {
        assert_eq!(
            reason(ActionWordPolicy::default().decide(&observation(), &peak(index))),
            expected
        );
    }
}

#[test]
fn missing_or_solid_word_evidence_cannot_be_overridden_by_high_scores() {
    for input in [ActionWordInput::Empty, ActionWordInput::Unlocalized] {
        assert_eq!(
            reason(ActionWordPolicy::default().decide(&input, &peak(0))),
            RejectReason::NoWord
        );
    }
    let white = RgbImage::new(128, 64, vec![255; 128 * 64 * 3]).unwrap();
    let input = prepare_complete_action_word(&white).unwrap();
    assert_eq!(
        reason(ActionWordPolicy::default().decide(&input, &peak(0))),
        RejectReason::SolidForeground
    );
}

#[test]
fn invalid_tensor_or_geometry_is_rejected_before_any_candidate() {
    let ActionWordInput::Prepared(original) = observation() else {
        panic!()
    };
    let mut cases = Vec::new();
    let mut bad = original.clone();
    bad.pixels.pop();
    cases.push(bad);
    let mut bad = original.clone();
    bad.pixels[0] = f32::NAN;
    cases.push(bad);
    let mut bad = original.clone();
    bad.pixels[0] = 1.01;
    cases.push(bad);
    let mut bad = original.clone();
    bad.pixels.fill(0.0);
    cases.push(bad);
    let mut bad = original.clone();
    bad.bounds.x = u32::MAX;
    cases.push(bad);
    let mut bad = original.clone();
    bad.foreground_pixels = u32::MAX;
    cases.push(bad);
    cases.push(PreparedActionWord {
        pixels: vec![0.5; INPUT_FLOATS],
        bounds: Rect {
            x: 0,
            y: 0,
            width: 0,
            height: 10,
        },
        foreground_pixels: 1,
    });
    for word in cases {
        assert_eq!(
            reason(ActionWordPolicy::default().decide(&ActionWordInput::Prepared(word), &peak(0))),
            RejectReason::InvalidInput
        );
    }
}

#[test]
fn nonfinite_logits_and_insufficient_scores_never_become_candidates() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for index in 0..7 {
            let mut logits = peak(0);
            logits[index] = value;
            assert_eq!(
                reason(ActionWordPolicy::default().decide(&observation(), &logits)),
                RejectReason::InvalidLogits
            );
        }
    }
    assert_eq!(
        reason(ActionWordPolicy::default().decide(&observation(), &[0.0; 7])),
        RejectReason::LowProbability
    );
    let ambiguous = ActionWordPolicy::new(0.5, 0.25, 1.0).unwrap();
    assert_eq!(
        reason(ambiguous.decide(
            &observation(),
            &[100.0, 100.0, -100.0, -100.0, -100.0, -100.0, -100.0]
        )),
        RejectReason::Ambiguous
    );
    let entropy = ActionWordPolicy::new(0.5, 0.25, 0.35).unwrap();
    let mut spread = [((0.4 / 6.0) as f32).ln(); 7];
    spread[0] = 0.6f32.ln();
    assert_eq!(
        reason(entropy.decide(&observation(), &spread)),
        RejectReason::HighEntropy
    );
}

#[test]
fn invalid_thresholds_are_refused_instead_of_silently_clamped() {
    for args in [
        (f64::NAN, 0.25, 0.35),
        (0.49, 0.25, 0.35),
        (1.01, 0.25, 0.35),
        (0.9, 0.0, 0.35),
        (0.9, f64::INFINITY, 0.35),
        (0.9, 0.25, 0.0),
        (0.9, 0.25, 1.01),
    ] {
        assert!(ActionWordPolicy::new(args.0, args.1, args.2).is_err());
    }
}

#[test]
fn stable_softmax_handles_large_finite_values_and_common_offsets() {
    let policy = ActionWordPolicy::default();
    let logits = [12.0, 0.0, -2.0, -4.0, -6.0, -8.0, -10.0];
    assert_eq!(
        policy.decide(&observation(), &logits),
        policy.decide(&observation(), &logits.map(|v| v + 10000.0))
    );
    let mut extreme = [f32::MIN; 7];
    extreme[4] = f32::MAX;
    let ActionWordDecision::Candidate { word, scores } = policy.decide(&observation(), &extreme)
    else {
        panic!("finite extrema rejected")
    };
    assert_eq!(word, ActionWord::Pass);
    assert_eq!(scores.probability, 1.0);
    assert_eq!(scores.margin, 1.0);
    assert_eq!(scores.normalized_entropy, 0.0);
}
