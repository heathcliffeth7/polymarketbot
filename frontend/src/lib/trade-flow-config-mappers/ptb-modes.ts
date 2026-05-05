export const PTB_MODE_VALUES = [
  'manual',
  'auto_last_3_avg_excursion',
  'auto_vol_pct',
  'signal_formula',
  'iv_mismatch_edge',
] as const;

export type PtbMode = (typeof PTB_MODE_VALUES)[number];

export const PTB_MODE_OPTIONS: Array<{ label: string; value: PtbMode }> = [
  { label: 'Manual', value: 'manual' },
  { label: 'Auto: son 3 market excursion ort.', value: 'auto_last_3_avg_excursion' },
  { label: 'Auto: volatility bazli yuzde', value: 'auto_vol_pct' },
  { label: 'Signal formula', value: 'signal_formula' },
  { label: 'IV mismatch edge', value: 'iv_mismatch_edge' },
];

const PTB_MODE_VALUE_SET = new Set<string>(PTB_MODE_VALUES);

export function isPtbMode(value: unknown): value is PtbMode {
  return typeof value === 'string' && PTB_MODE_VALUE_SET.has(value);
}

export function normalizePtbMode(value: unknown, fallback: PtbMode = 'manual'): PtbMode {
  const normalized = String(value ?? '').trim().toLowerCase();
  return isPtbMode(normalized) ? normalized : fallback;
}

export function isAutoPtbMode(value: unknown): boolean {
  return normalizePtbMode(value) !== 'manual';
}

export const PTB_CURRENT_PRICE_SOURCE_VALUES = ['chainlink', 'binance', 'coinbase'] as const;

export type PtbCurrentPriceSource = (typeof PTB_CURRENT_PRICE_SOURCE_VALUES)[number];

export const PTB_CURRENT_PRICE_SOURCE_OPTIONS: Array<{
  label: string;
  value: PtbCurrentPriceSource;
}> = [
  { label: 'Chainlink', value: 'chainlink' },
  { label: 'Binance', value: 'binance' },
  { label: 'Coinbase', value: 'coinbase' },
];

const PTB_CURRENT_PRICE_SOURCE_VALUE_SET = new Set<string>(PTB_CURRENT_PRICE_SOURCE_VALUES);

export function isPtbCurrentPriceSource(value: unknown): value is PtbCurrentPriceSource {
  return typeof value === 'string' && PTB_CURRENT_PRICE_SOURCE_VALUE_SET.has(value);
}

export function normalizePtbCurrentPriceSource(
  value: unknown,
  fallback: PtbCurrentPriceSource = 'chainlink'
): PtbCurrentPriceSource {
  const normalized = String(value ?? '').trim().toLowerCase();
  return isPtbCurrentPriceSource(normalized) ? normalized : fallback;
}

export function normalizeOptionalPtbCurrentPriceSource(
  value: unknown
): PtbCurrentPriceSource | '' {
  const normalized = String(value ?? '').trim().toLowerCase();
  return isPtbCurrentPriceSource(normalized) ? normalized : '';
}

export function normalizeOptionalPtbCurrentPriceSourceConfig(
  config: Record<string, unknown>,
  key: string,
  active: boolean
): void {
  const normalized = normalizeOptionalPtbCurrentPriceSource(config[key]);
  if (active && normalized) config[key] = normalized;
  else delete config[key];
}
