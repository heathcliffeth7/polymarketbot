CREATE OR REPLACE FUNCTION jsonb_deep_delete_keys(value JSONB, keys TEXT[])
RETURNS JSONB
LANGUAGE plpgsql
IMMUTABLE
AS $$
DECLARE
  result JSONB;
BEGIN
  IF value IS NULL THEN
    RETURN NULL;
  END IF;

  CASE jsonb_typeof(value)
    WHEN 'object' THEN
      SELECT COALESCE(
        jsonb_object_agg(entry.key, jsonb_deep_delete_keys(entry.value, keys)),
        '{}'::jsonb
      )
      INTO result
      FROM jsonb_each(value) AS entry
      WHERE NOT (entry.key = ANY(keys));

      RETURN result;
    WHEN 'array' THEN
      SELECT COALESCE(
        jsonb_agg(jsonb_deep_delete_keys(entry.value, keys)),
        '[]'::jsonb
      )
      INTO result
      FROM jsonb_array_elements(value) AS entry(value);

      RETURN result;
    ELSE
      RETURN value;
  END CASE;
END;
$$;

UPDATE trade_flow_runs
SET context_json = jsonb_deep_delete_keys(
  context_json,
  ARRAY[
    'severity_multiplier',
    'stop_loss_bump_severity_multiplier',
    'ptb_stop_loss_bump_last_severity_multiplier',
    'last_severity_multiplier'
  ]
)
WHERE context_json::text LIKE '%severity_multiplier%';

UPDATE trade_flow_run_steps
SET output_json = jsonb_deep_delete_keys(
  output_json,
  ARRAY[
    'severity_multiplier',
    'stop_loss_bump_severity_multiplier',
    'ptb_stop_loss_bump_last_severity_multiplier',
    'last_severity_multiplier'
  ]
)
WHERE output_json::text LIKE '%severity_multiplier%';

UPDATE trade_flow_node_runtime_snapshots
SET snapshot_json = jsonb_deep_delete_keys(
  snapshot_json,
  ARRAY[
    'severity_multiplier',
    'stop_loss_bump_severity_multiplier',
    'ptb_stop_loss_bump_last_severity_multiplier',
    'last_severity_multiplier'
  ]
)
WHERE snapshot_json::text LIKE '%severity_multiplier%';

UPDATE trade_builder_order_events
SET payload_json = jsonb_deep_delete_keys(
  payload_json,
  ARRAY[
    'severity_multiplier',
    'stop_loss_bump_severity_multiplier',
    'ptb_stop_loss_bump_last_severity_multiplier',
    'last_severity_multiplier'
  ]
)
WHERE payload_json::text LIKE '%severity_multiplier%';

UPDATE trade_flow_events
SET payload_json = jsonb_deep_delete_keys(
  payload_json,
  ARRAY[
    'severity_multiplier',
    'stop_loss_bump_severity_multiplier',
    'ptb_stop_loss_bump_last_severity_multiplier',
    'last_severity_multiplier'
  ]
)
WHERE payload_json::text LIKE '%severity_multiplier%';

DROP FUNCTION IF EXISTS jsonb_deep_delete_keys(JSONB, TEXT[]);
