import * as fs from 'fs/promises';
import * as path from 'path';
import * as TOML from '@iarna/toml';
import { pool } from '@/lib/db';
import type {
  ClaimRelayerConfigForServer,
  ClaimRuntimeValidationState,
} from '@/lib/claim-relayer-config';
import {
  normalizeClaimExecutionMode,
  resolvePlaintextConfigValueForServer,
  resolveSensitiveConfigValueForServer,
} from '@/lib/claim-relayer-config';
import {
  decryptConfigValue,
  encryptConfigValue,
  isEncryptedConfigValue,
} from '@/lib/crypto-config';
import {
  normalizeClaimShape,
  normalizeExchangeShape,
  normalizeStrategyShape,
  normalizeTelegramShape,
  validateConfigShape,
} from '@/lib/config-shapes';

export { isValidTelegramChatTarget } from '@/lib/config-shapes';

const CONFIG_DIR = process.env.BOT_CONFIG_DIR || path.resolve(process.cwd(), '..', 'config');
const MASKED_SECRET = '********';

const EXCHANGE_SENSITIVE_FIELDS = [
  'api_key', 'api_secret', 'api_passphrase',
  'builder_api_key', 'builder_api_secret', 'builder_api_passphrase',
  'signer_private_key',
] as const;
const CLAIM_SENSITIVE_FIELDS = ['private_key', 'relayer_api_key'] as const;
const TELEGRAM_SENSITIVE_FIELDS = ['bot_token'] as const;

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
  if (name === 'strategy') {
    return normalizeStrategyShape(raw);
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
const STRATEGY_BOOLEAN_KEYS = new Set(['flow_only', 'dual_side_enabled', 'max_price_relax_enabled']);

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
    validateConfigShape(name, normalized);
    await writeRawConfig(name, normalized, context);
    return;
  }

  if (name === 'claim') {
    const existing = await readRawConfig(name, context);
    const normalized = normalizeClaimConfigForWrite(data, existing);
    validateConfigShape(name, normalized);
    await writeRawConfig(name, normalized, context);
    return;
  }

  if (name === 'telegram') {
    const existing = await readRawConfig(name, context);
    const normalized = normalizeTelegramConfigForWrite(data, existing);
    validateConfigShape(name, normalized);
    await writeRawConfig(name, normalized, context);
    return;
  }

  let coerced = data;
  if (name === 'strategy') coerced = coerceConfigTypes(data, STRATEGY_NUMERIC_KEYS, STRATEGY_BOOLEAN_KEYS);
  else if (name === 'risk') coerced = coerceConfigTypes(data, RISK_NUMERIC_KEYS, RISK_BOOLEAN_KEYS);
  else if (name === 'bot') coerced = coerceConfigTypes(data, BOT_NUMERIC_KEYS, new Set());

  validateConfigShape(name, coerced);
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
  const safeValue = resolvePlaintextConfigValueForServer(
    raw.gnosis_safe_address,
    raw.gnosis_safe_address_env
  );
  if (safeValue) return safeValue;
  return readExchangeApiAddressForServer(context);
}

export interface DataApiActivityConfigForServer {
  baseUrl: string;
  pageSize: number;
  maxPages: number;
  walletAddress: string;
}

export async function readDataApiActivityConfigForServer(
  context: UserConfigContext
): Promise<DataApiActivityConfigForServer> {
  const claim = normalizeClaimShape(await readMergedConfigForServer('claim', context));
  const pageSize = Math.min(Math.max(Number(claim.positions_page_size ?? 500), 500), 500);
  const maxPages = Math.min(Math.max(Number(claim.positions_max_pages ?? 20), 20), 20);
  const baseUrl =
    String(claim.data_api_base_url ?? '').trim() || 'https://data-api.polymarket.com';
  return {
    baseUrl,
    pageSize,
    maxPages,
    walletAddress: await readPositionWalletAddress(context),
  };
}

export async function readClaimRelayerConfigForServer(
  context: UserConfigContext
): Promise<ClaimRelayerConfigForServer> {
  const claimSource = await readMergedConfigForServer('claim', context);
  const claim = normalizeClaimShape(claimSource);
  const exchange = normalizeExchangeShape(
    await readMergedConfigForServer('exchange', context)
  );

  return {
    executionMode: normalizeClaimExecutionMode(claim.execution_mode),
    chainId: Number(claim.chain_id ?? 137),
    rpcUrl: resolvePlaintextConfigValueForServer(claim.rpc_url, claim.rpc_url_env),
    ctfContractAddress: String(claim.ctf_contract_address ?? '').trim(),
    collateralTokenAddress: String(claim.collateral_token_address ?? '').trim(),
    autoActivateFunds: claim.auto_activate_funds === true,
    activateMinUsdc: Number(claim.activate_min_usdc ?? 0.01),
    usdceTokenAddress: String(claim.usdce_token_address ?? '').trim(),
    pusdTokenAddress: String(claim.pusd_token_address ?? '').trim(),
    collateralOnrampAddress: String(claim.collateral_onramp_address ?? '').trim(),
    userAddress: resolvePlaintextConfigValueForServer(claim.user_address, claim.user_address_env),
    privateKey: resolveSensitiveConfigValueForServer(claim.private_key, claim.private_key_env),
    safeAddress: resolvePlaintextConfigValueForServer(
      exchange.gnosis_safe_address,
      exchange.gnosis_safe_address_env
    ),
    builderApiKey: resolveSensitiveConfigValueForServer(
      exchange.builder_api_key || exchange.api_key,
      exchange.builder_api_key_env || exchange.api_key_env
    ),
    builderApiSecret: resolveSensitiveConfigValueForServer(
      exchange.builder_api_secret || exchange.api_secret,
      exchange.builder_api_secret_env || exchange.api_secret_env
    ),
    builderApiPassphrase: resolveSensitiveConfigValueForServer(
      exchange.builder_api_passphrase || exchange.api_passphrase,
      exchange.builder_api_passphrase_env || exchange.api_passphrase_env
    ),
    relayerApiKey: resolveSensitiveConfigValueForServer(claim.relayer_api_key, claim.relayer_api_key_env),
    relayerApiKeyAddress: resolvePlaintextConfigValueForServer(claim.relayer_api_key_address, claim.relayer_api_key_address_env),
  };
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

export async function readEffectiveClaimConfigForServer(
  context: UserConfigContext
): Promise<ClaimRuntimeValidationState> {
  // DB-stored seeded config overrides TOML with enabled=false and empty sensitive fields.
  // Merge: prefer non-empty DB values (explicit user settings), fall back to TOML.
  const source = await readMergedConfigForServer('claim', context);
  const raw = normalizeClaimShape(source);
  const exchange = normalizeExchangeShape(
    await readMergedConfigForServer('exchange', context)
  );

  return {
    enabled: raw.enabled === true,
    executionMode: normalizeClaimExecutionMode(raw.execution_mode),
    hasRpcSource:
      String(raw.rpc_url ?? '').trim().length > 0 ||
      String(raw.rpc_url_env ?? '').trim().length > 0,
    hasUserAddressSource:
      String(raw.user_address ?? '').trim().length > 0 ||
      String(raw.user_address_env ?? '').trim().length > 0,
    hasPrivateKeySource:
      String(raw.private_key ?? '').trim().length > 0 ||
      String(raw.private_key_env ?? '').trim().length > 0,
    hasSafeAddressSource:
      String(exchange.gnosis_safe_address ?? '').trim().length > 0 ||
      String(exchange.gnosis_safe_address_env ?? '').trim().length > 0,
    hasBuilderCredsSource:
      (String(exchange.builder_api_key ?? '').trim().length > 0 ||
        String(exchange.builder_api_key_env ?? '').trim().length > 0 ||
        String(exchange.api_key ?? '').trim().length > 0 ||
        String(exchange.api_key_env ?? '').trim().length > 0) &&
      (String(exchange.builder_api_secret ?? '').trim().length > 0 ||
        String(exchange.builder_api_secret_env ?? '').trim().length > 0 ||
        String(exchange.api_secret ?? '').trim().length > 0 ||
        String(exchange.api_secret_env ?? '').trim().length > 0) &&
      (String(exchange.builder_api_passphrase ?? '').trim().length > 0 ||
        String(exchange.builder_api_passphrase_env ?? '').trim().length > 0 ||
        String(exchange.api_passphrase ?? '').trim().length > 0 ||
        String(exchange.api_passphrase_env ?? '').trim().length > 0),
    hasRelayerApiKeySource:
      (String(raw.relayer_api_key ?? '').trim().length > 0 || String(raw.relayer_api_key_env ?? '').trim().length > 0) &&
      (String(raw.relayer_api_key_address ?? '').trim().length > 0 || String(raw.relayer_api_key_address_env ?? '').trim().length > 0),
  };
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

/**
 * DB-stored config ile TOML fallback'i merge eder.
 * Seeded config'ler sensitive alanları boş bırakır, bu durumda
 * TOML dosyasındaki gerçek değerler kullanılır.
 * DB değeri non-empty ise (kullanıcı bilerek değiştirmiş) tercih edilir.
 */
function mergeWithTomlFallback(
  stored: Record<string, unknown> | null,
  tomlFallback: Record<string, unknown>
): Record<string, unknown> {
  if (!stored) return tomlFallback;
  const merged = { ...tomlFallback };
  for (const [key, value] of Object.entries(stored)) {
    if (value !== '' && value !== false && value != null) {
      merged[key] = value;
    }
  }
  return merged;
}

async function readMergedConfigForServer(
  name: string,
  context: UserConfigContext
): Promise<Record<string, unknown>> {
  const [stored, toml] = await Promise.all([
    readStoredUserConfig(context.userId, name),
    readFallbackFileConfig(name).catch(() => ({} as Record<string, unknown>)),
  ]);
  return mergeWithTomlFallback(stored, toml);
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
  if (incoming.neg_risk_ctf_exchange_address !== undefined) {
    merged.neg_risk_ctf_exchange_address = String(
      incoming.neg_risk_ctf_exchange_address ?? ''
    ).trim();
  }
  if (incoming.builder_code !== undefined) {
    merged.builder_code = String(incoming.builder_code ?? '').trim();
  }
  if (incoming.builder_code_env !== undefined) {
    merged.builder_code_env = String(incoming.builder_code_env ?? '').trim();
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
  if (
    String(merged.builder_api_key ?? '').trim() &&
    String(merged.builder_api_secret ?? '').trim() &&
    String(merged.builder_api_passphrase ?? '').trim()
  ) {
    merged.builder_api_key_env = '';
    merged.builder_api_secret_env = '';
    merged.builder_api_passphrase_env = '';
  }
  if (String(merged.signer_private_key ?? '').trim()) {
    merged.signer_private_key_env = '';
  }
  if (String(merged.builder_code ?? '').trim()) {
    merged.builder_code_env = '';
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
  if (incoming.execution_mode !== undefined) {
    merged.execution_mode = String(incoming.execution_mode ?? '').trim();
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

  if (String(merged.rpc_url ?? '').trim()) merged.rpc_url_env = '';
  if (String(merged.user_address ?? '').trim()) merged.user_address_env = '';
  if (String(merged.private_key ?? '').trim()) merged.private_key_env = '';
  merged.relayer_api_key = resolveSensitiveFieldForWrite('relayer_api_key', incoming, existing);
  merged.relayer_api_key_address = resolvePlaintextFieldForWrite('relayer_api_key_address', incoming, existing);
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
  if (field !== 'api_secret' && field !== 'builder_api_secret') {
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
      builder_api_key: '',
      builder_api_secret: '',
      builder_api_passphrase: '',
      builder_code: '',
      builder_code_env: '',
      signer_private_key: '',
      gnosis_safe_address: '',
      api_address_env: '',
      api_key_env: '',
      api_secret_env: '',
      api_passphrase_env: '',
      builder_api_key_env: '',
      builder_api_secret_env: '',
      builder_api_passphrase_env: '',
      signer_private_key_env: '',
      gnosis_safe_address_env: '',
    };
  }
  if (name === 'claim') {
    const normalized = normalizeClaimShape(fallback);
    return {
      ...normalized,
      enabled: false,
      execution_mode: 'direct',
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
