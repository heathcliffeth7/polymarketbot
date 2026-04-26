#[derive(Debug, Clone, PartialEq)]
struct TriggerMarketEntryTimingProfileSelection {
    index: usize,
    start_remaining_sec: i64,
    end_remaining_sec: i64,
    remaining_ms: i64,
    remaining_sec: i64,
    max_price: Option<f64>,
    price_to_beat_trigger_min_gap: Option<f64>,
    price_to_beat_trigger_max_gap: Option<f64>,
    size_usdc: Option<f64>,
}

fn trigger_market_entry_timing_profiles_enabled(node: &TradeFlowNode) -> bool {
    node.node_type == "trigger.market_price"
        && node_market_mode(node) == "auto_scope"
        && is_trade_flow_market_price_once_node(node)
        && node
            .config
            .get("entryTimingProfiles")
            .and_then(Value::as_array)
            .is_some_and(|profiles| !profiles.is_empty())
}

fn trigger_market_entry_timing_remaining_ms(
    market_slug: &str,
    evaluated_at: chrono::DateTime<chrono::Utc>,
) -> Option<i64> {
    let scope = find_updown_scope_by_slug(market_slug)?;
    let window_start = MarketCycleId(market_slug.to_string()).start_time()?;
    let window_end =
        window_start + chrono::Duration::seconds(updown_scope_window_seconds(scope));
    Some(
        window_end
            .signed_duration_since(evaluated_at)
            .num_milliseconds(),
    )
}

fn parse_trigger_market_entry_timing_profile_selection(
    index: usize,
    profile: &Value,
    remaining_ms: i64,
) -> Option<TriggerMarketEntryTimingProfileSelection> {
    let profile = profile.as_object()?;
    let start_remaining_sec = profile
        .get("startRemainingSec")
        .and_then(value_as_i64)
        .filter(|value| *value > 0)?;
    let end_remaining_sec = profile
        .get("endRemainingSec")
        .and_then(value_as_i64)
        .filter(|value| *value >= 0)?;
    if start_remaining_sec <= end_remaining_sec {
        return None;
    }

    let start_remaining_ms = start_remaining_sec.saturating_mul(1000);
    let end_remaining_ms = end_remaining_sec.saturating_mul(1000);
    if remaining_ms > start_remaining_ms || remaining_ms <= end_remaining_ms {
        return None;
    }

    let max_price = profile
        .get("maxPriceCent")
        .and_then(value_as_f64)
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 100.0)
        .map(|value| clamp_probability(value / 100.0));
    let price_to_beat_trigger_min_gap = profile
        .get("priceToBeatTriggerMinGap")
        .and_then(value_as_f64)
        .filter(|value| value.is_finite() && *value > 0.0);
    let price_to_beat_trigger_max_gap = profile
        .get("priceToBeatTriggerMaxGap")
        .and_then(value_as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
        .filter(|value| {
            price_to_beat_trigger_min_gap
                .map(|min_gap| *value >= min_gap)
                .unwrap_or(true)
        });
    let size_usdc = profile
        .get("sizeUsdc")
        .and_then(value_as_f64)
        .filter(|value| value.is_finite() && *value > 0.0);

    Some(TriggerMarketEntryTimingProfileSelection {
        index,
        start_remaining_sec,
        end_remaining_sec,
        remaining_ms,
        remaining_sec: if remaining_ms <= 0 {
            0
        } else {
            ((remaining_ms + 999) / 1000).max(0)
        },
        max_price,
        price_to_beat_trigger_min_gap,
        price_to_beat_trigger_max_gap,
        size_usdc,
    })
}

fn resolve_trigger_market_entry_timing_profile(
    node: &TradeFlowNode,
    market_slug: &str,
    evaluated_at: chrono::DateTime<chrono::Utc>,
) -> Option<TriggerMarketEntryTimingProfileSelection> {
    if !trigger_market_entry_timing_profiles_enabled(node) {
        return None;
    }
    let remaining_ms = trigger_market_entry_timing_remaining_ms(market_slug, evaluated_at)?;
    if remaining_ms <= 0 {
        return None;
    }
    node.config
        .get("entryTimingProfiles")
        .and_then(Value::as_array)
        .and_then(|profiles| {
            profiles
                .iter()
                .enumerate()
                .find_map(|(index, profile)| {
                    parse_trigger_market_entry_timing_profile_selection(
                        index,
                        profile,
                        remaining_ms,
                    )
                })
        })
}

impl TriggerMarketEntryTimingProfileSelection {
    fn to_value(&self) -> Value {
        serde_json::json!({
            "index": self.index,
            "startRemainingSec": self.start_remaining_sec,
            "endRemainingSec": self.end_remaining_sec,
            "remainingMs": self.remaining_ms,
            "remainingSec": self.remaining_sec,
            "maxPrice": self.max_price,
            "maxPriceCent": self.max_price.map(|value| value * 100.0),
            "priceToBeatTriggerMinGap": self.price_to_beat_trigger_min_gap,
            "priceToBeatTriggerMaxGap": self.price_to_beat_trigger_max_gap,
            "sizeUsdc": self.size_usdc,
        })
    }
}

fn apply_trigger_market_entry_timing_context(
    context: &mut Value,
    selected_profile: Option<&TriggerMarketEntryTimingProfileSelection>,
) {
    if let Some(profile) = selected_profile {
        set_flow_context(context, "selectedEntryTimingProfile", profile.to_value());
        set_flow_context(
            context,
            "selectedEntryTimingProfileIndex",
            serde_json::json!(profile.index),
        );
        set_flow_context(
            context,
            "selectedEntryRemainingSec",
            serde_json::json!(profile.remaining_sec),
        );
        set_flow_context(
            context,
            "selectedEntryMaxPrice",
            serde_json::json!(profile.max_price),
        );
        set_flow_context(
            context,
            "selectedEntrySizeUsdc",
            serde_json::json!(profile.size_usdc),
        );
        return;
    }

    set_flow_context(context, "selectedEntryTimingProfile", Value::Null);
    set_flow_context(context, "selectedEntryTimingProfileIndex", Value::Null);
    set_flow_context(context, "selectedEntryRemainingSec", Value::Null);
    set_flow_context(context, "selectedEntryMaxPrice", Value::Null);
    set_flow_context(context, "selectedEntrySizeUsdc", Value::Null);
}

fn append_trigger_market_entry_timing_output_fields(target: &mut Value, context: &Value) {
    let Some(target_obj) = target.as_object_mut() else {
        return;
    };
    for key in [
        "selectedEntryTimingProfile",
        "selectedEntryTimingProfileIndex",
        "selectedEntryRemainingSec",
        "selectedEntryMaxPrice",
        "selectedEntrySizeUsdc",
    ] {
        if let Some(value) = flow_context_value(context, key) {
            target_obj.insert(key.to_string(), value);
        }
    }
}
