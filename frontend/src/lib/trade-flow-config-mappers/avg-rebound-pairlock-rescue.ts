import type { NodeFieldSchema } from './types';
import { isRecord, safeJsonStringify, toStringValue } from './utils';

export const AVG_REBOUND_PAIRLOCK_RESCUE_MODE = 'avg_rebound_pairlock_rescue_v1';
export const AVG_REBOUND_PAIRLOCK_RESCUE_BINDING_MODE = 'avg_rebound_pairlock_rescue_only';

export const DEFAULT_AVG_REBOUND_PAIRLOCK_RESCUE_CONFIG = {
  version: 'v1',
  sessionBudgetUsdc: '50',
  reservedBudgetBufferUsdc: '0.75',
  primaryOutcomeLabel: 'auto',
  oppositeOutcomeLabel: 'opposite',
  primarySideSelection: 'cheapest_eligible',
  orderType: 'FOK',
  executionMode: 'limit',
  vwapSource: 'rest_book',
  extraVwapSafetyBuffer: '0.005',
  allowPrimaryAfterPartialProfit: true,
  preFullGivebackGuardEnabled: false,
  fullGivebackGuardEnabled: true,
  primaryLadder: [
    { id: 'p50', priceCap: '0.50', qty: '8' },
    { id: 'p30', priceCap: '0.30', qty: '15' },
    { id: 'p10', priceCap: '0.10', qty: '24' },
  ],
  stages: [
    {
      id: 'stage_50',
      requiredPrimaryTierIds: ['p50'],
      profitLegs: [
        { id: 's50_profit_45', oppositeVwapCap: '0.45', qty: '4' },
        { id: 's50_profit_40', oppositeVwapCap: '0.40', qty: '4' },
      ],
      givebackGuard: { trigger: '0.47', maxExecutionVwap: '0.47' },
    },
    {
      id: 'stage_30',
      requiredPrimaryTierIds: ['p50', 'p30'],
      profitLegs: [
        { id: 's30_profit_59', oppositeVwapCap: '0.59', qty: '8' },
        { id: 's30_profit_52', oppositeVwapCap: '0.52', qty: '8' },
        { id: 's30_profit_45', oppositeVwapCap: '0.45', qty: '7' },
      ],
      givebackGuard: { trigger: '0.63', maxExecutionVwap: '0.63' },
    },
    {
      id: 'stage_full',
      requiredPrimaryTierIds: ['p50', 'p30', 'p10'],
      profitLegs: [
        { id: 'full_profit_72', oppositeVwapCap: '0.72', qty: '15' },
        { id: 'full_profit_64', oppositeVwapCap: '0.64', qty: '16' },
        { id: 'full_profit_54', oppositeVwapCap: '0.54', qty: '16' },
      ],
      givebackGuard: { trigger: '0.76', maxExecutionVwap: '0.78' },
    },
  ],
  rescue: {
    enabledOnlyAfterFullLadder: true,
    normalVwapCap: '0.78',
    emergencyVwapCap: '0.81',
    hardMaxVwapCap: '0.81',
  },
};

export const MICRO_AVG_REBOUND_PAIRLOCK_RESCUE_23USDC_CONFIG = {
  version: 'v1',
  sessionBudgetUsdc: '23',
  reservedBudgetBufferUsdc: '0.25',
  primaryOutcomeLabel: 'auto',
  oppositeOutcomeLabel: 'opposite',
  primarySideSelection: 'cheapest_eligible',
  orderType: 'FOK',
  executionMode: 'limit',
  vwapSource: 'rest_book',
  extraVwapSafetyBuffer: '0.005',
  targetProfitUsdc: '0.10',
  allowPrimaryAfterPartialProfit: true,
  preFullGivebackGuardEnabled: false,
  fullGivebackGuardEnabled: true,
  primaryLadder: [
    { id: 'p50', priceCap: '0.50', qty: '4' },
    { id: 'p30', priceCap: '0.30', qty: '5' },
    { id: 'p10', priceCap: '0.10', qty: '10' },
  ],
  stages: [
    {
      id: 'stage_50',
      requiredPrimaryTierIds: ['p50'],
      profitLegs: [
        { id: 's50_profit_10c', oppositeVwapCap: '0.480', qty: '4' },
      ],
      givebackGuard: { trigger: '0.480', maxExecutionVwap: '0.480' },
    },
    {
      id: 'stage_30',
      requiredPrimaryTierIds: ['p50', 'p30'],
      profitLegs: [
        { id: 's30_profit_10c', oppositeVwapCap: '0.605', qty: '9' },
      ],
      givebackGuard: { trigger: '0.605', maxExecutionVwap: '0.605' },
    },
    {
      id: 'stage_full',
      requiredPrimaryTierIds: ['p50', 'p30', 'p10'],
      profitLegs: [
        { id: 'full_profit_10c', oppositeVwapCap: '0.763', qty: '19' },
      ],
      givebackGuard: { trigger: '0.770', maxExecutionVwap: '0.770' },
    },
  ],
  rescue: {
    enabledOnlyAfterFullLadder: true,
    normalVwapCap: '0.770',
    emergencyVwapCap: '0.800',
    hardMaxVwapCap: '0.800',
    lastChanceVwapCap: '0.850',
  },
};

export const MICRO_AVG_REBOUND_PAIRLOCK_RESCUE_20USDC_CONFIG =
  MICRO_AVG_REBOUND_PAIRLOCK_RESCUE_23USDC_CONFIG;

export const AVG_REBOUND_PAIRLOCK_RESCUE_ACTION_FIELDS: NodeFieldSchema[] = [
  {
    key: 'avgReboundPairlockRescue',
    label: 'Avg-Rebound Pairlock Rescue JSON',
    input: 'textarea',
    help: 'Avg-Rebound Pairlock Rescue v1 config. Bos birakilirsa 50 USDC default profil kullanilir.',
  },
];

export function applyAvgReboundPairlockRescueFormDefaults(
  fields: Record<string, string>,
  config: Record<string, unknown>,
) {
  if (isRecord(config.avgReboundPairlockRescue)) {
    fields.avgReboundPairlockRescue = safeJsonStringify(config.avgReboundPairlockRescue);
  }
}

function parseAvgReboundObject(raw: unknown): Record<string, unknown> | string | null {
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

export function normalizeAvgReboundPairlockRescueBuildConfig(
  config: Record<string, unknown>,
  fields: Record<string, string>,
): boolean {
  if (toStringValue(config.mode).trim().toLowerCase() !== AVG_REBOUND_PAIRLOCK_RESCUE_MODE) {
    return false;
  }
  config.mode = AVG_REBOUND_PAIRLOCK_RESCUE_MODE;
  config.side = 'buy';
  config.executionMode = 'limit';
  config.orderType = 'FOK';
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

  const parsed = parseAvgReboundObject(
    fields.avgReboundPairlockRescue ?? config.avgReboundPairlockRescue,
  );
  config.avgReboundPairlockRescue = parsed == null
    ? DEFAULT_AVG_REBOUND_PAIRLOCK_RESCUE_CONFIG
    : parsed;
  return true;
}
