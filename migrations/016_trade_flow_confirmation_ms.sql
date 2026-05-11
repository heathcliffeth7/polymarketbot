-- Move trigger.market_price confirmation duration from seconds to milliseconds.
-- Converts `confirmationSeconds` -> `confirmationMs` in both graph_json and
-- denormalized trade_flow_nodes rows.

UPDATE trade_flow_versions v
SET graph_json = jsonb_set(
  v.graph_json,
  '{nodes}',
  (
    SELECT jsonb_agg(
      CASE
        WHEN n.node->>'type' = 'trigger.market_price'
             AND (n.node->'config' ? 'confirmationSeconds')
          THEN jsonb_set(
            n.node,
            '{config}',
            ((n.node->'config') - 'confirmationSeconds'::text)
            || jsonb_build_object(
              'confirmationMs',
              CASE
                WHEN (n.node->'config'->>'confirmationSeconds') ~ '^-?[0-9]+([.][0-9]+)?$'
                  THEN to_jsonb(
                    GREATEST(
                      0,
                      ROUND(((n.node->'config'->>'confirmationSeconds')::numeric) * 1000)
                    )::bigint
                  )
                ELSE to_jsonb(50)
              END
            ),
            true
          )
        ELSE n.node
      END
    )
    FROM jsonb_array_elements(COALESCE(v.graph_json->'nodes', '[]'::jsonb)) AS n(node)
  ),
  true
)
WHERE EXISTS (
  SELECT 1
  FROM jsonb_array_elements(COALESCE(v.graph_json->'nodes', '[]'::jsonb)) AS n(node)
  WHERE n.node->>'type' = 'trigger.market_price'
    AND (n.node->'config' ? 'confirmationSeconds')
);

UPDATE trade_flow_nodes
SET config_json = (config_json - 'confirmationSeconds'::text)
  || jsonb_build_object(
    'confirmationMs',
    CASE
      WHEN (config_json->>'confirmationSeconds') ~ '^-?[0-9]+([.][0-9]+)?$'
        THEN to_jsonb(
          GREATEST(
            0,
            ROUND(((config_json->>'confirmationSeconds')::numeric) * 1000)
          )::bigint
        )
      ELSE to_jsonb(50)
    END
  )
WHERE node_type = 'trigger.market_price'
  AND (config_json ? 'confirmationSeconds');
