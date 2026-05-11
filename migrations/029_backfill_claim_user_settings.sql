CREATE EXTENSION IF NOT EXISTS pgcrypto;

\set claim_json `node "${DEXTRABOT_REPO_DIR:-.}/scripts/config_to_json.mjs" claim`

INSERT INTO user_settings (user_id, config_name, payload_json, created_at, updated_at)
SELECT u.id, 'claim', :'claim_json'::jsonb, NOW(), NOW()
FROM app_users u
WHERE LOWER(u.username) = 'admin'
  AND NOT EXISTS (
    SELECT 1
    FROM user_settings us
    WHERE us.user_id = u.id
      AND us.config_name = 'claim'
  )
ON CONFLICT (user_id, config_name) DO NOTHING;
