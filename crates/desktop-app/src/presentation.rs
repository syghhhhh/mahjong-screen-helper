use game_state::ActionAdvice;
use vision_core::ActionLabel;

pub fn phase_label(phase: game_state::Phase) -> &'static str {
    match phase {
        game_state::Phase::Idle => "未开始",
        game_state::Phase::AwaitDraw => "等待摸牌",
        game_state::Phase::AwaitDiscard => "待切牌",
        game_state::Phase::Uncertain => "待确认",
        game_state::Phase::RoundEnded => "本局结束",
    }
}

pub struct ActionRow {
    pub text: String,
    pub recommended: Option<bool>,
}

pub fn action_rows(available: &[ActionLabel], advice: &[ActionAdvice]) -> Vec<ActionRow> {
    let mut labels = Vec::new();
    for label in available
        .iter()
        .copied()
        .chain(advice.iter().map(|item| item.action))
    {
        if label != ActionLabel::None && !labels.contains(&label) {
            labels.push(label);
        }
    }
    labels
        .into_iter()
        .map(|label| {
            let Some(item) = advice.iter().find(|item| item.action == label) else {
                return ActionRow {
                    text: format!("{}：信息不足，暂不建议", name(label)),
                    recommended: None,
                };
            };
            let action = if label == ActionLabel::Kan && item.reason.starts_with("暗杠") {
                "暗杠"
            } else if label == ActionLabel::Kan && item.reason.starts_with("加杠") {
                "加杠"
            } else {
                name(label)
            };
            let detail = match label {
                ActionLabel::Kan => "补牌未知",
                ActionLabel::Riichi => "切后听牌",
                _ if item.reason.contains("役种") => "役未确认",
                _ if item.recommended => "牌效改善",
                _ => "牌效未改善",
            };
            ActionRow {
                text: format!(
                    "{action} {} | {}→{} 向听，{}→{} 张 | {detail}",
                    if item.recommended {
                        "建议"
                    } else {
                        "暂不建议"
                    },
                    item.before_shanten,
                    item.after_shanten,
                    item.before_ukeire,
                    item.after_ukeire
                ),
                recommended: Some(item.recommended),
            }
        })
        .collect()
}

fn name(label: ActionLabel) -> &'static str {
    match label {
        ActionLabel::None => "无",
        ActionLabel::Chi => "吃",
        ActionLabel::Pon => "碰",
        ActionLabel::Kan => "杠",
        ActionLabel::Riichi => "立直",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advice(action: ActionLabel, recommended: bool, reason: &str) -> ActionAdvice {
        ActionAdvice {
            action,
            tiles_from_hand: vec![],
            discard_after_action: None,
            before_shanten: 1,
            after_shanten: 0,
            before_ukeire: 8,
            after_ukeire: 12,
            recommended,
            confidence: 0.99,
            reason: reason.into(),
        }
    }

    #[test]
    fn displays_every_available_action_with_an_explicit_decision() {
        let rows = action_rows(
            &[
                ActionLabel::Pon,
                ActionLabel::Kan,
                ActionLabel::Riichi,
                ActionLabel::None,
            ],
            &[
                advice(ActionLabel::Pon, true, "牌效改善"),
                advice(ActionLabel::Kan, false, "暗杠 1s：补牌未知"),
            ],
        );
        assert_eq!(rows.len(), 3);
        assert!(rows[0].text.contains("碰 建议"));
        assert_eq!(rows[0].recommended, Some(true));
        assert!(rows[1].text.contains("暗杠 暂不建议"));
        assert!(rows[1].text.contains("补牌未知"));
        assert_eq!(rows[1].recommended, Some(false));
        assert!(rows[2].text.contains("立直：信息不足，暂不建议"));
        assert_eq!(rows[2].recommended, None);
    }

    #[test]
    fn distinguishes_unconfirmed_yaku_and_deduplicates_buttons() {
        let rows = action_rows(
            &[ActionLabel::Chi, ActionLabel::Chi],
            &[advice(ActionLabel::Chi, false, "未确认役种，暂不建议鸣牌")],
        );
        assert_eq!(rows.len(), 1);
        assert!(rows[0].text.contains("役未确认"));
        assert!(rows[0].text.contains("1→0 向听，8→12 张"));
    }
}
