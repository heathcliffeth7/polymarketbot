use super::*;

pub(crate) const PRICE_TO_BEAT_GUARD_NOTIFICATION_SEED_KEY: &str = "lastGuardNotificationSeed";
const PRICE_TO_BEAT_GUARD_NOTIFICATION_STATE_KEY: &str = "priceToBeatGuardNotificationState";

pub(super) fn clear_price_to_beat_guard_waiting_context(context: &mut Value) {
    crate::set_flow_context(context, "priceToBeatGuardWaiting", Value::Null);
    crate::set_flow_context(context, "priceToBeatGuardWaitingReason", Value::Null);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PriceToBeatGuardNotificationPhase {
    BlockedNotified,
    PassedNotified,
}

impl PriceToBeatGuardNotificationPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::BlockedNotified => "blocked_notified",
            Self::PassedNotified => "passed_notified",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "blocked_notified" => Some(Self::BlockedNotified),
            "passed_notified" => Some(Self::PassedNotified),
            _ => None,
        }
    }
}

fn price_to_beat_guard_notification_identity(
    node_key: &str,
    market_slug: &str,
    token_id: &str,
) -> String {
    format!("{node_key}:{market_slug}:{token_id}")
}

pub(super) fn price_to_beat_guard_notification_phase(
    context: &Value,
    node_key: &str,
    market_slug: &str,
    token_id: &str,
) -> Option<PriceToBeatGuardNotificationPhase> {
    let state = crate::flow_context_value(context, PRICE_TO_BEAT_GUARD_NOTIFICATION_STATE_KEY)?;
    let identity = price_to_beat_guard_notification_identity(node_key, market_slug, token_id);
    let entry = state.get(&identity)?;
    let phase = entry.get("phase")?.as_str()?;
    PriceToBeatGuardNotificationPhase::parse(phase)
}

pub(super) fn set_price_to_beat_guard_notification_phase(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
    token_id: &str,
    phase: PriceToBeatGuardNotificationPhase,
    reason_code: &str,
) {
    let identity = price_to_beat_guard_notification_identity(node_key, market_slug, token_id);
    let mut state = crate::flow_context_value(context, PRICE_TO_BEAT_GUARD_NOTIFICATION_STATE_KEY)
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    state.insert(
        identity,
        json!({
            "phase": phase.as_str(),
            "reasonCode": reason_code,
        }),
    );
    crate::set_flow_context(
        context,
        PRICE_TO_BEAT_GUARD_NOTIFICATION_STATE_KEY,
        Value::Object(state),
    );
}

pub(crate) fn price_to_beat_guard_notification_seed_reason(
    context: &Value,
    node_key: &str,
    market_slug: &str,
    token_id: &str,
) -> Option<String> {
    let seed = crate::flow_context_value(context, PRICE_TO_BEAT_GUARD_NOTIFICATION_SEED_KEY)?;
    let seed_node_key = seed.get("nodeKey")?.as_str()?;
    let seed_market_slug = seed.get("marketSlug")?.as_str()?;
    let seed_token_id = seed.get("tokenId")?.as_str()?;
    let reason = seed.get("reason")?.as_str()?;
    if seed_node_key != node_key || seed_market_slug != market_slug || seed_token_id != token_id {
        return None;
    }
    Some(reason.to_string())
}

pub(super) fn set_price_to_beat_guard_notification_seed(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
    token_id: &str,
    reason: &str,
) {
    crate::set_flow_context(
        context,
        PRICE_TO_BEAT_GUARD_NOTIFICATION_SEED_KEY,
        json!({
            "nodeKey": node_key,
            "marketSlug": market_slug,
            "tokenId": token_id,
            "reason": reason,
        }),
    );
}

pub(crate) fn take_price_to_beat_guard_notification_seed(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
    token_id: &str,
) -> Option<String> {
    let reason =
        price_to_beat_guard_notification_seed_reason(context, node_key, market_slug, token_id)?;
    crate::set_flow_context(
        context,
        PRICE_TO_BEAT_GUARD_NOTIFICATION_SEED_KEY,
        Value::Null,
    );
    Some(reason)
}

pub(super) struct PriceToBeatGuardWaitingState {
    pub(super) market_slug: String,
    pub(super) reason_code: String,
}

pub(super) fn price_to_beat_guard_waiting_state(
    context: &Value,
) -> Option<PriceToBeatGuardWaitingState> {
    let obj = crate::flow_context_value(context, "priceToBeatGuardWaiting")?;
    let market_slug = obj.get("marketSlug")?.as_str()?.to_string();
    let reason_code = obj.get("reasonCode")?.as_str()?.to_string();
    if market_slug.is_empty() || reason_code.is_empty() {
        return None;
    }
    Some(PriceToBeatGuardWaitingState {
        market_slug,
        reason_code,
    })
}

pub(super) fn set_price_to_beat_guard_waiting_state(
    context: &mut Value,
    market_slug: &str,
    reason_code: &str,
) {
    crate::set_flow_context(
        context,
        "priceToBeatGuardWaiting",
        json!({ "marketSlug": market_slug, "reasonCode": reason_code }),
    );
    crate::set_flow_context(context, "priceToBeatGuardWaitingReason", Value::Null);
}
