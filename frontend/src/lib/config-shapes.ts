import {
  DEFAULT_COLLATERAL_ONRAMP_ADDRESS,
  DEFAULT_PUSD_TOKEN_ADDRESS,
  DEFAULT_USDCE_TOKEN_ADDRESS,
  normalizeClaimExecutionMode,
} from '@/lib/claim-relayer-config';
import { isEncryptedConfigValue } from '@/lib/crypto-config';

const MASKED_SECRET = '********';
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

export function normalizeExchangeShape(
  source: Record<string, unknown>
): Record<string, unknown> {
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
    builder_api_key: String(source.builder_api_key ?? ''),
    builder_api_secret: String(source.builder_api_secret ?? ''),
    builder_api_passphrase: String(source.builder_api_passphrase ?? ''),
    builder_code: String(source.builder_code ?? ''),
    builder_code_env: String(source.builder_code_env ?? ''),
    ctf_exchange_address: String(source.ctf_exchange_address ?? ''),
    neg_risk_ctf_exchange_address: String(
      source.neg_risk_ctf_exchange_address ?? ''
    ),
    signer_private_key: String(source.signer_private_key ?? ''),
    signer_private_key_env: String(source.signer_private_key_env ?? ''),
    api_address_env: String(source.api_address_env ?? ''),
    api_key_env: String(source.api_key_env ?? ''),
    api_secret_env: String(source.api_secret_env ?? ''),
    api_passphrase_env: String(source.api_passphrase_env ?? ''),
    builder_api_key_env: String(source.builder_api_key_env ?? ''),
    builder_api_secret_env: String(source.builder_api_secret_env ?? ''),
    builder_api_passphrase_env: String(source.builder_api_passphrase_env ?? ''),
    gnosis_safe_address: String(source.gnosis_safe_address ?? ''),
    gnosis_safe_address_env: String(source.gnosis_safe_address_env ?? ''),
  };
}

export function normalizeClaimShape(
  source: Record<string, unknown>
): Record<string, unknown> {
  const chainId = Number(source.chain_id);
  const discoveryIntervalSec = Number(source.discovery_interval_sec);
  const positionsPageSize = Number(source.positions_page_size);
  const positionsMaxPages = Number(source.positions_max_pages);
  const processBatchSize = Number(source.process_batch_size);
  const maxAttempts = Number(source.max_attempts);
  const retryBackoffMs = Number(source.retry_backoff_ms);
  const activateMinUsdc = Number(source.activate_min_usdc);
  return {
    enabled: Boolean(source.enabled),
    execution_mode: String(source.execution_mode ?? 'direct'),
    rpc_url: String(source.rpc_url ?? ''),
    rpc_url_env: String(source.rpc_url_env ?? ''),
    data_api_base_url: String(source.data_api_base_url ?? ''),
    user_address: String(source.user_address ?? ''),
    user_address_env: String(source.user_address_env ?? ''),
    private_key: String(source.private_key ?? ''),
    private_key_env: String(source.private_key_env ?? ''),
    chain_id: Number.isFinite(chainId) ? chainId : 137,
    ctf_contract_address: String(source.ctf_contract_address ?? ''),
    collateral_token_address: String(
      source.collateral_token_address ?? DEFAULT_USDCE_TOKEN_ADDRESS
    ),
    auto_activate_funds:
      source.auto_activate_funds === undefined
        ? true
        : Boolean(source.auto_activate_funds),
    activate_min_usdc: Number.isFinite(activateMinUsdc)
      ? activateMinUsdc
      : 0.01,
    usdce_token_address: String(
      source.usdce_token_address ?? DEFAULT_USDCE_TOKEN_ADDRESS
    ),
    pusd_token_address: String(
      source.pusd_token_address ?? DEFAULT_PUSD_TOKEN_ADDRESS
    ),
    collateral_onramp_address: String(
      source.collateral_onramp_address ?? DEFAULT_COLLATERAL_ONRAMP_ADDRESS
    ),
    discovery_interval_sec: Number.isFinite(discoveryIntervalSec)
      ? discoveryIntervalSec
      : 30,
    positions_page_size: Number.isFinite(positionsPageSize)
      ? positionsPageSize
      : 200,
    positions_max_pages: Number.isFinite(positionsMaxPages)
      ? positionsMaxPages
      : 5,
    process_batch_size: Number.isFinite(processBatchSize)
      ? processBatchSize
      : 10,
    max_attempts: Number.isFinite(maxAttempts) ? maxAttempts : 5,
    retry_backoff_ms: Number.isFinite(retryBackoffMs) ? retryBackoffMs : 10000,
    relayer_api_key: String(source.relayer_api_key ?? ''),
    relayer_api_key_env: String(source.relayer_api_key_env ?? ''),
    relayer_api_key_address: String(source.relayer_api_key_address ?? ''),
    relayer_api_key_address_env: String(
      source.relayer_api_key_address_env ?? ''
    ),
  };
}

export function normalizeTelegramShape(
  source: Record<string, unknown>
): Record<string, unknown> {
  return {
    bot_token: String(source.bot_token ?? ''),
    chat_id: String(source.chat_id ?? ''),
  };
}

export function normalizeStrategyShape(
  source: Record<string, unknown>
): Record<string, unknown> {
  return {
    max_price_relax_enabled: true,
    ...source,
  };
}

export function validateConfigShape(
  name: string,
  data: Record<string, unknown>
): void {
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
  const isHexBytes32 = (value: string): boolean =>
    /^0x[a-fA-F0-9]{64}$/.test(value);

  if (name === 'strategy') {
    const ep = requiredNumber('entry_price');
    if (ep < 0 || ep > 1) errors.push('entry_price must be in [0, 1]');
    if (requiredNumber('tp_pct') <= 0) errors.push('tp_pct must be > 0');
    if (requiredNumber('aggressive_sl_pct') <= 0)
      errors.push('aggressive_sl_pct must be > 0');

    const dualEnabled = Boolean(data.dual_side_enabled);
    if (dualEnabled) {
      const totalNotional = requiredNumber('total_notional_usdc');
      const perLegInitialNotional = requiredNumber(
        'per_leg_initial_notional_usdc'
      );
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
        errors.push(
          'per_leg_initial_notional_usdc * 2 must be <= total_notional_usdc'
        );
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
    if (
      data.kill_switch_mode === 'disabled' &&
      data.manual_kill_switch_active === true
    )
      errors.push(
        'manual_kill_switch_active cannot be true when kill_switch_mode is disabled'
      );
  }

  if (name === 'bot') {
    const marketScope = String(data.market_scope ?? '').trim();
    if (
      !SUPPORTED_MARKET_SCOPES.includes(
        marketScope as (typeof SUPPORTED_MARKET_SCOPES)[number]
      )
    ) {
      errors.push(
        `market_scope must be one of: ${SUPPORTED_MARKET_SCOPES.join(', ')}`
      );
    }
    const marketSlugOverride = String(data.market_slug_override ?? '').trim();
    const marketSlugOverrideLower = marketSlugOverride.toLowerCase();
    if (
      marketSlugOverride.length > 0 &&
      !SUPPORTED_MARKET_SLUG_PREFIXES.some((prefix) =>
        marketSlugOverrideLower.includes(prefix)
      )
    ) {
      errors.push(
        'market_slug_override must contain a supported slug prefix (e.g. btc-updown-5m-, eth-updown-15m-)'
      );
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
    if (requiredNumber('chain_id') <= 0) errors.push('chain_id must be > 0');
    if (!String(data.ctf_exchange_address ?? '').trim()) {
      errors.push('ctf_exchange_address is required');
    }

    const apiAddress = String(data.api_address ?? '').trim();
    const signerPrivateKey = String(data.signer_private_key ?? '').trim();
    const gnosisSafeAddress = String(data.gnosis_safe_address ?? '').trim();
    const ctfExchangeAddress = String(data.ctf_exchange_address ?? '').trim();
    const negRiskCtfExchangeAddress = String(
      data.neg_risk_ctf_exchange_address ?? ''
    ).trim();
    const builderCode = String(data.builder_code ?? '').trim();

    if (ctfExchangeAddress && !isHexAddress(ctfExchangeAddress)) {
      errors.push('ctf_exchange_address must be a valid 0x address');
    }
    if (
      negRiskCtfExchangeAddress &&
      !isHexAddress(negRiskCtfExchangeAddress)
    ) {
      errors.push('neg_risk_ctf_exchange_address must be a valid 0x address');
    }
    if (builderCode && !isHexBytes32(builderCode)) {
      errors.push('builder_code must be a valid 0x bytes32 value');
    }

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
    const executionMode = normalizeClaimExecutionMode(data.execution_mode);
    const userAddress = String(data.user_address ?? '').trim();
    const userAddressEnv = String(data.user_address_env ?? '').trim();
    const privateKey = String(data.private_key ?? '').trim();
    const privateKeyEnv = String(data.private_key_env ?? '').trim();

    if (rpcUrl && !rpcUrl.startsWith('http'))
      errors.push('rpc_url must start with http');
    if (!['direct', 'builder_relayer', 'relayer_api_key'].includes(executionMode)) {
      errors.push(
        'execution_mode must be direct, builder_relayer, or relayer_api_key'
      );
    }
    if (!dataApiBaseUrl.startsWith('http'))
      errors.push('data_api_base_url must start with http');
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
    const collateralTokenAddress = String(
      data.collateral_token_address ?? ''
    ).trim();
    const usdceTokenAddress = String(data.usdce_token_address ?? '').trim();
    const pusdTokenAddress = String(data.pusd_token_address ?? '').trim();
    const collateralOnrampAddress = String(
      data.collateral_onramp_address ?? ''
    ).trim();
    if (!isHexAddress(ctfContractAddress)) {
      errors.push('ctf_contract_address must be a valid 0x address');
    }
    if (!isHexAddress(collateralTokenAddress)) {
      errors.push('collateral_token_address must be a valid 0x address');
    }
    if (!isHexAddress(usdceTokenAddress)) {
      errors.push('usdce_token_address must be a valid 0x address');
    }
    if (!isHexAddress(pusdTokenAddress)) {
      errors.push('pusd_token_address must be a valid 0x address');
    }
    if (!isHexAddress(collateralOnrampAddress)) {
      errors.push('collateral_onramp_address must be a valid 0x address');
    }
    if (requiredNumber('activate_min_usdc') < 0) {
      errors.push('activate_min_usdc must be >= 0');
    }
    if (userAddress && !isHexAddress(userAddress)) {
      errors.push('user_address must be a valid 0x address');
    }
    if (
      privateKey &&
      !isHexPrivateKey(privateKey) &&
      !isEncryptedConfigValue(privateKey)
    ) {
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
      errors.push(
        'chat_id must be a Telegram chat ID like -1001234567890 or a @channelusername'
      );
    }
  }

  if (errors.length > 0) {
    throw new Error(`Validation failed: ${errors.join(', ')}`);
  }
}
