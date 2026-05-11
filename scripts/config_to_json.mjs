import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoDir = process.env.DEXTRABOT_REPO_DIR || path.resolve(scriptDir, '..');
const require = createRequire(path.join(repoDir, 'frontend', 'package.json'));
const TOML = require('@iarna/toml');

const configName = process.argv[2];
if (!configName || !/^[a-z_]+$/.test(configName)) {
  throw new Error('usage: node scripts/config_to_json.mjs <config-name>');
}

const sensitiveFieldsByConfig = {
  exchange: [
    'api_address',
    'api_key',
    'api_secret',
    'api_passphrase',
    'signer_private_key',
    'gnosis_safe_address',
  ],
  telegram: ['bot_token'],
  claim: ['private_key'],
};

const configPath = path.join(repoDir, 'config', `${configName}.toml`);
const examplePath = path.join(repoDir, 'config', `${configName}.toml.example`);
const sourcePath = fs.existsSync(configPath) ? configPath : examplePath;
const value = fs.existsSync(sourcePath)
  ? TOML.parse(fs.readFileSync(sourcePath, 'utf8'))
  : {};

const prefix = 'enc:v1:';
const encodedKey = String(process.env.CONFIG_ENCRYPTION_KEY || '').trim();
let key = null;
if (encodedKey) {
  try {
    const decoded = Buffer.from(encodedKey, 'base64');
    if (decoded.length === 32) key = decoded;
  } catch {
    key = null;
  }
}

function encryptIfNeeded(raw) {
  const text = String(raw ?? '').trim();
  if (!text || text.startsWith(prefix) || !key) return text;
  const nonce = crypto.randomBytes(12);
  const cipher = crypto.createCipheriv('aes-256-gcm', key, nonce);
  const encrypted = Buffer.concat([cipher.update(Buffer.from(text, 'utf8')), cipher.final()]);
  const tag = cipher.getAuthTag();
  return prefix + Buffer.concat([nonce, encrypted, tag]).toString('base64');
}

for (const field of sensitiveFieldsByConfig[configName] || []) {
  if (Object.prototype.hasOwnProperty.call(value, field)) {
    value[field] = encryptIfNeeded(value[field]);
  }
}

process.stdout.write(JSON.stringify(value));
