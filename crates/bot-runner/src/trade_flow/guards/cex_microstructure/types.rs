use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CexVenue {
    Binance,
    Coinbase,
    Hyperliquid,
    Bybit,
}

impl CexVenue {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Binance => "binance",
            Self::Coinbase => "coinbase",
            Self::Hyperliquid => "hyperliquid",
            Self::Bybit => "bybit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TakerSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CexTradeSample {
    pub(crate) venue: CexVenue,
    pub(crate) asset: String,
    pub(crate) timestamp_ms: i64,
    pub(crate) price: f64,
    pub(crate) size: f64,
    pub(crate) taker_side: TakerSide,
}

impl CexTradeSample {
    pub(crate) fn notional(&self) -> f64 {
        (self.price * self.size).max(0.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CexBookSample {
    pub(crate) venue: CexVenue,
    pub(crate) asset: String,
    pub(crate) timestamp_ms: i64,
    pub(crate) bid: f64,
    pub(crate) ask: f64,
    pub(crate) bid_size: Option<f64>,
    pub(crate) ask_size: Option<f64>,
    pub(crate) source: &'static str,
}

impl CexBookSample {
    pub(crate) fn mid(&self) -> f64 {
        (self.bid + self.ask) / 2.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CexImpulseSnapshot {
    pub(crate) side: Option<&'static str>,
    pub(crate) move_usd: f64,
    pub(crate) velocity_usd_per_sec: f64,
    pub(crate) taker_imbalance: f64,
    pub(crate) trade_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CexSourceSnapshot {
    pub(crate) venue: CexVenue,
    pub(crate) mid: f64,
    pub(crate) bid: f64,
    pub(crate) ask: f64,
    pub(crate) book_staleness_ms: i64,
    pub(crate) trade_staleness_ms: i64,
    pub(crate) ticker_staleness_ms: i64,
    pub(crate) impulse: CexImpulseSnapshot,
}

impl CexSourceSnapshot {
    pub(crate) fn to_value(&self) -> Value {
        json!({
            "venue": self.venue.as_str(),
            "mid": self.mid,
            "bid": self.bid,
            "ask": self.ask,
            "book_staleness_ms": self.book_staleness_ms,
            "trade_staleness_ms": self.trade_staleness_ms,
            "ticker_staleness_ms": self.ticker_staleness_ms,
            "impulse": {
                "side": self.impulse.side,
                "move_usd": self.impulse.move_usd,
                "velocity_usd_per_sec": self.impulse.velocity_usd_per_sec,
                "taker_imbalance": self.impulse.taker_imbalance,
                "trade_count": self.impulse.trade_count,
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CexCurrentPriceSnapshot {
    pub(crate) venue: CexVenue,
    pub(crate) mid: f64,
    pub(crate) bid: f64,
    pub(crate) ask: f64,
    pub(crate) book_staleness_ms: i64,
    pub(crate) ticker_staleness_ms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CexVenueDeltaSnapshot {
    pub(crate) venue: CexVenue,
    pub(crate) open_mid: f64,
    pub(crate) current_mid: f64,
    pub(crate) delta_usd: f64,
    pub(crate) side: Option<&'static str>,
    pub(crate) role: Option<&'static str>,
    pub(crate) directional_gap: Option<f64>,
    pub(crate) threshold_hit: Option<bool>,
    pub(crate) open_source: &'static str,
    pub(crate) open_timestamp_ms: i64,
    pub(crate) current_timestamp_ms: i64,
    pub(crate) open_lag_ms: i64,
    pub(crate) book_staleness_ms: i64,
}

impl CexVenueDeltaSnapshot {
    pub(crate) fn to_value(&self) -> Value {
        json!({
            "venue": self.venue.as_str(),
            "open_mid": self.open_mid,
            "current_mid": self.current_mid,
            "delta_usd": self.delta_usd,
            "side": self.side,
            "role": self.role,
            "directional_gap": self.directional_gap,
            "threshold_hit": self.threshold_hit,
            "open_source": self.open_source,
            "open_timestamp_ms": self.open_timestamp_ms,
            "current_timestamp_ms": self.current_timestamp_ms,
            "open_lag_ms": self.open_lag_ms,
            "book_staleness_ms": self.book_staleness_ms,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CexConsensusSnapshot {
    pub(crate) asset: String,
    pub(crate) binance: CexSourceSnapshot,
    pub(crate) coinbase: CexSourceSnapshot,
    pub(crate) consensus_side: Option<&'static str>,
    pub(crate) spot_mid: f64,
    pub(crate) source_skew_usd: f64,
    pub(crate) baseline_source_skew_usd: Option<f64>,
    pub(crate) normalized_source_skew_usd: f64,
}

impl CexConsensusSnapshot {
    pub(crate) fn to_value(&self) -> Value {
        json!({
            "asset": self.asset,
            "binance": self.binance.to_value(),
            "coinbase": self.coinbase.to_value(),
            "cex_consensus_side": self.consensus_side,
            "spot_mid": self.spot_mid,
            "source_skew_usd": self.source_skew_usd,
            "baseline_source_skew_usd": self.baseline_source_skew_usd,
            "normalized_source_skew_usd": self.normalized_source_skew_usd,
        })
    }
}

pub(crate) fn parse_f64(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(value)) => value.as_f64(),
        Some(Value::String(value)) => value.parse::<f64>().ok(),
        _ => None,
    }
    .filter(|value| value.is_finite())
}

pub(crate) fn parse_i64(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(value)) => value.as_i64(),
        Some(Value::String(value)) => value.parse::<i64>().ok(),
        _ => None,
    }
}
