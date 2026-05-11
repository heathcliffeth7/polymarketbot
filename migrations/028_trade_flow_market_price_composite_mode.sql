WITH rewritten_versions AS (
  SELECT
    v.id,
    v.definition_id,
    jsonb_set(
      v.graph_json,
      '{nodes}',
      COALESCE(
        (
          SELECT jsonb_agg(
                   CASE
                     WHEN node->>'type' = 'trigger.market_price' THEN
                       jsonb_set(
                         jsonb_set(
                           node,
                           '{config}',
                           CASE
                             WHEN jsonb_typeof(node->'config') = 'object' THEN node->'config'
                             ELSE '{}'::jsonb
                           END,
                           true
                         ),
                         '{config,priceMode}',
                         to_jsonb('composite'::text),
                         true
                       )
                     ELSE node
                   END
                   ORDER BY ord
                 )
          FROM jsonb_array_elements(COALESCE(v.graph_json->'nodes', '[]'::jsonb))
            WITH ORDINALITY AS elems(node, ord)
        ),
        '[]'::jsonb
      ),
      true
    ) AS next_graph_json
  FROM trade_flow_versions v
  WHERE EXISTS (
    SELECT 1
    FROM jsonb_array_elements(COALESCE(v.graph_json->'nodes', '[]'::jsonb)) AS elems(node)
    WHERE node->>'type' = 'trigger.market_price'
  )
),
updated_versions AS (
  UPDATE trade_flow_versions v
  SET graph_json = rewritten.next_graph_json
  FROM rewritten_versions rewritten
  WHERE v.id = rewritten.id
    AND v.graph_json IS DISTINCT FROM rewritten.next_graph_json
  RETURNING rewritten.definition_id
)
UPDATE trade_flow_definitions d
SET updated_at = NOW()
WHERE d.id IN (SELECT DISTINCT definition_id FROM updated_versions);

UPDATE trade_flow_nodes
SET config_json = jsonb_set(
  CASE
    WHEN jsonb_typeof(config_json) = 'object' THEN config_json
    ELSE '{}'::jsonb
  END,
  '{priceMode}',
  to_jsonb('composite'::text),
  true
)
WHERE node_type = 'trigger.market_price'
  AND (
    jsonb_typeof(config_json) <> 'object'
    OR config_json->>'priceMode' IS DISTINCT FROM 'composite'
  );
