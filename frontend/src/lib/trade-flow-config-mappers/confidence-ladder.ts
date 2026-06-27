import type { NodeFieldSchema } from './types';
import { isRecord, safeJsonStringify, toStringValue } from './utils';

export const CONFIDENCE_LADDER_MODE = 'confidence_ladder_hedge_lock_v1';
export const CONFIDENCE_LADDER_BINDING_MODE = 'confidence_ladder_only';

export const DEFAULT_CONFIDENCE_LADDER_CONFIG = {
  profile: 'aggressive_loss_capped',
  baseProbeShares: 2,
  maxLossPerMarketUsdc: 3,
  maxTotalCostPerMarketUsdc: 25,
  entryWindowStartSec: 3,
  entryWindowEndSec: 285,
  noNewDominantBuyLastSec: 15,
  probePriceMin: 0.45,
  probePriceMax: 0.55,
  maxSpread: 0.03,
  dominanceGap: 0.12,
  chopProbabilityMax: 0.45,
  hardNoChaseAbove: 0.93,
  takerFeeRate: 0.07,
  slippageBuffer: 0.005,
  preferPostOnly: true,
  takerAllowedOnlyIfEdgeAbove: 0.05,
  hedge: {
    oppositePriceMax: 0.20,
    minReversalEdge: 0.03,
    profitLockPairCostMax: 0.97,
    strongProfitLockPairCost: 0.94,
    damageControlPriceMin: 0.35,
    damageControlPriceMax: 0.60,
    targetHedgeRatioMin: 0.35,
    targetHedgeRatioMax: 1.0,
  },
  stop: {
    maxDirectionFlips: 2,
  },
};

export const CONFIDENCE_LADDER_ACTION_FIELDS: NodeFieldSchema[] = [
  {
    key: 'confidenceLadder',
    label: 'Confidence Ladder JSON',
    input: 'textarea',
    help: 'BTC 5m Confidence Ladder + Hedge Lock config. Bos birakilirsa MVP varsayilanlari kullanilir.',
  },
  { key: 'postOnly', label: 'Post Only', input: 'checkbox' },
  {
    key: 'orderType',
    label: 'CLOB Order Type',
    input: 'select',
    options: [
      { label: 'FAK', value: 'FAK' },
      { label: 'FOK', value: 'FOK' },
      { label: 'GTC', value: 'GTC' },
      { label: 'GTD', value: 'GTD' },
    ],
  },
];

export function applyConfidenceLadderFormDefaults(
  fields: Record<string, string>,
  config: Record<string, unknown>,
) {
  if (isRecord(config.confidenceLadder)) {
    fields.confidenceLadder = safeJsonStringify(config.confidenceLadder);
  }
}

function parseConfidenceLadderObject(raw: unknown): Record<string, unknown> | string | null {
  if (isRecord(raw)) return raw;
  const text = toStringValue(raw).trim();
  if (!text) return null;
  try {
    const parsed = JSON.parse(text);
    return isRecord(parsed) ? parsed : text;
  } catch {
    return text;
  }
}

export function normalizeConfidenceLadderBuildConfig(
  config: Record<string, unknown>,
  fields: Record<string, string>,
): boolean {
  if (toStringValue(config.mode).trim().toLowerCase() !== CONFIDENCE_LADDER_MODE) {
    return false;
  }
  config.mode = CONFIDENCE_LADDER_MODE;
  config.side = 'buy';
  config.executionMode = toStringValue(config.executionMode).trim().toLowerCase() || 'market';
  config.kind = 'immediate';
  config.tpEnabled = false;
  config.slEnabled = false;
  config.ptbStopLossEnabled = false;
  delete config.tpPriceCent;
  delete config.slPriceCent;
  delete config.tpRules;
  delete config.slRules;
  delete config.ptbStopLossRules;
  delete config.sizeUsdc;
  delete config.targetNotionalUsdc;
  delete config.sizePct;
  delete config.targetQty;
  delete config.triggerSizes;

  const parsed = parseConfidenceLadderObject(fields.confidenceLadder ?? config.confidenceLadder);
  if (parsed == null) {
    config.confidenceLadder = DEFAULT_CONFIDENCE_LADDER_CONFIG;
  } else {
    config.confidenceLadder = parsed;
  }
  return true;
}
