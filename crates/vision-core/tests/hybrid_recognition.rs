use riichi_core::Tile;
use std::{cell::Cell, rc::Rc};
use vision_core::{
    ActionLabel, MatchConfig, Rect, RgbImage, Rotation, TemplateBank,
    classifier::{
        ClassifierDecision, ClassifierFailure, FaceClassifier, FaceObservation, RejectReason,
        ScoreSummary,
    },
};

fn tile(code: &str) -> Tile {
    Tile::from_code(code).unwrap()
}
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
fn accepted(code: &str) -> ClassifierDecision {
    ClassifierDecision::Accepted {
        tile: tile(code),
        scores: ScoreSummary {
            probability: 0.96,
            margin: 0.8,
            normalized_entropy: 0.1,
        },
    }
}
struct Mock {
    calls: Rc<Cell<usize>>,
    fail: bool,
    conflict: bool,
    invalid: bool,
}
impl FaceClassifier for Mock {
    fn classify(&self, _: &FaceObservation) -> Result<ClassifierDecision, ClassifierFailure> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        if self.fail {
            return Err(ClassifierFailure::Unavailable);
        }
        if self.invalid {
            return Ok(ClassifierDecision::Accepted {
                tile: tile("0s"),
                scores: ScoreSummary {
                    probability: f64::NAN,
                    margin: 0.9,
                    normalized_entropy: 0.1,
                },
            });
        }
        Ok(accepted(if self.conflict && call % 2 == 1 {
            "1m"
        } else {
            "0s"
        }))
    }
}
fn mock() -> Mock {
    Mock {
        calls: Rc::new(Cell::new(0)),
        fail: false,
        conflict: false,
        invalid: false,
    }
}
fn actions() -> Vec<(ActionLabel, RgbImage)> {
    vec![(ActionLabel::None, art(30))]
}
fn scene(pictures: &[RgbImage]) -> (RgbImage, Vec<Rect>) {
    let mut image = RgbImage::new(
        4 + 22 * pictures.len() as u32,
        30,
        vec![30; (4 + 22 * pictures.len()) * 30 * 3],
    )
    .unwrap();
    let mut regions = Vec::new();
    for (i, p) in pictures.iter().enumerate() {
        let r = Rect {
            x: 2 + i as u32 * 22,
            y: 2,
            width: p.width(),
            height: p.height(),
        };
        paste(&mut image, p, r.x, r.y, false);
        regions.push(r);
    }
    (image, regions)
}
fn paste(image: &mut RgbImage, p: &RgbImage, left: u32, top: u32, green: bool) {
    for y in top.saturating_sub(2)..top + p.height() + 2 {
        for x in left.saturating_sub(2)..left + p.width() + 2 {
            if x >= image.width() || y >= image.height() {
                continue;
            }
            let dst = ((y * image.width() + x) * 3) as usize;
            if x >= left && x < left + p.width() && y >= top && y < top + p.height() {
                let src = (((y - top) * p.width() + x - left) * 3) as usize;
                image.pixels_mut()[dst..dst + 3].copy_from_slice(&p.pixels()[src..src + 3]);
            } else if green {
                image.pixels_mut()[dst..dst + 3].copy_from_slice(&[70, 225, 100]);
            }
        }
    }
}

#[test]
fn templates_keep_the_fast_path_and_only_uncertain_slots_call_the_model() {
    let bank = TemplateBank::new(vec![(tile("1m"), art(0))], actions()).unwrap();
    let classifier = mock();
    let (image, slots) = scene(&[art(0), art(1)]);
    let config = MatchConfig {
        min_confidence: 1.0,
        min_margin: 0.99,
    };
    let read = bank
        .recognize_hand_with_fallback(&image, &slots[..1], config, Some(&classifier))
        .unwrap();
    assert_eq!(read.tiles, Some(vec![tile("1m")]));
    assert_eq!(classifier.calls.get(), 0);
    let read = bank
        .recognize_hand_with_fallback(&image, &slots, config, Some(&classifier))
        .unwrap();
    assert!(read.tiles.is_none());
    assert_eq!(read.uncertain_slots, vec![1]);
    assert_eq!(classifier.calls.get(), 1);
    let read = bank
        .recognize_hand_with_fallback(
            &image,
            &slots,
            MatchConfig {
                min_confidence: 0.9,
                min_margin: 0.1,
            },
            Some(&classifier),
        )
        .unwrap();
    assert_eq!(read.tiles, Some(vec![tile("1m"), tile("0s")]));
    assert_eq!(classifier.calls.get(), 2);
}

#[test]
fn actions_only_mode_can_use_the_model_without_thirty_seven_tile_templates() {
    let bank = TemplateBank::actions_only(actions()).unwrap();
    assert!(bank.tile_templates().is_empty());
    let classifier = mock();
    let (image, slots) = scene(&[art(0)]);
    let read = bank
        .recognize_hand_with_fallback(&image, &slots, MatchConfig::default(), Some(&classifier))
        .unwrap();
    assert_eq!(read.tiles, Some(vec![tile("0s")]));
    assert_eq!(classifier.calls.get(), 1);
    assert!(TemplateBank::actions_only(vec![]).is_err());
}

#[test]
fn backend_failure_rejection_and_nonfinite_scores_do_not_drop_uncertain_slots() {
    let bank = TemplateBank::actions_only(actions()).unwrap();
    let (image, slots) = scene(&[art(0)]);
    for (fail, invalid) in [(true, false), (false, true)] {
        let mut classifier = mock();
        classifier.fail = fail;
        classifier.invalid = invalid;
        let read = bank
            .recognize_hand_with_fallback(&image, &slots, MatchConfig::default(), Some(&classifier))
            .unwrap();
        assert!(read.tiles.is_none());
        assert_eq!(read.uncertain_slots, vec![0]);
    }
    struct Rejected;
    impl FaceClassifier for Rejected {
        fn classify(&self, _: &FaceObservation) -> Result<ClassifierDecision, ClassifierFailure> {
            Ok(ClassifierDecision::Rejected {
                reason: RejectReason::LowProbability,
                scores: None,
            })
        }
    }
    assert!(
        bank.recognize_hand_with_fallback(&image, &slots, MatchConfig::default(), Some(&Rejected))
            .unwrap()
            .tiles
            .is_none()
    );
}

#[test]
fn repeated_rotations_of_one_label_are_allowed_but_conflicting_labels_are_not() {
    let bank = TemplateBank::actions_only(actions()).unwrap();
    let mut image = RgbImage::new(160, 100, vec![38; 160 * 100 * 3]).unwrap();
    paste(
        &mut image,
        &art(1).rotated(Rotation::Clockwise),
        110,
        35,
        true,
    );
    let classifier = mock();
    let read = bank.recognize_public_tiles_with_fallback(
        &image,
        MatchConfig::default(),
        Some(&classifier),
    );
    assert_eq!(read.tiles.unwrap()[0].tile, tile("0s"));
    assert!(read.offered.is_some());
    assert_eq!(classifier.calls.get(), 4);
    let mut conflict = mock();
    conflict.conflict = true;
    let read =
        bank.recognize_public_tiles_with_fallback(&image, MatchConfig::default(), Some(&conflict));
    assert!(read.tiles.is_none());
    assert!(read.offered.is_none());
}

#[test]
fn neural_public_results_still_obey_overlap_and_copy_count_constraints() {
    let bank = TemplateBank::actions_only(actions()).unwrap();
    let classifier = mock();
    let mut image = RgbImage::new(180, 100, vec![38; 180 * 100 * 3]).unwrap();
    for i in 0..5 {
        paste(&mut image, &art(1), 10 + i * 32, 35, i == 4);
    }
    let read = bank.recognize_public_tiles_with_fallback(
        &image,
        MatchConfig::default(),
        Some(&classifier),
    );
    assert!(read.tiles.is_none());
    assert!(read.offered.is_none());
    let regions = [
        Rect {
            x: 10,
            y: 35,
            width: 18,
            height: 26,
        },
        Rect {
            x: 11,
            y: 35,
            width: 18,
            height: 26,
        },
    ];
    let read = bank
        .recognize_public_regions_with_fallback(
            &image,
            &regions,
            MatchConfig::default(),
            Some(&classifier),
        )
        .unwrap();
    assert!(read.tiles.is_none());
    assert!(read.offered.is_none());
    assert!(!read.uncertain_regions.is_empty());
}

#[test]
fn sideways_blank_faces_require_border_evidence_in_the_current_public_image() {
    let mut image = RgbImage::new(90, 70, vec![30; 90 * 70 * 3]).unwrap();
    let white = RgbImage::new(40, 26, vec![240; 40 * 26 * 3]).unwrap();
    paste(&mut image, &white, 20, 20, false);
    let region = Rect {
        x: 20,
        y: 20,
        width: 40,
        height: 26,
    };
    let observation = FaceObservation::in_public(&image, region, Rotation::Clockwise).unwrap();
    assert!(observation.blank_face_verified());
    assert!(observation.glyph_bounds().is_none());
    let pale = RgbImage::new(90, 70, vec![240; 90 * 70 * 3]).unwrap();
    assert!(
        !FaceObservation::in_public(&pale, region, Rotation::Clockwise)
            .unwrap()
            .blank_face_verified()
    );
}
