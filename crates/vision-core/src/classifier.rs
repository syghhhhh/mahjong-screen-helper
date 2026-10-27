//! Model identity, conservative score decisions, and contextual blank handling.
//! A declared class map and softmax score do not prove a model's accuracy.
use crate::{
    Rect, RgbImage, Rotation, VisionError, detect_hand_slots, detect_public_slots,
    neural_input::{LETTERBOX_PREPROCESSING_ID, PREPROCESSING_ID, PreparedTile, prepare_tile},
};
use riichi_core::Tile;
use serde::Deserialize;

pub const CLASS_ORDER: [&str; 37] = [
    "1m", "2m", "3m", "4m", "5m", "6m", "7m", "8m", "9m", "1p", "2p", "3p", "4p", "5p", "6p", "7p",
    "8p", "9p", "1s", "2s", "3s", "4s", "5s", "6s", "7s", "8s", "9s", "1z", "2z", "3z", "4z", "5z",
    "6z", "7z", "0m", "0p", "0s",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidManifest;
impl std::fmt::Display for InvalidManifest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Invalid model manifest")
    }
}
impl std::error::Error for InvalidManifest {}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Thresholds {
    min_probability: f64,
    min_margin: f64,
    max_normalized_entropy: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    schema_version: String,
    model_id: String,
    model_file: String,
    model_sha256: String,
    preprocessing: String,
    class_order: Vec<String>,
    license: String,
    thresholds: Thresholds,
}

#[derive(Clone, Debug)]
pub struct ModelManifest(RawManifest);
impl ModelManifest {
    pub fn parse(bytes: &[u8]) -> Result<Self, InvalidManifest> {
        if bytes.len() > 16 * 1024 {
            return Err(InvalidManifest);
        }
        let raw: RawManifest = serde_json::from_slice(bytes).map_err(|_| InvalidManifest)?;
        let safe_name = |s: &str| {
            !s.is_empty()
                && s.len() <= 64
                && s.as_bytes()[0].is_ascii_alphanumeric()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                && !s.contains("..")
        };
        let stem = raw
            .model_file
            .split('.')
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'));
        if raw.schema_version != "1.0"
            || !matches!(
                raw.preprocessing.as_str(),
                PREPROCESSING_ID | LETTERBOX_PREPROCESSING_ID
            )
            || raw.class_order != CLASS_ORDER
            || !safe_name(&raw.model_id)
            || !safe_name(&raw.model_file)
            || reserved
            || !raw.model_file.ends_with(".onnx")
            || raw.model_sha256.len() != 64
            || !raw
                .model_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || raw.license.trim().is_empty()
            || raw.license.len() > 120
            || !raw.license.bytes().all(|b| (32..=126).contains(&b))
            || !(0.5..=1.0).contains(&raw.thresholds.min_probability)
            || !(0.0..=1.0).contains(&raw.thresholds.min_margin)
            || raw.thresholds.min_margin == 0.0
            || !(0.0..=1.0).contains(&raw.thresholds.max_normalized_entropy)
            || raw.thresholds.max_normalized_entropy == 0.0
        {
            return Err(InvalidManifest);
        }
        Ok(Self(raw))
    }
    pub fn model_id(&self) -> &str {
        &self.0.model_id
    }
    pub fn model_file(&self) -> &str {
        &self.0.model_file
    }
    pub fn model_sha256(&self) -> &str {
        &self.0.model_sha256
    }
    pub fn license(&self) -> &str {
        &self.0.license
    }
    pub fn preprocessing(&self) -> &str {
        &self.0.preprocessing
    }

    /// Scores are model probabilities, not calibrated real-world accuracy.
    pub fn decide(&self, face: &FaceObservation, logits: &[f32; 37]) -> ClassifierDecision {
        let reject = |reason, scores| ClassifierDecision::Rejected { reason, scores };
        if logits.iter().any(|v| !v.is_finite()) {
            return reject(RejectReason::InvalidLogits, None);
        }
        let maximum = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max) as f64;
        let values = logits.map(|v| (v as f64 - maximum).exp());
        let total: f64 = values.iter().sum();
        let probabilities = values.map(|v| v / total);
        let mut order: Vec<_> = (0..37).collect();
        order.sort_by(|&a, &b| probabilities[b].total_cmp(&probabilities[a]));
        let first = order[0];
        let scores = ScoreSummary {
            probability: probabilities[first],
            margin: probabilities[first] - probabilities[order[1]],
            normalized_entropy: -probabilities
                .iter()
                .filter(|&&p| p > 0.0)
                .map(|p| p * p.ln())
                .sum::<f64>()
                / 37f64.ln(),
        };
        let limits = &self.0.thresholds;
        if scores.probability < limits.min_probability {
            return reject(RejectReason::LowProbability, Some(scores));
        }
        if scores.margin < limits.min_margin {
            return reject(RejectReason::Ambiguous, Some(scores));
        }
        if scores.normalized_entropy > limits.max_normalized_entropy {
            return reject(RejectReason::HighEntropy, Some(scores));
        }
        if face.input.glyph_bounds.is_none() && (first != 31 || !face.enclosed_blank) {
            return reject(RejectReason::UnverifiedBlank, Some(scores));
        }
        if first == 31 && face.input.glyph_bounds.is_some() {
            return reject(RejectReason::UnexpectedGlyphForWhite, Some(scores));
        }
        ClassifierDecision::Accepted {
            tile: Tile::from_code(CLASS_ORDER[first]).expect("fixed valid class map"),
            scores,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScoreSummary {
    pub probability: f64,
    pub margin: f64,
    pub normalized_entropy: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejectReason {
    InvalidLogits,
    LowProbability,
    Ambiguous,
    HighEntropy,
    UnverifiedBlank,
    UnexpectedGlyphForWhite,
}
#[derive(Clone, Debug, PartialEq)]
pub enum ClassifierDecision {
    Accepted {
        tile: Tile,
        scores: ScoreSummary,
    },
    Rejected {
        reason: RejectReason,
        scores: Option<ScoreSummary>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassifierFailure {
    Unavailable,
    InvalidInput,
    InvalidOutput,
}
/// Implementations may be bound to the creating thread's native apartment.
/// No Send/Sync requirement or background thread is imposed on a local session.
pub trait FaceClassifier {
    fn classify(&self, face: &FaceObservation) -> Result<ClassifierDecision, ClassifierFailure>;
}

/// Prepared pixels and white-face evidence from the same immutable observation.
/// Keeping these private prevents passing a previous face's blank evidence.
pub struct FaceObservation {
    input: PreparedTile,
    source: RgbImage,
    enclosed_blank: bool,
}
impl FaceObservation {
    pub fn isolated(image: &RgbImage) -> Result<Self, VisionError> {
        Ok(Self {
            input: prepare_tile(image)?,
            source: image.clone(),
            enclosed_blank: false,
        })
    }
    pub fn in_hand(scene: &RgbImage, rect: Rect) -> Result<Self, VisionError> {
        if scene.width() > 16384
            || scene.height() > 1024
            || !(8..=1024).contains(&rect.width)
            || !(8..=1024).contains(&rect.height)
        {
            return Err(VisionError::InvalidImage);
        }
        let image = scene.crop(rect)?;
        let input = prepare_tile(&image)?;
        let enclosed_blank = input.glyph_bounds.is_none()
            && upright_ratio(image.width(), image.height())
            && detect_hand_slots(scene).contains(&rect)
            && enclosing_blank_edges(scene, rect);
        Ok(Self {
            input,
            source: image,
            enclosed_blank,
        })
    }
    pub fn in_public(
        scene: &RgbImage,
        rect: Rect,
        rotation: Rotation,
    ) -> Result<Self, VisionError> {
        if scene.width() > 16384
            || scene.height() > 1024
            || !(8..=1024).contains(&rect.width)
            || !(8..=1024).contains(&rect.height)
        {
            return Err(VisionError::InvalidImage);
        }
        let image = scene.crop(rect)?.rotated(rotation);
        let input = prepare_tile(&image)?;
        let enclosed_blank = input.glyph_bounds.is_none()
            && upright_ratio(image.width(), image.height())
            && detect_public_slots(scene).contains(&rect)
            && enclosing_blank_edges(scene, rect);
        Ok(Self {
            input,
            source: image,
            enclosed_blank,
        })
    }
    pub fn input(&self) -> &[f32] {
        &self.input.pixels
    }
    pub(crate) fn source_image(&self) -> &RgbImage {
        &self.source
    }
    pub fn glyph_bounds(&self) -> Option<Rect> {
        self.input.glyph_bounds
    }
    pub fn blank_face_verified(&self) -> bool {
        self.enclosed_blank
    }
}

fn upright_ratio(width: u32, height: u32) -> bool {
    height * 10 >= width * 12 && height * 10 <= width * 22
}

fn enclosing_blank_edges(scene: &RgbImage, r: Rect) -> bool {
    if r.x < 3
        || r.y < 3
        || r.x + r.width + 3 > scene.width()
        || r.y + r.height + 3 > scene.height()
    {
        return false;
    }
    let brightness = |x: u32, y: u32| {
        let i = ((y * scene.width() + x) * 3) as usize;
        scene.pixels()[i..i + 3]
            .iter()
            .map(|&v| v as f64)
            .sum::<f64>()
            / 3.0
    };
    let mut center = 0.0;
    let mut count = 0;
    for y in r.y + r.height / 4..r.y + r.height * 3 / 4 {
        for x in r.x + r.width / 4..r.x + r.width * 3 / 4 {
            center += brightness(x, y);
            count += 1;
        }
    }
    center /= count as f64;
    if center < 185.0 {
        return false;
    }
    // Each side must show a darker boundary outside the detected pale face.
    // Tight/truncated crops and unbounded pale areas intentionally fail.
    for side in 0..4 {
        let length = if side < 2 { r.width } else { r.height };
        let mut darker = 0;
        let mut samples = 0;
        for position in length / 5..length * 4 / 5 {
            let found = (1..=3).any(|distance| {
                let (x, y) = match side {
                    0 => (r.x + position, r.y - distance),
                    1 => (r.x + position, r.y + r.height - 1 + distance),
                    2 => (r.x - distance, r.y + position),
                    _ => (r.x + r.width - 1 + distance, r.y + position),
                };
                center - brightness(x, y) >= 20.0
            });
            darker += u32::from(found);
            samples += 1;
        }
        if samples == 0 || darker * 5 < samples * 3 {
            return false;
        }
    }
    true
}
