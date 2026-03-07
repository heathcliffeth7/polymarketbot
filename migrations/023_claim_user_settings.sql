CREATE EXTENSION IF NOT EXISTS pgcrypto;

\set claim_json `cd /home/heathcliff/polymarketbot/frontend && node -e "const fs=require('fs');const crypto=require('crypto');const TOML=require('@iarna/toml');const p='/home/heathcliff/polymarketbot/config/claim.toml';const value=fs.existsSync(p)?TOML.parse(fs.readFileSync(p,'utf8')):{};const prefix='enc:v1:';const encoded=(process.env.CONFIG_ENCRYPTION_KEY||'').trim();let key=null;if(encoded){try{const decoded=Buffer.from(encoded,'base64');if(decoded.length===32)key=decoded;}catch{}}const encrypt=(raw)=>{const text=String(raw??'').trim();if(!text||text.startsWith(prefix)||!key)return text;const nonce=crypto.randomBytes(12);const cipher=crypto.createCipheriv('aes-256-gcm',key,nonce);const encrypted=Buffer.concat([cipher.update(Buffer.from(text,'utf8')),cipher.final()]);const tag=cipher.getAuthTag();return prefix+Buffer.concat([nonce,encrypted,tag]).toString('base64');};if(Object.prototype.hasOwnProperty.call(value,'private_key')){value.private_key=encrypt(value.private_key);}process.stdout.write(JSON.stringify(value));"`

INSERT INTO user_settings (user_id, config_name, payload_json, created_at, updated_at)
SELECT u.id, 'claim', :'claim_json'::jsonb, NOW(), NOW()
FROM app_users u
WHERE LOWER(u.username) = 'heathcliffeth'
ON CONFLICT (user_id, config_name) DO UPDATE
SET
  payload_json = EXCLUDED.payload_json,
  updated_at = NOW();
