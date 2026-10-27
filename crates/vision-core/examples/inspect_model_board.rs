//! Local-only public-face/model diagnostic, including rejected rotations.
//! This is neither live acceptance nor an end-to-end performance measurement.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, fs, path::Path, time::Instant};
    use vision_core::{
        ActionLabel, MatchConfig, RgbImage, Rotation, TemplateBank,
        classifier::{CLASS_ORDER, ClassifierDecision, FaceObservation},
        detect_public_slots,
        neural_classifier::LocalTileClassifier,
    };
    let args: Vec<_> = env::args().collect();
    if args.len() != 5 {
        return Err("usage: inspect_model_board manifest.json scene.rgb width height".into());
    }
    let width: u32 = args[3].parse()?;
    let height: u32 = args[4].parse()?;
    if !(8..=16384).contains(&width) || !(8..=1024).contains(&height) {
        return Err("scene dimensions out of range".into());
    }
    if fs::metadata(&args[2])?.len() != width as u64 * height as u64 * 3 {
        return Err("raw RGB size mismatch".into());
    }
    let scene = RgbImage::new(width, height, fs::read(&args[2])?)?;
    let classifier = LocalTileClassifier::load(Path::new(&args[1]))?;
    // No action image is read or validated by this diagnostic.
    let bank = TemplateBank::actions_only(vec![(
        ActionLabel::None,
        RgbImage::new(8, 8, vec![30; 8 * 8 * 3])?,
    )])?;
    let code = |tile: riichi_core::Tile| {
        CLASS_ORDER
            .into_iter()
            .find(|s| riichi_core::Tile::from_code(s).ok() == Some(tile))
            .expect("valid tile")
    };
    let slots = detect_public_slots(&scene);
    let started = Instant::now();
    let read = bank.recognize_public_tiles_with_fallback(
        &scene,
        MatchConfig::default(),
        Some(&classifier),
    );
    println!("scope: offline public region; no capture, actions or UI");
    println!("candidates: {}", slots.len());
    println!("board_accepted: {}", read.tiles.is_some());
    println!("uncertain_regions: {}", read.uncertain_regions.len());
    println!("offered: {:?}", read.offered);
    println!(
        "board_processing_ms: {:.3}",
        started.elapsed().as_secs_f64() * 1000.0
    );
    for (index, rect) in slots.into_iter().enumerate() {
        let candidate = bank.recognize_public_regions_with_fallback(
            &scene,
            &[rect],
            MatchConfig::default(),
            Some(&classifier),
        )?;
        let label = candidate
            .tiles
            .as_ref()
            .and_then(|v| v.first())
            .map(|t| code(t.tile));
        println!(
            "candidate {index}: {},{},{},{} accepted={label:?}",
            rect.x, rect.y, rect.width, rect.height
        );
        for rotation in Rotation::ALL {
            let face = FaceObservation::in_public(&scene, rect, rotation)?;
            match classifier.classify(&face)? {
                ClassifierDecision::Accepted { tile, scores } => println!(
                    "rotation {index} {rotation:?}: {} probability={:.6} margin={:.6} entropy={:.6}",
                    code(tile),
                    scores.probability,
                    scores.margin,
                    scores.normalized_entropy,
                ),
                ClassifierDecision::Rejected { reason, scores } => {
                    println!("rotation {index} {rotation:?}: rejected={reason:?} scores={scores:?}")
                }
            }
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err("inspect_model_board needs Windows 10/11 native Windows ML".into())
}
