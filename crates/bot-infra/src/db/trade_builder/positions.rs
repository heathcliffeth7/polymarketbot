use super::super::*;

const TRADE_BUILDER_PARENT_POSITION_SELECT_COLUMNS: &str =
    "parent_builder_order_id, user_id, source_trade_id, market_slug, token_id, outcome_label, \
     baseline_qty, current_qty, last_fill_qty, last_fill_price, qty_source, created_at, updated_at";

fn map_trade_builder_parent_position_row(
    row: sqlx::postgres::PgRow,
) -> TradeBuilderParentPosition {
    TradeBuilderParentPosition {
        parent_builder_order_id: row.get("parent_builder_order_id"),
        user_id: row.get("user_id"),
        source_trade_id: row.get("source_trade_id"),
        market_slug: row.get("market_slug"),
        token_id: row.get("token_id"),
        outcome_label: row.get("outcome_label"),
        baseline_qty: row.get("baseline_qty"),
        current_qty: row.get("current_qty"),
        last_fill_qty: row.get("last_fill_qty"),
        last_fill_price: row.get("last_fill_price"),
        qty_source: row.get("qty_source"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

impl PostgresRepository {
    pub async fn get_trade_builder_parent_position(
        &self,
        parent_builder_order_id: i64,
    ) -> Result<Option<TradeBuilderParentPosition>> {
        let row = sqlx::query(&format!(
            "SELECT {TRADE_BUILDER_PARENT_POSITION_SELECT_COLUMNS} \
             FROM trade_builder_parent_positions \
             WHERE parent_builder_order_id = $1"
        ))
        .bind(parent_builder_order_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(map_trade_builder_parent_position_row))
    }

    pub async fn upsert_trade_builder_parent_position(
        &self,
        input: &TradeBuilderParentPositionInput,
    ) -> Result<TradeBuilderParentPosition> {
        let row = sqlx::query(&format!(
            "INSERT INTO trade_builder_parent_positions \
              (parent_builder_order_id, user_id, source_trade_id, market_slug, token_id, outcome_label, baseline_qty, current_qty, last_fill_qty, last_fill_price, qty_source, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NOW(), NOW()) \
             ON CONFLICT (parent_builder_order_id) DO UPDATE SET \
               user_id = EXCLUDED.user_id, \
               source_trade_id = EXCLUDED.source_trade_id, \
               market_slug = EXCLUDED.market_slug, \
               token_id = EXCLUDED.token_id, \
               outcome_label = EXCLUDED.outcome_label, \
               baseline_qty = EXCLUDED.baseline_qty, \
               current_qty = EXCLUDED.current_qty, \
               last_fill_qty = EXCLUDED.last_fill_qty, \
               last_fill_price = EXCLUDED.last_fill_price, \
               qty_source = EXCLUDED.qty_source, \
               updated_at = NOW() \
             RETURNING {TRADE_BUILDER_PARENT_POSITION_SELECT_COLUMNS}"
        ))
        .bind(input.parent_builder_order_id)
        .bind(input.user_id)
        .bind(input.source_trade_id)
        .bind(&input.market_slug)
        .bind(&input.token_id)
        .bind(&input.outcome_label)
        .bind(input.baseline_qty)
        .bind(input.current_qty)
        .bind(input.last_fill_qty)
        .bind(input.last_fill_price)
        .bind(&input.qty_source)
        .fetch_one(self.pool())
        .await?;

        Ok(map_trade_builder_parent_position_row(row))
    }

    pub async fn apply_trade_builder_parent_position_fill(
        &self,
        parent_builder_order_id: i64,
        filled_qty: f64,
        fill_price: Option<f64>,
        qty_source: &str,
    ) -> Result<Option<TradeBuilderParentPosition>> {
        let row = sqlx::query(&format!(
            "UPDATE trade_builder_parent_positions \
             SET current_qty = GREATEST(0, current_qty - $2), \
                 last_fill_qty = $2, \
                 last_fill_price = COALESCE($3, last_fill_price), \
                 qty_source = $4, \
                 updated_at = NOW() \
             WHERE parent_builder_order_id = $1 \
             RETURNING {TRADE_BUILDER_PARENT_POSITION_SELECT_COLUMNS}"
        ))
        .bind(parent_builder_order_id)
        .bind(filled_qty)
        .bind(fill_price)
        .bind(qty_source)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(map_trade_builder_parent_position_row))
    }

    pub async fn get_trade_builder_parent_position_seed(
        &self,
        parent_builder_order_id: i64,
    ) -> Result<Option<TradeBuilderParentPositionSeed>> {
        let row = sqlx::query(
            "SELECT actual_visible_qty, expected_visible_qty, reference_price, qty_source \
             FROM trade_builder_inventory_observations \
             WHERE parent_builder_order_id = $1 \
               AND observation_kind = 'first_visible_inventory' \
             ORDER BY created_at DESC \
             LIMIT 1",
        )
        .bind(parent_builder_order_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|row| TradeBuilderParentPositionSeed {
            actual_visible_qty: row.get("actual_visible_qty"),
            expected_visible_qty: row.get("expected_visible_qty"),
            reference_price: row.get("reference_price"),
            qty_source: row.get("qty_source"),
        }))
    }
}
