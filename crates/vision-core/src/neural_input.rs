//! Versioned RGB input contract for small local classifiers.
//!
//! This only prepares a tightly cropped, upright tile face. It does not detect
//! tiles, validate a model's class map, or establish recognition confidence.
//! Callers must match the preprocessing version to their model manifest. In particular,
//! a zero tensor is a blank face, not evidence that the tile is White Dragon.

use crate::{Rect, RgbImage, VisionError};

pub const PREPROCESSING_ID: &str = "rgb-inverse-glyph-box-v2";
pub const LETTERBOX_PREPROCESSING_ID: &str = "rgb-inverse-glyph-letterbox-v3";
pub const INPUT_WIDTH: usize = 32;
pub const INPUT_HEIGHT: usize = 48;
pub const INPUT_FLOATS: usize = 3 * INPUT_WIDTH * INPUT_HEIGHT;

#[derive(Clone, Debug)]
pub struct PreparedTile {
    /// Contiguous float32 CHW [3,48,32], values 1 - RGB/255.
    pub pixels: Vec<f32>,
    /// Glyph bounding box (including one pixel of padding) in caller pixels.
    /// None means no ink remained; a blank face needs separate validation.
    pub glyph_bounds: Option<Rect>,
    pub removed_corner_hints: u32,
}

/// Normalize a bounded RGB face without changing caller pixels or doing I/O.
/// PNG alpha, if present, must already have been composited onto white.
pub fn prepare_tile(image: &RgbImage) -> Result<PreparedTile, VisionError> {
    prepare(image, false)
}

/// Fit the glyph into 28x44 without stretching, centered on a white 32x48
/// canvas. This is a separate model contract; v2 weights must keep v2 inputs.
pub fn prepare_tile_letterbox(image: &RgbImage) -> Result<PreparedTile, VisionError> {
    prepare(image, true)
}

fn prepare(image: &RgbImage, letterbox: bool) -> Result<PreparedTile, VisionError> {
    let width = image.width() as usize;
    let height = image.height() as usize;
    if !(8..=1024).contains(&width) || !(8..=1024).contains(&height) {
        return Err(VisionError::InvalidImage);
    }
    let mut rgb = image.pixels().to_vec();
    let removed_corner_hints = remove_corner_hints(&mut rgb, width, height);
    let glyph_bounds = glyph_bounds(&rgb, width, height);
    let mut pixels = vec![0.0; INPUT_FLOATS];
    if let Some(bounds) = glyph_bounds {
        let (out_width, out_height) = if letterbox {
            letterbox_size(bounds.width as usize, bounds.height as usize)
        } else {
            (INPUT_WIDTH, INPUT_HEIGHT)
        };
        let resized = resize_glyph(&rgb, width, bounds, out_width, out_height);
        let offset_x = (INPUT_WIDTH - out_width) / 2;
        let offset_y = (INPUT_HEIGHT - out_height) / 2;
        for (index, pixel) in resized.chunks_exact(3).enumerate() {
            let destination =
                (offset_y + index / out_width) * INPUT_WIDTH + offset_x + index % out_width;
            for channel in 0..3 {
                pixels[channel * INPUT_WIDTH * INPUT_HEIGHT + destination] =
                    1.0 - pixel[channel] as f32 / 255.0;
            }
        }
    }
    Ok(PreparedTile {
        pixels,
        glyph_bounds,
        removed_corner_hints,
    })
}

fn letterbox_size(width: usize, height: usize) -> (usize, usize) {
    // Exact nearest/ties-to-even integer sizing, matching Python's Fraction
    // oracle without depending on floating-point boundary behavior.
    let round_ratio = |numerator: usize, denominator: usize| {
        let whole = numerator / denominator;
        let remainder = numerator % denominator;
        (whole
            + usize::from(
                remainder * 2 > denominator || (remainder * 2 == denominator && whole % 2 == 1),
            ))
        .max(1)
    };
    if width * 44 >= height * 28 {
        (28, round_ratio(height * 28, width))
    } else {
        (round_ratio(width * 44, height), 44)
    }
}

fn red(pixel: &[u8]) -> bool {
    let [r, g, b] = [pixel[0] as i16, pixel[1] as i16, pixel[2] as i16];
    r > g + 25 && r > b + 25 && g < 210 && b < 210
}

fn pale_median(rgb: &[u8]) -> [u8; 3] {
    let mut histograms = [[0usize; 256]; 3];
    let mut count = 0;
    for pixel in rgb.chunks_exact(3) {
        let low = *pixel.iter().min().expect("RGB pixel");
        let high = *pixel.iter().max().expect("RGB pixel");
        if low >= 175 && high - low <= 70 {
            for channel in 0..3 {
                histograms[channel][pixel[channel] as usize] += 1;
            }
            count += 1;
        }
    }
    if count == 0 {
        return [240; 3];
    }
    histograms.map(|histogram| {
        let mut below = 0;
        let mut pair = [0usize; 2];
        for (value, frequency) in histogram.into_iter().enumerate() {
            if below <= (count - 1) / 2 && below + frequency > (count - 1) / 2 {
                pair[0] = value;
            }
            if below <= count / 2 && below + frequency > count / 2 {
                pair[1] = value;
                break;
            }
            below += frequency;
        }
        ((pair[0] + pair[1]) / 2) as u8
    })
}

fn remove_corner_hints(rgb: &mut [u8], width: usize, height: usize) -> u32 {
    let background = pale_median(rgb);
    let mask: Vec<_> = rgb.chunks_exact(3).map(red).collect();
    let mut seen = vec![false; mask.len()];
    let mut todo = Vec::new();
    let mut removed = 0;
    for start in 0..mask.len() {
        if seen[start] || !mask[start] {
            continue;
        }
        todo.push(start);
        seen[start] = true;
        let (mut left, mut right) = (start % width, start % width);
        let (mut top, mut bottom) = (start / width, start / width);
        let mut area = 0;
        while let Some(index) = todo.pop() {
            let (x, y) = (index % width, index / width);
            left = left.min(x);
            right = right.max(x);
            top = top.min(y);
            bottom = bottom.max(y);
            area += 1;
            for neighbor in [
                (x > 0).then(|| index - 1),
                (x + 1 < width).then_some(index + 1),
                (y > 0).then(|| index - width),
                (y + 1 < height).then_some(index + width),
            ]
            .into_iter()
            .flatten()
            {
                if mask[neighbor] && !seen[neighbor] {
                    seen[neighbor] = true;
                    todo.push(neighbor);
                }
            }
        }
        // Integer ratios keep every boundary reproducible across platforms.
        // Only small, isolated red annotations in the far upper-right qualify.
        // Large red glyphs, central red fives, and left-side marks are retained.
        if (left + right) * 5 >= width * 8
            && bottom * 100 < height * 32
            && (right - left + 1) * 4 <= width
            && (bottom - top + 1) * 5 <= height
            && area * 100 <= width * height * 3
        {
            for y in top.saturating_sub(1)..(bottom + 2).min(height) {
                for x in left.saturating_sub(1)..(right + 2).min(width) {
                    let index = (y * width + x) * 3;
                    rgb[index..index + 3].copy_from_slice(&background);
                }
            }
            removed += 1;
        }
    }
    removed
}

fn glyph_bounds(rgb: &[u8], width: usize, height: usize) -> Option<Rect> {
    let (mx, my) = ((width / 20).max(1), (height / 20).max(1));
    let (mut left, mut right, mut top, mut bottom) = (width, 0, height, 0);
    for y in my..height - my {
        for x in mx..width - mx {
            let index = (y * width + x) * 3;
            let [r, g, b] = [
                rgb[index] as i16,
                rgb[index + 1] as i16,
                rgb[index + 2] as i16,
            ];
            let high = r.max(g).max(b);
            let low = r.min(g).min(b);
            // Orange tile edges do not define the glyph crop.
            if (high < 170 || (high - low > 70 && low < 160)) && !(r > g + 30 && g > b + 25) {
                left = left.min(x);
                right = right.max(x);
                top = top.min(y);
                bottom = bottom.max(y);
            }
        }
    }
    if left == width {
        return None;
    }
    let (left, top) = (left.saturating_sub(1), top.saturating_sub(1));
    Some(Rect {
        x: left as u32,
        y: top as u32,
        width: ((right + 2).min(width) - left) as u32,
        height: ((bottom + 2).min(height) - top) as u32,
    })
}

// Separable BOX sampling, with rounded 22-bit coefficients and an 8-bit
// intermediate. This explicit contract matches the saved Pillow RGB oracle;
// averaging an entire 2D rectangle with truncation would change model inputs.
fn box_taps(input: usize, output: usize) -> Vec<Vec<(usize, u64)>> {
    let scale = input as f64 / output as f64;
    let filter_scale = scale.max(1.0);
    let support = filter_scale * 0.5;
    (0..output)
        .map(|position| {
            let center = (position as f64 + 0.5) * scale;
            let low = ((center - support + 0.5) as usize).min(input - 1);
            let high = ((center + support + 0.5) as usize).min(input);
            let indices: Vec<_> = (low..high)
                .filter(|&index| {
                    let relative = (index as f64 - center + 0.5) / filter_scale;
                    relative > -0.5 && relative <= 0.5
                })
                .collect();
            let coefficient = ((1u64 << 22) as f64 / indices.len() as f64 + 0.5) as u64;
            indices.into_iter().map(|i| (i, coefficient)).collect()
        })
        .collect()
}

fn resize_glyph(
    rgb: &[u8],
    width: usize,
    bounds: Rect,
    output_width: usize,
    output_height: usize,
) -> Vec<u8> {
    let x_taps = box_taps(bounds.width as usize, output_width);
    let y_taps = box_taps(bounds.height as usize, output_height);
    let mut intermediate = vec![0u8; bounds.height as usize * output_width * 3];
    let rounded = |sum: u64| ((sum + (1 << 21)) >> 22).min(255) as u8;
    for y in 0..bounds.height as usize {
        for (x, taps) in x_taps.iter().enumerate() {
            for channel in 0..3 {
                let sum = taps
                    .iter()
                    .map(|&(source_x, coefficient)| {
                        let index =
                            ((y + bounds.y as usize) * width + source_x + bounds.x as usize) * 3;
                        rgb[index + channel] as u64 * coefficient
                    })
                    .sum();
                intermediate[(y * output_width + x) * 3 + channel] = rounded(sum);
            }
        }
    }
    let mut resized = vec![0u8; output_width * output_height * 3];
    for (y, taps) in y_taps.iter().enumerate() {
        for x in 0..output_width {
            for channel in 0..3 {
                let sum = taps
                    .iter()
                    .map(|&(source_y, coefficient)| {
                        intermediate[(source_y * output_width + x) * 3 + channel] as u64
                            * coefficient
                    })
                    .sum();
                resized[(y * output_width + x) * 3 + channel] = rounded(sum);
            }
        }
    }
    resized
}
