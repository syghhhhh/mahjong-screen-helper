use crate::{
    DetectedPublicTile, HandRead, MatchConfig, Rect, RgbImage, Rotation, TemplateBank, VisionError,
    classifier::{ClassifierDecision, FaceClassifier, FaceObservation, ScoreSummary},
};

fn valid_scores(scores: ScoreSummary) -> bool {
    [scores.probability, scores.margin, scores.normalized_entropy]
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        && scores.margin <= scores.probability
}

fn valid_face(face: &FaceObservation, tile: riichi_core::Tile) -> bool {
    let white = tile.index() == 31;
    if face.glyph_bounds().is_none() {
        white && face.blank_face_verified()
    } else {
        !white
    }
}

impl TemplateBank {
    /// Preserve accepted template slots; consult the model only for uncertainty.
    /// A failed/rejected slot prevents a partial hand from becoming a snapshot.
    pub fn recognize_hand_with_fallback(
        &self,
        image: &RgbImage,
        slots: &[Rect],
        config: MatchConfig,
        classifier: Option<&dyn FaceClassifier>,
    ) -> Result<HandRead, VisionError> {
        let Some(classifier) = classifier else {
            return self.recognize_hand(image, slots, config);
        };
        let mut tiles = Vec::with_capacity(slots.len());
        let mut uncertain_slots = Vec::new();
        let mut confidence = 1.0f32;
        for (index, &rect) in slots.iter().enumerate() {
            let template = self.recognize_hand(image, &[rect], config)?;
            if let Some(found) = template.tiles {
                tiles.extend(found);
                confidence = confidence.min(template.confidence);
                continue;
            }
            let face = FaceObservation::in_hand(image, rect)?;
            match classifier.classify(&face) {
                Ok(ClassifierDecision::Accepted { tile, scores })
                    if valid_scores(scores)
                        && valid_face(&face, tile)
                        && config.accepts(scores.probability as f32, scores.margin as f32) =>
                {
                    tiles.push(tile);
                    confidence = confidence.min(scores.probability as f32);
                }
                _ => {
                    uncertain_slots.push(index);
                    confidence = 0.0;
                }
            }
        }
        let mut counts = [0usize; 34];
        for tile in &tiles {
            counts[tile.index() as usize] += 1;
        }
        if counts.iter().any(|&n| n > 4) {
            uncertain_slots = (0..slots.len()).collect();
            confidence = 0.0;
        }
        Ok(HandRead {
            tiles: (uncertain_slots.is_empty() && !slots.is_empty()).then_some(tiles),
            uncertain_slots,
            confidence: if slots.is_empty() { 0.0 } else { confidence },
        })
    }
}

/// Consensus across shape-eligible rotations avoids treating a confident
/// interpretation as proof. Near-square faces keep all four alternatives.
/// A backend failure in an eligible view rejects the face.
pub(crate) fn neural_public_tile(
    image: &RgbImage,
    rect: Rect,
    classifier: &dyn FaceClassifier,
    config: MatchConfig,
) -> Option<DetectedPublicTile> {
    let mut selected: Option<DetectedPublicTile> = None;
    for rotation in Rotation::ALL
        .into_iter()
        .filter(|rotation| rotation.plausible_for_public_face(rect))
    {
        let face = FaceObservation::in_public(image, rect, rotation).ok()?;
        let decision = classifier.classify(&face).ok()?;
        if let ClassifierDecision::Accepted { tile, scores } = decision {
            if !valid_scores(scores) {
                return None;
            }
            if !valid_face(&face, tile) {
                continue;
            }
            if !config.accepts(scores.probability as f32, scores.margin as f32) {
                continue;
            }
            if selected.as_ref().is_some_and(|found| found.tile != tile) {
                return None;
            }
            if selected
                .as_ref()
                .is_none_or(|found| scores.probability as f32 > found.confidence)
            {
                selected = Some(DetectedPublicTile {
                    tile,
                    rect,
                    normalization: rotation,
                    confidence: scores.probability as f32,
                });
            }
        }
    }
    selected
}
