//! Local-only diagnostic: inspect a raw RGB frame without storing any output image.
//! Usage: cargo run -p vision-core --example inspect_raw -- path width height x y w h

use std::env;
use std::fs;
use vision_core::{Rect, RgbImage, detect_hand_slots};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 8 {
        return Err("usage: inspect_raw path width height x y w h".into());
    }
    let values: Vec<u32> = args[2..]
        .iter()
        .map(|s| s.parse())
        .collect::<Result<_, _>>()?;
    let bytes = fs::read(&args[1])?;
    let frame = RgbImage::new(values[0], values[1], bytes)?;
    let roi = frame.crop(Rect {
        x: values[2],
        y: values[3],
        width: values[4],
        height: values[5],
    })?;
    let slots = detect_hand_slots(&roi);
    println!("detected {} tile candidates", slots.len());
    for (index, slot) in slots.iter().enumerate() {
        println!(
            "{}: {},{},{}x{}",
            index + 1,
            slot.x,
            slot.y,
            slot.width,
            slot.height
        );
    }
    Ok(())
}
