import * as fs from 'fs/promises';
import * as path from 'path';
import * as TOML from '@iarna/toml';
import {
  decryptConfigValue,
  encryptConfigValue,
  isEncryptedConfigValue,
} from '@/lib/crypto-config';

const CONFIG_DIR = process.env.BOT_CONFIG_DIR || '/home/heathcliff/polymarketbot/config';
const MASKED_SECRET = '********';

const EXCHANGE_SENSITIVE_FIELDS = [
  'api_address',
  'api_key',
  'api_secret',
  'api_passphrase',
] as const;
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
};

export function isAllowedFile(name: string): boolean {
  return name in ALLOWED_FILES;
}

export function isWritable(name: string): boolean {
  return ALLOWED_FILES[name]?.writable ?? false;
}

export async function readConfig(name: string): Promise<Record<string, unknown>> {
  const raw = await readRawConfig(name);
  if (name === 'exchange') {
    return sanitizeExchangeConfigForRead(raw);
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

export async function writeConfig(name: string, data: Record<string, unknown>): Promise<void> {
  if (!isWritable(name)) {
    throw new Error(`Config file '${name}' is read-only`);
  }

  if (name === 'exchange') {
    const existing = await readRawConfig(name);
    const normalized = normalizeExchangeConfigForWrite(data, existing);
    validateConfig(name, normalized);
    await writeRawConfig(name, normalized);
    return;
  }

  let coerced = data;
  if (name === 'strategy') coerced = coerceConfigTypes(data, STRATEGY_NUMERIC_KEYS, STRATEGY_BOOLEAN_KEYS);
  else if (name === 'risk') coerced = coerceConfigTypes(data, RISK_NUMERIC_KEYS, RISK_BOOLEAN_KEYS);
  else if (name === 'bot') coerced = coerceConfigTypes(data, BOT_NUMERIC_KEYS, new Set());

  validateConfig(name, coerced);
  await writeRawConfig(name, coerced);
}

export async function listConfigs(): Promise<string[]> {
  return Object.keys(ALLOWED_FILES);
}

export async function readExchangeApiAddressForServer(): Promise<string> {
  const raw = normalizeExchangeShape(await readRawConfig('exchange'));
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

export async function readPositionWalletAddress(): Promise<string> {
  const raw = normalizeExchangeShape(await readRawConfig('exchange'));

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

  return readExchangeApiAddressForServer();
}

async function readRawConfig(name: string): Promise<Record<string, unknown>> {
  const filePath = path.join(CONFIG_DIR, `${name}.toml`);
  const raw = await fs.readFile(filePath, 'utf-8');
  return TOML.parse(raw) as Record<string, unknown>;
}

async function writeRawConfig(name: string, data: Record<string, unknown>): Promise<void> {
  const filePath = path.join(CONFIG_DIR, `${name}.toml`);
  const tomlString = TOML.stringify(data as TOML.JsonMap);
  await fs.writeFile(filePath, tomlString, 'utf-8');
}

function sanitizeExchangeConfigForRead(raw: Record<string, unknown>): Record<string, unknown> {
  const sanitized = normalizeExchangeShape(raw);
  for (const field of EXCHANGE_SENSITIVE_FIELDS) {
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

  for (const field of EXCHANGE_SENSITIVE_FIELDS) {
    merged[field] = resolveSensitiveFieldForWrite(field, incoming, existing);
  }

  if (incoming.api_address_env !== undefined) {
    merged.api_address_env = String(incoming.api_address_env ?? '').trim();
  }
  if (incoming.api_key_env !== undefined) {
    merged.api_key_env = String(incoming.api_key_env ?? '').trim();
  }
  if (incoming.api_secret_env !== undefined) {
    merged.api_secret_env = String(incoming.api_secret_env ?? '').trim();
  }
  if (incoming.api_passphrase_env !== undefined) {
    merged.api_passphrase_env = String(incoming.api_passphrase_env ?? '').trim();
  }

  return merged;
}

function resolveSensitiveFieldForWrite(
  field: (typeof EXCHANGE_SENSITIVE_FIELDS)[number],
  incoming: Record<string, unknown>,
  existing: Record<string, unknown>
): string {
  const current = String(existing[field] ?? '').trim();
  const nextRaw = incoming[field];

  if (nextRaw === undefined) {
    return migrateExistingIfNeeded(current);
  }

  const next = String(nextRaw).trim();

  if (next === MASKED_SECRET) {
    return migrateExistingIfNeeded(current);
  }

  if (next === '') {
    return '';
  }

  if (isEncryptedConfigValue(next)) {
    return next;
  }

  return encryptConfigValue(next);
}

function migrateExistingIfNeeded(currentValue: string): string {
  if (currentValue === '') {
    return '';
  }
  if (isEncryptedConfigValue(currentValue)) {
    return currentValue;
  }
  return encryptConfigValue(currentValue);
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
    api_address_env: String(source.api_address_env ?? ''),
    api_key_env: String(source.api_key_env ?? ''),
    api_secret_env: String(source.api_secret_env ?? ''),
    api_passphrase_env: String(source.api_passphrase_env ?? ''),
    gnosis_safe_address: String(source.gnosis_safe_address ?? ''),
    gnosis_safe_address_env: String(source.gnosis_safe_address_env ?? ''),
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

    const apiAddress = String(data.api_address ?? '').trim();
    const apiKey = String(data.api_key ?? '').trim();
    const apiSecret = String(data.api_secret ?? '').trim();
    const apiPassphrase = String(data.api_passphrase ?? '').trim();
    const inlineAny =
      apiAddress.length > 0 ||
      apiKey.length > 0 ||
      apiSecret.length > 0 ||
      apiPassphrase.length > 0;

    if (inlineAny) {
      if (!apiAddress || !apiKey || !apiSecret || !apiPassphrase) {
        errors.push(
          'api_address, api_key, api_secret, api_passphrase must all be set when using direct credentials'
        );
      }
    } else {
      const addressEnv = String(data.api_address_env ?? '').trim();
      const keyEnv = String(data.api_key_env ?? '').trim();
      const secretEnv = String(data.api_secret_env ?? '').trim();
      const passphraseEnv = String(data.api_passphrase_env ?? '').trim();
      if (!addressEnv) errors.push('api_address_env is required when api_address is empty');
      if (!keyEnv) errors.push('api_key_env is required when api_key is empty');
      if (!secretEnv) errors.push('api_secret_env is required when api_secret is empty');
      if (!passphraseEnv) errors.push('api_passphrase_env is required when api_passphrase is empty');
    }
  }

  if (errors.length > 0) {
    throw new Error(`Validation failed: ${errors.join(', ')}`);
  }
}
