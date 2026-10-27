//! Word-level model decisions, not proof of an enabled game action. A caller
//! must independently validate the button plate, position and current state.
use crate::{
    action_input::{ActionWordInput, INPUT_FLOATS},
    classifier::ScoreSummary,
};

pub const CLASS_ORDER: [&str; 7] = [
    "Chi",
    "Pon",
    "Kan",
    "Riichi",
    "Pass",
    "Unsupported",
    "Unknown",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionWord {
    Chi,
    Pon,
    Kan,
    Riichi,
    /// An explicit skip word, distinct from missing/unrecognized button input.
    Pass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejectReason {
    NoWord,
    InvalidInput,
    SolidForeground,
    InvalidLogits,
    LowProbability,
    Ambiguous,
    HighEntropy,
    UnsupportedWord,
    UnknownWord,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActionWordDecision {
    /// Text candidate only. Deliberately has no conversion to ActionLabel.
    Candidate {
        word: ActionWord,
        scores: ScoreSummary,
    },
    Rejected {
        reason: RejectReason,
        scores: Option<ScoreSummary>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidActionThresholds;
impl std::fmt::Display for InvalidActionThresholds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Invalid action word thresholds")
    }
}
impl std::error::Error for InvalidActionThresholds {}

#[derive(Clone, Copy, Debug)]
pub struct ActionWordPolicy {
    min_probability: f64,
    min_margin: f64,
    max_normalized_entropy: f64,
}

impl Default for ActionWordPolicy {
    fn default() -> Self {
        Self {
            min_probability: 0.9,
            min_margin: 0.25,
            max_normalized_entropy: 0.35,
        }
    }
}

impl ActionWordPolicy {
    pub fn new(
        min_probability: f64,
        min_margin: f64,
        max_normalized_entropy: f64,
    ) -> Result<Self, InvalidActionThresholds> {
        if !(0.5..=1.0).contains(&min_probability)
            || !(0.0..=1.0).contains(&min_margin)
            || min_margin == 0.0
            || !(0.0..=1.0).contains(&max_normalized_entropy)
            || max_normalized_entropy == 0.0
        {
            return Err(InvalidActionThresholds);
        }
        Ok(Self {
            min_probability,
            min_margin,
            max_normalized_entropy,
        })
    }

    pub fn decide(&self, input: &ActionWordInput, logits: &[f32; 7]) -> ActionWordDecision {
        let reject = |reason, scores| ActionWordDecision::Rejected { reason, scores };
        let ActionWordInput::Prepared(word) = input else {
            return reject(RejectReason::NoWord, None);
        };
        let bounds = word.bounds;
        let area = u64::from(bounds.width) * u64::from(bounds.height);
        if bounds.width == 0
            || bounds.height == 0
            || bounds.width > 1024
            || bounds.height > 1024
            || bounds.x.checked_add(bounds.width).is_none()
            || bounds.y.checked_add(bounds.height).is_none()
            || word.foreground_pixels == 0
            || u64::from(word.foreground_pixels) > area
            || word.pixels.len() != INPUT_FLOATS
            || word
                .pixels
                .iter()
                .any(|&v| !v.is_finite() || !(0.0..=1.0).contains(&v))
            || !word.pixels.iter().any(|&v| v > 0.0)
            || !word.pixels.iter().any(|&v| v < 1.0)
        {
            return reject(RejectReason::InvalidInput, None);
        }
        if u64::from(word.foreground_pixels) == area {
            return reject(RejectReason::SolidForeground, None);
        }
        if logits.iter().any(|v| !v.is_finite()) {
            return reject(RejectReason::InvalidLogits, None);
        }
        let maximum = f64::from(logits.iter().copied().fold(f32::NEG_INFINITY, f32::max));
        let values = logits.map(|v| (f64::from(v) - maximum).exp());
        let total: f64 = values.iter().sum();
        let probabilities = values.map(|v| v / total);
        let mut order = [0, 1, 2, 3, 4, 5, 6];
        order.sort_by(|&a, &b| probabilities[b].total_cmp(&probabilities[a]));
        let top = order[0];
        let scores = ScoreSummary {
            probability: probabilities[top],
            margin: probabilities[top] - probabilities[order[1]],
            normalized_entropy: -probabilities
                .iter()
                .filter(|&&p| p > 0.0)
                .map(|p| p * p.ln())
                .sum::<f64>()
                / 7f64.ln(),
        };
        // A confident Unknown/Unsupported prediction still supplies no action.
        let word = match top {
            0 => ActionWord::Chi,
            1 => ActionWord::Pon,
            2 => ActionWord::Kan,
            3 => ActionWord::Riichi,
            4 => ActionWord::Pass,
            5 => return reject(RejectReason::UnsupportedWord, Some(scores)),
            _ => return reject(RejectReason::UnknownWord, Some(scores)),
        };
        if scores.probability < self.min_probability {
            return reject(RejectReason::LowProbability, Some(scores));
        }
        if scores.margin < self.min_margin {
            return reject(RejectReason::Ambiguous, Some(scores));
        }
        if scores.normalized_entropy > self.max_normalized_entropy {
            return reject(RejectReason::HighEntropy, Some(scores));
        }
        ActionWordDecision::Candidate { word, scores }
    }
}
