const TRADE_FLOW_WS_FAST_PATH_STREAM_UNION_HEALTH_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Default)]
struct TradeFlowWsFastPathRefreshState {
    definition_signature: Option<u64>,
    token_signature: Option<u64>,
    last_stream_union_ensure_at: Option<Instant>,
}

static TRADE_FLOW_WS_FAST_PATH_REFRESH_STATE: LazyLock<
    StdMutex<TradeFlowWsFastPathRefreshState>,
> = LazyLock::new(|| StdMutex::new(TradeFlowWsFastPathRefreshState::default()));

fn trade_flow_ws_fast_path_definition_signature(
    definitions: &[TradeFlowDefinitionRuntime],
) -> u64 {
    let mut defs = definitions.iter().collect::<Vec<_>>();
    defs.sort_by_key(|definition| definition.id);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for definition in defs {
        definition.id.hash(&mut hasher);
        definition.user_id.hash(&mut hasher);
        definition.status.hash(&mut hasher);
        definition.published_version_id.hash(&mut hasher);
        definition.updated_at.timestamp_millis().hash(&mut hasher);
    }
    hasher.finish()
}

fn trade_flow_ws_fast_path_token_signature(cache: &TradeFlowWsFastPathCache) -> u64 {
    let mut tokens = cache.token_targets.keys().collect::<Vec<_>>();
    tokens.sort();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for token in tokens {
        token.hash(&mut hasher);
    }
    hasher.finish()
}

fn trade_flow_ws_fast_path_should_skip_rebuild(
    definitions: &[TradeFlowDefinitionRuntime],
    cache: &TradeFlowWsFastPathCache,
    refresh_required_now: bool,
) -> bool {
    if refresh_required_now || definitions.is_empty() || cache.run_specs.is_empty() {
        return false;
    }
    let signature = trade_flow_ws_fast_path_definition_signature(definitions);
    let Ok(state) = TRADE_FLOW_WS_FAST_PATH_REFRESH_STATE.lock() else {
        return false;
    };
    state.definition_signature == Some(signature)
}

fn trade_flow_ws_fast_path_note_rebuilt(
    definitions: &[TradeFlowDefinitionRuntime],
    cache: &TradeFlowWsFastPathCache,
) -> bool {
    let definition_signature = trade_flow_ws_fast_path_definition_signature(definitions);
    let token_signature = trade_flow_ws_fast_path_token_signature(cache);
    let now = Instant::now();
    let Ok(mut state) = TRADE_FLOW_WS_FAST_PATH_REFRESH_STATE.lock() else {
        return true;
    };
    let should_ensure = state.token_signature != Some(token_signature)
        || state
            .last_stream_union_ensure_at
            .map(|last| last.elapsed() >= TRADE_FLOW_WS_FAST_PATH_STREAM_UNION_HEALTH_TTL)
            .unwrap_or(true);
    state.definition_signature = Some(definition_signature);
    state.token_signature = Some(token_signature);
    if should_ensure {
        state.last_stream_union_ensure_at = Some(now);
    }
    should_ensure
}

fn trade_flow_ws_fast_path_note_skip_should_ensure(
    cache: &TradeFlowWsFastPathCache,
) -> bool {
    let token_signature = trade_flow_ws_fast_path_token_signature(cache);
    let now = Instant::now();
    let Ok(mut state) = TRADE_FLOW_WS_FAST_PATH_REFRESH_STATE.lock() else {
        return true;
    };
    let should_ensure = state.token_signature != Some(token_signature)
        || state
            .last_stream_union_ensure_at
            .map(|last| last.elapsed() >= TRADE_FLOW_WS_FAST_PATH_STREAM_UNION_HEALTH_TTL)
            .unwrap_or(true);
    state.token_signature = Some(token_signature);
    if should_ensure {
        state.last_stream_union_ensure_at = Some(now);
    }
    should_ensure
}

fn trade_flow_ws_fast_path_note_cleared() {
    if let Ok(mut state) = TRADE_FLOW_WS_FAST_PATH_REFRESH_STATE.lock() {
        *state = TradeFlowWsFastPathRefreshState::default();
    }
}

#[cfg(test)]
mod ws_fast_path_refresh_tests {
    use super::*;

    fn definition(version_id: i64) -> TradeFlowDefinitionRuntime {
        TradeFlowDefinitionRuntime {
            id: 4364,
            user_id: 1,
            name: "EQ77".to_string(),
            status: "published".to_string(),
            draft_version_id: None,
            published_version_id: Some(version_id),
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn cache() -> TradeFlowWsFastPathCache {
        let mut cache = TradeFlowWsFastPathCache::default();
        cache.run_specs.push(WsOpenPositionPriceRunSpec {
            run_id: 954,
            definition_id: 4364,
            version_id: 6156,
            version_no: 1,
            context: json!({}),
            nodes: Vec::new(),
            context_dirty: false,
        });
        cache
            .token_targets
            .insert("TOKEN".to_string(), vec![(0, 0)]);
        cache
    }

    #[test]
    fn fast_path_refresh_skip_requires_fresh_same_definition_signature() {
        trade_flow_ws_fast_path_note_cleared();
        let definitions = vec![definition(6156)];
        let cache = cache();

        assert!(!trade_flow_ws_fast_path_should_skip_rebuild(
            &definitions,
            &cache,
            false
        ));
        assert!(trade_flow_ws_fast_path_note_rebuilt(&definitions, &cache));
        assert!(trade_flow_ws_fast_path_should_skip_rebuild(
            &definitions,
            &cache,
            false
        ));
        assert!(!trade_flow_ws_fast_path_should_skip_rebuild(
            &[definition(6157)],
            &cache,
            false
        ));
        assert!(!trade_flow_ws_fast_path_should_skip_rebuild(
            &definitions,
            &cache,
            true
        ));
    }
}
