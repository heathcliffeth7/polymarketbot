macro_rules! append_action_place_order_flow_created_payload_fields {
    (
        $payload:expr, $run:expr, $node:expr, $source_trade_id:expr, $execution_mode:expr,
        $initial_status:expr, $sizing:expr, $eligible_after_at:expr, $eligible_before_at:expr,
        $trigger_sizes:expr, $selected_entry_timing_profile_value:expr, $buy_fill_lock:expr,
        $max_price:expr, $price_to_beat_guard_snapshot:expr, $guard_trigger_price:expr,
        $reentry_guard_resolution:expr, $trigger_price_guard_enabled:expr,
        $best_ask_floor_price:expr, $execution_floor_guard_enabled:expr,
        $ignored_existing_order:expr, $protection_output:expr, $effective_internal_mode:expr,
        $tp_enabled:expr, $tp_price:expr, $sl_enabled:expr, $sl_price:expr, $tp_rules:expr,
        $sl_rules:expr, $time_exit_rules:expr, $sl_trigger_price_mode:expr,
        $reenter_on_sl_hit:expr, $reentry_max_attempts:expr, $reentry_trigger_node_key:expr,
        $ptb_stop_loss_gap_usd:expr, $ptb_reference_price:expr, $ptb_stop_loss_rules:expr,
        $ptb_stop_loss_time_decay_mode:expr, $ptb_current_price_source:expr,
        $staged_sl_behavior:expr, $flags:expr, $price_to_beat_guard_notification_seed:expr,
        $should_inline_submit:expr, $runtime_snapshot:expr, $fresh_submit_lease_until:expr,
        $runtime_snapshot_for_persist:expr
    ) => {{
        $payload.insert("flow_run_id".to_string(), json!(($run).id));
        $payload.insert("node_key".to_string(), json!(($node).key));
        $payload.insert("source_trade_id".to_string(), json!($source_trade_id));
        $payload.insert("execution_mode".to_string(), json!($execution_mode));
        $payload.insert(
            "order_type".to_string(),
            json!(action_place_order_clob_order_type($node, $execution_mode)),
        );
        $payload.insert(
            "post_only".to_string(),
            json!(node_config_bool($node, "postOnly").unwrap_or(false)),
        );
        $payload.insert("initial_status".to_string(), json!($initial_status));
        $payload.insert("size_basis".to_string(), json!(($sizing).size_basis));
        $payload.insert("size_mode".to_string(), json!(($sizing).resolved_size_mode));
        $payload.insert("size_pct".to_string(), json!(($sizing).resolved_size_pct));
        $payload.insert("size_usdc".to_string(), json!(($sizing).size_usdc));
        $payload.insert("target_qty".to_string(), json!(($sizing).target_qty));
        $payload.insert("remaining_qty".to_string(), json!(($sizing).remaining_qty));
        $payload.insert(
            "eligible_after_at".to_string(),
            json!(($eligible_after_at).as_ref().map(|value| value.to_rfc3339())),
        );
        $payload.insert(
            "eligible_before_at".to_string(),
            json!(($eligible_before_at).as_ref().map(|value| value.to_rfc3339())),
        );
        $payload.insert("trigger_sizes".to_string(), json!($trigger_sizes));
        $payload.insert(
            "selected_entry_timing_profile".to_string(),
            ($selected_entry_timing_profile_value).clone(),
        );
        $payload.insert("buy_fill_lock".to_string(), ($buy_fill_lock).clone());
        $payload.insert("max_price".to_string(), json!($max_price));
        $payload.insert(
            "price_to_beat_guard".to_string(),
            ($price_to_beat_guard_snapshot).clone(),
        );
        $payload.insert("guard_trigger_price".to_string(), json!($guard_trigger_price));
        $payload.insert(
            "reentry_band".to_string(),
            json!({"generation": ($reentry_guard_resolution).generation, "band_active": ($reentry_guard_resolution).band_active, "configured_min_price": ($reentry_guard_resolution).configured_min_price, "configured_max_price": ($reentry_guard_resolution).configured_max_price, "effective_guard_trigger_price": $guard_trigger_price, "effective_max_price": $max_price}),
        );
        $payload.insert(
            "trigger_price_guard_enabled".to_string(),
            json!($trigger_price_guard_enabled),
        );
        $payload.insert("best_ask_floor_price".to_string(), json!($best_ask_floor_price));
        $payload.insert(
            "execution_floor_guard_enabled".to_string(),
            json!($execution_floor_guard_enabled),
        );
        $payload.insert(
            "ignored_stale_existing_order".to_string(),
            json!(($ignored_existing_order).is_some()),
        );
        $payload.insert(
            "ignored_existing_order_id".to_string(),
            json!(($ignored_existing_order).as_ref().and_then(|(id, _, _)| *id)),
        );
        $payload.insert(
            "ignored_existing_order_reason".to_string(),
            json!(($ignored_existing_order).as_ref().map(|(_, reason, _)| *reason)),
        );
        $payload.insert(
            "ignored_existing_order_scope".to_string(),
            json!(($ignored_existing_order)
                .as_ref()
                .and_then(|(_, _, scope)| scope.map(|scope| scope.as_str()))),
        );
        $payload.insert("protection".to_string(), ($protection_output).clone());
        $payload.insert("internal_mode".to_string(), json!(($effective_internal_mode).clone()));
        $payload.insert("tp_enabled".to_string(), json!($tp_enabled));
        $payload.insert("tp_price".to_string(), json!($tp_price));
        $payload.insert("sl_enabled".to_string(), json!($sl_enabled));
        $payload.insert("sl_price".to_string(), json!($sl_price));
        $payload.insert("tp_rules".to_string(), serde_json::to_value(&$tp_rules)?);
        $payload.insert("sl_rules".to_string(), serde_json::to_value(&$sl_rules)?);
        $payload.insert(
            "time_exit_rules".to_string(),
            serde_json::to_value(&$time_exit_rules)?,
        );
        $payload.insert("sl_trigger_price_mode".to_string(), json!($sl_trigger_price_mode));
        $payload.insert("reenter_on_sl_hit".to_string(), json!($reenter_on_sl_hit));
        $payload.insert("reentry_max_attempts".to_string(), json!($reentry_max_attempts));
        $payload.insert(
            "reentry_trigger_node_key".to_string(),
            json!(($reentry_trigger_node_key).as_deref()),
        );
        $payload.insert("ptb_stop_loss_gap_usd".to_string(), json!($ptb_stop_loss_gap_usd));
        $payload.insert("ptb_reference_price".to_string(), json!($ptb_reference_price));
        $payload.insert(
            "ptb_stop_loss_rules".to_string(),
            serde_json::to_value(&$ptb_stop_loss_rules)?,
        );
        $payload.insert(
            "ptb_stop_loss_time_decay_mode".to_string(),
            json!($ptb_stop_loss_time_decay_mode),
        );
        $payload.insert(
            "ptb_current_price_source".to_string(),
            json!($ptb_current_price_source),
        );
        append_action_place_order_ptb_oracle_lag_stop_payload(&mut $payload, $node);
        $payload.insert(
            "staged_sl_reentry_only_after_all_stages".to_string(),
            json!(($staged_sl_behavior).reentry_only_after_all_stages),
        );
        append_action_place_order_notification_and_retry_payload(&mut $payload, $flags);
        $payload.insert(
            "last_guard_notification_reason".to_string(),
            json!(($price_to_beat_guard_notification_seed).clone()),
        );
        $payload.insert("should_inline_submit".to_string(), json!($should_inline_submit));
        append_trade_builder_runtime_snapshot_payload(
            &mut $payload,
            ($runtime_snapshot).as_ref(),
            $fresh_submit_lease_until,
        );
        if ($runtime_snapshot_for_persist)
            .get(REVENGE_FLIP_RUNTIME_INTENT_KEY)
            .is_some()
        {
            $payload.insert(
                REVENGE_FLIP_RUNTIME_INTENT_KEY.to_string(),
                ($runtime_snapshot_for_persist)
                    .get(REVENGE_FLIP_RUNTIME_INTENT_KEY)
                    .cloned()
                    .unwrap_or(Value::Null),
            );
        }
    }};
}

macro_rules! append_action_place_order_execution_output_fields {
    (
        $output:expr, $node:expr, $builder_order_id:expr, $ref_key:expr, $source_trade_id:expr,
        $kind:expr, $side:expr, $execution_mode:expr, $market_slug:expr, $token_id:expr,
        $selected_entry_timing_profile_value:expr, $buy_fill_lock:expr, $max_price:expr,
        $price_to_beat_guard_snapshot:expr, $guard_trigger_price:expr,
        $reentry_guard_resolution:expr, $best_ask_floor_price:expr, $protection_output:expr,
        $sizing:expr, $tp_enabled:expr, $tp_price:expr, $sl_enabled:expr, $sl_price:expr,
        $effective_internal_mode:expr, $parent_builder_order_id:expr, $tp_rules:expr,
        $sl_rules:expr, $time_exit_rules:expr, $execution_floor_guard_enabled:expr,
        $sl_trigger_price_mode:expr, $reenter_on_sl_hit:expr, $reentry_max_attempts:expr,
        $reentry_trigger_node_key:expr, $ptb_stop_loss_gap_usd:expr, $ptb_reference_price:expr,
        $ptb_stop_loss_rules:expr, $ptb_stop_loss_time_decay_mode:expr,
        $ptb_current_price_source:expr, $staged_sl_behavior:expr, $flags:expr,
        $price_to_beat_guard_notification_seed:expr, $should_inline_submit:expr,
        $runtime_snapshot:expr, $fresh_submit_lease_until:expr
    ) => {{
        $output.insert("node_key".to_string(), json!(($node).key));
        $output.insert("builder_order_id".to_string(), json!($builder_order_id));
        $output.insert("ref_key".to_string(), json!($ref_key));
        $output.insert("source_trade_id".to_string(), json!($source_trade_id));
        $output.insert("kind".to_string(), json!($kind));
        $output.insert("side".to_string(), json!($side));
        $output.insert("execution_mode".to_string(), json!($execution_mode));
        $output.insert(
            "order_type".to_string(),
            json!(action_place_order_clob_order_type($node, $execution_mode)),
        );
        $output.insert("market_slug".to_string(), json!($market_slug));
        $output.insert("token_id".to_string(), json!($token_id));
        $output.insert(
            "selected_entry_timing_profile".to_string(),
            $selected_entry_timing_profile_value,
        );
        $output.insert("buy_fill_lock".to_string(), $buy_fill_lock);
        $output.insert("max_price".to_string(), json!($max_price));
        $output.insert(
            "price_to_beat_guard".to_string(),
            ($price_to_beat_guard_snapshot).clone(),
        );
        $output.insert("guard_trigger_price".to_string(), json!($guard_trigger_price));
        $output.insert(
            "reentry_band".to_string(),
            json!({"generation": ($reentry_guard_resolution).generation, "band_active": ($reentry_guard_resolution).band_active, "configured_min_price": ($reentry_guard_resolution).configured_min_price, "configured_max_price": ($reentry_guard_resolution).configured_max_price, "effective_guard_trigger_price": $guard_trigger_price, "effective_max_price": $max_price}),
        );
        $output.insert("best_ask_floor_price".to_string(), json!($best_ask_floor_price));
        $output.insert("protection".to_string(), $protection_output);
        $output.insert("size_basis".to_string(), json!(($sizing).size_basis));
        $output.insert("size_mode".to_string(), json!(($sizing).resolved_size_mode));
        $output.insert("size_pct".to_string(), json!(($sizing).resolved_size_pct));
        $output.insert("size_usdc".to_string(), json!(($sizing).size_usdc));
        $output.insert("target_qty".to_string(), json!(($sizing).target_qty));
        $output.insert("remaining_qty".to_string(), json!(($sizing).remaining_qty));
        $output.insert("tp_enabled".to_string(), json!($tp_enabled));
        $output.insert("tp_price".to_string(), json!($tp_price));
        $output.insert("sl_enabled".to_string(), json!($sl_enabled));
        $output.insert("sl_price".to_string(), json!($sl_price));
        $output.insert("internal_mode".to_string(), json!(($effective_internal_mode).clone()));
        $output.insert("parent_builder_order_id".to_string(), json!($parent_builder_order_id));
        $output.insert("tp_rules".to_string(), serde_json::to_value(&$tp_rules)?);
        $output.insert("sl_rules".to_string(), serde_json::to_value(&$sl_rules)?);
        $output.insert(
            "time_exit_rules".to_string(),
            serde_json::to_value(&$time_exit_rules)?,
        );
        $output.insert(
            "execution_floor_guard_enabled".to_string(),
            json!($execution_floor_guard_enabled),
        );
        $output.insert("sl_trigger_price_mode".to_string(), json!($sl_trigger_price_mode));
        $output.insert("reenter_on_sl_hit".to_string(), json!($reenter_on_sl_hit));
        $output.insert("reentry_max_attempts".to_string(), json!($reentry_max_attempts));
        $output.insert(
            "reentry_trigger_node_key".to_string(),
            json!(($reentry_trigger_node_key).as_deref()),
        );
        $output.insert("ptb_stop_loss_gap_usd".to_string(), json!($ptb_stop_loss_gap_usd));
        $output.insert("ptb_reference_price".to_string(), json!($ptb_reference_price));
        $output.insert(
            "ptb_stop_loss_rules".to_string(),
            serde_json::to_value(&$ptb_stop_loss_rules)?,
        );
        $output.insert(
            "ptb_stop_loss_time_decay_mode".to_string(),
            json!($ptb_stop_loss_time_decay_mode),
        );
        $output.insert(
            "ptb_current_price_source".to_string(),
            json!($ptb_current_price_source),
        );
        $output.insert(
            "staged_sl_reentry_only_after_all_stages".to_string(),
            json!(($staged_sl_behavior).reentry_only_after_all_stages),
        );
        append_action_place_order_notification_and_retry_payload(&mut $output, $flags);
        $output.insert(
            "last_guard_notification_reason".to_string(),
            json!(($price_to_beat_guard_notification_seed).clone()),
        );
        $output.insert("should_inline_submit".to_string(), json!($should_inline_submit));
        append_trade_builder_runtime_snapshot_payload(
            &mut $output,
            ($runtime_snapshot).as_ref(),
            $fresh_submit_lease_until,
        );
    }};
}
