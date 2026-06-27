pub(crate) const CONFIG_ENC_PREFIX: &str = "enc:v1:";
pub(crate) const CONFIG_ENC_NONCE_LEN: usize = 12;
pub(crate) const CONFIG_ENC_TAG_LEN: usize = 16;

pub(crate) const SUPPORTED_MARKET_SCOPE_SLUG_PREFIXES: [(&str, &str); 11] = [
    ("btc_5m_updown", "btc-updown-5m-"),
    ("btc_15m_updown", "btc-updown-15m-"),
    ("eth_5m_updown", "eth-updown-5m-"),
    ("eth_15m_updown", "eth-updown-15m-"),
    ("sol_5m_updown", "sol-updown-5m-"),
    ("sol_15m_updown", "sol-updown-15m-"),
    ("xrp_5m_updown", "xrp-updown-5m-"),
    ("xrp_15m_updown", "xrp-updown-15m-"),
    ("doge_5m_updown", "doge-updown-5m-"),
    ("bnb_5m_updown", "bnb-updown-5m-"),
    ("hype_5m_updown", "hype-updown-5m-"),
];

pub(crate) fn default_min_balance_usdc() -> f64 {
    5.0
}

pub(crate) fn default_entry_price() -> f64 {
    0.60
}

pub(crate) fn default_market_scope() -> String {
    "btc_5m_updown".to_string()
}

pub(crate) fn default_market_slug_override() -> String {
    String::new()
}

pub(crate) fn default_loop_interval_ms() -> u64 {
    1000
}

pub(crate) fn default_market_discovery_retry_interval_ms() -> u64 {
    5000
}

pub(crate) fn default_market_discovery_timeout_sec() -> u64 {
    0
}

pub(crate) fn default_market_selection() -> String {
    "latest_by_slug".to_string()
}

pub(crate) fn default_exchange_gamma_base_url() -> String {
    "https://gamma-api.polymarket.com".to_string()
}

pub(crate) fn default_exchange_clob_base_url() -> String {
    "https://clob.polymarket.com".to_string()
}

pub(crate) fn default_exchange_clob_ws_url() -> String {
    "wss://ws-subscriptions-clob.polymarket.com/ws/".to_string()
}

pub(crate) fn default_clob_order_warmup_enabled() -> bool {
    true
}

pub(crate) fn default_clob_order_warmup_interval_ms() -> u64 {
    25_000
}

pub(crate) fn default_clob_order_warmup_cooldown_ms() -> u64 {
    120_000
}

pub(crate) fn default_exchange_chain_id() -> u64 {
    137
}

pub(crate) fn default_exchange_ctf_exchange_address() -> String {
    "0xE111180000d2663C0091e4f400237545B87B996B".to_string()
}

pub(crate) fn default_neg_risk_ctf_exchange_address() -> String {
    "0xe2222d279d744050d28e00520010520000310F59".to_string()
}

pub(crate) fn default_tp_pct() -> f64 {
    0.12
}

pub(crate) fn default_base_sl_pct() -> f64 {
    0.08
}

pub(crate) fn default_aggressive_sl_pct() -> f64 {
    0.30
}

pub(crate) fn default_entry_window_sec() -> u64 {
    180
}

pub(crate) fn default_max_hold_sec() -> u64 {
    240
}

pub(crate) fn default_sl_renew_interval_ms() -> u64 {
    2000
}

pub(crate) fn default_max_price_relax_enabled() -> bool {
    true
}

pub(crate) fn default_total_notional_usdc() -> f64 {
    10.0
}

pub(crate) fn default_per_leg_initial_notional_usdc() -> f64 {
    5.0
}

pub(crate) fn default_dca_interval_sec() -> u64 {
    20
}

pub(crate) fn default_dca_step_pct() -> f64 {
    0.02
}

pub(crate) fn default_max_dca_levels_per_leg() -> u32 {
    3
}

pub(crate) fn default_leg_tp_pct() -> f64 {
    0.035
}

pub(crate) fn default_basket_tp_usdc() -> f64 {
    0.35
}

pub(crate) fn default_basket_sl_usdc() -> f64 {
    -0.60
}

pub(crate) fn default_force_flatten_sec_before_close() -> u64 {
    45
}

pub(crate) fn default_sl_bid_confirm_timeout_ms() -> u64 {
    5000
}

pub(crate) fn default_claim_rpc_url() -> String {
    "https://polygon-rpc.com".to_string()
}

pub(crate) fn default_claim_data_api_base_url() -> String {
    "https://data-api.polymarket.com".to_string()
}

pub(crate) fn default_claim_execution_mode() -> String {
    "direct".to_string()
}

pub(crate) fn default_claim_rpc_url_env() -> String {
    "CLAIM_RPC_URL".to_string()
}

pub(crate) fn default_claim_user_address_env() -> String {
    "POLYMARKET_ADDRESS".to_string()
}

pub(crate) fn default_claim_private_key_env() -> String {
    "CLAIMER_PRIVATE_KEY".to_string()
}

pub(crate) fn default_claim_chain_id() -> u64 {
    137
}

pub(crate) fn default_claim_ctf_contract_address() -> String {
    "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045".to_string()
}

pub(crate) fn default_claim_collateral_token_address() -> String {
    "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string()
}

pub(crate) fn default_claim_auto_activate_funds() -> bool {
    true
}

pub(crate) fn default_claim_activate_min_usdc() -> f64 {
    0.01
}

pub(crate) fn default_claim_usdce_token_address() -> String {
    "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_string()
}

pub(crate) fn default_claim_pusd_token_address() -> String {
    "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB".to_string()
}

pub(crate) fn default_claim_collateral_onramp_address() -> String {
    "0x93070a847efEf7F70739046A929D47a521F5B8ee".to_string()
}

pub(crate) fn default_claim_discovery_interval_sec() -> u64 {
    30
}

pub(crate) fn default_claim_positions_page_size() -> i64 {
    200
}

pub(crate) fn default_claim_positions_max_pages() -> i64 {
    5
}

pub(crate) fn default_claim_process_batch_size() -> i64 {
    10
}

pub(crate) fn default_claim_max_attempts() -> i32 {
    5
}

pub(crate) fn default_claim_retry_backoff_ms() -> u64 {
    10_000
}

pub(crate) fn default_claim_min_claim_usdc() -> f64 {
    0.0
}

pub(crate) fn supported_market_scope_names_csv() -> String {
    SUPPORTED_MARKET_SCOPE_SLUG_PREFIXES
        .iter()
        .map(|(scope, _)| *scope)
        .collect::<Vec<_>>()
        .join(", ")
}
