import * as fs from 'fs/promises';
import * as path from 'path';
import * as TOML from '@iarna/toml';
import { pool } from '@/lib/db';
import {
  decryptConfigValue,
  encryptConfigValue,
  isEncryptedConfigValue,
} from '@/lib/crypto-config';

const CONFIG_DIR = process.env.BOT_CONFIG_DIR || '/home/heathcliff/polymarketbot/config';
const MASKED_SECRET = '********';

const EXCHANGE_SENSITIVE_FIELDS = [
  'api_key',
  'api_secret',
  'api_passphrase',
  'signer_private_key',
] as const;
const CLAIM_SENSITIVE_FIELDS = ['private_key'] as const;
const TELEGRAM_SENSITIVE_FIELDS = ['bot_token'] as const;
const SUPPORTED_MARKET_SCOPES = [
  'btc_5m_updown',
  'btc_15m_updown',
  'eth_5m_updown',
  'eth_15m_updown',
  'sol_5m_updown',
  'sol_15m_updown',
  'xrp_5m_updown',
  'xrp_15m_updown',
] as const;
const SUPPORTED_MARKET_SLUG_PREFIXES = [
  'btc-updown-5m-',
  'btc-updown-15m-',
  'eth-updown-5m-',
  'eth-updown-15m-',
  'sol-updown-5m-',
  'sol-updown-15m-',
  'xrp-updown-5m-',
  'xrp-updown-15m-',
] as const;

const ALLOWED_FILES: Record<string, { writable: boolean }> = {
  bot: { writable: true },
  strategy: { writable: true },
  risk: { writable: true },
  execution: { writable: true },
  exchange: { writable: true },
  claim: { writable: true },
  telegram: { writable: true },
};

export interface UserConfigContext {
  userId: number;
  username: string;
}

export function isAllowedFile(name: string): boolean {
  return name in ALLOWED_FILES;
}

export function isWritable(name: string): boolean {
  return ALLOWED_FILES[name]?.writable ?? false;
}

export async function readConfig(
  name: string,
  context: UserConfigContext
): Promise<Record<string, unknown>> {
  const raw = await readRawConfig(name, context);
  if (name === 'exchange') {
    return sanitizeExchangeConfigForRead(raw);
  }
  if (name === 'claim') {
    return sanitizeClaimConfigForRead(raw);
  }
  if (name === 'telegram') {
    return sanitizeTelegramConfigForRead(raw);
  }
  return raw;
}

const STRATEGY_NUMERIC_KEYS = new Set([
  'entry_price', 'tp_pct', 'base_sl_pct', 'aggressive_sl_pct',
  'entry_window_sec', 'max_hold_sec', 'sl_renew_interval_ms',
  'total_notional_usdc', 'per_leg_initial_notional_usdc',
  'dca_interval_sec', 'dca_step_pct', 'max_dca_levels_per_leg',
  'leg_tp_pct', 'basket_tp_usdc', 'basket_sl_usdc',
  'force_flatten_sec_before_close',
]);
const STRATEGY_BOOLEAN_KEYS = new Set(['flow_only', 'dual_side_enabled']);

const RISK_NUMERIC_KEYS = new Set([
  'max_daily_loss_usdc', 'max_consecutive_losses',
  'max_notional_per_market_usdc', 'max_open_orders',
  'max_stale_data_ms', 'min_balance_usdc',
]);
const RISK_BOOLEAN_KEYS = new Set(['manual_kill_switch_active']);

const BOT_NUMERIC_KEYS = new Set([
  'loop_interval_ms', 'market_discovery_retry_interval_ms',
  'market_discovery_timeout_sec',
]);

function coerceConfigTypes(
  data: Record<string, unknown>,
  numericKeys: Set<string>,
  booleanKeys: Set<string>,
): Record<string, unknown> {
  const out: Record<string, unknown> = { ...data };
  for (const [k, v] of Object.entries(out)) {
    if (numericKeys.has(k)) {
      const n = Number(v);
      if (Number.isFinite(n)) out[k] = n;
    } else if (booleanKeys.has(k)) {
      if (typeof v === 'string') out[k] = v === 'true';
    }
  }
  return out;
}

export async function writeConfig(
  name: string,
  data: Record<string, unknown>,
  context: UserConfigContext
): Promise<void> {
  if (!isWritable(name)) {
    throw new Error(`Config file '${name}' is read-only`);
  }

  if (name === 'exchange') {
    const existing = await readRawConfig(name, context);
    const normalized = normalizeExchangeConfigForWrite(data, existing);
    validateConfig(name, normalized);
    await writeRawConfig(name, normalized, context);
    return;
  }

  if (name === 'claim') {
    const existing = await readRawConfig(name, context);
    const normalized = normalizeClaimConfigForWrite(data, existing);
    validateConfig(name, normalized);
    await writeRawConfig(name, normalized, context);
    return;
  }

  if (name === 'telegram') {
    const existing = await readRawConfig(name, context);
    const normalized = normalizeTelegramConfigForWrite(data, existing);
    validateConfig(name, normalized);
    await writeRawConfig(name, normalized, context);
    return;
  }

  let coerced = data;
  if (name === 'strategy') coerced = coerceConfigTypes(data, STRATEGY_NUMERIC_KEYS, STRATEGY_BOOLEAN_KEYS);
  else if (name === 'risk') coerced = coerceConfigTypes(data, RISK_NUMERIC_KEYS, RISK_BOOLEAN_KEYS);
  else if (name === 'bot') coerced = coerceConfigTypes(data, BOT_NUMERIC_KEYS, new Set());

  validateConfig(name, coerced);
  await writeRawConfig(name, coerced, context);
}

export async function listConfigs(): Promise<string[]> {
  return Object.keys(ALLOWED_FILES);
}

export async function seedUserConfigsFromFiles(context: UserConfigContext): Promise<void> {
  for (const name of Object.keys(ALLOWED_FILES)) {
    const existing = await readStoredUserConfig(context.userId, name);
    if (existing) {
      continue;
    }
    const seeded = await buildSeedConfigPayload(name);
    await upsertStoredUserConfig(context.userId, name, seeded);
  }
}

export async function readExchangeApiAddressForServer(
  context: UserConfigContext
): Promise<string> {
  const raw = normalizeExchangeShape(await readRawConfig('exchange', context));
  const envName = String(raw.api_address_env ?? '').trim();
  const fromEnv = envName ? String(process.env[envName] ?? '').trim() : '';
  if (fromEnv) return fromEnv;

  const inlineAddress = String(raw.api_address ?? '').trim();
  if (!inlineAddress) return '';
  if (!isEncryptedConfigValue(inlineAddress)) return inlineAddress;

  try {
    return decryptConfigValue(inlineAddress).trim();
  } catch {
    return '';
  }
}

export async function readPositionWalletAddress(
  context: UserConfigContext
): Promise<string> {
  const raw = normalizeExchangeShape(await readRawConfig('exchange', context));

  const safeEnvName = String(raw.gnosis_safe_address_env ?? '').trim();
  const safeFromEnv = safeEnvName ? String(process.env[safeEnvName] ?? '').trim() : '';
  if (safeFromEnv) return safeFromEnv;

  const safeInline = String(raw.gnosis_safe_address ?? '').trim();
  if (safeInline) {
    if (!isEncryptedConfigValue(safeInline)) return safeInline;
    try {
      const decrypted = decryptConfigValue(safeInline).trim();
      if (decrypted) return decrypted;
    } catch {}
  }

  return readExchangeApiAddressForServer(context);
}

export async function readTelegramBotTokenForServer(
  context: UserConfigContext
): Promise<string> {
  const raw = normalizeTelegramShape(await readRawConfig('telegram', context));
  const inlineToken = String(raw.bot_token ?? '').trim();
  if (!inlineToken) return '';
  if (!isEncryptedConfigValue(inlineToken)) return inlineToken;
  return decryptConfigValue(inlineToken).trim();
}

export async function readTelegramChatIdForServer(
  context: UserConfigContext
): Promise<string> {
  const raw = normalizeTelegramShape(await readRawConfig('telegram', context));
  return String(raw.chat_id ?? '').trim();
}

export function isValidTelegramChatTarget(rawValue: string): boolean {
  const value = rawValue.trim();
  if (!value) {
    return false;
  }

  if (/^-?\d+$/.test(value)) {
    return true;
  }

  return /^@[A-Za-z][A-Za-z0-9_]{4,}$/.test(value);
}

async function readRawConfig(
  name: string,
  context: UserConfigContext
): Promise<Record<string, unknown>> {
  const stored = await readStoredUserConfig(context.userId, name);
  if (stored) {
    return stored;
  }

  return buildDefaultUserConfig(name);
}

async function readFallbackFileConfig(name: string): Promise<Record<string, unknown>> {
  const filePath = path.join(CONFIG_DIR, `${name}.toml`);
  try {
    const raw = await fs.readFile(filePath, 'utf-8');
    return TOML.parse(raw) as Record<string, unknown>;
  } catch (err) {
    if (
      name === 'telegram' &&
      typeof err === 'object' &&
      err != null &&
      'code' in err &&
      err.code === 'ENOENT'
    ) {
      return {};
    }
    throw err;
  }
}

async function writeRawConfig(
  name: string,
  data: Record<string, unknown>,
  context: UserConfigContext
): Promise<void> {
  await upsertStoredUserConfig(context.userId, name, data);
}

async function readStoredUserConfig(
  userId: number,
  name: string
): Promise<Record<string, unknown> | null> {
  const result = await pool.query(
    `SELECT payload_json
     FROM user_settings
     WHERE user_id = $1 AND config_name = $2
     LIMIT 1`,
    [userId, name]
  );
  if ((result.rowCount ?? 0) === 0) {
    return null;
  }
  const payload = result.rows[0]?.payload_json;
  if (!payload || typeof payload !== 'object' || Array.isArray(payload)) {
    return {};
  }
  return payload as Record<string, unknown>;
}

async function upsertStoredUserConfig(
  userId: number,
  name: string,
  data: Record<string, unknown>
): Promise<void> {
  await pool.query(
    `INSERT INTO user_settings (user_id, config_name, payload_json, created_at, updated_at)
     VALUES ($1, $2, $3::jsonb, NOW(), NOW())
     ON CONFLICT (user_id, config_name) DO UPDATE SET
       payload_json = EXCLUDED.payload_json,
       updated_at = NOW()`,
    [userId, name, JSON.stringify(data)]
  );
}

function sanitizeExchangeConfigForRead(raw: Record<string, unknown>): Record<string, unknown> {
  const sanitized = normalizeExchangeShape(raw);
  sanitized.api_address = decryptReadableConfigValue(sanitized.api_address);
  sanitized.gnosis_safe_address = decryptReadableConfigValue(sanitized.gnosis_safe_address);
  for (const field of EXCHANGE_SENSITIVE_FIELDS) {
    const value = String(sanitized[field] ?? '').trim();
    const hasValue = value.length > 0;
    sanitized[`has_${field}`] = hasValue;
    sanitized[field] = hasValue ? MASKED_SECRET : '';
  }
  return sanitized;
}

function sanitizeClaimConfigForRead(raw: Record<string, unknown>): Record<string, unknown> {
  const sanitized = normalizeClaimShape(raw);
  sanitized.user_address = decryptReadableConfigValue(sanitized.user_address);
  for (const field of CLAIM_SENSITIVE_FIELDS) {
    const value = String(sanitized[field] ?? '').trim();
    const hasValue = value.length > 0;
    sanitized[`has_${field}`] = hasValue;
    sanitized[field] = hasValue ? MASKED_SECRET : '';
  }
  return sanitized;
}

function sanitizeTelegramConfigForRead(raw: Record<string, unknown>): Record<string, unknown> {
  const sanitized = normalizeTelegramShape(raw);
  for (const field of TELEGRAM_SENSITIVE_FIELDS) {
    const value = String(sanitized[field] ?? '').trim();
    const hasValue = value.length > 0;
    sanitized[`has_${field}`] = hasValue;
    sanitized[field] = hasValue ? MASKED_SECRET : '';
  }
  return sanitized;
}

function normalizeExchangeConfigForWrite(
  incoming: Record<string, unknown>,
  existing: Record<string, unknown>
): Record<string, unknown> {
  const merged: Record<string, unknown> = {
    ...existing,
    ...normalizeExchangeShape(existing),
  };

  if (incoming.gamma_base_url !== undefined) {
    merged.gamma_base_url = String(incoming.gamma_base_url ?? '').trim();
  }
  if (incoming.clob_base_url !== undefined) {
    merged.clob_base_url = String(incoming.clob_base_url ?? '').trim();
  }
  if (incoming.clob_ws_url !== undefined) {
    merged.clob_ws_url = String(incoming.clob_ws_url ?? '').trim();
  }
  if (incoming.chain_id !== undefined) {
    const chainId = Number(incoming.chain_id);
    merged.chain_id = Number.isFinite(chainId) ? chainId : 0;
  }
  if (incoming.ctf_exchange_address !== undefined) {
    merged.ctf_exchange_address = String(incoming.ctf_exchange_address ?? '').trim();
  }

  merged.api_address = resolvePlaintextFieldForWrite('api_address', incoming, existing);
  merged.gnosis_safe_address = resolvePlaintextFieldForWrite(
    'gnosis_safe_address',
    incoming,
    existing
  );

  for (const field of EXCHANGE_SENSITIVE_FIELDS) {
    merged[field] = resolveSensitiveFieldForWrite(field, incoming, existing);
  }

  if (
    String(merged.api_address ?? '').trim() &&
    String(merged.api_key ?? '').trim() &&
    String(merged.api_secret ?? '').trim() &&
    String(merged.api_passphrase ?? '').trim()
  ) {
    merged.api_address_env = '';
    merged.api_key_env = '';
    merged.api_secret_env = '';
    merged.api_passphrase_env = '';
  }
  if (String(merged.signer_private_key ?? '').trim()) {
    merged.signer_private_key_env = '';
  }

  return merged;
}

function normalizeClaimConfigForWrite(
  incoming: Record<string, unknown>,
  existing: Record<string, unknown>
): Record<string, unknown> {
  const merged: Record<string, unknown> = {
    ...existing,
    ...normalizeClaimShape(existing),
  };

  if (incoming.enabled !== undefined) {
    merged.enabled = Boolean(incoming.enabled);
  }
  if (incoming.rpc_url !== undefined) {
    merged.rpc_url = String(incoming.rpc_url ?? '').trim();
  }
  if (incoming.data_api_base_url !== undefined) {
    merged.data_api_base_url = String(incoming.data_api_base_url ?? '').trim();
  }
  merged.user_address = resolvePlaintextFieldForWrite('user_address', incoming, existing);
  merged.private_key = resolveSensitiveFieldForWrite('private_key', incoming, existing);

  if (incoming.chain_id !== undefined) {
    const chainId = Number(incoming.chain_id);
    merged.chain_id = Number.isFinite(chainId) ? chainId : 0;
  }
  if (incoming.ctf_contract_address !== undefined) {
    merged.ctf_contract_address = String(incoming.ctf_contract_address ?? '').trim();
  }
  if (incoming.collateral_token_address !== undefined) {
    merged.collateral_token_address = String(incoming.collateral_token_address ?? '').trim();
  }
  if (incoming.discovery_interval_sec !== undefined) {
    const value = Number(incoming.discovery_interval_sec);
    merged.discovery_interval_sec = Number.isFinite(value) ? value : 0;
  }
  if (incoming.positions_page_size !== undefined) {
    const value = Number(incoming.positions_page_size);
    merged.positions_page_size = Number.isFinite(value) ? value : 0;
  }
  if (incoming.positions_max_pages !== undefined) {
    const value = Number(incoming.positions_max_pages);
    merged.positions_max_pages = Number.isFinite(value) ? value : 0;
  }
  if (incoming.process_batch_size !== undefined) {
    const value = Number(incoming.process_batch_size);
    merged.process_batch_size = Number.isFinite(value) ? value : 0;
  }
  if (incoming.max_attempts !== undefined) {
    const value = Number(incoming.max_attempts);
    merged.max_attempts = Number.isFinite(value) ? value : 0;
  }
  if (incoming.retry_backoff_ms !== undefined) {
    const value = Number(incoming.retry_backoff_ms);
    merged.retry_backoff_ms = Number.isFinite(value) ? value : 0;
  }

  if (String(merged.rpc_url ?? '').trim()) {
    merged.rpc_url_env = '';
  }
  if (String(merged.user_address ?? '').trim()) {
    merged.user_address_env = '';
  }
  if (String(merged.private_key ?? '').trim()) {
    merged.private_key_env = '';
  }

  return merged;
}

function normalizeTelegramConfigForWrite(
  incoming: Record<string, unknown>,
  existing: Record<string, unknown>
): Record<string, unknown> {
  const merged: Record<string, unknown> = {
    ...existing,
    ...normalizeTelegramShape(existing),
  };

  merged.bot_token = resolveSensitiveFieldForWrite('bot_token', incoming, existing);
  if (incoming.chat_id !== undefined) {
    merged.chat_id = String(incoming.chat_id ?? '').trim();
  }

  return merged;
}

function resolveSensitiveFieldForWrite(
  field: string,
  incoming: Record<string, unknown>,
  existing: Record<string, unknown>
): string {
  const current = String(existing[field] ?? '').trim();
  const nextRaw = incoming[field];

  if (nextRaw === undefined) {
    return migrateExistingIfNeeded(field, current);
  }

  const next = String(nextRaw).trim();

  if (next === MASKED_SECRET) {
    return migrateExistingIfNeeded(field, current);
  }

  if (next === '') {
    return '';
  }

  if (isEncryptedConfigValue(next)) {
    return next;
  }

  return encryptConfigValue(normalizeSensitiveValueForWrite(field, next));
}

function resolvePlaintextFieldForWrite(
  field: string,
  incoming: Record<string, unknown>,
  existing: Record<string, unknown>
): string {
  if (incoming[field] !== undefined) {
    return String(incoming[field] ?? '').trim();
  }
  return decryptReadableConfigValue(existing[field]);
}

function migrateExistingIfNeeded(field: string, currentValue: string): string {
  if (currentValue === '') {
    return '';
  }
  if (isEncryptedConfigValue(currentValue)) {
    return currentValue;
  }
  return encryptConfigValue(normalizeSensitiveValueForWrite(field, currentValue));
}

function normalizeSensitiveValueForWrite(field: string, value: string): string {
  const trimmed = value.trim();
  if (!trimmed) {
    return '';
  }
  if (field !== 'api_secret') {
    return trimmed;
  }
  return normalizeBase64UrlSecretForStorage(trimmed);
}

function normalizeBase64UrlSecretForStorage(value: string): string {
  const trimmed = value.trim();
  const unpadded = trimmed.replace(/=+$/, '');
  if (!unpadded) {
    return '';
  }

  if (unpadded.length % 4 === 1 || !/^[A-Za-z0-9_-]+$/.test(unpadded)) {
    return trimmed;
  }

  const padded = `${unpadded}${'='.repeat((4 - (unpadded.length % 4)) % 4)}`;

  try {
    const decoded = Buffer.from(padded, 'base64url');
    const canonical = decoded
      .toString('base64')
      .replace(/\+/g, '-')
      .replace(/\//g, '_');
    if (canonical.replace(/=+$/, '') !== unpadded) {
      return trimmed;
    }
    return canonical;
  } catch {
    return trimmed;
  }
}

function normalizeExchangeShape(source: Record<string, unknown>): Record<string, unknown> {
  const chainId = Number(source.chain_id);
  return {
    gamma_base_url: String(source.gamma_base_url ?? ''),
    clob_base_url: String(source.clob_base_url ?? ''),
    clob_ws_url: String(source.clob_ws_url ?? ''),
    chain_id: Number.isFinite(chainId) ? chainId : 137,
    api_address: String(source.api_address ?? ''),
    api_key: String(source.api_key ?? ''),
    api_secret: String(source.api_secret ?? ''),
    api_passphrase: String(source.api_passphrase ?? ''),
    ctf_exchange_address: String(source.ctf_exchange_address ?? ''),
    signer_private_key: String(source.signer_private_key ?? ''),
    signer_private_key_env: String(source.signer_private_key_env ?? ''),
    api_address_env: String(source.api_address_env ?? ''),
    api_key_env: String(source.api_key_env ?? ''),
    api_secret_env: String(source.api_secret_env ?? ''),
    api_passphrase_env: String(source.api_passphrase_env ?? ''),
    gnosis_safe_address: String(source.gnosis_safe_address ?? ''),
    gnosis_safe_address_env: String(source.gnosis_safe_address_env ?? ''),
  };
}

function normalizeClaimShape(source: Record<string, unknown>): Record<string, unknown> {
  const chainId = Number(source.chain_id);
  const discoveryIntervalSec = Number(source.discovery_interval_sec);
  const positionsPageSize = Number(source.positions_page_size);
  const positionsMaxPages = Number(source.positions_max_pages);
  const processBatchSize = Number(source.process_batch_size);
  const maxAttempts = Number(source.max_attempts);
  const retryBackoffMs = Number(source.retry_backoff_ms);
  return {
    enabled: Boolean(source.enabled),
    rpc_url: String(source.rpc_url ?? ''),
    rpc_url_env: String(source.rpc_url_env ?? ''),
    data_api_base_url: String(source.data_api_base_url ?? ''),
    user_address: String(source.user_address ?? ''),
    user_address_env: String(source.user_address_env ?? ''),
    private_key: String(source.private_key ?? ''),
    private_key_env: String(source.private_key_env ?? ''),
    chain_id: Number.isFinite(chainId) ? chainId : 137,
    ctf_contract_address: String(source.ctf_contract_address ?? ''),
    collateral_token_address: String(source.collateral_token_address ?? ''),
    discovery_interval_sec: Number.isFinite(discoveryIntervalSec) ? discoveryIntervalSec : 30,
    positions_page_size: Number.isFinite(positionsPageSize) ? positionsPageSize : 200,
    positions_max_pages: Number.isFinite(positionsMaxPages) ? positionsMaxPages : 5,
    process_batch_size: Number.isFinite(processBatchSize) ? processBatchSize : 10,
    max_attempts: Number.isFinite(maxAttempts) ? maxAttempts : 5,
    retry_backoff_ms: Number.isFinite(retryBackoffMs) ? retryBackoffMs : 10000,
  };
}

function normalizeTelegramShape(source: Record<string, unknown>): Record<string, unknown> {
  return {
    bot_token: String(source.bot_token ?? ''),
    chat_id: String(source.chat_id ?? ''),
  };
}

function validateConfig(name: string, data: Record<string, unknown>): void {
  const errors: string[] = [];
  const n = (key: string): number => Number(data[key]);
  const requiredNumber = (key: string): number => {
    const value = n(key);
    if (!Number.isFinite(value)) errors.push(`${key} must be a valid number`);
    return value;
  };
  const isHexAddress = (value: string): boolean =>
    /^0x[a-fA-F0-9]{40}$/.test(value);
  const isHexPrivateKey = (value: string): boolean =>
    /^0x[a-fA-F0-9]{64}$/.test(value);

  if (name === 'strategy') {
    const ep = requiredNumber('entry_price');
    if (ep < 0 || ep > 1) errors.push('entry_price must be in [0, 1]');
    if (requiredNumber('tp_pct') <= 0) errors.push('tp_pct must be > 0');
    if (requiredNumber('aggressive_sl_pct') <= 0) errors.push('aggressive_sl_pct must be > 0');

    const dualEnabled = Boolean(data.dual_side_enabled);
    if (dualEnabled) {
      const totalNotional = requiredNumber('total_notional_usdc');
      const perLegInitialNotional = requiredNumber('per_leg_initial_notional_usdc');
      const dcaIntervalSec = requiredNumber('dca_interval_sec');
      const dcaStepPct = requiredNumber('dca_step_pct');
      const maxLevels = requiredNumber('max_dca_levels_per_leg');
      const legTpPct = requiredNumber('leg_tp_pct');
      const basketTp = requiredNumber('basket_tp_usdc');
      const basketSl = requiredNumber('basket_sl_usdc');
      const flattenSec = requiredNumber('force_flatten_sec_before_close');

      if (totalNotional <= 0) errors.push('total_notional_usdc must be > 0');
      if (perLegInitialNotional <= 0)
        errors.push('per_leg_initial_notional_usdc must be > 0');
      if (perLegInitialNotional * 2 > totalNotional)
        errors.push('per_leg_initial_notional_usdc * 2 must be <= total_notional_usdc');
      if (dcaIntervalSec <= 0) errors.push('dca_interval_sec must be > 0');
      if (maxLevels <= 0) errors.push('max_dca_levels_per_leg must be > 0');
      if (dcaStepPct <= 0 || dcaStepPct > 1)
        errors.push('dca_step_pct must be in (0, 1]');
      if (legTpPct <= 0 || legTpPct > 1)
        errors.push('leg_tp_pct must be in (0, 1]');
      if (basketTp <= 0) errors.push('basket_tp_usdc must be > 0');
      if (basketSl >= 0) errors.push('basket_sl_usdc must be < 0');
      if (flattenSec <= 0)
        errors.push('force_flatten_sec_before_close must be > 0');
    }
  }

  if (name === 'risk') {
    if (requiredNumber('max_notional_per_market_usdc') <= 0)
      errors.push('max_notional_per_market_usdc must be > 0');
    if (data.kill_switch_mode === 'disabled' && data.manual_kill_switch_active === true)
      errors.push('manual_kill_switch_active cannot be true when kill_switch_mode is disabled');
  }

  if (name === 'bot') {
    const marketScope = String(data.market_scope ?? '').trim();
    if (!SUPPORTED_MARKET_SCOPES.includes(marketScope as (typeof SUPPORTED_MARKET_SCOPES)[number])) {
      errors.push(`market_scope must be one of: ${SUPPORTED_MARKET_SCOPES.join(', ')}`);
    }
    const marketSlugOverride = String(data.market_slug_override ?? '').trim();
    const marketSlugOverrideLower = marketSlugOverride.toLowerCase();
    if (
      marketSlugOverride.length > 0 &&
      !SUPPORTED_MARKET_SLUG_PREFIXES.some((prefix) => marketSlugOverrideLower.includes(prefix))
    ) {
      errors.push('market_slug_override must contain a supported slug prefix (e.g. btc-updown-5m-, eth-updown-15m-)');
    }
    if (data.market_selection !== 'latest_by_slug')
      errors.push('market_selection must be latest_by_slug');
    if (requiredNumber('market_discovery_retry_interval_ms') < 500)
      errors.push('market_discovery_retry_interval_ms must be >= 500');
    if (requiredNumber('market_discovery_timeout_sec') < 0)
      errors.push('market_discovery_timeout_sec must be >= 0');
    if (requiredNumber('loop_interval_ms') < 100)
      errors.push('loop_interval_ms must be >= 100');
  }

  if (name === 'execution') {
    if (!data.order_type || (data.order_type as string).trim() === '')
      errors.push('order_type is required');
  }

  if (name === 'exchange') {
    const gammaBaseUrl = String(data.gamma_base_url ?? '').trim();
    const clobBaseUrl = String(data.clob_base_url ?? '').trim();
    const clobWsUrl = String(data.clob_ws_url ?? '').trim();
    if (!gammaBaseUrl.startsWith('http'))
      errors.push('gamma_base_url must start with http');
    if (!clobBaseUrl.startsWith('http'))
      errors.push('clob_base_url must start with http');
    if (!clobWsUrl.startsWith('ws'))
      errors.push('clob_ws_url must start with ws');
    if (requiredNumber('chain_id') <= 0)
      errors.push('chain_id must be > 0');
    if (!String(data.ctf_exchange_address ?? '').trim()) {
      errors.push('ctf_exchange_address is required');
    }

    const apiAddress = String(data.api_address ?? '').trim();
    const signerPrivateKey = String(data.signer_private_key ?? '').trim();
    const gnosisSafeAddress = String(data.gnosis_safe_address ?? '').trim();

    // Exchange settings auto-save while the user types. Do not require the
    // whole credential bundle here; runtime validation will reject incomplete
    // credentials when an execution path actually needs them.
    if (apiAddress && !isHexAddress(apiAddress)) {
      errors.push('api_address must be a valid 0x address');
    }
    if (gnosisSafeAddress && !isHexAddress(gnosisSafeAddress)) {
      errors.push('gnosis_safe_address must be a valid 0x address');
    }
    if (
      signerPrivateKey &&
      !isHexPrivateKey(signerPrivateKey) &&
      !isEncryptedConfigValue(signerPrivateKey)
    ) {
      errors.push('signer_private_key must be a valid 0x private key');
    }
  }

  if (name === 'claim') {
    const rpcUrl = String(data.rpc_url ?? '').trim();
    const rpcUrlEnv = String(data.rpc_url_env ?? '').trim();
    const dataApiBaseUrl = String(data.data_api_base_url ?? '').trim();
    const userAddress = String(data.user_address ?? '').trim();
    const userAddressEnv = String(data.user_address_env ?? '').trim();
    const privateKey = String(data.private_key ?? '').trim();
    const privateKeyEnv = String(data.private_key_env ?? '').trim();

    if (rpcUrl && !rpcUrl.startsWith('http')) {
      errors.push('rpc_url must start with http');
    }
    if (!dataApiBaseUrl.startsWith('http')) {
      errors.push('data_api_base_url must start with http');
    }
    if (requiredNumber('chain_id') <= 0) {
      errors.push('chain_id must be > 0');
    }
    if (requiredNumber('discovery_interval_sec') < 5) {
      errors.push('discovery_interval_sec must be >= 5');
    }
    if (requiredNumber('positions_page_size') <= 0) {
      errors.push('positions_page_size must be > 0');
    }
    if (requiredNumber('positions_max_pages') <= 0) {
      errors.push('positions_max_pages must be > 0');
    }
    if (requiredNumber('process_batch_size') <= 0) {
      errors.push('process_batch_size must be > 0');
    }
    if (requiredNumber('max_attempts') <= 0) {
      errors.push('max_attempts must be > 0');
    }
    if (requiredNumber('retry_backoff_ms') < 1000) {
      errors.push('retry_backoff_ms must be >= 1000');
    }

    const ctfContractAddress = String(data.ctf_contract_address ?? '').trim();
    const collateralTokenAddress = String(data.collateral_token_address ?? '').trim();
    if (!isHexAddress(ctfContractAddress)) {
      errors.push('ctf_contract_address must be a valid 0x address');
    }
    if (!isHexAddress(collateralTokenAddress)) {
      errors.push('collateral_token_address must be a valid 0x address');
    }
    if (userAddress && !isHexAddress(userAddress)) {
      errors.push('user_address must be a valid 0x address');
    }
    if (privateKey && !isHexPrivateKey(privateKey) && !isEncryptedConfigValue(privateKey)) {
      errors.push('private_key must be a valid 0x private key');
    }

    if (Boolean(data.enabled)) {
      if (!rpcUrl && !rpcUrlEnv) {
        errors.push('rpc_url is required when claim is enabled');
      }
      if (!userAddress && !userAddressEnv) {
        errors.push('user_address is required when claim is enabled');
      }
      if (!privateKey && !privateKeyEnv) {
        errors.push('private_key is required when claim is enabled');
      }
    }
  }

  if (name === 'telegram') {
    const botToken = String(data.bot_token ?? '').trim();
    const chatId = String(data.chat_id ?? '').trim();
    if (botToken === MASKED_SECRET) {
      errors.push('bot_token cannot be the masked placeholder');
    }
    if (chatId && !isValidTelegramChatTarget(chatId)) {
      errors.push('chat_id must be a Telegram chat ID like -1001234567890 or a @channelusername');
    }
  }

  if (errors.length > 0) {
    throw new Error(`Validation failed: ${errors.join(', ')}`);
  }
}

function decryptReadableConfigValue(rawValue: unknown): string {
  const value = String(rawValue ?? '').trim();
  if (!value) {
    return '';
  }
  if (!isEncryptedConfigValue(value)) {
    return value;
  }
  try {
    return decryptConfigValue(value).trim();
  } catch {
    return '';
  }
}

async function buildSeedConfigPayload(
  name: string
): Promise<Record<string, unknown>> {
  return buildDefaultUserConfig(name);
}

async function buildDefaultUserConfig(name: string): Promise<Record<string, unknown>> {
  const fallback = await readFallbackFileConfig(name).catch(() => ({}));
  if (name === 'exchange') {
    const normalized = normalizeExchangeShape(fallback);
    return {
      ...normalized,
      api_address: '',
      api_key: '',
      api_secret: '',
      api_passphrase: '',
      signer_private_key: '',
      gnosis_safe_address: '',
      api_address_env: '',
      api_key_env: '',
      api_secret_env: '',
      api_passphrase_env: '',
      signer_private_key_env: '',
      gnosis_safe_address_env: '',
    };
  }
  if (name === 'claim') {
    const normalized = normalizeClaimShape(fallback);
    return {
      ...normalized,
      enabled: false,
      user_address: '',
      user_address_env: '',
      private_key: '',
      private_key_env: '',
      rpc_url_env: '',
    };
  }
  if (name === 'telegram') {
    return {
      bot_token: '',
      chat_id: '',
    };
  }
  return fallback;
}
