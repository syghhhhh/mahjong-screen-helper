#![cfg(windows)]
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use vision_core::{
    RgbImage,
    classifier::{ClassifierDecision, FaceObservation, RejectReason},
    neural::NeuralError,
    neural_classifier::{ClassifierLoadError, LocalTileClassifier},
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synthetic/neural")
        .join(name)
}
struct Temporary(PathBuf);
impl Temporary {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("msh-classifier-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn manifest(&self, model: &str, hash: Option<&str>) -> PathBuf {
        fs::copy(fixture(model), self.0.join("affine-37.onnx")).unwrap();
        let mut json: serde_json::Value =
            serde_json::from_slice(&fs::read(fixture("classifier.json")).unwrap()).unwrap();
        if let Some(hash) = hash {
            json["model_sha256"] = hash.into();
        }
        let path = self.0.join("classifier.json");
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        path
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        for name in ["affine-37.onnx", "classifier.json"] {
            let _ = fs::remove_file(self.0.join(name));
        }
        let _ = fs::remove_dir(&self.0);
    }
}

#[test]
fn local_manifest_loads_bounded_model_and_routes_native_scores_through_rejection() {
    let classifier = LocalTileClassifier::load(&fixture("classifier.json")).unwrap();
    assert_eq!(classifier.manifest().model_id(), "synthetic-affine-37");
    let face =
        FaceObservation::isolated(&RgbImage::new(32, 48, vec![240; 32 * 48 * 3]).unwrap()).unwrap();
    assert!(matches!(
        classifier.classify(&face).unwrap(),
        ClassifierDecision::Rejected {
            reason: RejectReason::LowProbability,
            ..
        }
    ));
}

#[test]
fn local_manifest_selects_letterbox_pixels_instead_of_stretching_old_inputs() {
    use vision_core::{
        classifier::ModelManifest,
        neural::WinMlTileSession,
        neural_input::{prepare_tile, prepare_tile_letterbox},
    };
    let dir = Temporary::new();
    let path = dir.manifest("affine-37.onnx", None);
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["preprocessing"] = "rgb-inverse-glyph-letterbox-v3".into();
    let bytes = serde_json::to_vec(&value).unwrap();
    fs::write(&path, &bytes).unwrap();
    let expected_manifest = ModelManifest::parse(&bytes).unwrap();
    let classifier = LocalTileClassifier::load(&path).unwrap();
    let source = fixture("../neural-letterbox/narrow.png");
    let image = RgbImage::read_png(&source).unwrap();
    let face = FaceObservation::isolated(&image).unwrap();
    let direct = WinMlTileSession::load(&fixture("affine-37.onnx")).unwrap();
    let expected = expected_manifest.decide(
        &face,
        &direct
            .evaluate(&prepare_tile_letterbox(&image).unwrap().pixels)
            .unwrap(),
    );
    let old = expected_manifest.decide(
        &face,
        &direct
            .evaluate(&prepare_tile(&image).unwrap().pixels)
            .unwrap(),
    );
    let scores = |decision| match decision {
        ClassifierDecision::Rejected {
            scores: Some(scores),
            ..
        } => scores,
        other => panic!("expected finite low-confidence arithmetic fixture, got {other:?}"),
    };
    let actual = scores(classifier.classify(&face).unwrap());
    let expected = scores(expected);
    assert!((actual.probability - expected.probability).abs() < 1e-7);
    assert!((actual.normalized_entropy - expected.normalized_entropy).abs() < 1e-7);
    assert!(
        (actual.probability - scores(old).probability).abs() > 1e-4,
        "fixture must distinguish selecting the wrong preprocessing"
    );
}

#[test]
fn changed_bytes_and_invalid_shapes_cannot_inherit_a_valid_manifest() {
    let dir = Temporary::new();
    let path = dir.manifest("affine-37.onnx", Some(&"0".repeat(64)));
    assert!(matches!(
        LocalTileClassifier::load(&path),
        Err(ClassifierLoadError::ModelHashMismatch)
    ));
    let path = dir.manifest(
        "wrong-channels.onnx",
        Some("0faa1de3385290dc3a6a0817bdf9237c733a22747b76cef1191b1bbc4248e345"),
    );
    assert!(matches!(
        LocalTileClassifier::load(&path),
        Err(ClassifierLoadError::Backend(NeuralError::UnsupportedModel))
    ));
    assert!(matches!(
        LocalTileClassifier::load(Path::new("missing-model-manifest.json")),
        Err(ClassifierLoadError::Manifest)
    ));
}

#[test]
fn loaded_model_is_read_locked_until_its_native_session_is_released() {
    let dir = Temporary::new();
    let path = dir.manifest("affine-37.onnx", None);
    let classifier = LocalTileClassifier::load(&path).unwrap();
    assert!(
        fs::OpenOptions::new()
            .write(true)
            .open(dir.0.join("affine-37.onnx"))
            .is_err()
    );
    drop(classifier);
    assert!(
        fs::OpenOptions::new()
            .write(true)
            .open(dir.0.join("affine-37.onnx"))
            .is_ok()
    );
}
