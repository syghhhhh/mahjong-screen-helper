//! Local RGB preprocessing + native ONNX diagnostic; never enables a model in
//! the overlay or interprets logits as a validated Mahjong recommendation.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, fs::File, io::Write, path::Path, time::Instant};
    use vision_core::{
        RgbImage,
        neural::WinMlTileSession,
        neural_input::{prepare_tile, prepare_tile_letterbox},
    };

    let mut args: Vec<_> = env::args().collect();
    let letterbox = args.get(1).is_some_and(|arg| arg == "--letterbox");
    if letterbox {
        args.remove(1);
    }
    if !(4..=259).contains(&args.len()) {
        return Err("usage: inspect_neural_faces [--letterbox] classifier.onnx new-tensors.f32le face.png [face.png ...] (1..256 faces)".into());
    }
    // An explicit, new diagnostic artifact preserves older evidence.
    let mut tensor_file = File::create_new(&args[2])?;
    let started = Instant::now();
    let session = WinMlTileSession::load(Path::new(&args[1]))?;
    println!(
        "model_load_ms: {:.3}",
        started.elapsed().as_secs_f64() * 1000.0
    );
    let mut preparation_ms = 0.0;
    let mut evaluation_ms = 0.0;
    for (index, path) in args[3..].iter().enumerate() {
        let started = Instant::now();
        let image = RgbImage::read_png(Path::new(path))?;
        let prepared = if letterbox {
            prepare_tile_letterbox(&image)?
        } else {
            prepare_tile(&image)?
        };
        preparation_ms += started.elapsed().as_secs_f64() * 1000.0;
        for value in &prepared.pixels {
            tensor_file.write_all(&value.to_le_bytes())?;
        }
        println!(
            "glyph {index}: {:?}; removed_hints: {}",
            prepared.glyph_bounds, prepared.removed_corner_hints
        );
        let started = Instant::now();
        let logits = session.evaluate(&prepared.pixels)?;
        evaluation_ms += started.elapsed().as_secs_f64() * 1000.0;
        let values: Vec<_> = logits.iter().map(|v| format!("{v:.6}")).collect();
        println!("logits {index}: {}", values.join(","));
    }
    tensor_file.flush()?;
    println!("preparation_total_ms: {preparation_ms:.3}");
    println!("evaluation_total_ms: {evaluation_ms:.3}");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Native Windows ML diagnostic requires Windows 10 version 1809 or newer");
}
