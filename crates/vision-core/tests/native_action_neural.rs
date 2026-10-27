#![cfg(windows)]

use std::path::PathBuf;
use vision_core::neural::{ACTION_INPUT_FLOATS, NeuralError, WinMlActionSession, WinMlTileSession};

fn fixture(kind: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synthetic")
        .join(kind)
        .join(name)
}

#[test]
fn action_cpu_logits_match_an_independent_mean_affine_reference() {
    let session = WinMlActionSession::load(&fixture("action-neural", "affine-7.onnx")).unwrap();
    for constant in [0.0, 0.25, 1.0] {
        let values = session
            .evaluate(&vec![constant; ACTION_INPUT_FLOATS])
            .unwrap();
        for (i, actual) in values.into_iter().enumerate() {
            let expected = 0.03 * i as f32 + constant * 0.1 * (i as f32 - 3.0);
            assert!((actual - expected).abs() < 1e-5, "index {i}: {actual}");
        }
    }
}

#[test]
fn action_and_tile_models_cannot_cross_tensor_contracts() {
    assert!(matches!(
        WinMlActionSession::load(&fixture("neural", "affine-37.onnx")),
        Err(NeuralError::UnsupportedModel)
    ));
    assert!(matches!(
        WinMlTileSession::load(&fixture("action-neural", "affine-7.onnx")),
        Err(NeuralError::UnsupportedModel)
    ));
    for name in ["wrong-channels.onnx", "wrong-classes.onnx"] {
        let result = WinMlActionSession::load(&fixture("action-neural", name));
        assert!(
            matches!(result, Err(NeuralError::UnsupportedModel)),
            "{name}: {:?}",
            result.err()
        );
    }
}

#[test]
fn invalid_action_inputs_and_nonfinite_logits_are_rejected() {
    let session = WinMlActionSession::load(&fixture("action-neural", "affine-7.onnx")).unwrap();
    assert_eq!(session.evaluate(&[]), Err(NeuralError::InvalidInput));
    for value in [f32::NAN, f32::INFINITY, -0.01, 1.01] {
        let mut input = vec![0.0; ACTION_INPUT_FLOATS];
        input[17] = value;
        assert_eq!(session.evaluate(&input), Err(NeuralError::InvalidInput));
    }
    let invalid =
        WinMlActionSession::load(&fixture("action-neural", "nonfinite-output.onnx")).unwrap();
    assert_eq!(
        invalid.evaluate(&vec![0.0; ACTION_INPUT_FLOATS]),
        Err(NeuralError::InvalidOutput)
    );
    assert!(matches!(
        WinMlActionSession::load(&fixture("action-neural", "missing.onnx")),
        Err(NeuralError::InvalidModel)
    ));
}

#[test]
fn interleaved_action_and_tile_sessions_keep_their_shapes_and_apartment() {
    use windows::Win32::System::WinRT::{
        RO_INIT_MULTITHREADED, RO_INIT_SINGLETHREADED, RoInitialize, RoUninitialize,
    };
    unsafe { RoInitialize(RO_INIT_SINGLETHREADED) }.unwrap();
    for _ in 0..3 {
        let tile = WinMlTileSession::load(&fixture("neural", "affine-37.onnx")).unwrap();
        let action = WinMlActionSession::load(&fixture("action-neural", "affine-7.onnx")).unwrap();
        assert_eq!(
            tile.evaluate(&vec![0.0; vision_core::neural::INPUT_FLOATS])
                .unwrap()
                .len(),
            37
        );
        assert_eq!(
            action
                .evaluate(&vec![0.0; ACTION_INPUT_FLOATS])
                .unwrap()
                .len(),
            7
        );
    }
    let changed = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.unwrap_err();
    assert_eq!(changed.code().0 as u32, 0x80010106);
    unsafe { RoUninitialize() };
}
