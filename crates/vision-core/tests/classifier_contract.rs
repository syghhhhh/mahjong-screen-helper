use riichi_core::Tile;
use vision_core::{
    Rect, RgbImage,
    classifier::{ClassifierDecision, FaceObservation, ModelManifest, RejectReason},
};

fn json() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/synthetic/neural/classifier.json"
    ))
    .unwrap()
}
fn manifest(value: &serde_json::Value) -> ModelManifest {
    ModelManifest::parse(&serde_json::to_vec(value).unwrap()).unwrap()
}
fn ink() -> RgbImage {
    let mut rgb = vec![240; 32 * 48 * 3];
    for y in 10..38 {
        for x in 12..20 {
            let i = (y * 32 + x) * 3;
            rgb[i..i + 3].copy_from_slice(&[15, 25, 35]);
        }
    }
    RgbImage::new(32, 48, rgb).unwrap()
}
fn peaked(index: usize) -> [f32; 37] {
    let mut logits = [0.0; 37];
    logits[index] = 20.0;
    logits
}
fn rejected(decision: ClassifierDecision, reason: RejectReason) {
    assert!(
        matches!(decision, ClassifierDecision::Rejected { reason: actual, .. } if actual == reason)
    );
}

#[test]
fn manifest_binds_exact_class_map_preprocessing_and_file_identity() {
    let good = json();
    let m = manifest(&good);
    assert_eq!(m.model_id(), "synthetic-affine-37");
    assert_eq!(m.model_file(), "affine-37.onnx");
    assert_eq!(m.license(), "MIT");
    for (field, value) in [
        ("schema_version", serde_json::json!("2.0")),
        ("preprocessing", serde_json::json!("other-input")),
        ("model_sha256", serde_json::json!("bad-hash")),
        ("license", serde_json::json!("")),
        ("model_id", serde_json::json!("")),
    ] {
        let mut changed = good.clone();
        changed[field] = value;
        assert!(
            ModelManifest::parse(&serde_json::to_vec(&changed).unwrap()).is_err(),
            "{field}"
        );
    }
    for name in [
        "../model.onnx",
        "C:\\model.onnx",
        "dir/model.onnx",
        "model.bin",
        "..onnx",
        "model:stream.onnx",
        "NUL.onnx",
        "con.onnx",
        "LPT1.onnx",
    ] {
        let mut changed = good.clone();
        changed["model_file"] = name.into();
        assert!(
            ModelManifest::parse(&serde_json::to_vec(&changed).unwrap()).is_err(),
            "{name}"
        );
    }
    for order in [serde_json::json!(["1m"]), {
        let mut order = good["class_order"].as_array().unwrap().clone();
        order.swap(34, 36);
        serde_json::Value::Array(order)
    }] {
        let mut changed = good.clone();
        changed["class_order"] = order;
        assert!(ModelManifest::parse(&serde_json::to_vec(&changed).unwrap()).is_err());
    }
}

#[test]
fn rejects_unknown_duplicate_missing_oversized_and_unsafe_threshold_fields() {
    let good = json();
    let mut changed = good.clone();
    changed["silent_fallback"] = true.into();
    assert!(ModelManifest::parse(&serde_json::to_vec(&changed).unwrap()).is_err());
    let duplicate =
        serde_json::to_string(&good)
            .unwrap()
            .replacen('{', "{\"schema_version\":\"1.0\",", 1);
    assert!(ModelManifest::parse(duplicate.as_bytes()).is_err());
    let mut changed = good.clone();
    changed.as_object_mut().unwrap().remove("thresholds");
    assert!(ModelManifest::parse(&serde_json::to_vec(&changed).unwrap()).is_err());
    assert!(ModelManifest::parse(&vec![b' '; 16_385]).is_err());
    for (field, values) in [
        ("min_probability", [-0.1, 0.1, 1.1]),
        ("min_margin", [-0.1, 0.0, 1.1]),
        ("max_normalized_entropy", [-0.1, 0.0, 1.1]),
    ] {
        for value in values {
            let mut changed = good.clone();
            changed["thresholds"][field] = value.into();
            assert!(ModelManifest::parse(&serde_json::to_vec(&changed).unwrap()).is_err());
        }
    }
}

#[test]
fn stable_scoring_preserves_red_classes_and_rejects_invalid_or_ambiguous_logits() {
    let m = manifest(&json());
    let face = FaceObservation::isolated(&ink()).unwrap();
    for (index, code) in [(0, "1m"), (34, "0m"), (35, "0p"), (36, "0s")] {
        let result = m.decide(&face, &peaked(index));
        assert!(
            matches!(result, ClassifierDecision::Accepted { tile, .. } if tile == Tile::from_code(code).unwrap())
        );
    }
    rejected(m.decide(&face, &[0.0; 37]), RejectReason::LowProbability);
    let mut huge = [f32::MIN; 37];
    huge[1] = f32::MAX;
    assert!(matches!(
        m.decide(&face, &huge),
        ClassifierDecision::Accepted { .. }
    ));
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut bad = peaked(0);
        bad[36] = value;
        rejected(m.decide(&face, &bad), RejectReason::InvalidLogits);
    }
    let mut ambiguous = peaked(0);
    ambiguous[1] = 19.98;
    let mut loose = json();
    loose["thresholds"]["min_probability"] = 0.5.into();
    rejected(
        manifest(&loose).decide(&face, &ambiguous),
        RejectReason::Ambiguous,
    );
    let mut strict = json();
    strict["thresholds"]["max_normalized_entropy"] = 0.01.into();
    let mut spread = [0.0; 37];
    spread[0] = 6.0;
    rejected(
        manifest(&strict).decide(&face, &spread),
        RejectReason::HighEntropy,
    );
}

#[test]
fn a_high_scoring_blank_region_is_not_white_dragon_without_enclosing_tile_edges() {
    let m = manifest(&json());
    let blank = RgbImage::new(40, 64, vec![240; 40 * 64 * 3]).unwrap();
    let face = FaceObservation::isolated(&blank).unwrap();
    rejected(m.decide(&face, &peaked(31)), RejectReason::UnverifiedBlank);
    rejected(m.decide(&face, &peaked(0)), RejectReason::UnverifiedBlank);
    let glyph = FaceObservation::isolated(&ink()).unwrap();
    rejected(
        m.decide(&glyph, &peaked(31)),
        RejectReason::UnexpectedGlyphForWhite,
    );
    let region = Rect {
        x: 5,
        y: 5,
        width: 40,
        height: 64,
    };
    let mut scene = RgbImage::new(50, 74, vec![30; 50 * 74 * 3]).unwrap();
    for y in region.y..region.y + region.height {
        for x in region.x..region.x + region.width {
            let i = ((y * 50 + x) * 3) as usize;
            scene.pixels_mut()[i..i + 3].copy_from_slice(&[240; 3]);
        }
    }
    let observed = FaceObservation::in_hand(&scene, region).unwrap();
    assert!(
        matches!(m.decide(&observed, &peaked(31)), ClassifierDecision::Accepted { tile, .. } if tile == Tile::from_code("5z").unwrap())
    );
    // Losing one boundary invalidates evidence even when the white pixels and
    // hypothetical model logits remain exactly the same.
    for x in 0..50 {
        for y in 0..5 {
            let i = ((y * 50 + x) * 3) as usize;
            scene.pixels_mut()[i..i + 3].copy_from_slice(&[240; 3]);
        }
    }
    let observed = FaceObservation::in_hand(&scene, region).unwrap();
    rejected(
        m.decide(&observed, &peaked(31)),
        RejectReason::UnverifiedBlank,
    );
}

#[test]
fn locating_a_shadowed_public_face_does_not_verify_it_as_white_dragon() {
    let region = Rect {
        x: 5,
        y: 5,
        width: 40,
        height: 64,
    };
    let mut scene = RgbImage::new(50, 74, vec![30; 50 * 74 * 3]).unwrap();
    for y in region.y..region.y + region.height {
        for x in region.x..region.x + region.width {
            let at = ((y * scene.width() + x) * 3) as usize;
            scene.pixels_mut()[at..at + 3].copy_from_slice(&[170; 3]);
        }
    }
    assert!(vision_core::detect_public_slots(&scene).contains(&region));
    let face = FaceObservation::in_public(&scene, region, vision_core::Rotation::Upright).unwrap();
    assert!(face.glyph_bounds().is_none());
    assert!(!face.blank_face_verified());
    rejected(
        manifest(&json()).decide(&face, &peaked(31)),
        RejectReason::UnverifiedBlank,
    );
}
