//! Local-only public tile diagnostic. Raw images and templates stay outside Git.
use riichi_core::Tile;
use std::{env, fs, time::Instant};
use vision_core::{ActionLabel, MatchConfig, RgbImage, TemplateBank, detect_public_slots};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 5 {
        return Err("usage: inspect_board rgb-path width height tiles-dir".into());
    }
    let image = RgbImage::new(args[2].parse()?, args[3].parse()?, fs::read(&args[1])?)?;
    let mut templates = Vec::new();
    for entry in fs::read_dir(&args[4])? {
        let path = entry?.path();
        if path.extension().is_none_or(|extension| extension != "png") {
            continue;
        }
        let code = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("invalid filename")?;
        templates.push((Tile::from_code(code)?, RgbImage::read_png(&path)?));
    }
    templates.sort_by_key(|(tile, _)| (tile.index(), tile.is_red()));
    let bank = TemplateBank::new(
        templates,
        vec![(ActionLabel::None, RgbImage::new(1, 1, vec![0; 3])?)],
    )?;
    let started = Instant::now();
    let slots = detect_public_slots(&image);
    let read = bank.recognize_public_tiles(&image, MatchConfig::default());
    println!("detected public candidates: {}", slots.len());
    println!("accepted by default thresholds: {}", read.tiles.is_some());
    println!("minimum similarity: {:.3}", read.confidence);
    println!("offered: {:?}", read.offered);
    println!("board matching: {} ms", started.elapsed().as_millis());
    for rect in slots {
        let candidate = bank.recognize_public_regions(
            &image,
            &[rect],
            MatchConfig {
                min_confidence: 0.0,
                min_margin: 0.0,
            },
        )?;
        let tile = &candidate.tiles.ok_or("candidate matching failed")?[0];
        println!(
            "candidate: {},{},{},{} {} {:.3} {:?}",
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            code(tile.tile),
            tile.confidence,
            tile.normalization
        );
    }
    Ok(())
}

fn code(tile: Tile) -> String {
    let index = tile.index();
    let (rank, suit) = match index {
        0..=8 => (index + 1, 'm'),
        9..=17 => (index - 8, 'p'),
        18..=26 => (index - 17, 's'),
        _ => (index - 26, 'z'),
    };
    format!("{}{suit}", if tile.is_red() { 0 } else { rank })
}
