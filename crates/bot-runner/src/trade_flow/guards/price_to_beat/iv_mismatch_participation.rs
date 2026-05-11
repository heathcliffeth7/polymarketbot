use super::iv_mismatch_edge::PriceToBeatIvMismatchEdgeConfig;
use chrono::Utc;

pub(crate) fn price_to_beat_iv_participation_threshold_credit(
    config: &PriceToBeatIvMismatchEdgeConfig,
) -> f64 {
    if !config.participation_credit_enabled {
        return 0.0;
    }
    let Some(age_minutes) = config.participation_last_fill_age_minutes else {
        return 0.0;
    };
    if age_minutes >= config.participation_long_after_minutes.max(0.0) {
        config.participation_long_credit.max(0.0)
    } else if age_minutes >= config.participation_after_minutes.max(0.0) {
        config.participation_credit.max(0.0)
    } else {
        0.0
    }
}

pub(crate) async fn hydrate_price_to_beat_iv_participation(
    repo: Option<&crate::PostgresRepository>,
    flow_definition_id: Option<i64>,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    if !config.participation_credit_enabled {
        return;
    }
    let (Some(repo), Some(flow_definition_id)) = (repo, flow_definition_id) else {
        return;
    };
    match repo
        .latest_trade_builder_flow_entry_fill_at(flow_definition_id)
        .await
    {
        Ok(Some(filled_at)) => {
            config.participation_last_fill_age_minutes = Some(
                Utc::now()
                    .signed_duration_since(filled_at)
                    .num_seconds()
                    .max(0) as f64
                    / 60.0,
            );
        }
        Ok(None) => {
            config.participation_last_fill_age_minutes =
                Some(config.participation_long_after_minutes.max(0.0))
        }
        Err(err) => tracing::debug!(
            error = ?err,
            flow_definition_id,
            "price_to_beat participation credit lookup failed"
        ),
    }
}
