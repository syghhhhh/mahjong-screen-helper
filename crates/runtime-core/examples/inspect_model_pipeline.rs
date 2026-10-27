//! Local model -> frame stability -> state -> recommendations diagnostic.
//! This does not capture a window, validate action buttons, or measure UI latency.
#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use runtime_core::{PipelineOutput, PublicImages, RuntimeEngine};
    use std::{env, fs, path::Path, time::Instant};
    use vision_core::{
        ActionLabel, MatchConfig, RgbImage, TemplateBank, classifier::CLASS_ORDER,
        neural_classifier::LocalTileClassifier,
    };
    let args: Vec<_> = env::args().collect();
    if args.len() != 5 && args.len() != 11 {
        return Err("usage: inspect_model_pipeline manifest.json hand.rgb width height [river.png own.png left.png across.png right.png dora.png]".into());
    }
    let width: u32 = args[3].parse()?;
    let height: u32 = args[4].parse()?;
    if !(8..=16384).contains(&width) || !(8..=1024).contains(&height) {
        return Err("hand dimensions out of range".into());
    }
    if fs::metadata(&args[2])?.len() != width as u64 * height as u64 * 3 {
        return Err("raw RGB size mismatch".into());
    }
    let hand = RgbImage::new(width, height, fs::read(&args[2])?)?;
    let public_images = args[5..]
        .iter()
        .map(|p| RgbImage::read_png(Path::new(p)))
        .collect::<Result<Vec<_>, _>>()?;
    let started = Instant::now();
    let classifier = LocalTileClassifier::load(Path::new(&args[1]))?;
    println!(
        "model_load_ms: {:.3}",
        started.elapsed().as_secs_f64() * 1000.0
    );
    // Actions are absent from these diagnostic inputs, so this original placeholder
    // is never matched. The actual app still requires all five action templates.
    let bank = TemplateBank::actions_only(vec![(
        ActionLabel::None,
        RgbImage::new(8, 8, vec![30; 8 * 8 * 3])?,
    )])?;
    let mut engine = RuntimeEngine::new(bank, vec![], MatchConfig::default(), 3)?;
    engine.set_tile_classifier(Some(Box::new(classifier)));
    println!(
        "scope: {}; action images absent",
        if public_images.is_empty() {
            "hand-only"
        } else {
            "hand+public"
        }
    );
    let code = |tile: riichi_core::Tile| -> &'static str {
        CLASS_ORDER
            .into_iter()
            .find(|s| riichi_core::Tile::from_code(s).ok() == Some(tile))
            .expect("valid tile")
    };
    for frame in 1..=3 {
        let started = Instant::now();
        let output = if public_images.is_empty() {
            engine.process(frame, &hand, None)
        } else {
            let public = PublicImages {
                river: &public_images[0],
                own_melds: &public_images[1],
                opponent_melds: [&public_images[2], &public_images[3], &public_images[4]],
                dora: &public_images[5],
            };
            engine.process_with_public(frame, &hand, None, &public)
        };
        println!(
            "frame_processing_ms {frame}: {:.3}",
            started.elapsed().as_secs_f64() * 1000.0
        );
        match output {
            PipelineOutput::Ready {
                hand,
                confidence,
                phase,
                discards,
                ..
            } => {
                println!(
                    "ready {frame}: {phase:?}; confidence={confidence:.6}; hand={}",
                    hand.into_iter().map(code).collect::<Vec<_>>().join(" ")
                );
                for (index, r) in discards.iter().enumerate() {
                    println!(
                        "discard {}: {} shanten={} ukeire={}",
                        index + 1,
                        code(r.discard),
                        r.shanten,
                        r.ukeire
                    );
                }
            }
            other => println!("frame {frame}: {other:?}"),
        }
    }
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Native model pipeline requires Windows 10 version 1809 or newer");
}
