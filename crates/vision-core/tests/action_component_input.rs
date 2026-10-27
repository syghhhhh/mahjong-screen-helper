use vision_core::{
    Rect, RgbImage, VisionError,
    action_input::{
        ActionWordInput, COMPLETE_COMPONENT_PREPROCESSING_ID, PREPROCESSING_ID, PreparedActionWord,
        prepare_action_word, prepare_complete_action_word,
    },
};

fn scene(width: u32, height: u32) -> RgbImage {
    RgbImage::new(width, height, vec![0; (width * height * 3) as usize]).unwrap()
}

fn ink(image: &mut RgbImage, [x, y, width, height]: [u32; 4]) {
    for row in y..y + height {
        for column in x..x + width {
            let i = ((row * image.width() + column) * 3) as usize;
            image.pixels_mut()[i..i + 3].copy_from_slice(&[240; 3]);
        }
    }
}

fn prepared(result: ActionWordInput) -> PreparedActionWord {
    let ActionWordInput::Prepared(word) = result else {
        panic!("expected localized word")
    };
    word
}

fn rect([x, y, width, height]: [u32; 4]) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn boundary_strokes_are_completed_transitively_without_absorbing_distant_marks() {
    let mut image = scene(128, 64);
    for shape in [
        [20, 12, 8, 40],
        [68, 12, 8, 40],
        // Both short strokes fail the anchor height threshold. Only the first
        // touches the initial word rectangle; completing it reaches the second.
        [40, 50, 8, 6],
        [56, 54, 8, 6],
        [80, 58, 2, 2],
    ] {
        ink(&mut image, shape);
    }
    let original = image.clone();
    let old = prepared(prepare_action_word(&image).unwrap());
    let complete = prepared(prepare_complete_action_word(&image).unwrap());
    assert_eq!(old.bounds, rect([20, 12, 56, 40]));
    assert_eq!(old.foreground_pixels, 656);
    assert_eq!(complete.bounds, rect([20, 12, 56, 48]));
    assert_eq!(complete.foreground_pixels, 736);
    assert_eq!(image, original);
    assert_eq!(PREPROCESSING_ID, "action-bright-word-mask-v1");
    assert_eq!(
        COMPLETE_COMPONENT_PREPROCESSING_ID,
        "action-complete-word-mask-v2"
    );
}

#[test]
fn completion_extends_each_side_and_is_equivariant_under_quarter_turns() {
    let mut image = scene(128, 64);
    for shape in [
        [28, 12, 16, 32],
        [68, 20, 16, 32],
        [24, 46, 8, 2],
        [82, 14, 6, 2],
        [50, 10, 2, 4],
        [50, 50, 2, 6],
    ] {
        ink(&mut image, shape);
    }
    let mut expected = rect([24, 10, 64, 46]);
    for _ in 0..4 {
        let word = prepared(prepare_complete_action_word(&image).unwrap());
        assert_eq!(word.bounds, expected);
        assert_eq!(word.foreground_pixels, 1072);
        let mut rotated = scene(image.height(), image.width());
        for y in 0..image.height() {
            for x in 0..image.width() {
                let src = ((y * image.width() + x) * 3) as usize;
                let dst = ((x * rotated.width() + image.height() - 1 - y) * 3) as usize;
                rotated.pixels_mut()[dst..dst + 3].copy_from_slice(&image.pixels()[src..src + 3]);
            }
        }
        expected = rect([
            image.height() - expected.y - expected.height,
            expected.x,
            expected.height,
            expected.width,
        ]);
        image = rotated;
    }
}

#[test]
fn overlapping_component_box_without_ink_inside_does_not_expand_word() {
    let mut image = scene(128, 64);
    ink(&mut image, [40, 20, 20, 24]);
    ink(&mut image, [38, 18, 5, 1]);
    ink(&mut image, [38, 18, 1, 5]);
    let word = prepared(prepare_complete_action_word(&image).unwrap());
    assert_eq!(word.bounds, rect([40, 20, 20, 24]));
    assert_eq!(word.foreground_pixels, 480);
}

#[test]
fn unchanged_geometry_keeps_the_original_tensor_and_uncertainty_contract() {
    let mut image = scene(128, 64);
    assert!(matches!(
        prepare_complete_action_word(&image).unwrap(),
        ActionWordInput::Empty
    ));
    ink(&mut image, [20, 10, 2, 2]);
    assert!(matches!(
        prepare_complete_action_word(&image).unwrap(),
        ActionWordInput::Unlocalized
    ));
    ink(&mut image, [40, 8, 32, 48]);
    let old = prepared(prepare_action_word(&image).unwrap());
    let complete = prepared(prepare_complete_action_word(&image).unwrap());
    assert_eq!(old.bounds, complete.bounds);
    assert_eq!(old.pixels, complete.pixels);
    assert_eq!(old.foreground_pixels, complete.foreground_pixels);
    let invalid = scene(15, 64);
    assert!(matches!(
        prepare_complete_action_word(&invalid),
        Err(VisionError::InvalidImage)
    ));
}

#[test]
fn maximum_size_many_fragments_remains_bounded_and_unlocalized() {
    let mut image = scene(1024, 1024);
    for y in (0..1024).step_by(2) {
        for x in (0..1024).step_by(2) {
            ink(&mut image, [x, y, 1, 1]);
        }
    }
    assert!(matches!(
        prepare_complete_action_word(&image).unwrap(),
        ActionWordInput::Unlocalized
    ));
}

#[test]
fn complete_geometry_matches_independent_pillow_masks() {
    #[derive(serde::Deserialize)]
    struct Case {
        name: String,
        bounds: [u32; 4],
        foreground_pixels: u32,
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synthetic/action-components");
    let cases: Vec<Case> =
        serde_json::from_slice(&std::fs::read(root.join("cases.json")).unwrap()).unwrap();
    assert_eq!(cases.len(), 3);
    for case in cases {
        let image = RgbImage::read_png(&root.join(format!("{}.png", case.name))).unwrap();
        let word = prepared(prepare_complete_action_word(&image).unwrap());
        assert_eq!(word.bounds, rect(case.bounds), "{}", case.name);
        assert_eq!(
            word.foreground_pixels, case.foreground_pixels,
            "{}",
            case.name
        );
        let expected = std::fs::read(root.join(format!("{}.mask", case.name))).unwrap();
        assert_eq!(word.pixels.len(), expected.len());
        for (index, (&actual, reference)) in word.pixels.iter().zip(expected).enumerate() {
            assert_eq!(actual, f32::from(reference), "{} pixel {index}", case.name);
        }
    }
}
