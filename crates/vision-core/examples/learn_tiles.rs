//! Build a local tile-template folder from a user-labeled cropped RGB hand frame.
//! Usage: cargo run -p vision-core --example learn_tiles -- rgb-path width height pack-dir 1m 2m ...

use riichi_core::Tile;
use std::{env, fs, path::Path};
use vision_core::{RgbImage, save_labeled_hand};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 18 || args.len() > 19 {
        return Err("usage: learn_tiles rgb-path width height pack-dir 13-or-14 tile codes".into());
    }
    let width: u32 = args[2].parse()?;
    let height: u32 = args[3].parse()?;
    let image = RgbImage::new(width, height, fs::read(&args[1])?)?;
    let labels = args[5..]
        .iter()
        .map(|code| Tile::from_code(code))
        .collect::<Result<Vec<_>, _>>()?;
    let saved = save_labeled_hand(&image, &labels, Path::new(&args[4]))?;
    println!("saved {saved} new local tile templates");
    Ok(())
}
