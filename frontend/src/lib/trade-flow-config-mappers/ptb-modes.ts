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
