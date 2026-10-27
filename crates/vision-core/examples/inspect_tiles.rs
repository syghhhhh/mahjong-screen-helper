//! Local-only diagnostic for a cropped RGB hand frame and a tile template folder.
//! Usage: cargo run -p vision-core --example inspect_tiles -- path width height tiles-dir

use riichi_core::Tile;
use std::{env, fs};
use vision_core::{ActionLabel, MatchConfig, RgbImage, TemplateBank, detect_hand_slots};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        return Err("usage: inspect_tiles rgb-path width height tiles-dir".into());
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
        let code = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("invalid template filename")?;
        templates.push((Tile::from_code(code)?, RgbImage::read_png(&path)?));
    }
    templates.sort_by_key(|(tile, _)| (tile.index(), tile.is_red()));
    let placeholder = RgbImage::new(1, 1, vec![0, 0, 0])?;
    let bank = TemplateBank::new(templates, vec![(ActionLabel::None, placeholder)])?;
    let slots = detect_hand_slots(&image);
    let diagnostic = bank.recognize_hand(
        &image,
        &slots,
        MatchConfig {
            min_confidence: 0.0,
            min_margin: 0.0,
        },
    )?;
    let accepted = bank.recognize_hand(&image, &slots, MatchConfig::default())?;
    println!("detected {} tile candidates", slots.len());
    if let Some(tiles) = diagnostic.tiles {
        let codes: Vec<_> = tiles.iter().map(|tile| tile_code(*tile)).collect();
        println!("best labels: {}", codes.join(" "));
    }
    println!("minimum similarity: {:.3}", diagnostic.confidence);
    println!(
        "accepted by default thresholds: {}",
        accepted.tiles.is_some()
    );
    println!("uncertain slots: {:?}", accepted.uncertain_slots);
    Ok(())
}

fn tile_code(tile: Tile) -> String {
    let index = tile.index();
    let (number, suit) = if index < 9 {
        (index + 1, 'm')
    } else if index < 18 {
        (index - 8, 'p')
    } else if index < 27 {
        (index - 17, 's')
    } else {
        (index - 26, 'z')
    };
    format!("{}{}", if tile.is_red() { 0 } else { number }, suit)
}
