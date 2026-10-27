use vision_core::{
    Rect, RgbImage, VisionError,
    action_input::{ActionWordInput, INPUT_FLOATS, prepare_action_word},
};

fn fill(image: &mut RgbImage, rect: Rect, color: [u8; 3]) {
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            let i = ((y * image.width() + x) * 3) as usize;
            image.pixels_mut()[i..i + 3].copy_from_slice(&color);
        }
    }
}
fn scene() -> RgbImage {
    RgbImage::new(128, 64, vec![0; 128 * 64 * 3]).unwrap()
}

#[test]
fn empty_and_unsupported_fragments_remain_distinguishable() {
    let mut image = scene();
    assert!(matches!(
        prepare_action_word(&image).unwrap(),
        ActionWordInput::Empty
    ));
    fill(
        &mut image,
        Rect {
            x: 20,
            y: 10,
            width: 2,
            height: 2,
        },
        [240; 3],
    );
    assert!(matches!(
        prepare_action_word(&image).unwrap(),
        ActionWordInput::Unlocalized
    ));
    let invalid = RgbImage::new(15, 64, vec![0; 15 * 64 * 3]).unwrap();
    assert!(matches!(
        prepare_action_word(&invalid),
        Err(VisionError::InvalidImage)
    ));
}

#[test]
fn supported_ink_is_centered_and_tiny_external_decorations_are_excluded() {
    let mut image = scene();
    let glyph = Rect {
        x: 40,
        y: 8,
        width: 32,
        height: 48,
    };
    fill(&mut image, glyph, [220, 230, 150]);
    fill(
        &mut image,
        Rect {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        },
        [255; 3],
    );
    let original = image.clone();
    let ActionWordInput::Prepared(word) = prepare_action_word(&image).unwrap() else {
        panic!("missing word")
    };
    assert_eq!(image, original);
    assert_eq!(word.bounds, glyph);
    assert_eq!(word.foreground_pixels, 32 * 48);
    assert_eq!(word.pixels.len(), INPUT_FLOATS);
    // 32:48 fits to 37:56, with the odd horizontal spare pixel on the right.
    for y in 0..64 {
        for x in 0..128 {
            assert_eq!(
                word.pixels[y * 128 + x],
                if (4..60).contains(&y) && (45..82).contains(&x) {
                    1.0
                } else {
                    0.0
                }
            );
        }
    }
}

#[test]
fn disconnected_strokes_and_small_enclosed_marks_share_one_word() {
    let mut image = scene();
    for r in [
        Rect {
            x: 20,
            y: 12,
            width: 8,
            height: 40,
        },
        Rect {
            x: 60,
            y: 12,
            width: 8,
            height: 40,
        },
        Rect {
            x: 43,
            y: 31,
            width: 2,
            height: 2,
        },
    ] {
        fill(&mut image, r, [240; 3]);
    }
    let ActionWordInput::Prepared(word) = prepare_action_word(&image).unwrap() else {
        panic!("missing word")
    };
    assert_eq!(
        word.bounds,
        Rect {
            x: 20,
            y: 12,
            width: 48,
            height: 40
        }
    );
    assert_eq!(word.foreground_pixels, 644);
    assert!(word.pixels[32 * 128 + 64] > 0.0);
}

#[test]
fn saturated_background_color_does_not_become_word_ink() {
    for color in [[0, 80, 140], [150, 45, 115], [180, 80, 0]] {
        let mut image = scene();
        fill(
            &mut image,
            Rect {
                x: 0,
                y: 0,
                width: 128,
                height: 64,
            },
            color,
        );
        assert!(matches!(
            prepare_action_word(&image).unwrap(),
            ActionWordInput::Empty
        ));
    }
}

#[test]
fn original_geometry_matches_independent_pillow_nearest_masks() {
    #[derive(serde::Deserialize)]
    struct Case {
        name: String,
        kind: String,
        bounds: Option<[u32; 4]>,
        foreground_pixels: u32,
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synthetic/action-input");
    let cases: Vec<Case> =
        serde_json::from_slice(&std::fs::read(root.join("cases.json")).unwrap()).unwrap();
    assert_eq!(cases.len(), 15);
    for case in cases {
        let image = RgbImage::read_png(&root.join(format!("{}.png", case.name))).unwrap();
        match prepare_action_word(&image).unwrap() {
            ActionWordInput::Empty => assert_eq!(case.kind, "empty", "{}", case.name),
            ActionWordInput::Unlocalized => assert_eq!(case.kind, "unlocalized", "{}", case.name),
            ActionWordInput::Prepared(word) => {
                assert_eq!(case.kind, "prepared", "{}", case.name);
                let [x, y, width, height] = case.bounds.unwrap();
                assert_eq!(
                    word.bounds,
                    Rect {
                        x,
                        y,
                        width,
                        height
                    },
                    "{}",
                    case.name
                );
                assert_eq!(
                    word.foreground_pixels, case.foreground_pixels,
                    "{}",
                    case.name
                );
                let expected = std::fs::read(root.join(format!("{}.mask", case.name))).unwrap();
                assert_eq!(expected.len(), INPUT_FLOATS);
                for (i, (&actual, &reference)) in word.pixels.iter().zip(&expected).enumerate() {
                    assert_eq!(actual, f32::from(reference), "{} pixel {i}", case.name);
                }
            }
        }
    }
}
