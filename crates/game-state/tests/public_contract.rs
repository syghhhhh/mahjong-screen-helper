use game_state::{
    ActionAdvice, ActionContext, ClaimSource, GameTracker, ObservedHand, Phase, StateEvent,
    advise_action,
};
use riichi_core::Tile;
use vision_core::ActionLabel;

fn tiles(codes: &[&str]) -> Vec<Tile> {
    codes
        .iter()
        .map(|code| Tile::from_code(code).unwrap())
        .collect()
}

fn waiting_hand() -> Vec<Tile> {
    tiles(&[
        "1m", "2m", "3m", "4m", "5m", "6m", "2p", "3p", "4p", "2s", "3s", "4s", "7z",
    ])
}

fn observation(frame_id: u64, hand: Vec<Tile>, actions: Vec<ActionLabel>) -> ObservedHand {
    ObservedHand {
        frame_id,
        hand,
        actions,
        confidence: 0.98,
    }
}

#[test]
fn stable_public_observation_moves_a_tile_from_hand_to_river_atomically() {
    let mut tracker = GameTracker::new();
    let hand = tiles(&[
        "1m", "1m", "1m", "1m", "2m", "3m", "4m", "5m", "6m", "1p", "2p", "3p", "7z", "7z",
    ]);
    assert_eq!(
        tracker.observe(observation(1, hand.clone(), vec![])).phase,
        Phase::AwaitDiscard
    );
    let mut visible = [0; 34];
    visible[0] = 1;
    assert!(tracker.set_public_tiles(visible, vec![]).is_err());
    let mut after = hand;
    after.remove(0);
    let result = tracker.observe_with_public(observation(2, after, vec![]), visible, vec![]);
    assert_eq!(result.phase, Phase::AwaitDraw);
    assert!(
        result
            .events
            .contains(&StateEvent::Discard(Tile::from_code("1m").unwrap()))
    );

    visible[0] = 2;
    assert_eq!(
        tracker
            .observe_with_public(
                observation(3, tracker.hand().unwrap().to_vec(), vec![]),
                visible,
                vec![]
            )
            .phase,
        Phase::Uncertain
    );
    assert!(tracker.discard_recommendations().is_empty());
}

#[test]
fn table_observation_reconciles_pon_discard_and_added_kan_and_reconnects() {
    let mut tracker = GameTracker::new();
    let hand = tiles(&[
        "1m", "2m", "3m", "4m", "5m", "6m", "2p", "3p", "4p", "2s", "3s", "7z", "7z",
    ]);
    tracker.observe(observation(1, hand.clone(), vec![ActionLabel::Pon]));
    let mut after: Vec<_> = hand.into_iter().filter(|tile| tile.index() != 33).collect();
    let mut visible = [0; 34];
    visible[33] = 3;
    let meld = tiles(&["7z", "7z", "7z"]);
    let result = tracker.observe_table(
        observation(2, after.clone(), vec![]),
        visible,
        vec![],
        meld.clone(),
    );
    assert_eq!(result.phase, Phase::AwaitDiscard);
    assert_eq!(tracker.open_melds(), 1);
    assert!(
        result
            .events
            .contains(&StateEvent::CallDeclared(ActionLabel::Pon))
    );
    let discard = after.remove(0);
    visible[discard.index() as usize] += 1;
    assert!(
        tracker
            .observe_table(
                observation(3, after.clone(), vec![]),
                visible,
                vec![],
                meld.clone()
            )
            .events
            .contains(&StateEvent::Discard(discard))
    );
    after.push(Tile::from_code("7z").unwrap());
    assert_eq!(
        tracker
            .observe_table(
                observation(4, after.clone(), vec![]),
                visible,
                vec![],
                meld.clone()
            )
            .phase,
        Phase::AwaitDiscard
    );
    after.pop();
    visible[33] = 4;
    let kan = tiles(&["7z", "7z", "7z", "7z"]);
    let result = tracker.observe_table(
        observation(5, after.clone(), vec![]),
        visible,
        vec![],
        kan.clone(),
    );
    assert_eq!(result.phase, Phase::AwaitDraw);
    assert!(
        result
            .events
            .contains(&StateEvent::CallDeclared(ActionLabel::Kan))
    );
    assert!(
        !result
            .events
            .iter()
            .any(|event| matches!(event, StateEvent::Discard(_)))
    );
    let mut reconnected = GameTracker::new();
    assert_eq!(
        reconnected
            .observe_table(observation(1, after.clone(), vec![]), visible, vec![], kan)
            .phase,
        Phase::AwaitDraw
    );
    assert_eq!(reconnected.open_melds(), 1);
    assert_eq!(
        reconnected
            .observe_table(
                observation(2, after, vec![]),
                visible,
                vec![],
                tiles(&["1m", "2m", "4m"])
            )
            .phase,
        Phase::Uncertain
    );
}

#[test]
fn tracks_draw_discard_uncertainty_correction_and_round_end() {
    let mut tracker = GameTracker::new();
    let base = waiting_hand();
    let result = tracker.observe(observation(1, base.clone(), vec![]));
    assert_eq!(result.phase, Phase::AwaitDraw);
    assert!(result.events.is_empty());

    let mut drawn = base.clone();
    drawn.push(Tile::from_code("7z").unwrap());
    let result = tracker.observe(observation(2, drawn.clone(), vec![ActionLabel::Riichi]));
    assert_eq!(result.phase, Phase::AwaitDiscard);
    assert!(
        result
            .events
            .contains(&StateEvent::Draw(Tile::from_code("7z").unwrap()))
    );
    assert!(
        result
            .events
            .contains(&StateEvent::ActionAvailable(ActionLabel::Riichi))
    );
    assert!(!tracker.discard_recommendations().is_empty());

    let result = tracker.observe(observation(3, base.clone(), vec![]));
    assert_eq!(result.phase, Phase::AwaitDraw);
    assert!(
        result
            .events
            .contains(&StateEvent::Discard(Tile::from_code("7z").unwrap()))
    );

    let result = tracker.observe(observation(
        4,
        vec![Tile::from_code("1m").unwrap(); 13],
        vec![],
    ));
    assert_eq!(result.phase, Phase::Uncertain);
    assert!(tracker.discard_recommendations().is_empty());
    assert!(tracker.correct_hand(drawn, 0.99).is_ok());
    assert_eq!(tracker.phase(), Phase::AwaitDiscard);
    tracker.window_lost();
    assert_eq!(tracker.phase(), Phase::Uncertain);
    tracker.round_end();
    assert_eq!(tracker.phase(), Phase::RoundEnded);
}

#[test]
fn rejects_low_confidence_and_non_monotonic_frames() {
    let mut tracker = GameTracker::new();
    let mut low = observation(1, waiting_hand(), vec![]);
    low.confidence = 0.30;
    assert_eq!(tracker.observe(low).phase, Phase::Uncertain);
    assert_eq!(
        tracker
            .observe(observation(2, waiting_hand(), vec![]))
            .phase,
        Phase::AwaitDraw
    );
    assert_eq!(
        tracker
            .observe(observation(2, waiting_hand(), vec![]))
            .phase,
        Phase::Uncertain
    );
}

fn context(
    hand: Vec<Tile>,
    offered: Option<Tile>,
    source: ClaimSource,
    open_melds: u8,
) -> ActionContext {
    let mut visible = [0; 34];
    if let Some(tile) = offered {
        visible[tile.index() as usize] = 1;
    }
    ActionContext {
        hand,
        visible,
        dora_indicators: vec![],
        offered,
        source,
        open_melds,
        riichi_declared: false,
        known_yaku: false,
        confidence: 0.97,
    }
}

fn assert_comparison(advice: &ActionAdvice) {
    assert!((-1..=8).contains(&advice.before_shanten));
    assert!((-1..=8).contains(&advice.after_shanten));
    assert!(advice.before_ukeire <= 136);
    assert!(advice.after_ukeire <= 136);
    assert!(!advice.reason.is_empty());
}

#[test]
fn pon_and_chi_advice_compares_legal_before_and_after_states() {
    let pon_hand = tiles(&[
        "1m", "2m", "3m", "4m", "5m", "6m", "2p", "3p", "4p", "2s", "3s", "7z", "7z",
    ]);
    let pon = advise_action(
        &context(
            pon_hand,
            Some(Tile::from_code("7z").unwrap()),
            ClaimSource::Across,
            0,
        ),
        ActionLabel::Pon,
    )
    .unwrap();
    assert_eq!(pon.action, ActionLabel::Pon);
    assert_eq!(pon.tiles_from_hand.len(), 2);
    assert_comparison(&pon);

    let chi_hand = tiles(&[
        "1m", "3m", "4m", "5m", "6m", "2p", "3p", "4p", "2s", "3s", "4s", "7z", "7z",
    ]);
    let offered = Tile::from_code("2m").unwrap();
    let chi = advise_action(
        &context(chi_hand.clone(), Some(offered), ClaimSource::Left, 0),
        ActionLabel::Chi,
    )
    .unwrap();
    assert_eq!(chi.action, ActionLabel::Chi);
    assert_eq!(chi.tiles_from_hand.len(), 2);
    assert_comparison(&chi);
    assert!(!chi.recommended);
    assert!(chi.reason.contains("役"));
    assert!(
        advise_action(
            &context(chi_hand, Some(offered), ClaimSource::Right, 0),
            ActionLabel::Chi
        )
        .is_none()
    );
}

#[test]
fn kan_requires_three_matching_tiles_and_riichi_requires_closed_tenpai() {
    let kan_hand = tiles(&[
        "1m", "2m", "3m", "4m", "5m", "6m", "2p", "3p", "4p", "7z", "7z", "7z", "9p",
    ]);
    let kan = advise_action(
        &context(
            kan_hand,
            Some(Tile::from_code("7z").unwrap()),
            ClaimSource::Right,
            0,
        ),
        ActionLabel::Kan,
    )
    .unwrap();
    assert_eq!(kan.tiles_from_hand.len(), 3);
    assert_comparison(&kan);

    let mut riichi_hand = waiting_hand();
    riichi_hand.pop();
    riichi_hand.extend(tiles(&["7z", "9p"]));
    let riichi = advise_action(
        &context(riichi_hand.clone(), None, ClaimSource::SelfDraw, 0),
        ActionLabel::Riichi,
    )
    .unwrap();
    assert_eq!(riichi.after_shanten, 0);
    assert!(riichi.recommended);
    assert_comparison(&riichi);
    assert!(
        advise_action(
            &context(riichi_hand, None, ClaimSource::SelfDraw, 1),
            ActionLabel::Riichi
        )
        .is_none()
    );

    let mut completed = waiting_hand();
    completed.push(Tile::from_code("7z").unwrap());
    assert!(
        advise_action(
            &context(completed, None, ClaimSource::SelfDraw, 0),
            ActionLabel::Riichi
        )
        .is_none()
    );
}

#[test]
fn confirmed_calls_require_button_offer_and_legal_tiles() {
    let mut tracker = GameTracker::new();
    let pon_hand = tiles(&[
        "1m", "2m", "3m", "4m", "5m", "6m", "2p", "3p", "4p", "2s", "3s", "7z", "7z",
    ]);
    assert_eq!(
        tracker
            .observe(observation(1, pon_hand, vec![ActionLabel::Pon]))
            .phase,
        Phase::AwaitDraw
    );
    let offered = Tile::from_code("7z").unwrap();
    let mut visible = [0; 34];
    visible[offered.index() as usize] = 1;
    tracker.set_public_tiles(visible, vec![]).unwrap();
    assert!(
        tracker
            .confirm_call(
                ActionLabel::Chi,
                &[
                    Tile::from_code("1m").unwrap(),
                    Tile::from_code("2m").unwrap()
                ],
                offered,
                ClaimSource::Left
            )
            .is_err()
    );
    assert!(
        tracker
            .confirm_call(
                ActionLabel::Pon,
                &[
                    Tile::from_code("7z").unwrap(),
                    Tile::from_code("1m").unwrap()
                ],
                offered,
                ClaimSource::Across
            )
            .is_err()
    );
    let event = tracker
        .confirm_call(
            ActionLabel::Pon,
            &[offered, offered],
            offered,
            ClaimSource::Across,
        )
        .unwrap();
    assert_eq!(event, StateEvent::CallDeclared(ActionLabel::Pon));
    assert_eq!(tracker.open_melds(), 1);
    assert_eq!(tracker.phase(), Phase::AwaitDiscard);
    assert_eq!(tracker.hand().unwrap().len(), 11);
    let candidates = tracker.discard_recommendations();
    assert!(!candidates.is_empty());
    let chosen = candidates[0].discard;
    let mut after_discard = tracker.hand().unwrap().to_vec();
    after_discard.remove(
        after_discard
            .iter()
            .position(|tile| *tile == chosen)
            .unwrap(),
    );
    let result = tracker.observe(observation(2, after_discard, vec![]));
    assert_eq!(result.phase, Phase::AwaitDraw);
    assert!(result.events.contains(&StateEvent::Discard(chosen)));
}

#[test]
fn riichi_confirmation_needs_button_and_reconnect_accepts_new_frame_sequence() {
    let mut hand = waiting_hand();
    hand.pop();
    hand.extend(tiles(&["7z", "9p"]));
    let mut tracker = GameTracker::new();
    tracker.observe(observation(100, hand.clone(), vec![]));
    assert!(tracker.confirm_riichi().is_err());
    tracker.window_lost();
    assert_eq!(
        tracker
            .observe(observation(1, hand, vec![ActionLabel::Riichi]))
            .phase,
        Phase::AwaitDiscard
    );
    assert_eq!(
        tracker.confirm_riichi().unwrap(),
        StateEvent::RiichiDeclared
    );
    assert!(tracker.riichi_declared());
    assert!(tracker.discard_recommendations().is_empty());

    let mut completed = waiting_hand();
    completed.push(Tile::from_code("7z").unwrap());
    let mut completed_tracker = GameTracker::new();
    completed_tracker.observe(observation(1, completed, vec![ActionLabel::Riichi]));
    assert!(completed_tracker.confirm_riichi().is_err());
}
