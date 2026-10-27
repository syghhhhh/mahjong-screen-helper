//! Local-only raw action-word backend diagnostic; never activates the overlay.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, fs::File, io::Read, path::Path, time::Instant};
    use vision_core::neural::{ACTION_INPUT_FLOATS, WinMlActionSession};
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err(
            "usage: inspect_action_logits action.onnx masks.f32le (1..256 normalized words)".into(),
        );
    }
    const WORD_BYTES: usize = ACTION_INPUT_FLOATS * 4;
    const MAX_BYTES: usize = WORD_BYTES * 256;
    let mut bytes = Vec::new();
    File::open(&args[2])?
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.is_empty() || bytes.len() > MAX_BYTES || bytes.len() % WORD_BYTES != 0 {
        return Err("invalid action-word tensor file length".into());
    }
    let started = Instant::now();
    let session = WinMlActionSession::load(Path::new(&args[1]))?;
    println!(
        "model_load_ms: {:.3}",
        started.elapsed().as_secs_f64() * 1000.0
    );
    let mut elapsed = 0.0;
    for (index, word) in bytes.chunks_exact(WORD_BYTES).enumerate() {
        let input: Vec<f32> = word
            .chunks_exact(4)
            .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
            .collect();
        let started = Instant::now();
        let values = session.evaluate(&input)?;
        elapsed += started.elapsed().as_secs_f64() * 1000.0;
        println!(
            "logits {index}: {}",
            values
                .iter()
                .map(|v| format!("{v:.9}"))
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    println!("evaluation_total_ms: {elapsed:.3}");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Native Windows ML diagnostic requires Windows 10 version 1809 or newer");
}
