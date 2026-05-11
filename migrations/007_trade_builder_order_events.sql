CREATE TABLE IF NOT EXISTS trade_builder_order_events (
  id BIGSERIAL PRIMARY KEY,
  builder_order_id BIGINT NOT NULL REFERENCES trade_builder_orders(id) ON DELETE CASCADE,
  event_type TEXT NOT NULL,
  payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_trade_builder_order_events_order_time
  ON trade_builder_order_events(builder_order_id, created_at DESC);
