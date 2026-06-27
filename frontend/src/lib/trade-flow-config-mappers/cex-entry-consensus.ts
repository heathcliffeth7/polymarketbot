import type { NodeFieldSchema } from './types';
import { RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME } from './constants';
import { toStringValue } from './utils';

export const CEX_ENTRY_CONSENSUS_BASIS_VALUES = ['own_open_gap', 'current_price'] as const;
export const CEX_ENTRY_CONSENSUS_MODE_VALUES = [
  'binance_coinbase',
  'asset_auto_plus_one_or_clean_pair',
  'open_gap_okx_plus_one_or_clean_pair',
  'open_gap_okx_plus_one',
  'okx_plus_one_or_clean_pair',
  'okx_plus_one',
  'bybit_plus_one_or_clean_pair',
  'bybit_plus_one',
  'gate_plus_one_or_clean_pair',
  'gate_plus_one',
] as const;

export const CEX_ENTRY_CONSENSUS_FIELDS: NodeFieldSchema[] = [
  {
    key: 'cexEntryConsensusBasis',
    label: 'Entry CEX Basis',
    input: 'select',
    options: [
      { label: 'Own 5m Open Gap', value: 'own_open_gap' },
      { label: 'Current Price Rollback', value: 'current_price' },
    ],
  },
  {
    key: 'cexEntryConsensusMode',
    label: 'Entry CEX Mode',
    input: 'select',
    options: [
      { label: 'Asset auto + clean pair', value: 'asset_auto_plus_one_or_clean_pair' },
      { label: 'Binance + Coinbase', value: 'binance_coinbase' },
      { label: 'Open gap OKX + clean pair', value: 'open_gap_okx_plus_one_or_clean_pair' },
      { label: 'Open gap OKX + 1', value: 'open_gap_okx_plus_one' },
      { label: 'Rollback OKX + clean pair', value: 'okx_plus_one_or_clean_pair' },
      { label: 'Rollback OKX + 1', value: 'okx_plus_one' },
      { label: 'Rollback Bybit + clean pair', value: 'bybit_plus_one_or_clean_pair' },
      { label: 'Rollback Bybit + 1', value: 'bybit_plus_one' },
      { label: 'Rollback Gate + clean pair', value: 'gate_plus_one_or_clean_pair' },
      { label: 'Rollback Gate + 1', value: 'gate_plus_one' },
    ],
  },
  { key: 'cexEntryOpenGapThresholdUsd', label: 'Open Gap Threshold USD', input: 'number' },
  { key: 'cexEntryOpenGapMinVenues', label: 'Open Gap Min Venues', input: 'number' },
  {
    key: 'cexEntryOpenGapAllowCleanPairWithoutAnchor',
    label: 'Clean Pair Fallback',
    input: 'checkbox',
  },
  { key: 'cexEntryOpenGapRatioMin', label: 'Open Gap Ratio Min', input: 'number' },
  { key: 'cexEntryOpenGapSpreadFloorUsd', label: 'Open Gap Spread Floor', input: 'number' },
  {
    key: 'cexEntryOpenGapSpreadExpectedMoveMult',
    label: 'Open Gap Spread EM Mult',
    input: 'number',
  },
  { key: 'cexEntryChainlinkSanityCheck', label: 'Chainlink Sanity', input: 'checkbox' },
];

const BASIS_SET = new Set<string>(CEX_ENTRY_CONSENSUS_BASIS_VALUES);
const MODE_SET = new Set<string>(CEX_ENTRY_CONSENSUS_MODE_VALUES);

const DEFAULTS: Record<string, string> = {
  cexEntryConsensusBasis: 'own_open_gap',
  cexEntryConsensusMode: 'binance_coinbase',
  cexEntryOpenGapThresholdUsd: '0.30',
  cexEntryOpenGapMinVenues: '2',
  cexEntryOpenGapAllowCleanPairWithoutAnchor: 'true',
  cexEntryOpenGapRatioMin: '0.25',
  cexEntryOpenGapSpreadFloorUsd: '0.20',
  cexEntryOpenGapSpreadExpectedMoveMult: '0.75',
  cexEntryChainlinkSanityCheck: 'true',
};

function resolveAsset(source: Record<string, unknown>): string {
  const explicitAsset = toStringValue(source.asset).trim().toLowerCase();
  if (explicitAsset) return explicitAsset;

  const scope = toStringValue(source.marketScope).trim().toLowerCase();
  const scopeAsset = scope ? RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[scope]?.asset : '';
  if (scopeAsset) return scopeAsset;

  const marketSlug = toStringValue(source.marketSlug).trim().toLowerCase();
  const match = /^([a-z0-9]+)-updown-/.exec(marketSlug);
  return match?.[1] ?? '';
}

export function cexEntryOpenGapThresholdDefaultUsd(source: Record<string, unknown>): string {
  switch (resolveAsset(source)) {
    case 'btc':
      return '5';
    case 'eth':
      return '0.175';
    case 'sol':
      return '0.0075';
    default:
      return DEFAULTS.cexEntryOpenGapThresholdUsd;
  }
}

export function applyCexEntryConsensusFormDefaults(fields: Record<string, string>): void {
  if (fields.priceToBeatCurrentPriceSource !== 'chainlink_cex_consensus') return;
  for (const [key, value] of Object.entries(DEFAULTS)) {
    const fallback =
      key === 'cexEntryOpenGapThresholdUsd'
        ? cexEntryOpenGapThresholdDefaultUsd(fields)
        : value;
    if (!toStringValue(fields[key]).trim()) fields[key] = fallback;
  }
}

export function clearCexEntryConsensusBuildConfig(config: Record<string, unknown>): void {
  for (const key of Object.keys(DEFAULTS)) delete config[key];
}

export function normalizeCexEntryConsensusBuildConfig(config: Record<string, unknown>): void {
  if (config.priceToBeatCurrentPriceSource !== 'chainlink_cex_consensus') {
    clearCexEntryConsensusBuildConfig(config);
    return;
  }
  const basis = toStringValue(config.cexEntryConsensusBasis).trim().toLowerCase();
  config.cexEntryConsensusBasis = BASIS_SET.has(basis) ? basis : DEFAULTS.cexEntryConsensusBasis;
  const mode = toStringValue(config.cexEntryConsensusMode).trim().toLowerCase();
  config.cexEntryConsensusMode = MODE_SET.has(mode) ? mode : DEFAULTS.cexEntryConsensusMode;
  normalizeNumber(
    config,
    'cexEntryOpenGapThresholdUsd',
    cexEntryOpenGapThresholdDefaultUsd(config),
    (value) => value > 0
  );
  normalizeNumber(
    config,
    'cexEntryOpenGapMinVenues',
    DEFAULTS.cexEntryOpenGapMinVenues,
    (value) => Number.isInteger(value) && value >= 2 && value <= 3
  );
  normalizeNumber(
    config,
    'cexEntryOpenGapRatioMin',
    DEFAULTS.cexEntryOpenGapRatioMin,
    (value) => value > 0 && value <= 1
  );
  normalizeNumber(
    config,
    'cexEntryOpenGapSpreadFloorUsd',
    DEFAULTS.cexEntryOpenGapSpreadFloorUsd,
    (value) => value >= 0
  );
  normalizeNumber(
    config,
    'cexEntryOpenGapSpreadExpectedMoveMult',
    DEFAULTS.cexEntryOpenGapSpreadExpectedMoveMult,
    (value) => value > 0
  );
  normalizeBoolean(
    config,
    'cexEntryOpenGapAllowCleanPairWithoutAnchor',
    DEFAULTS.cexEntryOpenGapAllowCleanPairWithoutAnchor
  );
  normalizeBoolean(config, 'cexEntryChainlinkSanityCheck', DEFAULTS.cexEntryChainlinkSanityCheck);
}

function normalizeNumber(
  config: Record<string, unknown>,
  key: string,
  fallback: string,
  isValid: (value: number) => boolean
): void {
  const value = Number(toStringValue(config[key]).trim());
  config[key] = Number.isFinite(value) && isValid(value) ? value : Number(fallback);
}

function normalizeBoolean(
  config: Record<string, unknown>,
  key: string,
  fallback: string
): void {
  const value = config[key];
  if (value === true || value === 'true') {
    config[key] = true;
  } else if (value === false || value === 'false') {
    config[key] = false;
  } else {
    config[key] = fallback === 'true';
  }
}
