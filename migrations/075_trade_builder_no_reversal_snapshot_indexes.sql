CREATE INDEX IF NOT EXISTS idx_market_price_second_snapshots_asset_second
  ON market_price_second_snapshots (asset, second_ts)
  INCLUDE (
    market_slug,
    window_end,
    ptb_ref_price,
    chainlink_price,
    yes_best_ask,
    no_best_ask
  );

CREATE INDEX IF NOT EXISTS idx_market_price_second_snapshots_asset_market_second
  ON market_price_second_snapshots (asset, market_slug, second_ts);
