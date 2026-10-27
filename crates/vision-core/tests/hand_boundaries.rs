use vision_core::{Rect, RgbImage, detect_hand_slots};

fn fill(image: &mut RgbImage, rect: Rect, color: [u8; 3]) {
    let width = image.width();
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            let i = ((y * width + x) * 3) as usize;
            image.pixels_mut()[i..i + 3].copy_from_slice(&color);
        }
    }
}
fn scene() -> RgbImage {
    RgbImage::new(128, 104, vec![35; 128 * 104 * 3]).unwrap()
}
fn paper(x: u32, y: u32) -> Rect {
    Rect {
        x,
        y,
        width: 48,
        height: 80,
    }
}
fn caption(image: &mut RgbImage, x: u32, color: [u8; 3]) {
    fill(
        image,
        Rect {
            x,
            y: 0,
            width: 48,
            height: 14,
        },
        color,
    );
    // Sparse, disconnected pale text above the physical paper face.
    for dx in [7, 20, 34] {
        fill(
            image,
            Rect {
                x: x + dx,
                y: 2,
                width: 3,
                height: 9,
            },
            [250; 3],
        );
    }
}

#[test]
fn sparse_text_on_colored_headers_does_not_extend_the_paper_face() {
    for color in [[45, 125, 190], [245, 175, 35]] {
        let mut image = scene();
        let normal = paper(8, 16);
        let raised = paper(72, 3);
        fill(&mut image, normal, [225; 3]);
        fill(&mut image, raised, [225; 3]);
        caption(&mut image, normal.x, color);
        assert_eq!(detect_hand_slots(&image), vec![normal, raised]);
    }
}

#[test]
fn header_trimming_preserves_relative_boundaries_at_multiple_pixel_scales() {
    for scale in [1, 2, 3] {
        let mut image = scene();
        let normal = paper(8, 16);
        let raised = paper(72, 3);
        fill(&mut image, normal, [225; 3]);
        fill(&mut image, raised, [225; 3]);
        caption(&mut image, normal.x, [45, 125, 190]);
        let mut enlarged = RgbImage::new(
            image.width() * scale,
            image.height() * scale,
            vec![0; (image.width() * image.height() * scale * scale * 3) as usize],
        )
        .unwrap();
        for y in 0..enlarged.height() {
            for x in 0..enlarged.width() {
                let source = (((y / scale) * image.width() + x / scale) * 3) as usize;
                fill(
                    &mut enlarged,
                    Rect {
                        x,
                        y,
                        width: 1,
                        height: 1,
                    },
                    image.pixels()[source..source + 3].try_into().unwrap(),
                );
            }
        }
        let expected: Vec<_> = [normal, raised]
            .into_iter()
            .map(|r| Rect {
                x: r.x * scale,
                y: r.y * scale,
                width: r.width * scale,
                height: r.height * scale,
            })
            .collect();
        assert_eq!(detect_hand_slots(&enlarged), expected, "scale={scale}");
    }
}

#[test]
fn a_dense_leading_paper_row_preserves_raised_and_tightly_printed_faces() {
    let mut image = scene();
    let face = paper(8, 2);
    fill(&mut image, face, [225; 3]);
    // A legitimate colored glyph follows just one white leading row.
    // Do not seek a later broad pale run and crop away the glyph.
    fill(
        &mut image,
        Rect {
            x: 9,
            y: 3,
            width: 46,
            height: 10,
        },
        [35, 115, 190],
    );
    assert_eq!(detect_hand_slots(&image), vec![face]);
}

#[test]
fn neutral_or_red_content_is_not_mistaken_for_a_caption_header() {
    for color in [[85, 85, 85], [185, 25, 35]] {
        let mut image = scene();
        let body = paper(8, 16);
        fill(&mut image, body, [225; 3]);
        caption(&mut image, body.x, color);
        // Without the specific saturated header evidence, preserve the old
        // candidate instead of silently deleting a possible glyph.
        let mut extended = body;
        extended.y = 2;
        extended.height = 94;
        assert_eq!(detect_hand_slots(&image), vec![extended]);
    }
}

#[test]
fn a_short_colored_edge_is_not_enough_evidence_to_change_the_boundary() {
    let mut image = scene();
    let body = paper(8, 16);
    fill(&mut image, body, [225; 3]);
    fill(
        &mut image,
        Rect {
            x: 8,
            y: 12,
            width: 48,
            height: 4,
        },
        [245, 175, 35],
    );
    fill(
        &mut image,
        Rect {
            x: 18,
            y: 12,
            width: 2,
            height: 2,
        },
        [250; 3],
    );
    let expected = Rect {
        x: 8,
        y: 12,
        width: 48,
        height: 84,
    };
    assert_eq!(detect_hand_slots(&image), vec![expected]);
}

#[test]
fn removing_external_header_text_does_not_bypass_blank_context() {
    use vision_core::classifier::{
        ClassifierDecision, FaceObservation, ModelManifest, RejectReason,
    };
    let model = ModelManifest::parse(include_bytes!(
        "../../../tests/fixtures/synthetic/neural/classifier.json"
    ))
    .unwrap();
    let mut image = scene();
    let body = paper(8, 16);
    fill(&mut image, body, [225; 3]);
    caption(&mut image, body.x, [45, 125, 190]);
    let slots = detect_hand_slots(&image);
    assert_eq!(slots, vec![body]);
    let face = FaceObservation::in_hand(&image, slots[0]).unwrap();
    assert!(face.glyph_bounds().is_none());
    assert!(face.blank_face_verified());
    let mut white = [0.0; 37];
    white[31] = 20.0;
    assert!(matches!(
        model.decide(&face, &white),
        ClassifierDecision::Accepted { .. }
    ));
    let isolated = FaceObservation::isolated(&image.crop(body).unwrap()).unwrap();
    assert!(matches!(
        model.decide(&isolated, &white),
        ClassifierDecision::Rejected {
            reason: RejectReason::UnverifiedBlank,
            ..
        }
    ));
    fill(
        &mut image,
        Rect {
            x: 22,
            y: 38,
            width: 12,
            height: 30,
        },
        [15; 3],
    );
    let printed = FaceObservation::in_hand(&image, body).unwrap();
    assert!(matches!(
        model.decide(&printed, &white),
        ClassifierDecision::Rejected {
            reason: RejectReason::UnexpectedGlyphForWhite,
            ..
        }
    ));
}
