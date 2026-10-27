//! Local-only native ONNX diagnostic over preprocessed, little-endian tensors.
//! Does not accept a tile class map or produce Mahjong recommendations.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, fs, path::Path, time::Instant};
    use vision_core::neural::{INPUT_FLOATS, WinMlTileSession};
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: inspect_neural classifier.onnx tensors.f32le".into());
    }
    let size = fs::metadata(&args[2])?.len();
    if size == 0 || size > (INPUT_FLOATS * 4 * 256) as u64 || size % (INPUT_FLOATS * 4) as u64 != 0
    {
        return Err(
            "tensor file must contain 1..256 complete NCHW float32 [1,3,48,32] inputs".into(),
        );
    }
    let bytes = fs::read(&args[2])?;
    let started = Instant::now();
    let session = WinMlTileSession::load(Path::new(&args[1]))?;
    println!(
        "model_load_ms: {:.3}",
        started.elapsed().as_secs_f64() * 1000.0
    );
    let started = Instant::now();
    for (index, sample) in bytes.chunks_exact(INPUT_FLOATS * 4).enumerate() {
        let input: Vec<_> = sample
            .chunks_exact(4)
            .map(|v| f32::from_le_bytes(v.try_into().expect("four-byte float")))
            .collect();
        let logits = session.evaluate(&input)?;
        let values: Vec<_> = logits.iter().map(|v| format!("{v:.6}")).collect();
        println!("logits {index}: {}", values.join(","));
    }
    println!(
        "evaluation_total_ms: {:.3}",
        started.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Native Windows ML diagnostic requires Windows 10 version 1809 or newer");
}
