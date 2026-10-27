//! In-memory pipeline from calibrated images to stable read-only recommendations.

use game_state::{ActionAdvice, ClaimSource, GameTracker, Phase, StateError, StateEvent};
use riichi_core::{Recommendation, Tile};
use vision_core::classifier::FaceClassifier;
use vision_core::{
    ActionLabel, FilterDecision, MatchConfig, PublicSnapshot, Rect, RelativeSeat, RgbImage,
    StabilityFilter, TemplateBank, VisionError, VisionSnapshot, detect_hand_slots,
};

mod lifecycle;

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

pub struct PublicImages<'a> {
    pub river: &'a RgbImage,
    pub own_melds: &'a RgbImage,
    pub opponent_melds: [&'a RgbImage; 3],
    pub dora: &'a RgbImage,
}

pub struct RuntimeEngine {
    bank: TemplateBank,
    tile_classifier: Option<Box<dyn FaceClassifier>>,
    action_slots: Vec<Rect>,
    match_config: MatchConfig,
    filter: StabilityFilter,
    stable_frames: usize,
    tracker: GameTracker,
    offered: Option<(Tile, ClaimSource)>,
    cached_discards: Option<(Vec<Tile>, Vec<Recommendation>)>,
    recommendation_calculations: u64,
    last_public: Option<PublicSnapshot>,
    /// Last accepted round evidence survives transition animations and pauses.
    round_baseline: Option<(PublicSnapshot, Vec<Tile>)>,
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
            tile_classifier: None,
            action_slots,
            match_config,
            filter: StabilityFilter::new(stable_frames)?,
            stable_frames,
            tracker: GameTracker::new(),
            offered: None,
            cached_discards: None,
            recommendation_calculations: 0,
            last_public: None,
            round_baseline: None,
            paused: false,
        })
    }

    pub fn tracker(&self) -> &GameTracker {
        &self.tracker
    }
    /// Installing or removing a model invalidates prior stability and advice.
    /// Native sessions remain on the caller's thread.
    pub fn set_tile_classifier(&mut self, classifier: Option<Box<dyn FaceClassifier>>) {
        self.tile_classifier = classifier;
        // Round transitions need evidence from one recognition source; model
        // replacement must not compare its labels with the old model's baseline.
        self.round_baseline = None;
        self.reset_recognition();
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
            self.offered = None;
            self.last_public = None;
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
        self.offered = None;
        self.last_public = None;
    }
    /// End the current round and discard every piece of round-specific evidence.
    /// Capture/calibration and pause state belong to the session and are retained.
    pub fn end_round(&mut self) -> StateEvent {
        let event = self.tracker.round_end();
        self.filter = StabilityFilter::new(self.stable_frames).expect("validated frame count");
        self.cached_discards = None;
        self.offered = None;
        self.last_public = None;
        self.round_baseline = None;
        event
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
        self.process_frame(frame_id, hand_image, action_image, None)
    }

    pub fn process_with_public(
        &mut self,
        frame_id: u64,
        hand_image: &RgbImage,
        action_image: Option<&RgbImage>,
        public: &PublicImages<'_>,
    ) -> PipelineOutput {
        self.process_frame(frame_id, hand_image, action_image, Some(public))
    }

    fn process_frame(
        &mut self,
        frame_id: u64,
        hand_image: &RgbImage,
        action_image: Option<&RgbImage>,
        public: Option<&PublicImages<'_>>,
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
        let valid_count = if public.is_some() {
            (1..=14).contains(&slots.len()) && matches!(slots.len() % 3, 1 | 2)
        } else {
            expected.contains(&slots.len())
        };
        let candidate = if valid_count {
            self.read_candidate(hand_image, action_image, &slots, public)
        } else {
            None
        };
        match self.filter.observe(frame_id, candidate) {
            FilterDecision::Waiting => PipelineOutput::Waiting,
            FilterDecision::Uncertain => {
                self.tracker.mark_uncertain();
                self.cached_discards = None;
                self.offered = None;
                self.last_public = None;
                PipelineOutput::Uncertain
            }
            FilterDecision::Stable(snapshot) => {
                let observation = game_state::ObservedHand {
                    frame_id,
                    hand: snapshot.hand.clone(),
                    actions: snapshot.actions.clone(),
                    confidence: snapshot.confidence,
                };
                let result = if let Some(public) = &snapshot.public_info {
                    let round_ended =
                        self.round_baseline
                            .as_ref()
                            .is_some_and(|(previous, hand)| {
                                lifecycle::fresh_round(previous, public, hand, &snapshot.hand)
                            });
                    if self.last_public.as_ref() != Some(public) {
                        self.cached_discards = None;
                    }
                    let mut visible = [0u8; 34];
                    for tile in &public.visible_tiles {
                        visible[tile.index() as usize] =
                            visible[tile.index() as usize].saturating_add(1);
                    }
                    self.offered = public.offered.and_then(|(tile, seat)| {
                        let source = match seat {
                            RelativeSeat::Left => ClaimSource::Left,
                            RelativeSeat::Across => ClaimSource::Across,
                            RelativeSeat::Right => ClaimSource::Right,
                            RelativeSeat::SelfSeat => return None,
                        };
                        Some((tile, source))
                    });
                    self.last_public = Some(public.clone());
                    if round_ended {
                        // A reset must not erase the old round until the whole
                        // incoming hand/public combination is physically valid.
                        let mut staged = self.tracker.clone();
                        staged.round_end();
                        let mut result = staged.observe_table(
                            observation,
                            visible,
                            public.dora_indicators.clone(),
                            public.own_meld_tiles.clone(),
                        );
                        if result.phase == Phase::Uncertain {
                            self.tracker.mark_uncertain();
                        } else {
                            self.tracker = staged;
                            result.events.insert(0, StateEvent::RoundEnd);
                        }
                        result
                    } else {
                        self.tracker.observe_table(
                            observation,
                            visible,
                            public.dora_indicators.clone(),
                            public.own_meld_tiles.clone(),
                        )
                    }
                } else {
                    self.tracker.observe(observation)
                };
                if result.phase == Phase::Uncertain {
                    self.cached_discards = None;
                    return PipelineOutput::Uncertain;
                }
                if let Some(public) = &snapshot.public_info {
                    self.round_baseline = Some((public.clone(), snapshot.hand.clone()));
                }
                let mut discards = if let Some((cached_hand, cached)) = &self.cached_discards {
                    if *cached_hand == snapshot.hand {
                        cached.clone()
                    } else {
                        self.compute_discards(&snapshot.hand)
                    }
                } else {
                    self.compute_discards(&snapshot.hand)
                };
                for recommendation in &mut discards {
                    recommendation.confidence = snapshot.confidence;
                }
                let mut actions = Vec::new();
                for action in snapshot.actions {
                    let (offered, source) = if action == ActionLabel::Riichi
                        || (action == ActionLabel::Kan && result.phase == Phase::AwaitDiscard)
                    {
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
        public: Option<&PublicImages<'_>>,
    ) -> Option<VisionSnapshot> {
        let hand = self
            .bank
            .recognize_hand_with_fallback(
                hand_image,
                slots,
                self.match_config,
                self.tile_classifier.as_deref(),
            )
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
        let (public_info, public_confidence) = if let Some(public) = public {
            let mut visible_tiles = Vec::new();
            let mut confidence = 1.0f32;
            let river = self.bank.recognize_public_tiles_with_fallback(
                public.river,
                self.match_config,
                self.tile_classifier.as_deref(),
            );
            confidence = confidence.min(river.confidence);
            let offered = river.offered.map(|offer| (offer.tile, offer.source));
            visible_tiles.extend(river.tiles?.into_iter().map(|tile| tile.tile));
            let own = self.bank.recognize_public_tiles_with_fallback(
                public.own_melds,
                self.match_config,
                self.tile_classifier.as_deref(),
            );
            confidence = confidence.min(own.confidence);
            let mut own_meld_tiles: Vec<_> = own.tiles?.into_iter().map(|tile| tile.tile).collect();
            let open = (14 - hand.0.len()) / 3;
            if own_meld_tiles.len() < 3 * open || own_meld_tiles.len() > 4 * open {
                return None;
            }
            visible_tiles.extend_from_slice(&own_meld_tiles);
            for image in public.opponent_melds {
                let read = self.bank.recognize_public_tiles_with_fallback(
                    image,
                    self.match_config,
                    self.tile_classifier.as_deref(),
                );
                confidence = confidence.min(read.confidence);
                visible_tiles.extend(read.tiles?.into_iter().map(|tile| tile.tile));
            }
            let dora = self.bank.recognize_public_tiles_with_fallback(
                public.dora,
                self.match_config,
                self.tile_classifier.as_deref(),
            );
            confidence = confidence.min(dora.confidence);
            let dora_indicators: Vec<_> = dora.tiles?.into_iter().map(|tile| tile.tile).collect();
            visible_tiles.extend_from_slice(&dora_indicators);
            let mut counts = [0u8; 34];
            for tile in &visible_tiles {
                let count = &mut counts[tile.index() as usize];
                *count = count.saturating_add(1);
            }
            if counts.iter().any(|count| *count > 4) || dora_indicators.len() > 5 {
                return None;
            }
            visible_tiles.sort();
            own_meld_tiles.sort();
            (
                Some(PublicSnapshot {
                    visible_tiles,
                    own_meld_tiles,
                    dora_indicators,
                    offered,
                }),
                confidence,
            )
        } else {
            (None, 1.0)
        };
        Some(VisionSnapshot {
            hand: hand.0,
            actions,
            confidence: hand.1.min(action_confidence).min(public_confidence),
            public_info,
        })
    }
}
