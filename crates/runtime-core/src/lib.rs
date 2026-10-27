//! In-memory pipeline from calibrated images to stable read-only recommendations.

use game_state::{ActionAdvice, ClaimSource, GameTracker, Phase, StateError, StateEvent};
use riichi_core::{Recommendation, Tile};
use vision_core::{
    ActionLabel, FilterDecision, MatchConfig, Rect, RgbImage, StabilityFilter, TemplateBank,
    VisionError, VisionSnapshot, detect_hand_slots,
};

#[derive(Clone, Debug, PartialEq)]
pub enum PipelineOutput {
    Paused,
    Waiting,
    Uncertain,
    Ready {
        hand: Vec<Tile>,
        confidence: f32,
        phase: Phase,
        events: Vec<StateEvent>,
        discards: Vec<Recommendation>,
        available_actions: Vec<ActionLabel>,
        actions: Vec<ActionAdvice>,
    },
}

pub struct RuntimeEngine {
    bank: TemplateBank,
    action_slots: Vec<Rect>,
    match_config: MatchConfig,
    filter: StabilityFilter,
    stable_frames: usize,
    tracker: GameTracker,
    offered: Option<(Tile, ClaimSource)>,
    cached_discards: Option<(Vec<Tile>, Vec<Recommendation>)>,
    recommendation_calculations: u64,
    paused: bool,
}

impl RuntimeEngine {
    pub fn new(
        bank: TemplateBank,
        action_slots: Vec<Rect>,
        match_config: MatchConfig,
        stable_frames: usize,
    ) -> Result<Self, VisionError> {
        Ok(Self {
            bank,
            action_slots,
            match_config,
            filter: StabilityFilter::new(stable_frames)?,
            stable_frames,
            tracker: GameTracker::new(),
            offered: None,
            cached_discards: None,
            recommendation_calculations: 0,
            paused: false,
        })
    }

    pub fn tracker(&self) -> &GameTracker {
        &self.tracker
    }
    pub fn tracker_mut(&mut self) -> &mut GameTracker {
        self.cached_discards = None;
        &mut self.tracker
    }
    pub fn recommendation_calculations(&self) -> u64 {
        self.recommendation_calculations
    }
    pub fn set_paused(&mut self, paused: bool) {
        if self.paused != paused {
            self.filter = StabilityFilter::new(self.stable_frames).expect("validated frame count");
            self.tracker.mark_uncertain();
            self.cached_discards = None;
        }
        self.paused = paused;
    }
    pub fn is_paused(&self) -> bool {
        self.paused
    }
    pub fn reset_recognition(&mut self) {
        self.filter = StabilityFilter::new(self.stable_frames).expect("validated frame count");
        self.tracker.mark_uncertain();
        self.cached_discards = None;
    }
    pub fn set_offer(&mut self, offered: Option<(Tile, ClaimSource)>) {
        self.offered = offered;
    }

    pub fn set_public_tiles(
        &mut self,
        visible: [u8; 34],
        dora_indicators: Vec<Tile>,
    ) -> Result<(), StateError> {
        self.tracker.set_public_tiles(visible, dora_indicators)?;
        self.cached_discards = None;
        Ok(())
    }

    pub fn process(
        &mut self,
        frame_id: u64,
        hand_image: &RgbImage,
        action_image: Option<&RgbImage>,
    ) -> PipelineOutput {
        if self.paused {
            return PipelineOutput::Paused;
        }
        let slots = detect_hand_slots(hand_image);
        let open = self.tracker.open_melds() as usize;
        let expected = [
            13usize.saturating_sub(3 * open),
            14usize.saturating_sub(3 * open),
        ];
        let candidate = if expected.contains(&slots.len()) {
            self.read_candidate(hand_image, action_image, &slots)
        } else {
            None
        };
        match self.filter.observe(frame_id, candidate) {
            FilterDecision::Waiting => PipelineOutput::Waiting,
            FilterDecision::Uncertain => {
                self.tracker.mark_uncertain();
                self.cached_discards = None;
                PipelineOutput::Uncertain
            }
            FilterDecision::Stable(snapshot) => {
                let result = self.tracker.observe(game_state::ObservedHand {
                    frame_id,
                    hand: snapshot.hand.clone(),
                    actions: snapshot.actions.clone(),
                    confidence: snapshot.confidence,
                });
                if result.phase == Phase::Uncertain {
                    self.cached_discards = None;
                    return PipelineOutput::Uncertain;
                }
                let discards = if let Some((cached_hand, cached)) = &self.cached_discards {
                    if *cached_hand == snapshot.hand {
                        cached.clone()
                    } else {
                        self.compute_discards(&snapshot.hand)
                    }
                } else {
                    self.compute_discards(&snapshot.hand)
                };
                let mut actions = Vec::new();
                for action in snapshot.actions {
                    let (offered, source) = if action == ActionLabel::Riichi {
                        (None, ClaimSource::SelfDraw)
                    } else {
                        self.offered
                            .map_or((None, ClaimSource::SelfDraw), |(tile, source)| {
                                (Some(tile), source)
                            })
                    };
                    if let Some(advice) = self.tracker.action_advice(action, offered, source) {
                        actions.push(advice);
                    }
                }
                PipelineOutput::Ready {
                    hand: snapshot.hand,
                    confidence: snapshot.confidence,
                    phase: result.phase,
                    events: result.events,
                    discards,
                    available_actions: self.tracker_actions(),
                    actions,
                }
            }
        }
    }

    fn tracker_actions(&self) -> Vec<ActionLabel> {
        self.tracker.available_actions().to_vec()
    }

    fn compute_discards(&mut self, hand: &[Tile]) -> Vec<Recommendation> {
        let discards = self.tracker.discard_recommendations();
        self.cached_discards = Some((hand.to_vec(), discards.clone()));
        self.recommendation_calculations = self.recommendation_calculations.saturating_add(1);
        discards
    }

    fn read_candidate(
        &self,
        hand_image: &RgbImage,
        action_image: Option<&RgbImage>,
        slots: &[Rect],
    ) -> Option<VisionSnapshot> {
        let hand = self
            .bank
            .recognize_hand(hand_image, slots, self.match_config)
            .ok()?;
        let hand = hand.tiles.map(|tiles| (tiles, hand.confidence))?;
        let (actions, action_confidence) = if self.action_slots.is_empty() {
            if let Some(image) = action_image {
                let read = self.bank.scan_actions(image, self.match_config);
                (read.actions?, read.confidence)
            } else {
                (Vec::new(), 1.0)
            }
        } else {
            let image = action_image?;
            let read = self
                .bank
                .recognize_actions(image, &self.action_slots, self.match_config)
                .ok()?;
            (read.actions?, read.confidence)
        };
        Some(VisionSnapshot {
            hand: hand.0,
            actions,
            confidence: hand.1.min(action_confidence),
        })
    }
}
