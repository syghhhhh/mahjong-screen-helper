use super::{
    MatchConfig, Rect, RgbImage, TemplateBank, TileDescriptor, VisionError, best_match, is_pale,
};
use crate::classifier::FaceClassifier;
use riichi_core::Tile;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    Upright,
    Clockwise,
    HalfTurn,
    CounterClockwise,
}

impl Rotation {
    pub const ALL: [Self; 4] = [
        Self::Upright,
        Self::Clockwise,
        Self::HalfTurn,
        Self::CounterClockwise,
    ];
}

impl RgbImage {
    /// Rotate a local crop without changing its colors or resampling pixels.
    pub fn rotated(&self, rotation: Rotation) -> Self {
        let sideways = matches!(rotation, Rotation::Clockwise | Rotation::CounterClockwise);
        let (width, height) = if sideways {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        };
        let mut rgb = vec![0; self.rgb.len()];
        for y in 0..self.height {
            for x in 0..self.width {
                let (target_x, target_y) = match rotation {
                    Rotation::Upright => (x, y),
                    Rotation::Clockwise => (self.height - 1 - y, x),
                    Rotation::HalfTurn => (self.width - 1 - x, self.height - 1 - y),
                    Rotation::CounterClockwise => (y, self.width - 1 - x),
                };
                let source = ((y * self.width + x) * 3) as usize;
                let target = ((target_y * width + target_x) * 3) as usize;
                rgb[target..target + 3].copy_from_slice(&self.rgb[source..source + 3]);
            }
        }
        Self { width, height, rgb }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelativeSeat {
    SelfSeat,
    Right,
    Across,
    Left,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DetectedPublicTile {
    pub tile: Tile,
    pub rect: Rect,
    /// Rotation applied to the screen crop to match an upright tile template.
    pub normalization: Rotation,
    pub confidence: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OfferedTile {
    pub tile: Tile,
    pub source: RelativeSeat,
    pub rect: Rect,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PublicTileRead {
    pub tiles: Option<Vec<DetectedPublicTile>>,
    pub uncertain_regions: Vec<Rect>,
    pub confidence: f32,
    /// Only a unique green border around an accepted tile can identify an offer.
    pub offered: Option<OfferedTile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicSnapshot {
    /// Visible tiles outside the concealed hand, including dora indicators.
    pub visible_tiles: Vec<Tile>,
    pub own_meld_tiles: Vec<Tile>,
    pub dora_indicators: Vec<Tile>,
    pub offered: Option<(Tile, RelativeSeat)>,
}

/// Locate separated pale tile faces in a tightly calibrated public-information region.
/// This does not prove that every tile on a real table has been detected.
pub fn detect_public_slots(image: &RgbImage) -> Vec<Rect> {
    let count = image.rgb.len() / 3;
    let mut visited = vec![false; count];
    let mut slots = Vec::new();
    let width = image.width as usize;
    let height = image.height as usize;
    let mut pending = Vec::new();
    for start in 0..count {
        if visited[start] || !is_pale(&image.rgb[start * 3..start * 3 + 3]) {
            continue;
        }
        visited[start] = true;
        pending.push(start);
        let (mut left, mut right) = (start % width, start % width);
        let (mut top, mut bottom) = (start / width, start / width);
        let mut area = 0usize;
        while let Some(index) = pending.pop() {
            let x = index % width;
            let y = index / width;
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
                if !visited[neighbor] && is_pale(&image.rgb[neighbor * 3..neighbor * 3 + 3]) {
                    visited[neighbor] = true;
                    pending.push(neighbor);
                }
            }
        }
        let w = right - left + 1;
        let h = bottom - top + 1;
        if (14..=256).contains(&w)
            && (14..=256).contains(&h)
            && w * 5 >= h * 2
            && h * 5 >= w * 2
            && area * 100 >= w * h * 45
        {
            slots.push(Rect {
                x: left as u32,
                y: top as u32,
                width: w as u32,
                height: h as u32,
            });
        }
    }
    slots.sort_by_key(|rect| (rect.y, rect.x));
    slots
}

impl TemplateBank {
    /// Classify separated public tiles, comparing all rotations per class before
    /// computing the class margin. Repeated rotations of one label are not rivals.
    pub fn recognize_public_tiles(&self, image: &RgbImage, config: MatchConfig) -> PublicTileRead {
        self.recognize_public_tiles_with_fallback(image, config, None)
    }

    pub fn recognize_public_tiles_with_fallback(
        &self,
        image: &RgbImage,
        config: MatchConfig,
        classifier: Option<&dyn FaceClassifier>,
    ) -> PublicTileRead {
        let slots = detect_public_slots(image);
        if slots.is_empty()
            && image
                .rgb
                .chunks_exact(3)
                .filter(|pixel| is_pale(pixel))
                .count()
                * 5
                > image.rgb.len() / 3
        {
            return PublicTileRead {
                tiles: None,
                uncertain_regions: vec![Rect {
                    x: 0,
                    y: 0,
                    width: image.width,
                    height: image.height,
                }],
                confidence: 0.0,
                offered: None,
            };
        }
        self.recognize_public_regions_with_fallback(image, &slots, config, classifier)
            .expect("detected regions stay inside image")
    }

    /// Classify explicitly calibrated public tile regions with the same rotation
    /// and class-margin checks as automatic public candidate detection.
    pub fn recognize_public_regions(
        &self,
        image: &RgbImage,
        slots: &[Rect],
        config: MatchConfig,
    ) -> Result<PublicTileRead, VisionError> {
        self.recognize_public_regions_with_fallback(image, slots, config, None)
    }

    pub fn recognize_public_regions_with_fallback(
        &self,
        image: &RgbImage,
        slots: &[Rect],
        config: MatchConfig,
        classifier: Option<&dyn FaceClassifier>,
    ) -> Result<PublicTileRead, VisionError> {
        if !config.accepts(1.0, 1.0) {
            return Ok(PublicTileRead {
                tiles: None,
                uncertain_regions: slots.to_vec(),
                confidence: 0.0,
                offered: None,
            });
        }
        let mut tiles = Vec::new();
        let mut uncertain_regions = Vec::new();
        let mut confidence = 1.0f32;
        for (index, first) in slots.iter().enumerate() {
            for second in &slots[index + 1..] {
                let width = (first
                    .x
                    .saturating_add(first.width)
                    .min(second.x.saturating_add(second.width)))
                .saturating_sub(first.x.max(second.x));
                let height = (first
                    .y
                    .saturating_add(first.height)
                    .min(second.y.saturating_add(second.height)))
                .saturating_sub(first.y.max(second.y));
                if width as u64 * height as u64 * 5
                    > (first.width as u64 * first.height as u64)
                        .min(second.width as u64 * second.height as u64)
                {
                    uncertain_regions.extend([*first, *second]);
                }
            }
        }
        for &rect in slots {
            let crop = image.crop(rect)?;
            let rotations: Vec<_> = Rotation::ALL
                .into_iter()
                .map(|rotation| {
                    let rotated = crop.rotated(rotation);
                    let descriptor = glyph_descriptor(&rotated);
                    (rotation, rotated, descriptor)
                })
                .collect();
            let score = |candidate: usize,
                         _template: &RgbImage,
                         _rotated: &RgbImage,
                         descriptor: &TileDescriptor| {
                descriptor.similarity(&self.public_descriptors[candidate])
            };
            let result = best_match(&self.tile_templates, |candidate, template| {
                rotations
                    .iter()
                    .map(|(_, rotated, descriptor)| score(candidate, template, rotated, descriptor))
                    .fold(0.0f32, f32::max)
            });
            let template_tile = match result {
                Some((tile, best, margin)) if config.accepts(best, margin) => {
                    let mut normalization = (Rotation::Upright, -1.0f32);
                    for (candidate, (label, template)) in self.tile_templates.iter().enumerate() {
                        if *label != tile {
                            continue;
                        }
                        for (rotation, rotated, descriptor) in &rotations {
                            let similarity = score(candidate, template, rotated, descriptor);
                            if similarity > normalization.1 {
                                normalization = (*rotation, similarity);
                            }
                        }
                    }
                    Some(DetectedPublicTile {
                        tile,
                        rect,
                        normalization: normalization.0,
                        confidence: best,
                    })
                }
                _ => None,
            };
            let found = template_tile.or_else(|| {
                classifier
                    .and_then(|model| crate::hybrid::neural_public_tile(image, rect, model, config))
            });
            if let Some(tile) = found {
                confidence = confidence.min(tile.confidence);
                tiles.push(tile);
            } else {
                confidence = confidence.min(result.map_or(0.0, |(_, score, _)| score));
                uncertain_regions.push(rect);
            }
        }
        let mut counts = [0u8; 34];
        for tile in &tiles {
            let count = &mut counts[tile.tile.index() as usize];
            *count = count.saturating_add(1);
        }
        if counts.iter().any(|count| *count > 4) {
            uncertain_regions.extend(tiles.iter().map(|tile| tile.rect));
        }
        let offered = if uncertain_regions.is_empty() {
            unique_offer(image, &tiles)
        } else {
            None
        };
        Ok(PublicTileRead {
            tiles: uncertain_regions.is_empty().then_some(tiles),
            uncertain_regions,
            confidence,
            offered,
        })
    }
}

// Public tiles have different front-face margins and sizes from the concealed
// hand. Normalize ink bounds independently while keeping orange tile bases out.
pub(super) fn glyph_descriptor(image: &RgbImage) -> TileDescriptor {
    let margin_x = (image.width / 14).max(1);
    let margin_y = (image.height / 14).max(1);
    let (mut left, mut top, mut right, mut bottom) = (image.width, image.height, 0, 0);
    for y in margin_y..image.height.saturating_sub(margin_y) {
        for x in margin_x..image.width.saturating_sub(margin_x) {
            let at = ((y * image.width + x) * 3) as usize;
            let (r, g, b) = (
                image.rgb[at] as u16,
                image.rgb[at + 1] as u16,
                image.rgb[at + 2] as u16,
            );
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let orange_base = r > g + 30 && g > b + 25;
            if !orange_base && (max < 165 || max - min > 70 && min < 145) {
                left = left.min(x);
                top = top.min(y);
                right = right.max(x + 1);
                bottom = bottom.max(y + 1);
            }
        }
    }
    if right <= left || bottom <= top {
        return TileDescriptor::new(&RgbImage::new(1, 1, vec![240; 3]).expect("white pixel"));
    }
    let left = left.saturating_sub(1);
    let top = top.saturating_sub(1);
    let right = (right + 1).min(image.width);
    let bottom = (bottom + 1).min(image.height);
    TileDescriptor::new(
        &image
            .crop(Rect {
                x: left,
                y: top,
                width: right - left,
                height: bottom - top,
            })
            .expect("ink bounds stay inside crop"),
    )
}

fn unique_offer(image: &RgbImage, tiles: &[DetectedPublicTile]) -> Option<OfferedTile> {
    let mut highlighted = tiles
        .iter()
        .filter(|tile| green_border(image, tile.rect) >= 0.12);
    let tile = highlighted.next()?;
    if highlighted.next().is_some() {
        return None;
    }
    let x = (tile.rect.x as f32 + tile.rect.width as f32 / 2.0) / image.width as f32;
    let y = (tile.rect.y as f32 + tile.rect.height as f32 / 2.0) / image.height as f32;
    let mut edges = [
        (x, RelativeSeat::Left),
        (1.0 - x, RelativeSeat::Right),
        (y, RelativeSeat::Across),
        (1.0 - y, RelativeSeat::SelfSeat),
    ];
    edges.sort_by(|a, b| a.0.total_cmp(&b.0));
    if edges[1].0 - edges[0].0 < 0.05 || edges[0].1 == RelativeSeat::SelfSeat {
        return None;
    }
    Some(OfferedTile {
        tile: tile.tile,
        source: edges[0].1,
        rect: tile.rect,
    })
}

fn green_border(image: &RgbImage, rect: Rect) -> f32 {
    let padding = (rect.width.min(rect.height) / 10).clamp(2, 6);
    let left = rect.x.saturating_sub(padding);
    let top = rect.y.saturating_sub(padding);
    let right = (rect.x + rect.width + padding).min(image.width);
    let bottom = (rect.y + rect.height + padding).min(image.height);
    let (mut green, mut samples) = (0u32, 0u32);
    for y in top..bottom {
        for x in left..right {
            if x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height {
                continue;
            }
            let at = ((y * image.width + x) * 3) as usize;
            let (r, g, b) = (
                image.rgb[at] as u16,
                image.rgb[at + 1] as u16,
                image.rgb[at + 2] as u16,
            );
            samples += 1;
            if g >= 140 && g >= r + 20 && g >= b + 10 {
                green += 1;
            }
        }
    }
    if samples == 0 {
        0.0
    } else {
        green as f32 / samples as f32
    }
}
