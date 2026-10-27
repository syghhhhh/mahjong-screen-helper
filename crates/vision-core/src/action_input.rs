//! Bounded, local-only input for action-word research. A prepared word is not
//! proof of an enabled game button; the caller must verify plate/context and
//! confidence before interpreting any model output as an available action.
use crate::{Rect, RgbImage, VisionError};

pub const PREPROCESSING_ID: &str = "action-bright-word-mask-v1";
pub const COMPLETE_COMPONENT_PREPROCESSING_ID: &str = "action-complete-word-mask-v2";
pub const INPUT_WIDTH: usize = 128;
pub const INPUT_HEIGHT: usize = 64;
pub const INPUT_FLOATS: usize = INPUT_WIDTH * INPUT_HEIGHT;

#[derive(Clone, Debug)]
pub struct PreparedActionWord {
    pub pixels: Vec<f32>,
    pub bounds: Rect,
    pub foreground_pixels: u32,
}

#[derive(Clone, Debug)]
pub enum ActionWordInput {
    /// No bright ink was found. This alone does not prove the absence of a button.
    Empty,
    /// Bright pixels exist, but no sufficiently supported word can be localized.
    Unlocalized,
    Prepared(PreparedActionWord),
}

pub fn prepare_action_word(image: &RgbImage) -> Result<ActionWordInput, VisionError> {
    prepare(image, false)
}

/// Complete every four-connected foreground component reached by the word
/// rectangle, including components reached by subsequent rectangle expansion.
/// This is a separate input contract; a v1 model must not silently receive v2.
/// Completion still supplies no evidence of an enabled button or its context.
pub fn prepare_complete_action_word(image: &RgbImage) -> Result<ActionWordInput, VisionError> {
    prepare(image, true)
}

#[derive(Clone, Copy)]
struct Bounds {
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
}

fn complete_components(labels: &[u32], components: &[Bounds], w: usize, seed: Bounds) -> Bounds {
    let mut bounds = seed;
    let mut included = vec![false; components.len()];
    let mut pending = vec![seed];
    while let Some(region) = pending.pop() {
        for y in region.top..region.bottom {
            for x in region.left..region.right {
                let label = labels[y * w + x];
                if label == 0 || included[label as usize - 1] {
                    continue;
                }
                let index = label as usize - 1;
                included[index] = true;
                let component = components[index];
                let old = bounds;
                bounds.left = bounds.left.min(component.left);
                bounds.top = bounds.top.min(component.top);
                bounds.right = bounds.right.max(component.right);
                bounds.bottom = bounds.bottom.max(component.bottom);
                // Scan only newly added, disjoint strips. Every pixel is visited
                // at most once, even for a chain of tiny boundary components.
                for strip in [
                    Bounds {
                        bottom: old.top,
                        ..bounds
                    },
                    Bounds {
                        top: old.bottom,
                        ..bounds
                    },
                    Bounds {
                        top: old.top,
                        bottom: old.bottom,
                        right: old.left,
                        ..bounds
                    },
                    Bounds {
                        top: old.top,
                        bottom: old.bottom,
                        left: old.right,
                        ..bounds
                    },
                ] {
                    if strip.left < strip.right && strip.top < strip.bottom {
                        pending.push(strip);
                    }
                }
            }
        }
    }
    bounds
}

fn prepare(image: &RgbImage, complete: bool) -> Result<ActionWordInput, VisionError> {
    let w = image.width() as usize;
    let h = image.height() as usize;
    if !(16..=1024).contains(&w) || !(16..=1024).contains(&h) {
        return Err(VisionError::InvalidImage);
    }
    let mask: Vec<bool> = image
        .pixels()
        .chunks_exact(3)
        .map(|p| {
            let high = *p.iter().max().unwrap();
            let low = *p.iter().min().unwrap();
            high >= 180 && low >= 60 && p.iter().map(|&v| u16::from(v)).sum::<u16>() >= 420
        })
        .collect();
    if !mask.iter().any(|&v| v) {
        return Ok(ActionWordInput::Empty);
    }
    let mut labels = vec![0u32; mask.len()];
    let mut components = Vec::new();
    let mut stack = Vec::new();
    let (mut left, mut top, mut right, mut bottom) = (w, h, 0, 0);
    for start in 0..mask.len() {
        if !mask[start] || labels[start] != 0 {
            continue;
        }
        stack.push(start);
        let label = components.len() as u32 + 1;
        labels[start] = label;
        let (mut x0, mut y0, mut x1, mut y1) = (start % w, start / w, start % w, start / w);
        let mut area = 0;
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
            area += 1;
            for neighbor in [
                (x > 0).then(|| i - 1),
                (x + 1 < w).then_some(i + 1),
                (y > 0).then(|| i - w),
                (y + 1 < h).then_some(i + w),
            ]
            .into_iter()
            .flatten()
            {
                if mask[neighbor] && labels[neighbor] == 0 {
                    labels[neighbor] = label;
                    stack.push(neighbor);
                }
            }
        }
        if area * 1000 >= w * h * 3 && (y1 - y0 + 1) * 8 >= h && (x1 - x0 + 1) * 32 >= w {
            left = left.min(x0);
            right = right.max(x1 + 1);
            top = top.min(y0);
            bottom = bottom.max(y1 + 1);
        }
        if complete {
            components.push(Bounds {
                left: x0,
                top: y0,
                right: x1 + 1,
                bottom: y1 + 1,
            });
        }
    }
    if right <= left || bottom <= top {
        return Ok(ActionWordInput::Unlocalized);
    }
    if complete {
        let bounds = complete_components(
            &labels,
            &components,
            w,
            Bounds {
                left,
                top,
                right,
                bottom,
            },
        );
        (left, top, right, bottom) = (bounds.left, bounds.top, bounds.right, bounds.bottom);
    }
    let width = right - left;
    let height = bottom - top;
    let round_ratio = |n: usize, d: usize| {
        let q = n / d;
        let r = n % d;
        (q + usize::from(r * 2 > d || (r * 2 == d && q % 2 == 1))).max(1)
    };
    let (out_w, out_h) = if width * 56 >= height * 120 {
        (120, round_ratio(height * 120, width))
    } else {
        (round_ratio(width * 56, height), 56)
    };
    let (ox, oy) = ((INPUT_WIDTH - out_w) / 2, (INPUT_HEIGHT - out_h) / 2);
    let mut pixels = vec![0.0; INPUT_FLOATS];
    // Pillow NEAREST advances f64 coordinates from the first pixel center.
    // Replacing this with an exact integer formula selects different pixels
    // at some boundaries (e.g. 64 -> 56), changing learned-model inputs.
    let step_x = width as f64 / out_w as f64;
    let step_y = height as f64 / out_h as f64;
    let mut source_x = step_x * 0.5;
    let xs: Vec<_> = (0..out_w)
        .map(|_| {
            let x = left + (source_x as usize).min(width - 1);
            source_x += step_x;
            x
        })
        .collect();
    let mut source_y = step_y * 0.5;
    for y in 0..out_h {
        let sy = top + (source_y as usize).min(height - 1);
        for (x, &sx) in xs.iter().enumerate() {
            pixels[(oy + y) * INPUT_WIDTH + ox + x] = f32::from(mask[sy * w + sx]);
        }
        source_y += step_y;
    }
    let foreground_pixels = (top..bottom)
        .map(|y| (left..right).filter(|&x| mask[y * w + x]).count() as u32)
        .sum();
    Ok(ActionWordInput::Prepared(PreparedActionWord {
        pixels,
        bounds: Rect {
            x: left as u32,
            y: top as u32,
            width: width as u32,
            height: height as u32,
        },
        foreground_pixels,
    }))
}
