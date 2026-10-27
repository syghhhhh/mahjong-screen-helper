use std::{fs, path::PathBuf};
use vision_core::{
    Rect, RgbImage, VisionError,
    neural_input::{
        INPUT_FLOATS, LETTERBOX_PREPROCESSING_ID, prepare_tile, prepare_tile_letterbox,
    },
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/synthetic/neural-letterbox")
        .join(name)
}

#[test]
fn letterbox_matches_independent_oracles_and_retains_original_observation_metadata() {
    assert_eq!(LETTERBOX_PREPROCESSING_ID, "rgb-inverse-glyph-letterbox-v3");
    let cases = fs::read_to_string(fixture("cases.txt")).unwrap();
    let mut checked = 0;
    for line in cases.lines().filter(|line| !line.starts_with('#')) {
        let fields: Vec<_> = line.split_whitespace().collect();
        let name = fields[0];
        let values: Vec<i32> = fields[1..].iter().map(|s| s.parse().unwrap()).collect();
        let image = RgbImage::read_png(&fixture(&format!("{name}.png"))).unwrap();
        let original = image.clone();
        let actual = prepare_tile_letterbox(&image).unwrap();
        let old = prepare_tile(&image).unwrap();
        assert_eq!(image, original, "{name}: caller pixels changed");
        let bounds = (values[0] >= 0).then_some(Rect {
            x: values[0] as u32,
            y: values[1] as u32,
            width: values[2] as u32,
            height: values[3] as u32,
        });
        assert_eq!(actual.glyph_bounds, bounds, "{name}");
        assert_eq!(actual.glyph_bounds, old.glyph_bounds, "{name}");
        assert_eq!(actual.removed_corner_hints, values[4] as u32, "{name}");
        assert_eq!(
            actual.removed_corner_hints, old.removed_corner_hints,
            "{name}"
        );
        let bytes = fs::read(fixture(&format!("{name}.rgb"))).unwrap();
        assert_eq!(bytes.len(), INPUT_FLOATS);
        assert_eq!(actual.pixels.len(), INPUT_FLOATS);
        for channel in 0..3 {
            for y in 0..48 {
                for x in 0..32 {
                    let value = actual.pixels[channel * 1536 + y * 32 + x];
                    let expected = 1.0 - bytes[(y * 32 + x) * 3 + channel] as f32 / 255.0;
                    assert_eq!(value, expected, "{name}: ({x},{y}) channel {channel}");
                    assert!(value.is_finite() && (0.0..=1.0).contains(&value));
                    if !(2..30).contains(&x) || !(2..46).contains(&y) {
                        assert_eq!(value, 0.0, "{name}: required white margin");
                    }
                }
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 14);
}

#[test]
fn narrow_glyph_is_not_stretched_to_the_width_of_a_wide_glyph() {
    let narrow = RgbImage::read_png(&fixture("narrow.png")).unwrap();
    let actual = prepare_tile_letterbox(&narrow).unwrap();
    let old = prepare_tile(&narrow).unwrap();
    let ink_width = |pixels: &[f32]| {
        (0..32)
            .filter(|&x| (0..48).any(|y| pixels[y * 32 + x] > 0.3))
            .count()
    };
    assert!(ink_width(&actual.pixels) <= 3);
    assert!(ink_width(&old.pixels) > 12);
    for name in ["blank.png", "hint-only.png"] {
        let image = RgbImage::read_png(&fixture(name)).unwrap();
        let blank = prepare_tile_letterbox(&image).unwrap();
        assert!(blank.glyph_bounds.is_none());
        assert!(blank.pixels.iter().all(|&v| v == 0.0));
    }
}

#[test]
fn letterbox_rejects_the_same_unbounded_input_dimensions_as_v2() {
    for (w, h) in [(7, 32), (32, 7), (1025, 8), (8, 1025)] {
        let image = RgbImage::new(w, h, vec![255; (w * h * 3) as usize]).unwrap();
        assert!(matches!(
            prepare_tile_letterbox(&image),
            Err(VisionError::InvalidImage)
        ));
    }
}
