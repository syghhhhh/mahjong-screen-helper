use std::{fs, path::PathBuf};
use vision_core::{
    Rect, RgbImage, VisionError,
    neural_input::{INPUT_FLOATS, PREPROCESSING_ID, prepare_tile},
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/synthetic/neural-input")
}

#[test]
fn matches_independent_pillow_oracles_for_geometry_color_and_corner_hints() {
    assert_eq!(PREPROCESSING_ID, "rgb-inverse-glyph-box-v2");
    let manifest = fs::read_to_string(root().join("cases.txt")).unwrap();
    let mut checked = 0;
    for line in manifest.lines().filter(|line| !line.starts_with('#')) {
        let fields: Vec<_> = line.split_whitespace().collect();
        assert_eq!(fields.len(), 6);
        let name = fields[0];
        let values: Vec<i32> = fields[1..].iter().map(|s| s.parse().unwrap()).collect();
        let image = RgbImage::read_png(&root().join(format!("{name}.png"))).unwrap();
        let original = image.clone();
        let result = prepare_tile(&image).unwrap();
        assert_eq!(image, original, "{name}: must not change caller pixels");
        let expected_bounds = (values[0] >= 0).then_some(Rect {
            x: values[0] as u32,
            y: values[1] as u32,
            width: values[2] as u32,
            height: values[3] as u32,
        });
        assert_eq!(result.glyph_bounds, expected_bounds, "{name}");
        assert_eq!(result.removed_corner_hints, values[4] as u32, "{name}");
        let expected = fs::read(root().join(format!("{name}.rgb"))).unwrap();
        assert_eq!(expected.len(), INPUT_FLOATS);
        assert_eq!(result.pixels.len(), INPUT_FLOATS);
        for y in 0..48 {
            for x in 0..32 {
                for channel in 0..3 {
                    let value = result.pixels[channel * 48 * 32 + y * 32 + x];
                    let reference = 1.0 - expected[(y * 32 + x) * 3 + channel] as f32 / 255.0;
                    assert_eq!(value, reference, "{name} at ({x},{y}) channel {channel}");
                    assert!(value.is_finite() && (0.0..=1.0).contains(&value));
                }
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 7);
}

#[test]
fn blank_faces_and_hint_only_faces_have_no_glyph_and_zero_tensor() {
    for name in ["blank", "hint-only"] {
        let image = RgbImage::read_png(&root().join(format!("{name}.png"))).unwrap();
        let result = prepare_tile(&image).unwrap();
        assert_eq!(result.glyph_bounds, None);
        assert!(result.pixels.iter().all(|value| *value == 0.0));
    }
}

#[test]
fn central_and_large_red_marks_survive_while_isolated_corner_hint_is_removed() {
    for (name, removed) in [("preserve-red", 0), ("hint-and-red-glyph", 1)] {
        let image = RgbImage::read_png(&root().join(format!("{name}.png"))).unwrap();
        let result = prepare_tile(&image).unwrap();
        assert_eq!(result.removed_corner_hints, removed);
        assert!(result.glyph_bounds.is_some());
        let plane = 32 * 48;
        let red_pixels = (0..plane)
            .filter(|&i| result.pixels[plane + i] - result.pixels[i] > 0.2)
            .count();
        assert!(
            red_pixels > 15,
            "{name}: red glyph information must survive"
        );
    }
}

#[test]
fn rejects_unbounded_or_too_small_inputs_before_working_copies() {
    for (width, height) in [(7, 32), (32, 7), (1025, 8), (8, 1025)] {
        let image = RgbImage::new(width, height, vec![255; (width * height * 3) as usize]).unwrap();
        assert!(matches!(
            prepare_tile(&image),
            Err(VisionError::InvalidImage)
        ));
    }
    for (width, height) in [(8, 8), (1024, 8), (8, 1024)] {
        let image = RgbImage::new(width, height, vec![255; (width * height * 3) as usize]).unwrap();
        assert!(prepare_tile(&image).is_ok());
    }
}
