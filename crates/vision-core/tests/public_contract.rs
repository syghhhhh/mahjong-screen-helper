use riichi_core::Tile;
use std::fs::{self, File};
use std::io::BufWriter;
use vision_core::{
    ActionLabel, FilterDecision, MatchConfig, Rect, RgbImage, StabilityFilter, TemplateBank,
    VisionSnapshot, detect_hand_slots,
};

const W: u32 = 16;
const H: u32 = 20;

fn picture(seed: u8, red: bool) -> RgbImage {
    let mut rgb = vec![245u8; (W * H * 3) as usize];
    for y in 2..H - 2 {
        for x in 2..W - 2 {
            let stripe = (x / 2 + y / 3 + seed as u32).is_multiple_of(5);
            let glyph = (x + seed as u32 * y) % 7 < 3;
            let on = stripe ^ glyph;
            let i = ((y * W + x) * 3) as usize;
            if on {
                rgb[i] = if red { 210 } else { 25 };
                rgb[i + 1] = if red { 30 } else { 40 };
                rgb[i + 2] = 45;
            }
        }
    }
    RgbImage::new(W, H, rgb).unwrap()
}

fn bank() -> TemplateBank {
    let tiles = ["1m", "2m", "3m", "5m", "0m"]
        .into_iter()
        .enumerate()
        .map(|(i, code)| {
            (
                Tile::from_code(code).unwrap(),
                picture(i as u8 + 1, code == "0m"),
            )
        })
        .collect();
    let actions = [
        ActionLabel::None,
        ActionLabel::Chi,
        ActionLabel::Pon,
        ActionLabel::Kan,
        ActionLabel::Riichi,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, label)| (label, picture(i as u8 + 10, false)))
    .collect();
    TemplateBank::new(tiles, actions).unwrap()
}

fn paste(dst: &mut [u8], width: u32, src: &RgbImage, left: u32, top: u32, scale: u32) {
    for y in 0..src.height() * scale {
        for x in 0..src.width() * scale {
            let si = (((y / scale) * src.width() + x / scale) * 3) as usize;
            let di = (((top + y) * width + left + x) * 3) as usize;
            dst[di..di + 3].copy_from_slice(&src.pixels()[si..si + 3]);
        }
    }
}

fn hand_scene(scale: u32) -> (RgbImage, Vec<Rect>, Vec<Tile>) {
    let codes = include_str!("../../../tests/fixtures/synthetic/vision-hand.txt")
        .lines()
        .find(|line| !line.starts_with('#'))
        .unwrap();
    let hand: Vec<Tile> = codes
        .split_whitespace()
        .map(|code| Tile::from_code(code).unwrap())
        .collect();
    let width = (W + 2) * scale * hand.len() as u32 + 8 * scale;
    let height = (H + 4) * scale;
    let mut rgb = vec![38u8; (width * height * 3) as usize];
    let bank = bank();
    let mut slots = Vec::new();
    for (index, tile) in hand.iter().enumerate() {
        let x = (index as u32 * (W + 2) + 2 + if index == 13 { 8 } else { 0 }) * scale;
        let y = 2 * scale;
        let template = bank
            .tile_templates()
            .iter()
            .find(|(label, _)| label == tile)
            .unwrap();
        paste(&mut rgb, width, &template.1, x, y, scale);
        slots.push(Rect {
            x,
            y,
            width: W * scale,
            height: H * scale,
        });
    }
    (RgbImage::new(width, height, rgb).unwrap(), slots, hand)
}

#[test]
fn recognizes_fourteen_tiles_across_scaling_and_red_five() {
    let bank = bank();
    for scale in [1, 2, 3] {
        let (image, slots, expected) = hand_scene(scale);
        let read = bank
            .recognize_hand(&image, &slots, MatchConfig::default())
            .unwrap();
        assert_eq!(read.tiles, Some(expected));
        assert!(read.uncertain_slots.is_empty());
        assert!(read.confidence > 0.99);
    }
}

#[test]
fn finds_separate_hand_tile_bands_in_a_calibrated_region() {
    for scale in [1, 2, 3] {
        let (image, expected_slots, _) = hand_scene(scale);
        let detected = detect_hand_slots(&image);
        assert_eq!(detected, expected_slots);
    }
    let dark = RgbImage::new(32, 24, vec![38; 32 * 24 * 3]).unwrap();
    assert!(detect_hand_slots(&dark).is_empty());

    let (mut image, expected_slots, _) = hand_scene(3);
    for rect in &expected_slots {
        for y in rect.y + 10..rect.y + rect.height - 10 {
            for x in rect.x + 10..rect.x + rect.width - 10 {
                let i = ((y * image.width() + x) * 3) as usize;
                image.pixels_mut()[i..i + 3].fill(15);
            }
        }
    }
    assert_eq!(detect_hand_slots(&image).len(), 14);
}

#[test]
fn rejects_blank_or_ambiguous_tile_instead_of_guessing() {
    let bank = bank();
    let (mut image, slots, _) = hand_scene(1);
    let last = slots[13];
    for y in last.y..last.y + last.height {
        for x in last.x..last.x + last.width {
            let i = ((y * image.width() + x) * 3) as usize;
            image.pixels_mut()[i..i + 3].fill(245);
        }
    }
    let read = bank
        .recognize_hand(&image, &slots, MatchConfig::default())
        .unwrap();
    assert_eq!(read.tiles, None);
    assert_eq!(read.uncertain_slots, vec![13]);
    assert!(read.confidence < MatchConfig::default().min_confidence);
}

#[test]
fn recognizes_action_buttons_and_blank_slot() {
    let bank = bank();
    let labels = [
        ActionLabel::None,
        ActionLabel::Chi,
        ActionLabel::Pon,
        ActionLabel::Kan,
        ActionLabel::Riichi,
    ];
    let width = 5 * (W + 2);
    let mut rgb = vec![38; (width * H * 3) as usize];
    let mut slots = Vec::new();
    for (index, label) in labels.iter().enumerate() {
        let x = index as u32 * (W + 2);
        let art = &bank
            .action_templates()
            .iter()
            .find(|(candidate, _)| candidate == label)
            .unwrap()
            .1;
        paste(&mut rgb, width, art, x, 0, 1);
        slots.push(Rect {
            x,
            y: 0,
            width: W,
            height: H,
        });
    }
    let image = RgbImage::new(width, H, rgb).unwrap();
    let read = bank
        .recognize_actions(&image, &slots, MatchConfig::default())
        .unwrap();
    assert_eq!(read.actions, Some(labels[1..].to_vec()));
    assert!(read.uncertain_slots.is_empty());

    let mut obscured = image;
    for y in 0..H {
        for x in slots[2].x..slots[2].x + W {
            let i = ((y * width + x) * 3) as usize;
            obscured.pixels_mut()[i..i + 3].fill(245);
        }
    }
    let read = bank
        .recognize_actions(&obscured, &slots, MatchConfig::default())
        .unwrap();
    assert_eq!(read.actions, None);
    assert_eq!(read.uncertain_slots, vec![2]);
}

#[test]
fn scans_action_region_without_fixed_button_coordinates() {
    let bank = bank();
    let width = 96;
    let height = 42;
    let mut rgb = vec![38; (width * height * 3) as usize];
    let template = &bank
        .action_templates()
        .iter()
        .find(|(label, _)| *label == ActionLabel::Riichi)
        .unwrap()
        .1;
    paste(&mut rgb, width, template, 48, 10, 1);
    let image = RgbImage::new(width, height, rgb).unwrap();
    let scan = bank.scan_actions(&image, MatchConfig::default());
    assert_eq!(scan.actions, Some(vec![ActionLabel::Riichi]));
    assert!(scan.confidence > 0.9);
    assert_eq!(scan.regions.len(), 1);
    assert!((scan.regions[0].x as i32 - 48).abs() <= 2);

    let blank = RgbImage::new(width, height, vec![38; (width * height * 3) as usize]).unwrap();
    let scan = bank.scan_actions(&blank, MatchConfig::default());
    assert_eq!(scan.actions, Some(vec![]));
}

#[test]
fn requires_three_distinct_consecutive_stable_frames() {
    let hand = vec![Tile::from_code("1m").unwrap(); 13];
    let snapshot = VisionSnapshot {
        hand,
        actions: vec![ActionLabel::Riichi],
        confidence: 0.99,
    };
    let mut filter = StabilityFilter::new(3).unwrap();
    assert_eq!(
        filter.observe(1, Some(snapshot.clone())),
        FilterDecision::Waiting
    );
    assert_eq!(
        filter.observe(1, Some(snapshot.clone())),
        FilterDecision::Waiting
    );
    assert_eq!(
        filter.observe(2, Some(snapshot.clone())),
        FilterDecision::Waiting
    );
    assert_eq!(
        filter.observe(3, Some(snapshot.clone())),
        FilterDecision::Stable(snapshot.clone())
    );
    assert_eq!(filter.observe(4, None), FilterDecision::Uncertain);
    assert_eq!(
        filter.observe(5, Some(snapshot.clone())),
        FilterDecision::Waiting
    );
}

#[test]
fn validates_image_length_and_slot_bounds() {
    assert!(RgbImage::new(4, 4, vec![0; 47]).is_err());
    let image = RgbImage::new(4, 4, vec![0; 48]).unwrap();
    assert!(
        image
            .crop(Rect {
                x: 3,
                y: 3,
                width: 2,
                height: 2
            })
            .is_err()
    );
}

#[test]
fn converts_capture_bgra_without_swapping_red_and_blue() {
    let rgb = RgbImage::from_bgra(2, 1, &[7, 11, 231, 255, 223, 9, 3, 255]).unwrap();
    assert_eq!(rgb.pixels(), [231, 11, 7, 3, 9, 223]);
    assert!(RgbImage::from_bgra(2, 1, &[0; 7]).is_err());
}

fn write_png(path: &std::path::Path, image: &RgbImage) {
    let file = File::create(path).unwrap();
    let mut encoder = png::Encoder::new(BufWriter::new(file), image.width(), image.height());
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(image.pixels())
        .unwrap();
}

#[test]
fn loads_only_a_complete_local_template_pack() {
    let root = std::env::temp_dir().join(format!("mahjong-vision-pack-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("tiles")).unwrap();
    fs::create_dir_all(root.join("actions")).unwrap();
    assert!(TemplateBank::load_dir(&root).is_err());
    for suit in ['m', 'p', 's'] {
        for number in 0..=9 {
            let code = format!("{number}{suit}");
            write_png(
                &root.join("tiles").join(format!("{code}.png")),
                &picture(number as u8 + 1, number == 0),
            );
        }
    }
    for number in 1..=7 {
        write_png(
            &root.join("tiles").join(format!("{number}z.png")),
            &picture(number as u8 + 15, false),
        );
    }
    for (index, name) in ["none", "chi", "pon", "kan", "riichi"].iter().enumerate() {
        write_png(
            &root.join("actions").join(format!("{name}.png")),
            &picture(index as u8 + 25, false),
        );
    }
    let loaded = TemplateBank::load_dir(&root).unwrap();
    assert_eq!(loaded.tile_templates().len(), 37);
    assert_eq!(loaded.action_templates().len(), 5);
    assert_eq!(
        RgbImage::read_png(&root.join("tiles").join("0m.png")).unwrap(),
        picture(1, true)
    );
    fs::remove_file(root.join("actions").join("kan.png")).unwrap();
    assert!(TemplateBank::load_dir(&root).is_err());
    fs::remove_dir_all(root).unwrap();
}
