CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS app_users (
  id BIGSERIAL PRIMARY KEY,
  username TEXT NOT NULL,
  password_hash TEXT NOT NULL DEFAULT '',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_app_users_username_lower
  ON app_users (LOWER(username));

CREATE TABLE IF NOT EXISTS user_settings (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
  config_name TEXT NOT NULL,
  payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_user_settings_user_config
  ON user_settings (user_id, config_name);

\set auth_secret `bash -lc 'if [[ -n "${AUTH_SECRET-}" ]]; then printf %s "$AUTH_SECRET"; elif [[ -f /home/heathcliff/polymarketbot/frontend/.env.local ]]; then grep -m1 "^AUTH_SECRET=" /home/heathcliff/polymarketbot/frontend/.env.local | cut -d= -f2- | tr -d "\r\n"; fi'`
\set bot_json `cd /home/heathcliff/polymarketbot/frontend && node -e "const fs=require('fs');const TOML=require('@iarna/toml');const p='/home/heathcliff/polymarketbot/config/bot.toml';const value=fs.existsSync(p)?TOML.parse(fs.readFileSync(p,'utf8')):{};process.stdout.write(JSON.stringify(value));"`
\set strategy_json `cd /home/heathcliff/polymarketbot/frontend && node -e "const fs=require('fs');const TOML=require('@iarna/toml');const p='/home/heathcliff/polymarketbot/config/strategy.toml';const value=fs.existsSync(p)?TOML.parse(fs.readFileSync(p,'utf8')):{};process.stdout.write(JSON.stringify(value));"`
\set risk_json `cd /home/heathcliff/polymarketbot/frontend && node -e "const fs=require('fs');const TOML=require('@iarna/toml');const p='/home/heathcliff/polymarketbot/config/risk.toml';const value=fs.existsSync(p)?TOML.parse(fs.readFileSync(p,'utf8')):{};process.stdout.write(JSON.stringify(value));"`
\set execution_json `cd /home/heathcliff/polymarketbot/frontend && node -e "const fs=require('fs');const TOML=require('@iarna/toml');const p='/home/heathcliff/polymarketbot/config/execution.toml';const value=fs.existsSync(p)?TOML.parse(fs.readFileSync(p,'utf8')):{};process.stdout.write(JSON.stringify(value));"`
\set exchange_json `cd /home/heathcliff/polymarketbot/frontend && node -e "const fs=require('fs');const crypto=require('crypto');const TOML=require('@iarna/toml');const p='/home/heathcliff/polymarketbot/config/exchange.toml';const value=fs.existsSync(p)?TOML.parse(fs.readFileSync(p,'utf8')):{};const prefix='enc:v1:';const encoded=(process.env.CONFIG_ENCRYPTION_KEY||'').trim();let key=null;if(encoded){try{const decoded=Buffer.from(encoded,'base64');if(decoded.length===32)key=decoded;}catch{}}const encrypt=(raw)=>{const text=String(raw??'').trim();if(!text||text.startsWith(prefix)||!key)return text;if(!text)return '';const nonce=crypto.randomBytes(12);const cipher=crypto.createCipheriv('aes-256-gcm',key,nonce);const encrypted=Buffer.concat([cipher.update(Buffer.from(text,'utf8')),cipher.final()]);const tag=cipher.getAuthTag();return prefix+Buffer.concat([nonce,encrypted,tag]).toString('base64');};for(const field of ['api_address','api_key','api_secret','api_passphrase','signer_private_key','gnosis_safe_address']){if(Object.prototype.hasOwnProperty.call(value,field)){value[field]=encrypt(value[field]);}}process.stdout.write(JSON.stringify(value));"`
\set telegram_json `cd /home/heathcliff/polymarketbot/frontend && node -e "const fs=require('fs');const crypto=require('crypto');const TOML=require('@iarna/toml');const p='/home/heathcliff/polymarketbot/config/telegram.toml';const value=fs.existsSync(p)?TOML.parse(fs.readFileSync(p,'utf8')):{};const prefix='enc:v1:';const encoded=(process.env.CONFIG_ENCRYPTION_KEY||'').trim();let key=null;if(encoded){try{const decoded=Buffer.from(encoded,'base64');if(decoded.length===32)key=decoded;}catch{}}const encrypt=(raw)=>{const text=String(raw??'').trim();if(!text||text.startsWith(prefix)||!key)return text;const nonce=crypto.randomBytes(12);const cipher=crypto.createCipheriv('aes-256-gcm',key,nonce);const encrypted=Buffer.concat([cipher.update(Buffer.from(text,'utf8')),cipher.final()]);const tag=cipher.getAuthTag();return prefix+Buffer.concat([nonce,encrypted,tag]).toString('base64');};if(Object.prototype.hasOwnProperty.call(value,'bot_token')){value.bot_token=encrypt(value.bot_token);}process.stdout.write(JSON.stringify(value));"`

INSERT INTO app_users (username, password_hash, created_at, updated_at)
SELECT
  'heathcliffeth',
  CASE
    WHEN LENGTH(:'auth_secret') > 0 THEN crypt(:'auth_secret', gen_salt('bf', 10))
    ELSE ''
  END,
  NOW(),
  NOW()
WHERE NOT EXISTS (
  SELECT 1
  FROM app_users
  WHERE LOWER(username) = 'heathcliffeth'
);

UPDATE app_users
SET
  password_hash = CASE
    WHEN password_hash = '' AND LENGTH(:'auth_secret') > 0
      THEN crypt(:'auth_secret', gen_salt('bf', 10))
    ELSE password_hash
  END,
  updated_at = NOW()
WHERE LOWER(username) = 'heathcliffeth';

ALTER TABLE trade_flow_definitions
  ADD COLUMN IF NOT EXISTS user_id BIGINT;

ALTER TABLE trade_flow_runs
  ADD COLUMN IF NOT EXISTS user_id BIGINT;

ALTER TABLE trade_builder_workflows
  ADD COLUMN IF NOT EXISTS user_id BIGINT;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS user_id BIGINT;

ALTER TABLE trades
  ADD COLUMN IF NOT EXISTS user_id BIGINT;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'fk_trade_flow_definitions_user'
  ) THEN
    ALTER TABLE trade_flow_definitions
      ADD CONSTRAINT fk_trade_flow_definitions_user
      FOREIGN KEY (user_id) REFERENCES app_users(id) ON DELETE CASCADE;
  END IF;
END $$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'fk_trade_flow_runs_user'
  ) THEN
    ALTER TABLE trade_flow_runs
      ADD CONSTRAINT fk_trade_flow_runs_user
      FOREIGN KEY (user_id) REFERENCES app_users(id) ON DELETE CASCADE;
  END IF;
END $$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'fk_trade_builder_workflows_user'
  ) THEN
    ALTER TABLE trade_builder_workflows
      ADD CONSTRAINT fk_trade_builder_workflows_user
      FOREIGN KEY (user_id) REFERENCES app_users(id) ON DELETE CASCADE;
  END IF;
END $$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'fk_trade_builder_orders_user'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT fk_trade_builder_orders_user
      FOREIGN KEY (user_id) REFERENCES app_users(id) ON DELETE CASCADE;
  END IF;
END $$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'fk_trades_user'
  ) THEN
    ALTER TABLE trades
      ADD CONSTRAINT fk_trades_user
      FOREIGN KEY (user_id) REFERENCES app_users(id) ON DELETE CASCADE;
  END IF;
END $$;

DO $$
DECLARE
  primary_user_id BIGINT;
BEGIN
  SELECT id
  INTO primary_user_id
  FROM app_users
  WHERE LOWER(username) = 'heathcliffeth'
  LIMIT 1;

  IF primary_user_id IS NULL THEN
    RAISE EXCEPTION 'Primary user heathcliffeth could not be created';
  END IF;

  UPDATE trade_flow_definitions
  SET user_id = primary_user_id
  WHERE user_id IS NULL;

  UPDATE trade_flow_runs r
  SET user_id = d.user_id
  FROM trade_flow_definitions d
  WHERE r.definition_id = d.id
    AND r.user_id IS NULL;

  UPDATE trade_builder_workflows
  SET user_id = primary_user_id
  WHERE user_id IS NULL;

  UPDATE trades
  SET user_id = primary_user_id
  WHERE user_id IS NULL;

  UPDATE trade_builder_orders o
  SET user_id = t.user_id
  FROM trades t
  WHERE o.trade_id = t.id
    AND o.user_id IS NULL;
END $$;

INSERT INTO user_settings (user_id, config_name, payload_json, created_at, updated_at)
SELECT u.id, seed.config_name, seed.payload_json, NOW(), NOW()
FROM app_users u
CROSS JOIN (
  VALUES
    ('bot', :'bot_json'::jsonb),
    ('strategy', :'strategy_json'::jsonb),
    ('risk', :'risk_json'::jsonb),
    ('execution', :'execution_json'::jsonb),
    ('exchange', :'exchange_json'::jsonb),
    ('telegram', :'telegram_json'::jsonb)
) AS seed(config_name, payload_json)
WHERE LOWER(u.username) = 'heathcliffeth'
ON CONFLICT (user_id, config_name) DO UPDATE
SET
  payload_json = EXCLUDED.payload_json,
  updated_at = NOW();

ALTER TABLE trade_flow_definitions
  ALTER COLUMN user_id SET NOT NULL;

ALTER TABLE trade_flow_runs
  ALTER COLUMN user_id SET NOT NULL;

ALTER TABLE trade_builder_workflows
  ALTER COLUMN user_id SET NOT NULL;

ALTER TABLE trade_builder_orders
  ALTER COLUMN user_id SET NOT NULL;

ALTER TABLE trades
  ALTER COLUMN user_id SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_trade_flow_definitions_user_updated
  ON trade_flow_definitions (user_id, updated_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_trade_flow_runs_user_created
  ON trade_flow_runs (user_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_trade_builder_workflows_user_created
  ON trade_builder_workflows (user_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_trade_builder_orders_user_created
  ON trade_builder_orders (user_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_trades_user_opened
  ON trades (user_id, opened_at DESC, id DESC);
