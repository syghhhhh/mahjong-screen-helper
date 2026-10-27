//! Offline template recognition for calibrated tile and action regions.
//! Raw image pixels stay in memory; this crate performs no I/O.

use riichi_core::Tile;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
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
}

/// Detect pale, upright tile faces inside a tightly calibrated hand region.
/// This is a fast candidate detector; callers still have to classify and validate every slot.
pub fn detect_hand_slots(image: &RgbImage) -> Vec<Rect> {
    let min_column = (image.height / 3).max(2);
    let min_width = (image.height / 4).max(3);
    let mut columns = Vec::with_capacity(image.width as usize);
    for x in 0..image.width {
        let mut bright = 0u32;
        for y in 0..image.height {
            let i = ((y * image.width + x) * 3) as usize;
            let pixel = &image.rgb[i..i + 3];
            let min = *pixel.iter().min().unwrap_or(&0);
            let max = *pixel.iter().max().unwrap_or(&0);
            if min >= 175 && max - min <= 70 {
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
                            let pixel = &image.rgb[i..i + 3];
                            let min = *pixel.iter().min().unwrap_or(&0);
                            let max = *pixel.iter().max().unwrap_or(&0);
                            if min >= 175 && max - min <= 70 {
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

#[derive(Clone, Debug)]
pub struct TemplateBank {
    pub tile_templates: Vec<(Tile, RgbImage)>,
    pub action_templates: Vec<(ActionLabel, RgbImage)>,
}

impl TemplateBank {
    pub fn new(
        tile_templates: Vec<(Tile, RgbImage)>,
        action_templates: Vec<(ActionLabel, RgbImage)>,
    ) -> Result<Self, VisionError> {
        if tile_templates.is_empty() || action_templates.is_empty() {
            return Err(VisionError::EmptyTemplates);
        }
        Ok(Self {
            tile_templates,
            action_templates,
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
            let result = best_match(&crop, &self.tile_templates);
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
            let result = best_match(&crop, &self.action_templates);
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
}

fn best_match<T: Copy + Eq>(
    image: &RgbImage,
    templates: &[(T, RgbImage)],
) -> Option<(T, f32, f32)> {
    let mut best: Option<(T, f32)> = None;
    let mut second = 0.0f32;
    for (label, template) in templates {
        let score = similarity(image, template);
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
