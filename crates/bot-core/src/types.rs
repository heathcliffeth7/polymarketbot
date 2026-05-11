use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    Paper,
    Live,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TradeState {
    Idle,
    WaitingEntry,
    EntryPlaced,
    EntryPartiallyFilled,
    EntryFilled,
    TpPlaced,
    SlArmed,
    ExitPartiallyFilled,
    ExitFilled,
    Settled,
    Halted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderIntent {
    Entry,
    Tp,
    Sl,
    Renewal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionIntent {
    Entry,
    TakeProfit,
    AggressiveStop,
    CancelReplace,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Open,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExchangeRejectClass {
    NonceDrift,
    SignatureInvalid,
    InsufficientBalance,
    PolicyRejected,
    RateLimited,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReconnectState {
    Healthy,
    Reconnecting,
    BackfillRequired,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KillSwitchMode {
    Disabled,
    ManualOnly,
    ManualOrPolicy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RiskDecision {
    Allow,
    Block,
    Halt,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum LegSide {
    Yes,
    No,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegRuntime {
    pub side: LegSide,
    pub token_id: String,
    pub qty: f64,
    pub avg_entry: f64,
    pub levels_filled: u32,
    pub last_fill_price: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasketRuntime {
    pub market_slug: String,
    pub yes_leg: LegRuntime,
    pub no_leg: LegRuntime,
    pub basket_pnl_usdc: f64,
}
