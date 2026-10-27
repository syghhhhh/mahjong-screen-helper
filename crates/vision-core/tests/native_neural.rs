#![cfg(windows)]

use std::path::PathBuf;
use vision_core::neural::{INPUT_FLOATS, NeuralError, WinMlTileSession};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synthetic/neural")
        .join(name)
}

#[test]
fn native_cpu_inference_matches_the_independent_affine_reference() {
    let session = WinMlTileSession::load(&fixture("affine-37.onnx")).unwrap();
    let zeros = session.evaluate(&vec![0.0; INPUT_FLOATS]).unwrap();
    for (index, value) in zeros.iter().enumerate() {
        assert!((*value - index as f32 * 0.03).abs() < 0.00001);
    }
    let mut pixels = vec![0.0; INPUT_FLOATS];
    let mut means = [0.0f64; 3];
    for (channel, mean) in means.iter_mut().enumerate() {
        for pixel in 0..48 * 32 {
            let value = channel as f32 * 0.2 + (pixel % 7) as f32 * 0.05;
            pixels[channel * 48 * 32 + pixel] = value;
            *mean += value as f64 / (48 * 32) as f64;
        }
    }
    let output = session.evaluate(&pixels).unwrap();
    for (index, value) in output.iter().enumerate() {
        let expected = index as f64 * 0.03
            + (0..3)
                .map(|channel| means[channel] * (((index + channel) % 5) as f64 - 2.0) * 0.1)
                .sum::<f64>();
        assert!(
            (*value as f64 - expected).abs() < 0.00001,
            "index {index}: {value} != {expected}"
        );
    }
}

#[test]
fn rejects_invalid_input_without_returning_logits() {
    let session = WinMlTileSession::load(&fixture("affine-37.onnx")).unwrap();
    assert_eq!(session.evaluate(&[]), Err(NeuralError::InvalidInput));
    for invalid in [f32::NAN, f32::INFINITY, -0.01, 1.01] {
        let mut pixels = vec![0.0; INPUT_FLOATS];
        pixels[30] = invalid;
        assert_eq!(session.evaluate(&pixels), Err(NeuralError::InvalidInput));
    }
}

#[test]
fn rejects_wrong_tensor_shapes_and_non_models() {
    for name in ["wrong-channels.onnx", "wrong-classes.onnx"] {
        assert!(matches!(
            WinMlTileSession::load(&fixture(name)),
            Err(NeuralError::UnsupportedModel)
        ));
    }
    assert!(matches!(
        WinMlTileSession::load(&fixture("missing.onnx")),
        Err(NeuralError::InvalidModel)
    ));
    assert!(matches!(
        WinMlTileSession::load(&fixture("README.md")),
        Err(NeuralError::InvalidModel)
    ));
    assert!(matches!(
        WinMlTileSession::load(&fixture("not-a-model.onnx")),
        Err(NeuralError::InvalidModel)
    ));
}

#[test]
fn rejects_nonfinite_model_output() {
    let session = WinMlTileSession::load(&fixture("nonfinite-output.onnx")).unwrap();
    assert_eq!(
        session.evaluate(&vec![0.0; INPUT_FLOATS]),
        Err(NeuralError::InvalidOutput)
    );
}

#[test]
fn preserves_the_callers_sta_apartment() {
    use windows::Win32::System::WinRT::{
        RO_INIT_MULTITHREADED, RO_INIT_SINGLETHREADED, RoInitialize, RoUninitialize,
    };
    unsafe { RoInitialize(RO_INIT_SINGLETHREADED) }.unwrap();
    {
        let session = WinMlTileSession::load(&fixture("affine-37.onnx")).unwrap();
        session.evaluate(&vec![0.0; INPUT_FLOATS]).unwrap();
    }
    let mode = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.unwrap_err();
    assert_eq!(mode.code().0 as u32, 0x80010106);
    unsafe {
        RoUninitialize();
    }
}
