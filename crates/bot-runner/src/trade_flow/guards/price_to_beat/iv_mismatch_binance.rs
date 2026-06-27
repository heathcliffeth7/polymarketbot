use super::iv_mismatch_edge::PriceToBeatIvMismatchEdgeConfig;
use super::iv_mismatch_math::normal_cdf;
use crate::trade_flow::guards::binance_price::get_binance_price_snapshot;

pub(super) struct BinanceDisagreement {
    pub(super) adverse: Option<f64>,
    pub(super) absolute: Option<f64>,
    pub(super) bucket: Option<&'static str>,
    pub(super) penalty: f64,
}

pub(super) fn evaluate_binance_disagreement_penalty(
    q_chain_adj: f64,
    q_binance: Option<f64>,
    config: &PriceToBeatIvMismatchEdgeConfig,
) -> BinanceDisagreement {
    let Some(q_binance) = q_binance else {
        return BinanceDisagreement {
            adverse: None,
            absolute: None,
            bucket: None,
            penalty: 0.0,
        };
    };
    let absolute = (q_chain_adj - q_binance).abs();
    let adverse = (q_chain_adj - q_binance).max(0.0);
    if config
        .large_binance_disagreement_threshold
        .filter(|threshold| adverse > *threshold)
        .is_some()
    {
        return BinanceDisagreement {
            adverse: Some(adverse),
            absolute: Some(absolute),
            bucket: Some("large"),
            penalty: config.large_binance_disagreement_penalty.max(0.0),
        };
    }
    if config
        .binance_disagreement_threshold
        .filter(|threshold| adverse > *threshold)
        .is_some()
    {
        return BinanceDisagreement {
            adverse: Some(adverse),
            absolute: Some(absolute),
            bucket: Some("small"),
            penalty: config.binance_disagreement_penalty.max(0.0),
        };
    }
    BinanceDisagreement {
        adverse: Some(adverse),
        absolute: Some(absolute),
        bucket: Some("none"),
        penalty: 0.0,
    }
}

pub(super) struct BinanceAdjustment {
    pub(super) q_final: f64,
    pub(super) q_binance: Option<f64>,
    pub(super) binance_price: Option<f64>,
    pub(super) binance_staleness_ms: Option<i64>,
    pub(super) status: String,
}

impl BinanceAdjustment {
    pub(super) fn is_fresh(&self) -> bool {
        self.status == "fresh_conservative_min"
    }

    pub(super) fn is_missing(&self) -> bool {
        self.status == "fail_open_stale" || self.status.starts_with("fail_open_unavailable:")
    }
}

pub(super) fn evaluate_binance_veto(
    asset: &str,
    side: &str,
    price_to_beat: f64,
    expected_move_eff: f64,
    q_chain_adj: f64,
    now_ms: i64,
    config: &PriceToBeatIvMismatchEdgeConfig,
) -> BinanceAdjustment {
    let snapshot = match get_binance_price_snapshot(asset, now_ms) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            return BinanceAdjustment {
                q_final: q_chain_adj,
                q_binance: None,
                binance_price: None,
                binance_staleness_ms: None,
                status: format!("fail_open_unavailable:{err}"),
            };
        }
    };
    if snapshot.staleness_ms > config.binance_stale_ms {
        return BinanceAdjustment {
            q_final: q_chain_adj,
            q_binance: None,
            binance_price: Some(snapshot.price),
            binance_staleness_ms: Some(snapshot.staleness_ms),
            status: "fail_open_stale".to_string(),
        };
    }

    let q_binance = normal_cdf(side_gap(side, snapshot.price, price_to_beat) / expected_move_eff);
    BinanceAdjustment {
        q_final: q_chain_adj.min(q_binance + config.binance_q_buffer.max(0.0)),
        q_binance: Some(q_binance),
        binance_price: Some(snapshot.price),
        binance_staleness_ms: Some(snapshot.staleness_ms),
        status: "fresh_conservative_min".to_string(),
    }
}

fn side_gap(side: &str, price: f64, price_to_beat: f64) -> f64 {
    if side == "up" {
        price - price_to_beat
    } else {
        price_to_beat - price
    }
}
