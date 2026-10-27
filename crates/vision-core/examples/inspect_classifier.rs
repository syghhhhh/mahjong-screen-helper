//! Explicit manifest-bound classifier diagnostic; never activates the overlay.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, fs, path::Path, time::Instant};
    use vision_core::{
        RgbImage,
        classifier::{CLASS_ORDER, ClassifierDecision, FaceObservation},
        detect_hand_slots,
        neural_classifier::LocalTileClassifier,
    };
    let args: Vec<_> = env::args().collect();
    if args.len() < 4 || args.len() > 259 {
        return Err("usage: inspect_classifier manifest.json --faces face.png [...] | --hand image.rgb width height".into());
    }
    let started = Instant::now();
    let classifier = LocalTileClassifier::load(Path::new(&args[1]))?;
    println!(
        "model_load_ms: {:.3}",
        started.elapsed().as_secs_f64() * 1000.0
    );
    let started = Instant::now();
    let faces = match args[2].as_str() {
        "--faces" => args[3..]
            .iter()
            .map(|p| FaceObservation::isolated(&RgbImage::read_png(Path::new(p))?))
            .collect::<Result<Vec<_>, _>>()?,
        "--hand" if args.len() == 6 => {
            let width: u32 = args[4].parse()?;
            let height: u32 = args[5].parse()?;
            if !(8..=16384).contains(&width) || !(8..=1024).contains(&height) {
                return Err("hand dimensions out of range".into());
            }
            let expected = width as u64 * height as u64 * 3;
            if fs::metadata(&args[3])?.len() != expected {
                return Err("raw RGB size does not match dimensions".into());
            }
            let image = RgbImage::new(width, height, fs::read(&args[3])?)?;
            let slots = detect_hand_slots(&image);
            if slots.is_empty() || slots.len() > 14 {
                return Err("hand must have 1..14 detected candidates".into());
            }
            println!("detected_slots: {}", slots.len());
            slots
                .iter()
                .map(|&r| FaceObservation::in_hand(&image, r))
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => return Err("expected --faces or --hand mode".into()),
    };
    println!(
        "preparation_total_ms: {:.3}",
        started.elapsed().as_secs_f64() * 1000.0
    );
    let started = Instant::now();
    for (index, face) in faces.iter().enumerate() {
        println!(
            "face_context {index}: glyph={} enclosed_blank={}",
            face.glyph_bounds().is_some(),
            face.blank_face_verified()
        );
        match classifier.classify(face)? {
            ClassifierDecision::Accepted { tile, scores } => {
                let code = CLASS_ORDER
                    .iter()
                    .find(|code| riichi_core::Tile::from_code(code).ok() == Some(tile))
                    .expect("fixed class map");
                println!(
                    "accepted {index}: {code} {:.9} {:.9} {:.9}",
                    scores.probability, scores.margin, scores.normalized_entropy
                );
            }
            ClassifierDecision::Rejected { reason, scores } => {
                println!("rejected {index}: {reason:?} {scores:?}")
            }
        }
    }
    println!(
        "evaluation_total_ms: {:.3}",
        started.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Classifier diagnostic requires Windows 10 version 1809 or newer");
}
