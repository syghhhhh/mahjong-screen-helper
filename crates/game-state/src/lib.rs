//! Deterministic observed-hand tracking and conservative action comparisons.

use riichi_core::{Recommendation, Tile, recommend_discards, shanten, ukeire};
use vision_core::ActionLabel;

const MIN_CONFIDENCE: f32 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    AwaitDraw,
    AwaitDiscard,
    Uncertain,
    RoundEnded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateEvent {
    Draw(Tile),
    Discard(Tile),
    ActionAvailable(ActionLabel),
    CallDeclared(ActionLabel),
    RiichiDeclared,
    RoundEnd,
    WindowLost,
    Uncertain,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObservedHand {
    pub frame_id: u64,
    pub hand: Vec<Tile>,
    pub actions: Vec<ActionLabel>,
    pub confidence: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservationResult {
    pub phase: Phase,
    pub events: Vec<StateEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateError {
    InvalidHand,
    InvalidAction,
    InvalidConfidence,
}

#[derive(Clone, Debug)]
pub struct GameTracker {
    phase: Phase,
    hand: Option<Vec<Tile>>,
    actions: Vec<ActionLabel>,
    confidence: f32,
    last_frame_id: Option<u64>,
    open_melds: u8,
    concealed_kans: u8,
    riichi_declared: bool,
    known_yaku: bool,
    visible: [u8; 34],
    dora_indicators: Vec<Tile>,
    own_meld_tiles: Vec<Tile>,
}

impl Default for GameTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl GameTracker {
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            hand: None,
            actions: Vec::new(),
            confidence: 0.0,
            last_frame_id: None,
            open_melds: 0,
            concealed_kans: 0,
            riichi_declared: false,
            known_yaku: false,
            visible: [0; 34],
            dora_indicators: Vec::new(),
            own_meld_tiles: Vec::new(),
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn hand(&self) -> Option<&[Tile]> {
        self.hand.as_deref()
    }
    pub fn available_actions(&self) -> &[ActionLabel] {
        &self.actions
    }
    /// Number of fixed groups, including declared concealed quads.
    pub fn open_melds(&self) -> u8 {
        self.open_melds
    }
    pub fn concealed_kans(&self) -> u8 {
        self.concealed_kans
    }
    pub fn is_closed(&self) -> bool {
        self.open_melds == self.concealed_kans
    }
    pub fn riichi_declared(&self) -> bool {
        self.riichi_declared
    }

    pub fn observe(&mut self, observation: ObservedHand) -> ObservationResult {
        if self
            .last_frame_id
            .is_some_and(|last| observation.frame_id <= last)
        {
            return self.enter_uncertain();
        }
        self.last_frame_id = Some(observation.frame_id);
        if !valid_confidence(observation.confidence)
            || !valid_hand(&observation.hand, self.open_melds)
            || !valid_visible(&observation.hand, &self.visible)
        {
            return self.enter_uncertain();
        }
        let expected_discard = 14 - 3 * self.open_melds as usize;
        let phase = if observation.hand.len() == expected_discard {
            Phase::AwaitDiscard
        } else {
            Phase::AwaitDraw
        };
        let mut events = Vec::new();
        if let Some(previous) = &self.hand {
            match hand_change(previous, &observation.hand) {
                HandChange::Same => {}
                HandChange::Draw(tile) => events.push(StateEvent::Draw(tile)),
                HandChange::Discard(tile) => events.push(StateEvent::Discard(tile)),
                HandChange::Invalid => return self.enter_uncertain(),
            }
        }
        for action in &observation.actions {
            if *action != ActionLabel::None && !self.actions.contains(action) {
                events.push(StateEvent::ActionAvailable(*action));
            }
        }
        self.phase = phase;
        self.hand = Some(observation.hand);
        self.actions = observation.actions;
        self.confidence = observation.confidence;
        ObservationResult { phase, events }
    }

    pub fn discard_recommendations(&self) -> Vec<Recommendation> {
        if self.phase != Phase::AwaitDiscard || self.riichi_declared {
            return Vec::new();
        }
        self.hand
            .as_ref()
            .and_then(|hand| {
                recommend_discards(
                    hand,
                    &self.visible,
                    &self.dora_indicators,
                    self.open_melds,
                    self.confidence,
                )
                .ok()
            })
            .unwrap_or_default()
    }

    pub fn action_advice(
        &self,
        action: ActionLabel,
        offered: Option<Tile>,
        source: ClaimSource,
    ) -> Option<ActionAdvice> {
        if self.phase == Phase::Uncertain || !self.actions.contains(&action) {
            return None;
        }
        let hand = self.hand.clone()?;
        let context = ActionContext {
            hand,
            visible: self.visible,
            dora_indicators: self.dora_indicators.clone(),
            offered,
            source,
            open_melds: self.open_melds,
            concealed_kans: self.concealed_kans,
            own_meld_tiles: self.own_meld_tiles.clone(),
            riichi_declared: self.riichi_declared,
            known_yaku: self.known_yaku,
            confidence: self.confidence,
        };
        advise_action(&context, action)
    }

    pub fn correct_hand(&mut self, hand: Vec<Tile>, confidence: f32) -> Result<(), StateError> {
        if !valid_confidence(confidence) {
            return Err(StateError::InvalidConfidence);
        }
        if !valid_hand(&hand, self.open_melds) || !valid_visible(&hand, &self.visible) {
            return Err(StateError::InvalidHand);
        }
        self.phase = if hand.len() == 14 - 3 * self.open_melds as usize {
            Phase::AwaitDiscard
        } else {
            Phase::AwaitDraw
        };
        self.hand = Some(hand);
        self.actions.clear();
        self.confidence = confidence;
        Ok(())
    }

    pub fn set_public_tiles(
        &mut self,
        visible: [u8; 34],
        dora_indicators: Vec<Tile>,
    ) -> Result<(), StateError> {
        if visible.iter().any(|count| *count > 4)
            || self
                .hand
                .as_ref()
                .is_some_and(|hand| !valid_visible(hand, &visible))
        {
            return Err(StateError::InvalidHand);
        }
        self.visible = visible;
        self.dora_indicators = dora_indicators;
        Ok(())
    }

    /// Apply a hand and its public counts from one stable visual observation.
    /// Validating against the incoming hand avoids counting a just-discarded
    /// tile both in the previous hand and the newly updated river.
    pub fn observe_with_public(
        &mut self,
        observation: ObservedHand,
        visible: [u8; 34],
        dora_indicators: Vec<Tile>,
    ) -> ObservationResult {
        let mut indicators = [0u8; 34];
        for tile in &dora_indicators {
            indicators[tile.index() as usize] = indicators[tile.index() as usize].saturating_add(1);
        }
        if visible.iter().any(|count| *count > 4)
            || dora_indicators.len() > 5
            || (0..34).any(|index| indicators[index] > visible[index])
            || !valid_visible(&observation.hand, &visible)
        {
            return self.enter_uncertain();
        }
        self.visible = visible;
        self.dora_indicators = dora_indicators;
        self.observe(observation)
    }

    /// Reconcile a stable table after a call or a reconnect. A reduced hand
    /// alone cannot create a meld: the observed public melds must partition
    /// into legal sequences/triplets/quads and agree with the concealed count.
    pub fn observe_table(
        &mut self,
        observation: ObservedHand,
        visible: [u8; 34],
        dora_indicators: Vec<Tile>,
        mut own_meld_tiles: Vec<Tile>,
    ) -> ObservationResult {
        let len = observation.hand.len();
        if !(1..=14).contains(&len) || !matches!(len % 3, 1 | 2) || own_meld_tiles.len() > 16 {
            return self.enter_uncertain();
        }
        let open = ((14 - len) / 3) as u8;
        let own_counts = counts(&own_meld_tiles);
        let mut meld_counts = own_counts;
        if own_counts.iter().any(|count| *count > 4)
            || !meld_partition(&mut meld_counts, open)
            || (0..34).any(|index| own_counts[index] > visible[index])
        {
            return self.enter_uncertain();
        }
        own_meld_tiles.sort();
        if open == self.open_melds && own_meld_tiles == self.own_meld_tiles {
            return self.observe_with_public(observation, visible, dora_indicators);
        }
        let mut concealed_kans = if self.hand.is_some() {
            self.concealed_kans
        } else {
            0
        };
        let event = if let Some(previous) = self.hand.as_ref() {
            let Some(added) = added_tiles(&self.own_meld_tiles, &own_meld_tiles) else {
                return self.enter_uncertain();
            };
            let action = if open == self.open_melds + 1 {
                if !meld_partition(&mut counts(&added), 1) {
                    return self.enter_uncertain();
                }
                if added.len() == 4 && added.iter().all(|tile| tile.index() == added[0].index()) {
                    ActionLabel::Kan
                } else if added.len() == 3
                    && added.iter().all(|tile| tile.index() == added[0].index())
                {
                    ActionLabel::Pon
                } else if added.len() == 3 {
                    ActionLabel::Chi
                } else {
                    return self.enter_uncertain();
                }
            } else if open == self.open_melds
                && added.len() == 1
                && self
                    .own_meld_tiles
                    .iter()
                    .filter(|tile| tile.index() == added[0].index())
                    .count()
                    == 3
            {
                ActionLabel::Kan
            } else {
                return self.enter_uncertain();
            };
            if !call_hand_matches(previous, &observation.hand, &added, action) {
                return self.enter_uncertain();
            }
            if action == ActionLabel::Kan
                && open == self.open_melds + 1
                && added.len() == 4
                && self_kan_hand_matches(previous, &observation.hand, &added)
            {
                concealed_kans += 1;
            }
            Some(StateEvent::CallDeclared(action))
        } else {
            None
        };
        let mut staged = self.clone();
        staged.open_melds = open;
        staged.concealed_kans = concealed_kans;
        staged.own_meld_tiles = own_meld_tiles;
        staged.hand = None;
        staged.known_yaku |= (31..34).any(|index| own_counts[index] >= 3);
        let mut result = staged.observe_with_public(observation, visible, dora_indicators);
        if result.phase == Phase::Uncertain {
            return self.enter_uncertain();
        }
        if let Some(event) = event {
            result.events.push(event);
        }
        *self = staged;
        result
    }

    pub fn set_known_yaku(&mut self, known_yaku: bool) {
        self.known_yaku = known_yaku;
    }

    pub fn confirm_call(
        &mut self,
        action: ActionLabel,
        tiles_from_hand: &[Tile],
        offered: Tile,
        source: ClaimSource,
    ) -> Result<StateEvent, StateError> {
        let required = match action {
            ActionLabel::Chi | ActionLabel::Pon => 2,
            ActionLabel::Kan => 3,
            _ => return Err(StateError::InvalidAction),
        };
        if self.phase != Phase::AwaitDraw
            || self.riichi_declared
            || self.open_melds >= 4
            || !self.actions.contains(&action)
            || tiles_from_hand.len() != required
            || self.visible[offered.index() as usize] == 0
            || !legal_claim(action, tiles_from_hand, offered, source)
        {
            return Err(StateError::InvalidAction);
        }
        let mut hand = self.hand.clone().ok_or(StateError::InvalidHand)?;
        for tile in tiles_from_hand {
            let position = hand
                .iter()
                .position(|candidate| candidate == tile)
                .ok_or(StateError::InvalidAction)?;
            hand.remove(position);
        }
        let next_open = self.open_melds + 1;
        if !valid_hand(&hand, next_open) {
            return Err(StateError::InvalidHand);
        }
        for tile in tiles_from_hand {
            self.visible[tile.index() as usize] += 1;
        }
        self.own_meld_tiles.extend_from_slice(tiles_from_hand);
        self.own_meld_tiles.push(offered);
        self.own_meld_tiles.sort();
        self.open_melds = next_open;
        self.phase = if action == ActionLabel::Kan {
            Phase::AwaitDraw
        } else {
            Phase::AwaitDiscard
        };
        self.hand = Some(hand);
        self.actions.clear();
        Ok(StateEvent::CallDeclared(action))
    }

    pub fn confirm_riichi(&mut self) -> Result<StateEvent, StateError> {
        if !self.is_closed()
            || self.phase != Phase::AwaitDiscard
            || self.riichi_declared
            || !self.actions.contains(&ActionLabel::Riichi)
        {
            return Err(StateError::InvalidAction);
        }
        let hand = self.hand.as_ref().ok_or(StateError::InvalidHand)?;
        if shanten(&counts(hand), self.open_melds).map_err(|_| StateError::InvalidHand)? < 0 {
            return Err(StateError::InvalidAction);
        }
        let candidates = recommend_discards(
            hand,
            &self.visible,
            &self.dora_indicators,
            self.open_melds,
            self.confidence,
        )
        .map_err(|_| StateError::InvalidHand)?;
        if !candidates.iter().any(|item| item.shanten == 0) {
            return Err(StateError::InvalidAction);
        }
        self.riichi_declared = true;
        Ok(StateEvent::RiichiDeclared)
    }

    /// Confirm a self-drawn concealed or added kan only after a visible Kan
    /// button and physical tile validation. Post-riichi kan needs a separate
    /// wait/interpretation check and is deliberately not confirmed here.
    pub fn confirm_self_kan(&mut self, tiles_from_hand: &[Tile]) -> Result<StateEvent, StateError> {
        if self.phase != Phase::AwaitDiscard
            || self.riichi_declared
            || !self.actions.contains(&ActionLabel::Kan)
            || !matches!(tiles_from_hand.len(), 1 | 4)
        {
            return Err(StateError::InvalidAction);
        }
        let tile = tiles_from_hand[0];
        if !tiles_from_hand
            .iter()
            .all(|candidate| candidate.index() == tile.index())
        {
            return Err(StateError::InvalidAction);
        }
        let concealed = tiles_from_hand.len() == 4;
        if concealed && self.open_melds >= 4
            || !concealed && counts(&self.own_meld_tiles)[tile.index() as usize] != 3
        {
            return Err(StateError::InvalidAction);
        }
        let mut hand = self.hand.clone().ok_or(StateError::InvalidHand)?;
        for tile in tiles_from_hand {
            hand.remove(
                hand.iter()
                    .position(|candidate| candidate == tile)
                    .ok_or(StateError::InvalidAction)?,
            );
        }
        let next_melds = self.open_melds + u8::from(concealed);
        let mut visible = self.visible;
        for tile in tiles_from_hand {
            visible[tile.index() as usize] = visible[tile.index() as usize].saturating_add(1);
        }
        if !valid_hand(&hand, next_melds)
            || !valid_visible(&hand, &visible)
            || visible.iter().any(|count| *count > 4)
        {
            return Err(StateError::InvalidHand);
        }
        self.hand = Some(hand);
        self.visible = visible;
        self.own_meld_tiles.extend_from_slice(tiles_from_hand);
        self.own_meld_tiles.sort();
        self.open_melds = next_melds;
        self.concealed_kans += u8::from(concealed);
        self.phase = Phase::AwaitDraw;
        self.actions.clear();
        Ok(StateEvent::CallDeclared(ActionLabel::Kan))
    }

    pub fn window_lost(&mut self) -> StateEvent {
        self.enter_uncertain();
        self.last_frame_id = None;
        StateEvent::WindowLost
    }

    pub fn mark_uncertain(&mut self) {
        self.enter_uncertain();
    }

    pub fn round_end(&mut self) -> StateEvent {
        self.phase = Phase::RoundEnded;
        self.hand = None;
        self.actions.clear();
        self.confidence = 0.0;
        self.last_frame_id = None;
        self.open_melds = 0;
        self.concealed_kans = 0;
        self.riichi_declared = false;
        self.known_yaku = false;
        self.visible = [0; 34];
        self.dora_indicators.clear();
        self.own_meld_tiles.clear();
        StateEvent::RoundEnd
    }

    fn enter_uncertain(&mut self) -> ObservationResult {
        self.phase = Phase::Uncertain;
        self.hand = None;
        self.actions.clear();
        self.confidence = 0.0;
        ObservationResult {
            phase: Phase::Uncertain,
            events: vec![StateEvent::Uncertain],
        }
    }
}

fn meld_partition(counts: &mut [u8; 34], groups: u8) -> bool {
    let Some(index) = counts.iter().position(|count| *count > 0) else {
        return groups == 0;
    };
    if groups == 0 {
        return false;
    }
    for size in [3, 4] {
        if counts[index] >= size {
            counts[index] -= size;
            let valid = meld_partition(counts, groups - 1);
            counts[index] += size;
            if valid {
                return true;
            }
        }
    }
    if index < 27 && index % 9 <= 6 && counts[index + 1] > 0 && counts[index + 2] > 0 {
        for slot in &mut counts[index..=index + 2] {
            *slot -= 1;
        }
        let valid = meld_partition(counts, groups - 1);
        for slot in &mut counts[index..=index + 2] {
            *slot += 1;
        }
        if valid {
            return true;
        }
    }
    false
}

fn added_tiles(before: &[Tile], after: &[Tile]) -> Option<Vec<Tile>> {
    let mut added = after.to_vec();
    for tile in before {
        added.remove(added.iter().position(|candidate| candidate == tile)?);
    }
    Some(added)
}

fn call_hand_matches(
    before: &[Tile],
    after: &[Tile],
    added_meld: &[Tile],
    action: ActionLabel,
) -> bool {
    let mut choices = Vec::new();
    if added_meld.len() == 1 || action == ActionLabel::Kan {
        choices.push(added_meld.to_vec());
    }
    if added_meld.len() >= 3 {
        for offered in 0..added_meld.len() {
            let mut consumed = added_meld.to_vec();
            consumed.remove(offered);
            choices.push(consumed);
        }
    }
    choices.into_iter().any(|consumed| {
        let Some(remaining) = added_tiles(&consumed, before) else {
            return false;
        };
        matches!(hand_change(&remaining, after), HandChange::Same)
            || (action != ActionLabel::Kan
                && matches!(hand_change(&remaining, after), HandChange::Discard(_)))
            || (action == ActionLabel::Kan
                && matches!(hand_change(&remaining, after), HandChange::Draw(_)))
    })
}

fn self_kan_hand_matches(before: &[Tile], after: &[Tile], consumed: &[Tile]) -> bool {
    let Some(remaining) = added_tiles(consumed, before) else {
        return false;
    };
    matches!(
        hand_change(&remaining, after),
        HandChange::Same | HandChange::Draw(_)
    )
}

fn valid_confidence(confidence: f32) -> bool {
    confidence.is_finite() && (MIN_CONFIDENCE..=1.0).contains(&confidence)
}

fn valid_hand(hand: &[Tile], open_melds: u8) -> bool {
    let mut counts = [0u8; 34];
    for tile in hand {
        counts[tile.index() as usize] = counts[tile.index() as usize].saturating_add(1);
    }
    shanten(&counts, open_melds).is_ok()
}

fn valid_visible(hand: &[Tile], visible: &[u8; 34]) -> bool {
    let mut counts = [0u8; 34];
    for tile in hand {
        counts[tile.index() as usize] = counts[tile.index() as usize].saturating_add(1);
    }
    (0..34).all(|i| counts[i] as u16 + visible[i] as u16 <= 4)
}

enum HandChange {
    Same,
    Draw(Tile),
    Discard(Tile),
    Invalid,
}

fn hand_change(before: &[Tile], after: &[Tile]) -> HandChange {
    let mut old = before.to_vec();
    let mut new = after.to_vec();
    old.sort();
    new.sort();
    if old == new {
        return HandChange::Same;
    }
    if new.len() == old.len() + 1 {
        for index in 0..new.len() {
            let mut reduced = new.clone();
            let tile = reduced.remove(index);
            if reduced == old {
                return HandChange::Draw(tile);
            }
        }
    }
    if old.len() == new.len() + 1 {
        for index in 0..old.len() {
            let mut reduced = old.clone();
            let tile = reduced.remove(index);
            if reduced == new {
                return HandChange::Discard(tile);
            }
        }
    }
    HandChange::Invalid
}

fn legal_claim(
    action: ActionLabel,
    tiles_from_hand: &[Tile],
    offered: Tile,
    source: ClaimSource,
) -> bool {
    if source == ClaimSource::SelfDraw {
        return false;
    }
    match action {
        ActionLabel::Pon => {
            tiles_from_hand.len() == 2
                && tiles_from_hand
                    .iter()
                    .all(|tile| tile.index() == offered.index())
        }
        ActionLabel::Kan => {
            tiles_from_hand.len() == 3
                && tiles_from_hand
                    .iter()
                    .all(|tile| tile.index() == offered.index())
        }
        ActionLabel::Chi => {
            if source != ClaimSource::Left || offered.index() >= 27 || tiles_from_hand.len() != 2 {
                return false;
            }
            let mut indices = [
                tiles_from_hand[0].index(),
                tiles_from_hand[1].index(),
                offered.index(),
            ];
            indices.sort();
            indices[0] / 9 == indices[2] / 9
                && indices[0] + 1 == indices[1]
                && indices[1] + 1 == indices[2]
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimSource {
    SelfDraw,
    Left,
    Across,
    Right,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActionContext {
    pub hand: Vec<Tile>,
    pub visible: [u8; 34],
    pub dora_indicators: Vec<Tile>,
    pub offered: Option<Tile>,
    pub source: ClaimSource,
    pub open_melds: u8,
    /// Fixed concealed quads preserve menzen even though they reduce hand size.
    pub concealed_kans: u8,
    pub own_meld_tiles: Vec<Tile>,
    pub riichi_declared: bool,
    /// True only when an already secured yaku has been confirmed by state or user.
    pub known_yaku: bool,
    pub confidence: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActionAdvice {
    pub action: ActionLabel,
    pub tiles_from_hand: Vec<Tile>,
    pub discard_after_action: Option<Tile>,
    pub before_shanten: i8,
    pub after_shanten: i8,
    pub before_ukeire: u16,
    pub after_ukeire: u16,
    pub recommended: bool,
    pub confidence: f32,
    pub reason: String,
}

/// Evaluate a legal action against the current visible tiles. Missing offered tiles,
/// low confidence, and incomplete hand counts suppress deterministic advice.
pub fn advise_action(context: &ActionContext, action: ActionLabel) -> Option<ActionAdvice> {
    if !valid_confidence(context.confidence)
        || context.concealed_kans > context.open_melds
        || !valid_hand(&context.hand, context.open_melds)
        || !valid_visible(&context.hand, &context.visible)
        || context.riichi_declared
    {
        return None;
    }
    match action {
        ActionLabel::Riichi => advise_riichi(context),
        ActionLabel::Kan if context.source == ClaimSource::SelfDraw => advise_self_kan(context),
        ActionLabel::Chi | ActionLabel::Pon | ActionLabel::Kan => advise_call(context, action),
        ActionLabel::None => None,
    }
}

fn counts(hand: &[Tile]) -> [u8; 34] {
    let mut result = [0; 34];
    for tile in hand {
        result[tile.index() as usize] += 1;
    }
    result
}

fn advise_riichi(context: &ActionContext) -> Option<ActionAdvice> {
    if context.open_melds != context.concealed_kans
        || context.source != ClaimSource::SelfDraw
        || context.hand.len() != 14 - 3 * context.open_melds as usize
    {
        return None;
    }
    let before = shanten(&counts(&context.hand), context.open_melds).ok()?;
    if before < 0 {
        return None;
    }
    let choices = recommend_discards(
        &context.hand,
        &context.visible,
        &context.dora_indicators,
        context.open_melds,
        context.confidence,
    )
    .ok()?;
    let chosen = choices.into_iter().find(|item| item.shanten == 0)?;
    Some(ActionAdvice {
        action: ActionLabel::Riichi,
        tiles_from_hand: vec![],
        discard_after_action: Some(chosen.discard),
        before_shanten: before,
        after_shanten: chosen.shanten,
        before_ukeire: 0,
        after_ukeire: chosen.ukeire,
        recommended: true,
        confidence: context.confidence,
        reason: format!("切牌后听牌，{} 张有效牌；未计打点与防守", chosen.ukeire),
    })
}

fn advise_call(context: &ActionContext, action: ActionLabel) -> Option<ActionAdvice> {
    if context.open_melds >= 4 || context.source == ClaimSource::SelfDraw {
        return None;
    }
    let offered = context.offered?;
    if context.visible[offered.index() as usize] == 0 {
        return None;
    }
    let before = ukeire(&counts(&context.hand), &context.visible, context.open_melds).ok()?;
    let mut choices: Vec<Vec<u8>> = Vec::new();
    match action {
        ActionLabel::Chi => {
            if context.source != ClaimSource::Left || offered.index() >= 27 {
                return None;
            }
            let index = offered.index() as i16;
            let suit_start = index / 9 * 9;
            for start in index - 2..=index {
                if start < suit_start || start + 2 >= suit_start + 9 {
                    continue;
                }
                choices.push(
                    (start..=start + 2)
                        .filter(|candidate| *candidate != index)
                        .map(|value| value as u8)
                        .collect(),
                );
            }
        }
        ActionLabel::Pon => choices.push(vec![offered.index(); 2]),
        ActionLabel::Kan => choices.push(vec![offered.index(); 3]),
        _ => return None,
    }
    let mut advice = Vec::new();
    for indices in choices {
        let mut remaining = context.hand.clone();
        let mut consumed = Vec::new();
        for index in indices {
            let position = remaining
                .iter()
                .position(|tile| tile.index() == index && !tile.is_red())
                .or_else(|| remaining.iter().position(|tile| tile.index() == index));
            let Some(position) = position else {
                consumed.clear();
                break;
            };
            consumed.push(remaining.remove(position));
        }
        let required = if action == ActionLabel::Kan { 3 } else { 2 };
        if consumed.len() != required {
            continue;
        }
        let mut after_visible = context.visible;
        for tile in &consumed {
            after_visible[tile.index() as usize] += 1;
        }
        let next_open = context.open_melds + 1;
        let (after_shanten, after_ukeire, discard) = if action == ActionLabel::Kan {
            let result = ukeire(&counts(&remaining), &after_visible, next_open).ok()?;
            (result.shanten, result.count, None)
        } else {
            let best = recommend_discards(
                &remaining,
                &after_visible,
                &context.dora_indicators,
                next_open,
                context.confidence,
            )
            .ok()?
            .into_iter()
            .next()?;
            (best.shanten, best.ukeire, Some(best.discard))
        };
        let efficiency_gain = after_shanten < before.shanten
            || (action != ActionLabel::Kan
                && after_shanten == before.shanten
                && after_ukeire > before.count);
        let guaranteed_yaku = context.known_yaku
            || (matches!(action, ActionLabel::Pon | ActionLabel::Kan) && offered.index() >= 31);
        let recommended = efficiency_gain && guaranteed_yaku && action != ActionLabel::Kan;
        let reason = if action == ActionLabel::Kan {
            format!(
                "杠前 {} 向听/{} 张，杠后补牌前 {} 向听/{} 张；补牌结果未知",
                before.shanten, before.count, after_shanten, after_ukeire
            )
        } else if !guaranteed_yaku {
            format!(
                "行动前 {} 向听/{} 张，行动并切牌后 {} 向听/{} 张；未确认役种，暂不建议鸣牌",
                before.shanten, before.count, after_shanten, after_ukeire
            )
        } else {
            format!(
                "行动前 {} 向听/{} 张，行动并切牌后 {} 向听/{} 张",
                before.shanten, before.count, after_shanten, after_ukeire
            )
        };
        advice.push(ActionAdvice {
            action,
            tiles_from_hand: consumed,
            discard_after_action: discard,
            before_shanten: before.shanten,
            after_shanten,
            before_ukeire: before.count,
            after_ukeire,
            recommended,
            confidence: context.confidence,
            reason,
        });
    }
    advice.sort_by(|a, b| {
        a.after_shanten
            .cmp(&b.after_shanten)
            .then_with(|| b.after_ukeire.cmp(&a.after_ukeire))
    });
    advice.into_iter().next()
}

fn advise_self_kan(context: &ActionContext) -> Option<ActionAdvice> {
    if context.hand.len() != 14 - 3 * context.open_melds as usize
        || context.own_meld_tiles.len() > 16
        || shanten(&counts(&context.hand), context.open_melds).ok()? < 0
    {
        return None;
    }
    let own = counts(&context.own_meld_tiles);
    if (0..34).any(|index| own[index] > context.visible[index]) {
        return None;
    }
    let hand_counts = counts(&context.hand);
    let before = recommend_discards(
        &context.hand,
        &context.visible,
        &context.dora_indicators,
        context.open_melds,
        context.confidence,
    )
    .ok()?
    .into_iter()
    .next()?;
    let mut options = Vec::new();
    for index in 0..34 {
        let concealed = hand_counts[index] == 4 && context.open_melds < 4;
        let added = own[index] == 3 && hand_counts[index] == 1;
        if !concealed && !added {
            continue;
        }
        let consumed: Vec<_> = context
            .hand
            .iter()
            .copied()
            .filter(|tile| tile.index() as usize == index)
            .collect();
        let remaining: Vec<_> = context
            .hand
            .iter()
            .copied()
            .filter(|tile| tile.index() as usize != index)
            .collect();
        let next_melds = context.open_melds + u8::from(concealed);
        let mut visible = context.visible;
        visible[index] += consumed.len() as u8;
        let after = ukeire(&counts(&remaining), &visible, next_melds).ok()?;
        let label = tile_label(consumed[0]);
        options.push(ActionAdvice {
            action: ActionLabel::Kan,
            tiles_from_hand: consumed,
            discard_after_action: None,
            before_shanten: before.shanten,
            after_shanten: after.shanten,
            before_ukeire: before.ukeire,
            after_ukeire: after.count,
            recommended: false,
            confidence: context.confidence,
            reason: format!("{} {label}：杠前最优切牌后 {} 向听/{} 张，杠后补牌前 {} 向听/{} 张；补牌与新宝牌未知", if concealed { "暗杠" } else { "加杠" }, before.shanten, before.ukeire, after.shanten, after.count),
        });
    }
    options.sort_by(|a, b| {
        a.after_shanten
            .cmp(&b.after_shanten)
            .then_with(|| b.after_ukeire.cmp(&a.after_ukeire))
    });
    options.into_iter().next()
}

fn tile_label(tile: Tile) -> String {
    let index = tile.index();
    let (rank, suit) = match index {
        0..=8 => (index + 1, 'm'),
        9..=17 => (index - 8, 'p'),
        18..=26 => (index - 17, 's'),
        _ => (index - 26, 'z'),
    };
    format!("{}{}", if tile.is_red() { 0 } else { rank }, suit)
}
