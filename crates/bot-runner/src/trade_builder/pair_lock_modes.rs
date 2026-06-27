const ACTION_PLACE_ORDER_MODE_SINGLE: &str = "single";
const ACTION_PLACE_ORDER_MODE_PAIR_LOCK: &str = "pair_lock";
const ACTION_PLACE_ORDER_MODE_DCA_LIVE_V1: &str = "dca_live_v1";
const ACTION_PLACE_ORDER_MODE_LIVE_GAP_COLLECTOR_V1: &str = "live_gap_collector_v1";

fn action_place_order_mode(node: &TradeFlowNode) -> &'static str {
    match node_config_string(node, "mode")
        .unwrap_or_else(|| ACTION_PLACE_ORDER_MODE_SINGLE.to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        ACTION_PLACE_ORDER_MODE_PAIR_LOCK => ACTION_PLACE_ORDER_MODE_PAIR_LOCK,
        ACTION_PLACE_ORDER_MODE_DCA_LIVE_V1 => ACTION_PLACE_ORDER_MODE_DCA_LIVE_V1,
        ACTION_PLACE_ORDER_MODE_LIVE_GAP_COLLECTOR_V1 => {
            ACTION_PLACE_ORDER_MODE_LIVE_GAP_COLLECTOR_V1
        }
        ACTION_PLACE_ORDER_MODE_POSITIVE_QUANTITY_FLIP_GRID_V1 => {
            ACTION_PLACE_ORDER_MODE_POSITIVE_QUANTITY_FLIP_GRID_V1
        }
        ACTION_PLACE_ORDER_MODE_POSITIVE_FLIP_PAIRLOCK_COMPRESSION_V1 => {
            ACTION_PLACE_ORDER_MODE_POSITIVE_FLIP_PAIRLOCK_COMPRESSION_V1
        }
        ACTION_PLACE_ORDER_MODE_REVENGE_FLIP_V1 => ACTION_PLACE_ORDER_MODE_REVENGE_FLIP_V1,
        ACTION_PLACE_ORDER_MODE_AVG_REBOUND_PAIRLOCK_RESCUE_V1 => {
            ACTION_PLACE_ORDER_MODE_AVG_REBOUND_PAIRLOCK_RESCUE_V1
        }
        ACTION_PLACE_ORDER_MODE_CONFIDENCE_LADDER_HEDGE_LOCK_V1 => {
            ACTION_PLACE_ORDER_MODE_CONFIDENCE_LADDER_HEDGE_LOCK_V1
        }
        _ => ACTION_PLACE_ORDER_MODE_SINGLE,
    }
}

fn action_place_order_uses_pair_lock(node: &TradeFlowNode) -> bool {
    action_place_order_mode(node) == ACTION_PLACE_ORDER_MODE_PAIR_LOCK
}

fn action_place_order_uses_dca_live(node: &TradeFlowNode) -> bool {
    action_place_order_mode(node) == ACTION_PLACE_ORDER_MODE_DCA_LIVE_V1
}

fn action_place_order_uses_live_gap_collector(node: &TradeFlowNode) -> bool {
    action_place_order_mode(node) == ACTION_PLACE_ORDER_MODE_LIVE_GAP_COLLECTOR_V1
}
