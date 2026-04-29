import {
  isPairLockSupportedStopLossField,
  normalizePairLockStrategy,
} from '@/lib/trade-flow-config-mappers/pair-lock';
import { normalizePtbMode } from '@/lib/trade-flow-config-mappers';

export function isPairLockField(key: string): boolean {
  return (
    key === 'pairMaxTotalCent' ||
    key === 'pairLockStrategy' ||
    key === 'pairLockDecisionQty' ||
    key === 'pairLockSingleEdgeThreshold' ||
    key === 'pairLockCostBuffer' ||
    key.startsWith('adaptiveMaxPrice') ||
    key.startsWith('notifyOnAdaptiveMaxPrice') ||
    key.startsWith('biasedHedge') ||
    key === 'pairSizingMode' ||
    key === 'pairTotalBudgetUsdc' ||
    key === 'pairOrphanGraceMs' ||
    key === 'pairProtectiveUnwindEnabled' ||
    key === 'pairIgnoreStopLossAfterLocked' ||
    key === 'notifyOnPairLocked' ||
    key === 'notifyOnPairUnwind' ||
    key === 'counterLegEnabled' ||
    key === 'counterLegSizeUsdc' ||
    key === 'counterLegOutcomeLabel' ||
    key === 'counterLegTriggerCondition' ||
    key === 'counterLegTriggerPriceCent' ||
    key === 'counterLegMaxPriceCent' ||
    key === 'counterLegPriceToBeatGuardEnabled' ||
    key === 'counterLegPriceToBeatMode' ||
    key === 'counterLegPriceToBeatMaxDiff' ||
    key === 'counterLegPriceToBeatMaxDiffUnit' ||
    key === 'counterLegExecutionFloorGuardEnabled' ||
    key === 'counterLegExecutionFloorPriceCent' ||
    key === 'counterLegRetryOnPriceToBeatGuardBlock' ||
    key === 'counterLegRetryOnExecutionFloorGuardBlock' ||
    key === 'counterLegRetryOnMaxPriceBlock' ||
    key === 'counterLegTpEnabled' ||
    key === 'counterLegTpPriceCent' ||
    key === 'counterLegNotifyOnTpHit' ||
    key === 'counterLegPtbStopLossGapUnit'
  );
}

export function isPairLockIncompatibleField(key: string): boolean {
  if (isPairLockSupportedStopLossField(key)) {
    return false;
  }
  return (
    key === 'stagedSlReentryOnlyAfterAllStages' ||
    key === 'reentryMinPriceCent' ||
    key === 'reentryMaxPriceCent' ||
    key === 'reentrySkipCurrentWindow' ||
    key === 'reentryPriceToBeatMaxDiff' ||
    key === 'reentryPriceToBeatMaxDiffUnit' ||
    key === 'reentryThresholdDecay' ||
    key === 'reentryMaxPriceTightenBps' ||
    key === 'notifyOnTriggerPriceBlocked' ||
    key === 'notifyOnExecutionFloorBlocked'
  );
}

export function resolvePairLockTakeProfitFieldVisibility(
  key: string,
  pairLockEnabled: boolean,
  fields: Record<string, string>
): boolean | null {
  if (
    key !== 'counterLegTpEnabled' &&
    key !== 'counterLegTpPriceCent' &&
    key !== 'counterLegNotifyOnTpHit'
  ) {
    return null;
  }

  const counterLegEnabled = (fields.counterLegEnabled ?? '').trim().toLowerCase() === 'true';
  const counterLegTpEnabled = (fields.counterLegTpEnabled ?? '').trim().toLowerCase() === 'true';

  switch (key) {
    case 'counterLegTpEnabled':
      return pairLockEnabled && counterLegEnabled;
    case 'counterLegTpPriceCent':
    case 'counterLegNotifyOnTpHit':
      return pairLockEnabled && counterLegEnabled && counterLegTpEnabled;
    default:
      return null;
  }
}

export function resolvePairLockStopLossFieldVisibility(
  key: string,
  pairLockEnabled: boolean,
  fields: Record<string, string>
): boolean | null {
  if (!isPairLockSupportedStopLossField(key)) {
    return null;
  }

  const slEnabled = (fields.slEnabled ?? '').trim().toLowerCase() === 'true';
  const ptbStopLossEnabled = (fields.ptbStopLossEnabled ?? '').trim().toLowerCase() === 'true';
  const anyStopLossEnabled = slEnabled || ptbStopLossEnabled;
  const reenterOnSlHit = (fields.reenterOnSlHit ?? '').trim().toLowerCase() === 'true';
  const counterLegEnabled = (fields.counterLegEnabled ?? '').trim().toLowerCase() === 'true';
  const counterLegSlEnabled = (fields.counterLegSlEnabled ?? '').trim().toLowerCase() === 'true';
  const counterLegPtbStopLossEnabled =
    (fields.counterLegPtbStopLossEnabled ?? '').trim().toLowerCase() === 'true';
  const anyCounterStopLossEnabled = counterLegSlEnabled || counterLegPtbStopLossEnabled;

  switch (key) {
    case 'slEnabled':
    case 'ptbStopLossEnabled':
      return pairLockEnabled;
    case 'slPriceCent':
    case 'slTriggerPriceMode':
      return pairLockEnabled && slEnabled;
    case 'ptbStopLossGapUsd':
    case 'ptbStopLossGapUnit':
    case 'ptbStopLossTimeDecayMode':
      return pairLockEnabled && ptbStopLossEnabled;
    case 'notifyOnSlHit':
    case 'reenterOnSlHit':
      return pairLockEnabled && anyStopLossEnabled;
    case 'reentryMaxAttempts':
    case 'reentryCooldownSec':
      return pairLockEnabled && anyStopLossEnabled && reenterOnSlHit;
    case 'counterLegSlEnabled':
    case 'counterLegPtbStopLossEnabled':
      return pairLockEnabled && counterLegEnabled;
    case 'counterLegSlPriceCent':
    case 'counterLegSlTriggerPriceMode':
      return pairLockEnabled && counterLegEnabled && counterLegSlEnabled;
    case 'counterLegPtbStopLossGapUsd':
    case 'counterLegPtbStopLossGapUnit':
    case 'counterLegPtbStopLossTimeDecayMode':
      return pairLockEnabled && counterLegEnabled && counterLegPtbStopLossEnabled;
    case 'counterLegNotifyOnSlHit':
      return pairLockEnabled && counterLegEnabled && anyCounterStopLossEnabled;
    default:
      return pairLockEnabled;
  }
}

export function resolvePairLockCounterOutcomePreview(primaryOutcomeLabel: string): string {
  const normalized = primaryOutcomeLabel.trim().toLowerCase();
  if (normalized === 'up' || normalized === 'yes') return 'Down';
  if (normalized === 'down' || normalized === 'no') return 'Up';
  return '';
}

export function normalizePairLockSizingMode(
  value: string
): 'manual' | 'auto_remaining_budget' {
  return value.trim().toLowerCase() === 'auto_remaining_budget'
    ? 'auto_remaining_budget'
    : 'manual';
}

function normalizePairLockBinaryOutcome(
  value: string
): 'yes' | 'no' | null {
  switch (value.trim().toLowerCase()) {
    case 'yes':
    case 'up':
    case 'true':
    case '1':
      return 'yes';
    case 'no':
    case 'down':
    case 'false':
    case '0':
      return 'no';
    default:
      return null;
  }
}

function estimatePairLockBuyFeeQty(
  executionPrice: number,
  grossQty: number,
  feeRateBps: number
): number {
  if (!Number.isFinite(executionPrice) || executionPrice <= 0 || !Number.isFinite(grossQty) || grossQty <= 0 || !Number.isFinite(feeRateBps) || feeRateBps <= 0) {
    return 0;
  }
  const feeCurveRate = feeRateBps / 4000;
  const curveInput = Math.max(0, Math.min(1, executionPrice * (1 - executionPrice)));
  const feeQuote = grossQty * feeCurveRate * curveInput * curveInput;
  return Math.max(0, feeQuote / executionPrice);
}

export interface PairLockAutoPreviewOutcome {
  token_id: string;
  label: string;
  price: number | null;
  legSide: 'yes' | 'no';
  feeRateBps?: number | null;
}

export interface PairLockAutoPreview {
  primaryPrice: number;
  counterPrice: number;
  primaryBudgetUsdc: number;
  remainingBudgetUsdc: number;
  commonQty: number;
  projectedNetProfitUsdc: number;
  residueQty: number;
  blockedReason: 'missing_outcomes' | 'missing_primary_outcome' | 'missing_counter_outcome' | 'missing_price' | 'invalid_budget' | 'above_max_total' | null;
}

function resolvePairLockAutoPreviewOutcome(
  outcomes: PairLockAutoPreviewOutcome[],
  tokenId: string,
  outcomeLabel: string
): PairLockAutoPreviewOutcome | null {
  const normalizedLabel = normalizePairLockBinaryOutcome(outcomeLabel);
  if (tokenId.trim()) {
    const matched = outcomes.find((outcome) => outcome.token_id === tokenId.trim());
    if (matched) return matched;
  }
  if (!normalizedLabel) {
    return null;
  }
  return outcomes.find((outcome) => outcome.legSide === normalizedLabel) ?? null;
}

export function resolvePairLockSizingFieldVisibility(
  key: string,
  pairLockEnabled: boolean,
  fields: Record<string, string>
): boolean | null {
  if (
    key !== 'pairLockStrategy' &&
    key !== 'pairLockDecisionQty' &&
    key !== 'pairLockSingleEdgeThreshold' &&
    key !== 'pairLockCostBuffer' &&
    !key.startsWith('adaptiveMaxPrice') &&
    !key.startsWith('notifyOnAdaptiveMaxPrice') &&
    !key.startsWith('biasedHedge') &&
    key !== 'pairSizingMode' &&
    key !== 'pairTotalBudgetUsdc' &&
    key !== 'counterLegSizeUsdc'
  ) {
    return null;
  }
  if (key === 'pairLockStrategy') {
    return pairLockEnabled;
  }
  const strategy = normalizePairLockStrategy(fields.pairLockStrategy ?? '');
  if (
    key === 'pairLockDecisionQty' ||
    key === 'pairLockSingleEdgeThreshold' ||
    key === 'pairLockCostBuffer'
  ) {
    return pairLockEnabled && strategy === 'edge_pairlock_v1';
  }
  if (key.startsWith('biasedHedge')) {
    return pairLockEnabled && strategy === 'biased_hedge_v1';
  }
  if (key.startsWith('adaptiveMaxPrice') || key.startsWith('notifyOnAdaptiveMaxPrice')) {
    return pairLockEnabled && strategy === 'adaptive_max_price_v1';
  }
  if (strategy === 'edge_pairlock_v1' || strategy === 'biased_hedge_v1') {
    return false;
  }
  if (key === 'pairSizingMode') {
    return pairLockEnabled;
  }
  const sizingMode = normalizePairLockSizingMode(fields.pairSizingMode ?? '');
  if (key === 'pairTotalBudgetUsdc') {
    return pairLockEnabled && sizingMode === 'auto_remaining_budget';
  }
  return pairLockEnabled && sizingMode === 'manual';
}

export function estimatePairLockAutoRemainingBudgetPreview(
  fields: Record<string, string>,
  outcomes: PairLockAutoPreviewOutcome[]
): PairLockAutoPreview | null {
  if (normalizePairLockSizingMode(fields.pairSizingMode ?? '') !== 'auto_remaining_budget') {
    return null;
  }
  if (outcomes.length === 0) {
    return {
      primaryPrice: 0,
      counterPrice: 0,
      primaryBudgetUsdc: 0,
      remainingBudgetUsdc: 0,
      commonQty: 0,
      projectedNetProfitUsdc: 0,
      residueQty: 0,
      blockedReason: 'missing_outcomes',
    };
  }
  const primaryBudgetUsdc = Number(fields.sizeUsdc ?? '');
  const totalBudgetUsdc = Number(fields.pairTotalBudgetUsdc ?? '');
  if (!Number.isFinite(primaryBudgetUsdc) || primaryBudgetUsdc <= 0 || !Number.isFinite(totalBudgetUsdc) || totalBudgetUsdc <= primaryBudgetUsdc) {
    return {
      primaryPrice: 0,
      counterPrice: 0,
      primaryBudgetUsdc: Number.isFinite(primaryBudgetUsdc) ? primaryBudgetUsdc : 0,
      remainingBudgetUsdc: Number.isFinite(totalBudgetUsdc) && Number.isFinite(primaryBudgetUsdc)
        ? Math.max(0, totalBudgetUsdc - primaryBudgetUsdc)
        : 0,
      commonQty: 0,
      projectedNetProfitUsdc: 0,
      residueQty: 0,
      blockedReason: 'invalid_budget',
    };
  }
  const primaryOutcome = resolvePairLockAutoPreviewOutcome(
    outcomes,
    fields.tokenId ?? '',
    fields.outcomeLabel ?? ''
  );
  if (!primaryOutcome) {
    return {
      primaryPrice: 0,
      counterPrice: 0,
      primaryBudgetUsdc,
      remainingBudgetUsdc: totalBudgetUsdc - primaryBudgetUsdc,
      commonQty: 0,
      projectedNetProfitUsdc: 0,
      residueQty: 0,
      blockedReason: 'missing_primary_outcome',
    };
  }
  const primarySide = primaryOutcome.legSide;
  const counterSide = (fields.counterLegOutcomeLabel ?? '').trim().toLowerCase() === 'opposite'
    ? primarySide === 'yes'
      ? 'no'
      : 'yes'
    : normalizePairLockBinaryOutcome(fields.counterLegOutcomeLabel ?? '');
  const counterOutcome = outcomes.find((outcome) => outcome.legSide === counterSide) ?? null;
  if (!counterOutcome) {
    return {
      primaryPrice: Number(primaryOutcome.price ?? 0),
      counterPrice: 0,
      primaryBudgetUsdc,
      remainingBudgetUsdc: totalBudgetUsdc - primaryBudgetUsdc,
      commonQty: 0,
      projectedNetProfitUsdc: 0,
      residueQty: 0,
      blockedReason: 'missing_counter_outcome',
    };
  }
  const primaryPrice = Number(primaryOutcome.price ?? NaN);
  const counterPrice = Number(counterOutcome.price ?? NaN);
  if (!Number.isFinite(primaryPrice) || primaryPrice <= 0 || !Number.isFinite(counterPrice) || counterPrice <= 0) {
    return {
      primaryPrice: Number.isFinite(primaryPrice) ? primaryPrice : 0,
      counterPrice: Number.isFinite(counterPrice) ? counterPrice : 0,
      primaryBudgetUsdc,
      remainingBudgetUsdc: totalBudgetUsdc - primaryBudgetUsdc,
      commonQty: 0,
      projectedNetProfitUsdc: 0,
      residueQty: 0,
      blockedReason: 'missing_price',
    };
  }
  const pairMaxTotalCent = Number(fields.pairMaxTotalCent ?? '');
  if (Number.isFinite(pairMaxTotalCent) && pairMaxTotalCent > 0 && primaryPrice + counterPrice > pairMaxTotalCent / 100) {
    return {
      primaryPrice,
      counterPrice,
      primaryBudgetUsdc,
      remainingBudgetUsdc: totalBudgetUsdc - primaryBudgetUsdc,
      commonQty: 0,
      projectedNetProfitUsdc: 0,
      residueQty: 0,
      blockedReason: 'above_max_total',
    };
  }
  const remainingBudgetUsdc = totalBudgetUsdc - primaryBudgetUsdc;
  const primaryGrossQty = primaryBudgetUsdc / primaryPrice;
  const counterGrossQty = remainingBudgetUsdc / counterPrice;
  const primaryFeeQty = estimatePairLockBuyFeeQty(
    primaryPrice,
    primaryGrossQty,
    Number(primaryOutcome.feeRateBps ?? 0)
  );
  const counterFeeQty = estimatePairLockBuyFeeQty(
    counterPrice,
    counterGrossQty,
    Number(counterOutcome.feeRateBps ?? 0)
  );
  const primaryNetQty = Math.max(0, primaryGrossQty - primaryFeeQty);
  const counterNetQty = Math.max(0, counterGrossQty - counterFeeQty);
  const commonQty = Math.max(0, Math.min(primaryNetQty, counterNetQty));
  const primaryNetRatio = primaryGrossQty > 0 ? primaryNetQty / primaryGrossQty : 0;
  const counterNetRatio = counterGrossQty > 0 ? counterNetQty / counterGrossQty : 0;
  const primaryCommonCost =
    primaryNetRatio > 0 ? (commonQty / primaryNetRatio) * primaryPrice : 0;
  const counterCommonCost =
    counterNetRatio > 0 ? (commonQty / counterNetRatio) * counterPrice : 0;
  return {
    primaryPrice,
    counterPrice,
    primaryBudgetUsdc,
    remainingBudgetUsdc,
    commonQty,
    projectedNetProfitUsdc: commonQty - primaryCommonCost - counterCommonCost,
    residueQty: Math.abs(primaryNetQty - counterNetQty),
    blockedReason: null,
  };
}

export function resolvePairLockCounterPtbVisibility(
  key: string,
  pairLockEnabled: boolean,
  fields: Record<string, string>
): boolean | null {
  if (
    key !== 'counterLegPriceToBeatMode' &&
    key !== 'counterLegPriceToBeatMaxDiff' &&
    key !== 'counterLegPriceToBeatMaxDiffUnit'
  ) {
    return null;
  }
  const guardEnabled =
    (fields.counterLegPriceToBeatGuardEnabled ?? '').trim().toLowerCase() === 'true';
  if (key === 'counterLegPriceToBeatMode') {
    return pairLockEnabled && guardEnabled;
  }
  const manualMode = normalizePtbMode(fields.counterLegPriceToBeatMode) === 'manual';
  return pairLockEnabled && guardEnabled && manualMode;
}

export function resolvePairLockUiState(
  nodeType: string,
  fields: Record<string, string>
): {
  triggerBindingMode: string;
  placeOrderMode: 'single' | 'pair_lock';
  placeOrderPairLockEnabled: boolean;
  placeOrderCounterOutcomePreview: string;
} {
  const triggerBindingMode = (fields.bindingMode ?? '').trim().toLowerCase();
  const placeOrderMode =
    nodeType === 'action.place_order' && (fields.mode ?? '').trim().toLowerCase() === 'pair_lock'
      ? 'pair_lock'
      : 'single';
  return {
    triggerBindingMode,
    placeOrderMode,
    placeOrderPairLockEnabled: nodeType === 'action.place_order' && placeOrderMode === 'pair_lock',
    placeOrderCounterOutcomePreview: resolvePairLockCounterOutcomePreview(
      (fields.outcomeLabel ?? '').trim()
    ),
  };
}
