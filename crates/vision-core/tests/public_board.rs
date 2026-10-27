use riichi_core::Tile;
use vision_core::{
    ActionLabel, MatchConfig, RelativeSeat, RgbImage, Rotation, TemplateBank, detect_public_slots,
};

fn art(seed: u32) -> RgbImage {
    let (width, height) = (18, 26);
    let mut rgb = vec![240; width * height * 3];
    for y in 3..height - 3 {
        for x in 3..width - 3 {
            if ((x * (seed as usize + 3) + y * 7 + y * x) % 29) < 13 {
                let at = (y * width + x) * 3;
                rgb[at..at + 3].copy_from_slice(if seed == 2 {
                    &[210, 20, 30]
                } else {
                    &[20, 30, 40]
                });
            }
        }
    }
    RgbImage::new(width as u32, height as u32, rgb).unwrap()
}

fn bank() -> TemplateBank {
    let tiles = ["1m", "2p", "0s", "1z"]
        .into_iter()
        .enumerate()
        .map(|(i, code)| (Tile::from_code(code).unwrap(), art(i as u32)))
        .collect();
    TemplateBank::new(tiles, vec![(ActionLabel::None, art(9))]).unwrap()
}

fn paste(canvas: &mut RgbImage, source: &RgbImage, left: u32, top: u32, highlighted: bool) {
    for y in top.saturating_sub(2)..top + source.height() + 2 {
        for x in left.saturating_sub(2)..left + source.width() + 2 {
            let at = ((y * canvas.width() + x) * 3) as usize;
            if x >= left && x < left + source.width() && y >= top && y < top + source.height() {
                let from = (((y - top) * source.width() + x - left) * 3) as usize;
                canvas.pixels_mut()[at..at + 3].copy_from_slice(&source.pixels()[from..from + 3]);
            } else if highlighted {
                canvas.pixels_mut()[at..at + 3].copy_from_slice(&[70, 225, 100]);
            }
        }
    }
}

#[test]
fn rotation_preserves_each_pixel_and_swaps_sideways_dimensions() {
    let input = RgbImage::new(2, 3, (1..=6).flat_map(|value| [value; 3]).collect()).unwrap();
    let rotated = input.rotated(Rotation::Clockwise);
    assert_eq!((rotated.width(), rotated.height()), (3, 2));
    assert_eq!(
        rotated
            .pixels()
            .chunks_exact(3)
            .map(|pixel| pixel[0])
            .collect::<Vec<_>>(),
        vec![5, 3, 1, 6, 4, 2]
    );
    assert_eq!(rotated.rotated(Rotation::CounterClockwise), input);
    assert_eq!(
        input
            .rotated(Rotation::HalfTurn)
            .rotated(Rotation::HalfTurn),
        input
    );
}

#[test]
fn reads_scattered_rotated_tiles_and_only_one_highlighted_opponent_offer() {
    let bank = bank();
    let mut image = RgbImage::new(180, 160, vec![38; 180 * 160 * 3]).unwrap();
    let positions = [(8, 70), (78, 8), (140, 70), (78, 120)];
    for (index, rotation) in Rotation::ALL.into_iter().enumerate() {
        paste(
            &mut image,
            &art(index as u32).rotated(rotation),
            positions[index].0,
            positions[index].1,
            index == 2,
        );
    }
    assert_eq!(detect_public_slots(&image).len(), 4);
    let read = bank.recognize_public_tiles(&image, MatchConfig::default());
    let tiles = read.tiles.unwrap();
    assert_eq!(tiles.len(), 4);
    assert!(read.uncertain_regions.is_empty());
    for (index, tile) in bank.tile_templates().iter().enumerate() {
        assert!(tiles.iter().any(|found| found.tile == tile.0
            && found.rect.x == positions[index].0
            && found.rect.y == positions[index].1));
    }
    let offer = read.offered.unwrap();
    assert_eq!(offer.tile, Tile::from_code("0s").unwrap());
    assert_eq!(offer.source, RelativeSeat::Right);

    paste(&mut image, &art(0), positions[0].0, positions[0].1, true);
    assert!(
        bank.recognize_public_tiles(&image, MatchConfig::default())
            .offered
            .is_none()
    );
}

#[test]
fn pale_green_offer_glow_does_not_join_or_expand_adjacent_tile_faces() {
    let mut image = RgbImage::new(80, 100, vec![38; 80 * 100 * 3]).unwrap();
    paste(&mut image, &art(0), 20, 10, true);
    paste(&mut image, &art(1), 20, 38, false);
    // A bright, washed-out glow passes the neutral pale-face threshold. It
    // touches both faces, unlike the saturated green used by older fixtures.
    for pixel in image.pixels_mut().chunks_exact_mut(3) {
        if pixel == [70, 225, 100] {
            pixel.copy_from_slice(&[190, 235, 205]);
        }
    }
    for rotation in Rotation::ALL {
        let scene = image.rotated(rotation);
        let slots = detect_public_slots(&scene);
        assert_eq!(slots.len(), 2, "{rotation:?}: {slots:?}");
        assert!(slots.iter().all(|r| r.width * r.height == 18 * 26));
        let read = bank().recognize_public_tiles(&scene, MatchConfig::default());
        let tiles = read
            .tiles
            .expect("separated original faces should classify");
        assert_eq!(tiles.len(), 2);
        for code in ["1m", "2p"] {
            assert!(
                tiles
                    .iter()
                    .any(|t| t.tile == Tile::from_code(code).unwrap())
            );
        }
        // The shared glow is ambiguous as an offer and must stay rejected.
        assert!(read.offered.is_none());
    }

    let mut isolated = RgbImage::new(180, 160, vec![38; 180 * 160 * 3]).unwrap();
    paste(&mut isolated, &art(0), 140, 70, true);
    for pixel in isolated.pixels_mut().chunks_exact_mut(3) {
        if pixel == [70, 225, 100] {
            pixel.copy_from_slice(&[190, 235, 205]);
        }
    }
    let read = bank().recognize_public_tiles(&isolated, MatchConfig::default());
    assert_eq!(read.tiles.unwrap().len(), 1);
    assert_eq!(read.offered.unwrap().tile, Tile::from_code("1m").unwrap());
}

#[test]
fn a_green_washed_face_cannot_become_an_accepted_empty_region() {
    let mut image = RgbImage::new(100, 100, vec![38; 100 * 100 * 3]).unwrap();
    let rect = vision_core::Rect {
        x: 50,
        y: 30,
        width: 18,
        height: 26,
    };
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            let at = ((y * image.width() + x) * 3) as usize;
            image.pixels_mut()[at..at + 3].copy_from_slice(&[190, 235, 205]);
        }
    }
    // The color is excluded from face matching but remains possible-tile
    // evidence. Its small area must not bypass the old large-pale-area guard.
    assert!(detect_public_slots(&image).is_empty());
    let read = bank().recognize_public_tiles(&image, MatchConfig::default());
    assert!(read.tiles.is_none());
    assert!(read.uncertain_regions.contains(&rect));
    assert!(read.offered.is_none());
    paste(&mut image, &art(0), 10, 30, false);
    assert_eq!(detect_public_slots(&image).len(), 1);
    let read = bank().recognize_public_tiles(&image, MatchConfig::default());
    assert!(read.tiles.is_none());
    assert!(read.uncertain_regions.contains(&rect));
    assert!(read.offered.is_none());
}

#[test]
fn rejects_unknown_tiles_and_impossible_public_counts() {
    let bank = bank();
    let mut image = RgbImage::new(160, 50, vec![38; 160 * 50 * 3]).unwrap();
    for index in 0..5 {
        paste(&mut image, &art(0), 4 + index * 28, 8, false);
    }
    let read = bank.recognize_public_tiles(&image, MatchConfig::default());
    assert!(read.tiles.is_none());
    assert!(!read.uncertain_regions.is_empty());

    let mut image = RgbImage::new(50, 50, vec![38; 50 * 50 * 3]).unwrap();
    let blank = RgbImage::new(18, 26, vec![240; 18 * 26 * 3]).unwrap();
    paste(&mut image, &blank, 8, 8, false);
    assert!(
        bank.recognize_public_tiles(&image, MatchConfig::default())
            .tiles
            .is_none()
    );
    let empty = RgbImage::new(50, 50, vec![38; 50 * 50 * 3]).unwrap();
    assert_eq!(
        bank.recognize_public_tiles(&empty, MatchConfig::default())
            .tiles,
        Some(vec![])
    );
    let covered = RgbImage::new(400, 300, vec![240; 400 * 300 * 3]).unwrap();
    assert!(
        bank.recognize_public_tiles(&covered, MatchConfig::default())
            .tiles
            .is_none()
    );
    assert!(
        bank.recognize_public_tiles(
            &empty,
            MatchConfig {
                min_confidence: f32::NAN,
                min_margin: 0.0
            }
        )
        .tiles
        .is_none()
    );
}

#[test]
fn own_river_highlight_does_not_become_an_opponent_offer() {
    let bank = bank();
    let mut image = RgbImage::new(160, 160, vec![38; 160 * 160 * 3]).unwrap();
    paste(&mut image, &art(0), 70, 125, true);
    let read = bank.recognize_public_tiles(&image, MatchConfig::default());
    assert_eq!(read.tiles.unwrap().len(), 1);
    assert!(read.offered.is_none());
}

#[test]
fn public_glyph_matching_tolerates_different_face_margins() {
    let bank = bank();
    for index in 0..4 {
        let mut padded = RgbImage::new(36, 48, vec![240; 36 * 48 * 3]).unwrap();
        paste(&mut padded, &art(index), 9, 9, false);
        let mut scene = RgbImage::new(80, 80, vec![38; 80 * 80 * 3]).unwrap();
        paste(
            &mut scene,
            &padded.rotated(Rotation::Clockwise),
            10,
            10,
            false,
        );
        let read = bank.recognize_public_tiles(&scene, MatchConfig::default());
        assert_eq!(
            read.tiles
                .map(|tiles| tiles.into_iter().map(|tile| tile.tile).collect::<Vec<_>>()),
            Some(vec![bank.tile_templates()[index as usize].0])
        );
    }
}
