//! Local-only diagnostic for a cropped RGB action-button region.
//! Usage: cargo run -p vision-core --example inspect_actions -- path width height actions-dir

use riichi_core::Tile;
use std::{env, fs, time::Instant};
use vision_core::{ActionLabel, MatchConfig, RgbImage, TemplateBank};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        return Err("usage: inspect_actions rgb-path width height actions-dir".into());
    }
    let width: u32 = args[2].parse()?;
    let height: u32 = args[3].parse()?;
    let image = RgbImage::new(width, height, fs::read(&args[1])?)?;
    let mut templates = Vec::new();
    for entry in fs::read_dir(&args[4])? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "png") {
            continue;
        }
        let label = match path.file_stem().and_then(|stem| stem.to_str()) {
            Some("none" | "pass") => ActionLabel::None,
            Some("chi") => ActionLabel::Chi,
            Some("pon") => ActionLabel::Pon,
            Some("kan") => ActionLabel::Kan,
            Some("riichi") => ActionLabel::Riichi,
            _ => continue,
        };
        templates.push((label, RgbImage::read_png(&path)?));
    }
    let placeholder = RgbImage::new(1, 1, vec![0, 0, 0])?;
    let bank = TemplateBank::new(vec![(Tile::from_code("1m")?, placeholder)], templates)?;
    let started = Instant::now();
    let scan = bank.scan_actions(&image, MatchConfig::default());
    println!("actions: {:?}", scan.actions);
    println!("regions: {:?}", scan.regions);
    println!("confidence: {:.3}", scan.confidence);
    println!("matching: {} ms", started.elapsed().as_millis());
    Ok(())
}
