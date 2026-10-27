//! Offline template recognition for calibrated tile and action regions.
//! Raw image pixels stay in memory; this crate performs no I/O.

use riichi_core::Tile;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RgbImage {
    width: u32,
    height: u32,
    /// Top-down RGB pixels, three bytes per pixel.
    rgb: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VisionError {
    InvalidImage,
    InvalidRegion,
    EmptyTemplates,
    InvalidStabilityCount,
    TemplateIo,
    InvalidTemplate,
}

impl fmt::Display for VisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for VisionError {}

impl RgbImage {
    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    pub fn pixels(&self) -> &[u8] {
        &self.rgb
    }
    pub fn pixels_mut(&mut self) -> &mut [u8] {
        &mut self.rgb
    }

    pub fn new(width: u32, height: u32, rgb: Vec<u8>) -> Result<Self, VisionError> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|value| value.checked_mul(3))
            .ok_or(VisionError::InvalidImage)?;
        if width == 0 || height == 0 || rgb.len() != expected {
            return Err(VisionError::InvalidImage);
        }
        Ok(Self { width, height, rgb })
    }

    pub fn from_bgra(width: u32, height: u32, bgra: &[u8]) -> Result<Self, VisionError> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|value| value.checked_mul(4))
            .ok_or(VisionError::InvalidImage)?;
        if width == 0 || height == 0 || bgra.len() != expected {
            return Err(VisionError::InvalidImage);
        }
        let mut rgb = Vec::with_capacity(expected / 4 * 3);
        for pixel in bgra.chunks_exact(4) {
            rgb.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        }
        Self::new(width, height, rgb)
    }

    pub fn crop(&self, rect: Rect) -> Result<Self, VisionError> {
        let right = rect
            .x
            .checked_add(rect.width)
            .ok_or(VisionError::InvalidRegion)?;
        let bottom = rect
            .y
            .checked_add(rect.height)
            .ok_or(VisionError::InvalidRegion)?;
        if rect.width == 0 || rect.height == 0 || right > self.width || bottom > self.height {
            return Err(VisionError::InvalidRegion);
        }
        let row_len = rect.width as usize * 3;
        let mut rgb = Vec::with_capacity(row_len * rect.height as usize);
        for y in rect.y..bottom {
            let start = ((y * self.width + rect.x) * 3) as usize;
            rgb.extend_from_slice(&self.rgb[start..start + row_len]);
        }
        Self::new(rect.width, rect.height, rgb)
    }

    pub fn read_png(path: &Path) -> Result<Self, VisionError> {
        let file = File::open(path).map_err(|_| VisionError::TemplateIo)?;
        let decoder = png::Decoder::new(BufReader::new(file));
        let mut reader = decoder
            .read_info()
            .map_err(|_| VisionError::InvalidTemplate)?;
        let info = reader.info();
        if info.width == 0 || info.height == 0 || info.width > 1024 || info.height > 1024 {
            return Err(VisionError::InvalidTemplate);
        }
        let buffer_size = reader
            .output_buffer_size()
            .ok_or(VisionError::InvalidTemplate)?;
        if buffer_size > 4 * 1024 * 1024 {
            return Err(VisionError::InvalidTemplate);
        }
        let mut bytes = vec![0; buffer_size];
        let output = reader
            .next_frame(&mut bytes)
            .map_err(|_| VisionError::InvalidTemplate)?;
        if output.bit_depth != png::BitDepth::Eight {
            return Err(VisionError::InvalidTemplate);
        }
        let raw = &bytes[..output.buffer_size()];
        let rgb = match output.color_type {
            png::ColorType::Rgb => raw.to_vec(),
            png::ColorType::Rgba => {
                let mut rgb = Vec::with_capacity(raw.len() / 4 * 3);
                for pixel in raw.chunks_exact(4) {
                    let alpha = pixel[3] as u16;
                    for channel in &pixel[..3] {
                        rgb.push(((*channel as u16 * alpha + 255 * (255 - alpha)) / 255) as u8);
                    }
                }
                rgb
            }
            _ => return Err(VisionError::InvalidTemplate),
        };
        Self::new(output.width, output.height, rgb)
    }

    pub fn write_png(&self, path: &Path) -> Result<(), VisionError> {
        if self.width > 1024 || self.height > 1024 {
            return Err(VisionError::InvalidTemplate);
        }
        let file = File::create_new(path).map_err(|_| VisionError::TemplateIo)?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), self.width, self.height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .and_then(|mut writer| writer.write_image_data(&self.rgb))
            .map_err(|_| VisionError::TemplateIo)
    }
}

/// Save tile faces from a user-labeled, calibrated 13- or 14-tile hand.
/// Existing local templates are never replaced. The caller owns the image and destination.
pub fn save_labeled_hand(
    image: &RgbImage,
    tiles: &[Tile],
    pack_dir: &Path,
) -> Result<usize, VisionError> {
    let slots = detect_hand_slots(image);
    if !matches!(tiles.len(), 13 | 14) || slots.len() != tiles.len() {
        return Err(VisionError::InvalidRegion);
    }
    let mut counts = [0u8; 34];
    for tile in tiles {
        let count = &mut counts[tile.index() as usize];
        *count += 1;
        if *count > 4 {
            return Err(VisionError::InvalidRegion);
        }
    }
    let tile_dir = pack_dir.join("tiles");
    std::fs::create_dir_all(&tile_dir).map_err(|_| VisionError::TemplateIo)?;
    let mut saved = 0;
    for (&tile, &slot) in tiles.iter().zip(&slots) {
        let name = tile_code(tile);
        let path = tile_dir.join(format!("{name}.png"));
        if path.exists() {
            continue;
        }
        let crop = image.crop(slot)?;
        crop.write_png(&path)?;
        saved += 1;
    }
    Ok(saved)
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

/// Detect pale, upright tile faces inside a tightly calibrated hand region.
/// This is a fast candidate detector; callers still have to classify and validate every slot.
pub fn detect_hand_slots(image: &RgbImage) -> Vec<Rect> {
    let band_start = image.height / 12;
    let band_end = (image.height / 4).max(band_start + 1).min(image.height);
    let min_column = (band_end - band_start).div_ceil(2);
    let min_width = (image.height / 4).max(3);
    let mut columns = Vec::with_capacity(image.width as usize);
    for x in 0..image.width {
        let mut bright = 0u32;
        for y in band_start..band_end {
            let i = ((y * image.width + x) * 3) as usize;
            if is_pale(&image.rgb[i..i + 3]) {
                bright += 1;
            }
        }
        columns.push(bright >= min_column);
    }
    let mut slots = Vec::new();
    let mut start = None;
    for x in 0..=image.width {
        let present = x < image.width && columns[x as usize];
        match (start, present) {
            (None, true) => start = Some(x),
            (Some(left), false) => {
                if x - left >= min_width {
                    let mut top = image.height;
                    let mut bottom = 0;
                    for y in 0..image.height {
                        for xx in left..x {
                            let i = ((y * image.width + xx) * 3) as usize;
                            if is_pale(&image.rgb[i..i + 3]) {
                                top = top.min(y);
                                bottom = bottom.max(y + 1);
                            }
                        }
                    }
                    if bottom > top {
                        slots.push(Rect {
                            x: left,
                            y: top,
                            width: x - left,
                            height: bottom - top,
                        });
                    }
                }
                start = None;
            }
            _ => {}
        }
    }
    slots
}

fn is_pale(pixel: &[u8]) -> bool {
    let min = *pixel.iter().min().unwrap_or(&0);
    let max = *pixel.iter().max().unwrap_or(&0);
    min >= 175 && max - min <= 70
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionLabel {
    None,
    Chi,
    Pon,
    Kan,
    Riichi,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchConfig {
    pub min_confidence: f32,
    pub min_margin: f32,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.78,
            min_margin: 0.05,
        }
    }
}

impl MatchConfig {
    fn accepts(self, score: f32, margin: f32) -> bool {
        self.min_confidence.is_finite()
            && self.min_margin.is_finite()
            && (0.0..=1.0).contains(&self.min_confidence)
            && (0.0..=1.0).contains(&self.min_margin)
            && score >= self.min_confidence
            && margin >= self.min_margin
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HandRead {
    pub tiles: Option<Vec<Tile>>,
    pub uncertain_slots: Vec<usize>,
    pub confidence: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActionRead {
    pub actions: Option<Vec<ActionLabel>>,
    pub uncertain_slots: Vec<usize>,
    pub confidence: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScannedActionRead {
    pub actions: Option<Vec<ActionLabel>>,
    pub regions: Vec<Rect>,
    pub confidence: f32,
}

#[derive(Clone, Debug)]
pub struct TemplateBank {
    tile_templates: Vec<(Tile, RgbImage)>,
    action_templates: Vec<(ActionLabel, RgbImage)>,
    tile_descriptors: Vec<TileDescriptor>,
}

impl TemplateBank {
    pub fn tile_templates(&self) -> &[(Tile, RgbImage)] {
        &self.tile_templates
    }

    pub fn action_templates(&self) -> &[(ActionLabel, RgbImage)] {
        &self.action_templates
    }

    pub fn new(
        tile_templates: Vec<(Tile, RgbImage)>,
        action_templates: Vec<(ActionLabel, RgbImage)>,
    ) -> Result<Self, VisionError> {
        if tile_templates.is_empty() || action_templates.is_empty() {
            return Err(VisionError::EmptyTemplates);
        }
        let tile_descriptors = tile_templates
            .iter()
            .map(|(_, image)| TileDescriptor::new(image))
            .collect();
        Ok(Self {
            tile_templates,
            action_templates,
            tile_descriptors,
        })
    }

    /// Load a complete local template pack. Files are never downloaded or uploaded.
    /// The pack must contain all 34 normal tiles, three red fives and five action states.
    pub fn load_dir(path: &Path) -> Result<Self, VisionError> {
        let mut tile_templates = Vec::new();
        for suit in ['m', 'p', 's'] {
            for number in 1..=9 {
                let code = format!("{number}{suit}");
                let tile = Tile::from_code(&code).map_err(|_| VisionError::InvalidTemplate)?;
                tile_templates.push((
                    tile,
                    RgbImage::read_png(&path.join("tiles").join(format!("{code}.png")))?,
                ));
            }
            let code = format!("0{suit}");
            let tile = Tile::from_code(&code).map_err(|_| VisionError::InvalidTemplate)?;
            tile_templates.push((
                tile,
                RgbImage::read_png(&path.join("tiles").join(format!("{code}.png")))?,
            ));
        }
        for number in 1..=7 {
            let code = format!("{number}z");
            let tile = Tile::from_code(&code).map_err(|_| VisionError::InvalidTemplate)?;
            tile_templates.push((
                tile,
                RgbImage::read_png(&path.join("tiles").join(format!("{code}.png")))?,
            ));
        }
        let actions = [
            (ActionLabel::None, "none"),
            (ActionLabel::Chi, "chi"),
            (ActionLabel::Pon, "pon"),
            (ActionLabel::Kan, "kan"),
            (ActionLabel::Riichi, "riichi"),
        ];
        let mut action_templates = Vec::new();
        for (label, name) in actions {
            action_templates.push((
                label,
                RgbImage::read_png(&path.join("actions").join(format!("{name}.png")))?,
            ));
        }
        Self::new(tile_templates, action_templates)
    }

    pub fn recognize_hand(
        &self,
        image: &RgbImage,
        slots: &[Rect],
        config: MatchConfig,
    ) -> Result<HandRead, VisionError> {
        let mut tiles = Vec::with_capacity(slots.len());
        let mut uncertain_slots = Vec::new();
        let mut confidence = 1.0f32;
        for (index, &rect) in slots.iter().enumerate() {
            let crop = image.crop(rect)?;
            let descriptor = TileDescriptor::new(&crop);
            let result = best_match(&self.tile_templates, |candidate, template| {
                if crop.width < 32
                    || crop.height < 32
                    || template.width < 32
                    || template.height < 32
                {
                    similarity(&crop, template)
                } else {
                    descriptor.similarity(&self.tile_descriptors[candidate])
                }
            });
            match result {
                Some((tile, score, margin)) if config.accepts(score, margin) => {
                    tiles.push(tile);
                    confidence = confidence.min(score);
                }
                Some((_, score, _)) => {
                    confidence = confidence.min(score);
                    uncertain_slots.push(index);
                }
                None => {
                    confidence = 0.0;
                    uncertain_slots.push(index);
                }
            }
        }
        Ok(HandRead {
            tiles: if uncertain_slots.is_empty() && !slots.is_empty() {
                Some(tiles)
            } else {
                None
            },
            uncertain_slots,
            confidence: if slots.is_empty() { 0.0 } else { confidence },
        })
    }

    pub fn recognize_actions(
        &self,
        image: &RgbImage,
        slots: &[Rect],
        config: MatchConfig,
    ) -> Result<ActionRead, VisionError> {
        let mut actions = Vec::new();
        let mut uncertain_slots = Vec::new();
        let mut confidence = 1.0f32;
        for (index, &rect) in slots.iter().enumerate() {
            let crop = image.crop(rect)?;
            let result = best_match(&self.action_templates, |_, template| {
                similarity(&crop, template)
            });
            match result {
                Some((label, score, margin)) if config.accepts(score, margin) => {
                    if label != ActionLabel::None {
                        actions.push(label);
                    }
                    confidence = confidence.min(score);
                }
                Some((_, score, _)) => {
                    confidence = confidence.min(score);
                    uncertain_slots.push(index);
                }
                None => {
                    confidence = 0.0;
                    uncertain_slots.push(index);
                }
            }
        }
        Ok(ActionRead {
            actions: if uncertain_slots.is_empty() {
                Some(actions)
            } else {
                None
            },
            uncertain_slots,
            confidence: if slots.is_empty() { 0.0 } else { confidence },
        })
    }

    /// Locate individual action buttons anywhere in a calibrated action region.
    /// An ambiguous overlap suppresses the whole action result.
    pub fn scan_actions(&self, image: &RgbImage, config: MatchConfig) -> ScannedActionRead {
        if !config.min_confidence.is_finite() || !config.min_margin.is_finite() {
            return ScannedActionRead {
                actions: None,
                regions: vec![],
                confidence: 0.0,
            };
        }
        let mut candidates = Vec::new();
        for (label, template) in &self.action_templates {
            if *label == ActionLabel::None {
                continue;
            }
            let mut best = (
                0.0f32,
                Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
            );
            let scales: &[u32] = if template.width < 32 || template.height < 32 {
                &[80, 100, 125]
            } else {
                &[50, 60, 70, 80, 100, 125]
            };
            for &scale in scales {
                let width = (template.width * scale / 100).max(1);
                let height = (template.height * scale / 100).max(1);
                if width > image.width || height > image.height {
                    continue;
                }
                let xs = scan_positions(image.width - width, (width / 24).max(1));
                let ys = scan_positions(image.height - height, (height / 10).max(1));
                for y in ys {
                    for &x in &xs {
                        let rect = Rect {
                            x,
                            y,
                            width,
                            height,
                        };
                        let score = sampled_similarity(image, rect, template);
                        if score > best.0 {
                            best = (score, rect);
                        }
                    }
                }
            }
            candidates.push((*label, best.0, best.1));
        }
        candidates.sort_by(|a, b| b.1.total_cmp(&a.1));
        let mut actions = Vec::new();
        let mut regions = Vec::new();
        let mut confidence = 1.0f32;
        for (label, score, rect) in &candidates {
            if *score < config.min_confidence {
                continue;
            }
            for (other_label, other_score, other_rect) in &candidates {
                if other_label == label {
                    continue;
                }
                if overlap(*rect, *other_rect) > 0.5
                    && (*score - *other_score).abs() < config.min_margin
                {
                    return ScannedActionRead {
                        actions: None,
                        regions: vec![],
                        confidence: *score,
                    };
                }
            }
            if regions
                .iter()
                .any(|selected| overlap(*rect, *selected) > 0.5)
            {
                continue;
            }
            actions.push(*label);
            regions.push(*rect);
            confidence = confidence.min(*score);
        }
        ScannedActionRead {
            actions: Some(actions),
            regions,
            confidence,
        }
    }
}

fn scan_positions(max: u32, step: u32) -> Vec<u32> {
    let mut positions: Vec<u32> = (0..=max).step_by(step as usize).collect();
    if positions.last().copied() != Some(max) {
        positions.push(max);
    }
    positions
}

fn sampled_similarity(image: &RgbImage, rect: Rect, template: &RgbImage) -> f32 {
    const GRID_WIDTH: u32 = 32;
    const GRID_HEIGHT: u32 = 20;
    let mut squared = 0.0f64;
    let mut weight_sum = 0.0f64;
    let corners = [
        0,
        (template.width - 1) * 3,
        (template.height - 1) * template.width * 3,
        (template.width * template.height - 1) * 3,
    ];
    let mut background = [0.0f64; 3];
    for corner in corners {
        for (channel, value) in background.iter_mut().enumerate() {
            *value += template.rgb[corner as usize + channel] as f64 / 4.0;
        }
    }
    for gy in 0..GRID_HEIGHT {
        let image_y =
            rect.y + ((gy * 2 + 1) * rect.height / (GRID_HEIGHT * 2)).min(rect.height - 1);
        let template_y =
            ((gy * 2 + 1) * template.height / (GRID_HEIGHT * 2)).min(template.height - 1);
        for gx in 0..GRID_WIDTH {
            let image_x =
                rect.x + ((gx * 2 + 1) * rect.width / (GRID_WIDTH * 2)).min(rect.width - 1);
            let template_x =
                ((gx * 2 + 1) * template.width / (GRID_WIDTH * 2)).min(template.width - 1);
            let a = ((image_y * image.width + image_x) * 3) as usize;
            let b = ((template_y * template.width + template_x) * 3) as usize;
            let foreground = (0..3)
                .map(|channel| (template.rgb[b + channel] as f64 - background[channel]).abs())
                .sum::<f64>()
                / (3.0 * 255.0);
            let weight = 1.0 + 20.0 * foreground * foreground;
            for channel in 0..3 {
                let delta = image.rgb[a + channel] as f64 - template.rgb[b + channel] as f64;
                squared += weight * delta * delta;
            }
            weight_sum += weight;
        }
    }
    (1.0 - (squared / (weight_sum * 3.0 * 255.0 * 255.0)).sqrt()) as f32
}

fn overlap(a: Rect, b: Rect) -> f32 {
    let left = a.x.max(b.x);
    let top = a.y.max(b.y);
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);
    if right <= left || bottom <= top {
        return 0.0;
    }
    let intersection = (right - left) as f32 * (bottom - top) as f32;
    let union = (a.width * a.height + b.width * b.height) as f32 - intersection;
    intersection / union
}

fn best_match<T: Copy + Eq>(
    templates: &[(T, RgbImage)],
    mut compare: impl FnMut(usize, &RgbImage) -> f32,
) -> Option<(T, f32, f32)> {
    let mut best: Option<(T, f32)> = None;
    let mut second = 0.0f32;
    for (index, (label, template)) in templates.iter().enumerate() {
        let score = compare(index, template);
        match best {
            None => best = Some((*label, score)),
            Some((current, current_score)) if score > current_score => {
                if current != *label {
                    second = second.max(current_score);
                }
                best = Some((*label, score));
            }
            Some((current, _)) if current != *label => second = second.max(score),
            _ => {}
        }
    }
    best.map(|(label, score)| (label, score, score - second))
}

// The upper glyph carries the rank of a suited tile. A shared lower glyph
// (notably the 萬 character) can otherwise overwhelm the smaller difference.
// Descriptors are cached for templates because they are compared every frame.
#[derive(Clone, Debug)]
struct TileDescriptor {
    full: Vec<[u8; 3]>,
    upper: Vec<[u8; 3]>,
}

impl TileDescriptor {
    fn new(image: &RgbImage) -> Self {
        Self {
            full: box_region(image, 0, 100, 0, 100),
            upper: box_region(image, 12, 83, 19, 48),
        }
    }

    fn similarity(&self, other: &Self) -> f32 {
        fn score(a: &[[u8; 3]], b: &[[u8; 3]]) -> f32 {
            let mut squared = 0.0f64;
            for (first, second) in a.iter().zip(b) {
                for channel in 0..3 {
                    let delta = first[channel] as f64 - second[channel] as f64;
                    squared += delta * delta;
                }
            }
            (1.0 - (squared / (a.len() as f64 * 3.0 * 255.0 * 255.0)).sqrt()) as f32
        }
        score(&self.full, &other.full) * 0.55 + score(&self.upper, &other.upper) * 0.45
    }
}

fn box_region(
    image: &RgbImage,
    left_percent: u32,
    right_percent: u32,
    top_percent: u32,
    bottom_percent: u32,
) -> Vec<[u8; 3]> {
    let mut result = Vec::with_capacity(36 * 56);
    for gy in 0..56u32 {
        for gx in 0..36u32 {
            result.push(box_pixel(
                image,
                left_percent,
                right_percent,
                top_percent,
                bottom_percent,
                gx,
                gy,
            ));
        }
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn box_pixel(
    image: &RgbImage,
    left_percent: u32,
    right_percent: u32,
    top_percent: u32,
    bottom_percent: u32,
    gx: u32,
    gy: u32,
) -> [u8; 3] {
    let left = image.width * left_percent / 100;
    let right = (image.width * right_percent / 100).max(left + 1);
    let top = image.height * top_percent / 100;
    let bottom = (image.height * bottom_percent / 100).max(top + 1);
    let x0 = left + (right - left) * gx / 36;
    let x1 = (left + (right - left) * (gx + 1) / 36).max(x0 + 1);
    let y0 = top + (bottom - top) * gy / 56;
    let y1 = (top + (bottom - top) * (gy + 1) / 56).max(y0 + 1);
    let mut rgb = [0u32; 3];
    let mut count = 0u32;
    for y in y0..y1.min(image.height) {
        for x in x0..x1.min(image.width) {
            let offset = ((y * image.width + x) * 3) as usize;
            for (c, channel) in rgb.iter_mut().enumerate() {
                *channel += image.rgb[offset + c] as u32;
            }
            count += 1;
        }
    }
    if count > 0 {
        for channel in &mut rgb {
            *channel /= count;
        }
    }
    rgb.map(|channel| channel as u8)
}

fn similarity(image: &RgbImage, template: &RgbImage) -> f32 {
    let mut squared = 0.0f64;
    let pixels = template.width as usize * template.height as usize;
    for y in 0..template.height {
        let sy = y as u64 * image.height as u64 / template.height as u64;
        for x in 0..template.width {
            let sx = x as u64 * image.width as u64 / template.width as u64;
            let a = ((sy * image.width as u64 + sx) * 3) as usize;
            let b = ((y * template.width + x) * 3) as usize;
            for c in 0..3 {
                let delta = image.rgb[a + c] as f64 - template.rgb[b + c] as f64;
                squared += delta * delta;
            }
        }
    }
    (1.0 - (squared / (pixels as f64 * 3.0 * 255.0 * 255.0)).sqrt()) as f32
}

#[derive(Clone, Debug, PartialEq)]
pub struct VisionSnapshot {
    pub hand: Vec<Tile>,
    pub actions: Vec<ActionLabel>,
    pub confidence: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FilterDecision {
    Waiting,
    Stable(VisionSnapshot),
    Uncertain,
}

pub struct StabilityFilter {
    required: usize,
    last_frame_id: Option<u64>,
    last: Option<VisionSnapshot>,
    streak: usize,
}

impl StabilityFilter {
    pub fn new(required: usize) -> Result<Self, VisionError> {
        if required == 0 {
            return Err(VisionError::InvalidStabilityCount);
        }
        Ok(Self {
            required,
            last_frame_id: None,
            last: None,
            streak: 0,
        })
    }

    pub fn observe(&mut self, frame_id: u64, candidate: Option<VisionSnapshot>) -> FilterDecision {
        if self.last_frame_id.is_some_and(|last| frame_id <= last) {
            return FilterDecision::Waiting;
        }
        self.last_frame_id = Some(frame_id);
        let Some(candidate) = candidate
            .filter(|item| item.confidence.is_finite() && (0.0..=1.0).contains(&item.confidence))
        else {
            self.last = None;
            self.streak = 0;
            return FilterDecision::Uncertain;
        };
        let same = self
            .last
            .as_ref()
            .is_some_and(|last| last.hand == candidate.hand && last.actions == candidate.actions);
        self.streak = if same {
            self.streak.saturating_add(1)
        } else {
            1
        };
        self.last = Some(candidate.clone());
        if self.streak >= self.required {
            FilterDecision::Stable(candidate)
        } else {
            FilterDecision::Waiting
        }
    }
}
