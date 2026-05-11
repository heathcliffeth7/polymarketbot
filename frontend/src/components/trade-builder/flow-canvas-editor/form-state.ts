import {
  createEmptyConditionDraft,
  createEmptyDrawdownRuleRow,
  createEmptyKeyValueDraft,
  createEmptyOutcomeConditionRow,
  isPresetPlaceOrderMarker,
  normalizePtbMode,
  normalizePtbStopLossGapUnit,
  type ConditionDraft,
  type DrawdownRuleRow,
  type NodeConfigFormState,
  type OutcomeConditionRow,
  type PrimitiveValueType,
} from '@/lib/trade-flow-config-mappers';
import {
  applyLiveGapCollectorFormDefaults,
  LIVE_GAP_COLLECTOR_MODE,
} from '@/lib/trade-flow-config-mappers/live-gap-collector';
import { normalizePairLockStrategy } from '@/lib/trade-flow-config-mappers/pair-lock';
import type {
  UpstreamFixedMarketResolution,
  UpstreamMaxPriceResolution,
} from '../flow-canvas-utils';

function getTriggerMarketSource(fields: Record<string, string>): string | null {
  const marketMode = (fields.marketMode ?? '').trim().toLowerCase();
  const marketSlug = (fields.marketSlug ?? '').trim();
  const marketScope = (fields.marketScope ?? '').trim();
  const source = marketMode === 'auto_scope' ? marketScope : marketSlug;
  return source || null;
}

function getOutcomeSource(nodeType: string, fields: Record<string, string>): string | null {
  if (nodeType === 'trigger.market_price') {
    return getTriggerMarketSource(fields);
  }
  if (nodeType === 'trigger.open_positions' || nodeType === 'trigger.position_drawdown') {
    return (fields.marketSlug ?? '').trim() || null;
  }
  return null;
}

function shouldResetDependentSelections(nodeType: string, key: string): boolean {
  if (nodeType === 'trigger.market_price') {
    return key === 'marketMode' || key === 'marketSlug' || key === 'marketScope';
  }
  if (nodeType === 'trigger.open_positions' || nodeType === 'trigger.position_drawdown') {
    return key === 'marketSlug';
  }
  return false;
}

function normalizePairLockCounterPtbMode(value: string): string {
  return normalizePtbMode(value);
}

function normalizePairLockCounterPtbUnit(value: string): 'usd' | 'cent' {
  return value.trim().toLowerCase() === 'cent' ? 'cent' : 'usd';
}

function normalizeStopLossTriggerPriceMode(value: string): string {
  const normalized = value.trim().toLowerCase();
  return ['best_bid', 'composite', 'composite_safe', 'composite_fast', 'last_trade'].includes(
    normalized
  )
    ? normalized
    : 'best_bid';
}

function isLevelTriggerCondition(value: unknown): boolean {
  const normalized = String(value ?? '').trim().toLowerCase();
  return normalized === 'level_above' || normalized === 'level_below';
}

export function updateNodeFieldState(
  prev: NodeConfigFormState | null,
  nodeType: string,
  key: string,
  value: string
): NodeConfigFormState | null {
  if (!prev) return prev;

  let nextFields = { ...prev.fields, [key]: value };
  let next: NodeConfigFormState = { ...prev, fields: nextFields };
  if (nodeType === 'action.place_order' && key === 'maxPriceCent' && prev.placeOrderMaxPriceUi) {
    next = {
      ...next,
      placeOrderMaxPriceUi: {
        ...prev.placeOrderMaxPriceUi,
        isInheritedValue: false,
      },
    };
  }
  if (nodeType === 'action.place_order' && prev.placeOrderMarketSeedUi) {
    const nextUi = { ...prev.placeOrderMarketSeedUi };
    if (key === 'marketSlug') {
      nextUi.isInheritedMarketSlug = false;
      if (prev.placeOrderMarketSeedUi.isInheritedTokenId || prev.placeOrderMarketSeedUi.isInheritedOutcomeLabel) {
        nextFields = { ...nextFields, tokenId: '', outcomeLabel: '' };
        next = { ...next, fields: nextFields };
      }
      nextUi.isInheritedTokenId = false;
      nextUi.isInheritedOutcomeLabel = false;
      next = { ...next, placeOrderMarketSeedUi: nextUi };
    } else if (key === 'tokenId') {
      nextUi.isInheritedTokenId = false;
      next = { ...next, placeOrderMarketSeedUi: nextUi };
    } else if (key === 'outcomeLabel') {
      nextUi.isInheritedOutcomeLabel = false;
      next = { ...next, placeOrderMarketSeedUi: nextUi };
    }
  }
  if (!shouldResetDependentSelections(nodeType, key)) {
    if (nodeType === 'action.place_order' && key === 'mode' && value === 'pair_lock') {
      const nextFields = {
        ...next.fields,
        mode: 'pair_lock',
        pairSizingMode:
          (next.fields.pairSizingMode ?? '').trim().toLowerCase() === 'auto_remaining_budget'
            ? 'auto_remaining_budget'
            : 'manual',
        pairLockStrategy: normalizePairLockStrategy(next.fields.pairLockStrategy ?? ''),
        side: 'buy',
        executionMode: 'limit',
        kind: 'immediate',
        sizeMode: 'usdc',
        sizePct: '',
        counterLegEnabled: 'true',
        pairProtectiveUnwindEnabled: 'true',
        counterLegOutcomeLabel:
          (next.fields.counterLegOutcomeLabel ?? '').trim() || 'opposite',
      };
      return { ...next, fields: nextFields };
    }
    if (
      nodeType === 'action.place_order' &&
      key === 'mode' &&
      value === LIVE_GAP_COLLECTOR_MODE
    ) {
      const nextFields = {
        ...next.fields,
        mode: LIVE_GAP_COLLECTOR_MODE,
        side: 'buy',
        executionMode: 'market',
        kind: 'immediate',
        sizeMode: 'usdc',
        tpEnabled: 'true',
        tpPriceCent: '98',
        maxPriceCent: '93',
        liveGapCollectorEnabled: 'true',
        notifyOnLiveGapCollectorDecision: 'true',
      };
      applyLiveGapCollectorFormDefaults(nextFields);
      return { ...next, fields: nextFields };
    }
    if (nodeType === 'action.place_order' && key === 'pairLockStrategy') {
      const pairLockStrategy = normalizePairLockStrategy(value);
      const nextFields: Record<string, string> = {
        ...next.fields,
        pairLockStrategy,
      };
      if (pairLockStrategy === 'edge_pairlock_v1') {
        nextFields.priceToBeatGuardEnabled = 'true';
        nextFields.priceToBeatMode = 'iv_mismatch_edge';
        nextFields.pairSizingMode = 'manual';
        nextFields.counterLegEnabled = 'true';
        nextFields.pairLockDecisionQty = (nextFields.pairLockDecisionQty ?? '').trim() || '5';
        nextFields.pairLockSingleEdgeThreshold =
          (nextFields.pairLockSingleEdgeThreshold ?? '').trim() || '0.10';
        nextFields.pairLockCostBuffer = (nextFields.pairLockCostBuffer ?? '').trim() || '0.005';
        nextFields.pairMaxTotalCent = (nextFields.pairMaxTotalCent ?? '').trim() || '95';
      } else if (pairLockStrategy === 'adaptive_max_price_v1') {
        nextFields.priceToBeatGuardEnabled = 'true';
        nextFields.priceToBeatMode = 'iv_mismatch_edge';
        nextFields.counterLegEnabled = 'true';
        nextFields.adaptiveMaxPriceMissCount =
          (nextFields.adaptiveMaxPriceMissCount ?? '').trim() || '3';
        nextFields.adaptiveMaxPriceRequiredGoodMissCount =
          (nextFields.adaptiveMaxPriceRequiredGoodMissCount ?? '').trim() || '2';
        nextFields.adaptiveMaxPriceRelaxCreditCent =
          (nextFields.adaptiveMaxPriceRelaxCreditCent ?? '').trim() || '2';
        nextFields.adaptiveMaxPriceMaxRelaxCreditCent =
          (nextFields.adaptiveMaxPriceMaxRelaxCreditCent ?? '').trim() || '5';
        nextFields.adaptiveMaxPriceHardCapCent =
          (nextFields.adaptiveMaxPriceHardCapCent ?? '').trim() || '76';
        nextFields.adaptiveMaxPriceExtraBufferCent =
          (nextFields.adaptiveMaxPriceExtraBufferCent ?? '').trim() || '1';
        nextFields.adaptiveMaxPricePairBufferCent =
          (nextFields.adaptiveMaxPricePairBufferCent ?? '').trim() || '1';
        nextFields.adaptiveMaxPriceSizeMultiplier =
          (nextFields.adaptiveMaxPriceSizeMultiplier ?? '').trim() || '0.5';
        nextFields.adaptiveMaxPriceLateRiskEnabled =
          (nextFields.adaptiveMaxPriceLateRiskEnabled ?? '').trim() || 'true';
        nextFields.adaptiveMaxPriceLateRiskAfterSec =
          (nextFields.adaptiveMaxPriceLateRiskAfterSec ?? '').trim() || '210';
        nextFields.adaptiveMaxPriceLateExtraBufferCent =
          (nextFields.adaptiveMaxPriceLateExtraBufferCent ?? '').trim() || '1';
        nextFields.adaptiveMaxPriceLateSizeMultiplier =
          (nextFields.adaptiveMaxPriceLateSizeMultiplier ?? '').trim() || '0.35';
        nextFields.adaptiveMaxPriceSlCooldownMarkets =
          (nextFields.adaptiveMaxPriceSlCooldownMarkets ?? '').trim() || '3';
        nextFields.notifyOnAdaptiveMaxPriceEvaluated =
          (nextFields.notifyOnAdaptiveMaxPriceEvaluated ?? '').trim() || 'false';
        nextFields.notifyOnAdaptiveMaxPriceRelax =
          (nextFields.notifyOnAdaptiveMaxPriceRelax ?? '').trim() || 'true';
        nextFields.notifyOnAdaptiveMaxPriceRelaxSl =
          (nextFields.notifyOnAdaptiveMaxPriceRelaxSl ?? '').trim() || 'true';
        nextFields.notifyOnAdaptiveMaxPriceNoRelaxImportant =
          (nextFields.notifyOnAdaptiveMaxPriceNoRelaxImportant ?? '').trim() || 'true';
        nextFields.notifyOnAdaptiveMaxPriceMissResolved =
          (nextFields.notifyOnAdaptiveMaxPriceMissResolved ?? '').trim() || 'true';
        nextFields.notifyOnAdaptiveMaxPriceCooldown =
          (nextFields.notifyOnAdaptiveMaxPriceCooldown ?? '').trim() || 'true';
        nextFields.notifyOnAdaptiveMaxPriceSummary =
          (nextFields.notifyOnAdaptiveMaxPriceSummary ?? '').trim() || 'true';
        nextFields.notifyOnAdaptiveMaxPriceAllNoRelax =
          (nextFields.notifyOnAdaptiveMaxPriceAllNoRelax ?? '').trim() || 'false';
        nextFields.adaptiveMaxPriceNotifyMinIntervalSec =
          (nextFields.adaptiveMaxPriceNotifyMinIntervalSec ?? '').trim() || '30';
        nextFields.adaptiveMaxPriceNotifyIncludePayload =
          (nextFields.adaptiveMaxPriceNotifyIncludePayload ?? '').trim() || 'false';
        nextFields.adaptiveMaxPriceSummaryEveryMarkets =
          (nextFields.adaptiveMaxPriceSummaryEveryMarkets ?? '').trim() || '5';
      } else if (pairLockStrategy === 'manual_adaptive_risk_v1') {
        nextFields.priceToBeatGuardEnabled = 'true';
        nextFields.priceToBeatMode = 'manual';
        nextFields.counterLegEnabled = 'true';
        nextFields.manualAdaptiveVolumeNormalLt =
          (nextFields.manualAdaptiveVolumeNormalLt ?? '').trim() || '1.5';
        nextFields.manualAdaptiveVolumeElevatedLt =
          (nextFields.manualAdaptiveVolumeElevatedLt ?? '').trim() || '2.5';
        nextFields.manualAdaptiveVolumeHighLt =
          (nextFields.manualAdaptiveVolumeHighLt ?? '').trim() || '4';
        nextFields.manualAdaptiveTrendDeltaUsd =
          (nextFields.manualAdaptiveTrendDeltaUsd ?? '').trim() || '0.05';
        nextFields.manualAdaptiveNormalFlatMaxPriceSubCent =
          (nextFields.manualAdaptiveNormalFlatMaxPriceSubCent ?? '').trim() || '2';
        nextFields.manualAdaptiveNormalFlatSizeMultiplier =
          (nextFields.manualAdaptiveNormalFlatSizeMultiplier ?? '').trim() || '0.8';
        nextFields.manualAdaptiveNormalFlatPtbGapAddCent =
          (nextFields.manualAdaptiveNormalFlatPtbGapAddCent ?? '').trim() || '5';
        nextFields.manualAdaptiveNormalCollapsingMaxPriceCent =
          (nextFields.manualAdaptiveNormalCollapsingMaxPriceCent ?? '').trim() || '62';
        nextFields.manualAdaptiveNormalCollapsingSizeMultiplier =
          (nextFields.manualAdaptiveNormalCollapsingSizeMultiplier ?? '').trim() || '0.4';
        nextFields.manualAdaptiveNormalCollapsingPtbGapAddCent =
          (nextFields.manualAdaptiveNormalCollapsingPtbGapAddCent ?? '').trim() || '15';
        nextFields.manualAdaptiveElevatedMaxPriceCent =
          (nextFields.manualAdaptiveElevatedMaxPriceCent ?? '').trim() || '66';
        nextFields.manualAdaptiveElevatedSizeMultiplier =
          (nextFields.manualAdaptiveElevatedSizeMultiplier ?? '').trim() || '0.6';
        nextFields.manualAdaptiveElevatedPtbGapAddCent =
          (nextFields.manualAdaptiveElevatedPtbGapAddCent ?? '').trim() || '10';
        nextFields.manualAdaptiveHighMaxPriceCent =
          (nextFields.manualAdaptiveHighMaxPriceCent ?? '').trim() || '58';
        nextFields.manualAdaptiveHighSizeMultiplier =
          (nextFields.manualAdaptiveHighSizeMultiplier ?? '').trim() || '0.3';
        nextFields.manualAdaptiveHighPtbGapAddCent =
          (nextFields.manualAdaptiveHighPtbGapAddCent ?? '').trim() || '25';
        nextFields.manualAdaptiveAfterSlMaxPriceSubCent =
          (nextFields.manualAdaptiveAfterSlMaxPriceSubCent ?? '').trim() || '5';
        nextFields.manualAdaptiveAfterSlPtbGapAddCent =
          (nextFields.manualAdaptiveAfterSlPtbGapAddCent ?? '').trim() || '15';
        nextFields.manualAdaptiveSlCooldownMarkets =
          (nextFields.manualAdaptiveSlCooldownMarkets ?? '').trim() || '3';
        nextFields.manualAdaptivePairBufferCent =
          (nextFields.manualAdaptivePairBufferCent ?? '').trim() || '1';
        nextFields.manualAdaptiveSelfTuneEnabled =
          (nextFields.manualAdaptiveSelfTuneEnabled ?? '').trim() || 'false';
        nextFields.manualAdaptiveMissRelaxEnabled =
          (nextFields.manualAdaptiveMissRelaxEnabled ?? '').trim() || 'true';
        nextFields.manualAdaptiveMissRelaxAfterNoOrderMarkets =
          (nextFields.manualAdaptiveMissRelaxAfterNoOrderMarkets ?? '').trim() || '3';
        nextFields.manualAdaptiveTrendDeltaUsdByScope =
          (nextFields.manualAdaptiveTrendDeltaUsdByScope ?? '').trim() ||
          '{"eth_5m_updown":0.5,"btc_5m_updown":10,"sol_5m_updown":0.05}';
        nextFields.manualAdaptivePtbRelaxStepCent =
          (nextFields.manualAdaptivePtbRelaxStepCent ?? '').trim() || '5';
        nextFields.manualAdaptivePtbRelaxMaxCent =
          (nextFields.manualAdaptivePtbRelaxMaxCent ?? '').trim() || '20';
        nextFields.manualAdaptiveMaxPriceRelaxStepCent =
          (nextFields.manualAdaptiveMaxPriceRelaxStepCent ?? '').trim() || '1';
        nextFields.manualAdaptiveMaxPriceRelaxMaxCent =
          (nextFields.manualAdaptiveMaxPriceRelaxMaxCent ?? '').trim() || '5';
        nextFields.manualAdaptiveMaxPriceRelaxHardCapCent =
          (nextFields.manualAdaptiveMaxPriceRelaxHardCapCent ?? '').trim() || '90';
        nextFields.manualAdaptiveMissRelaxSizeMultiplier =
          (nextFields.manualAdaptiveMissRelaxSizeMultiplier ?? '').trim() || '0.8';
        nextFields.manualAdaptiveSlTightenEnabled =
          (nextFields.manualAdaptiveSlTightenEnabled ?? '').trim() || 'true';
        nextFields.manualAdaptivePtbSlBumpStepCent =
          (nextFields.manualAdaptivePtbSlBumpStepCent ?? '').trim() || '15';
        nextFields.manualAdaptivePtbSlBumpMaxCent =
          (nextFields.manualAdaptivePtbSlBumpMaxCent ?? '').trim() || '45';
        nextFields.manualAdaptiveMaxPriceSlPenaltyStepCent =
          (nextFields.manualAdaptiveMaxPriceSlPenaltyStepCent ?? '').trim() || '5';
        nextFields.manualAdaptiveMaxPriceSlPenaltyMaxCent =
          (nextFields.manualAdaptiveMaxPriceSlPenaltyMaxCent ?? '').trim() || '15';
        nextFields.manualAdaptiveSlDisableReentry =
          (nextFields.manualAdaptiveSlDisableReentry ?? '').trim() || 'true';
        nextFields.manualAdaptiveConsecutiveSlLockdownAfter =
          (nextFields.manualAdaptiveConsecutiveSlLockdownAfter ?? '').trim() || '3';
        nextFields.manualAdaptiveLockdownReleaseCleanMarkets =
          (nextFields.manualAdaptiveLockdownReleaseCleanMarkets ?? '').trim() || '3';
        nextFields.manualAdaptiveLockdownMaxMarkets =
          (nextFields.manualAdaptiveLockdownMaxMarkets ?? '').trim() || '5';
        nextFields.manualAdaptiveCleanMarketDecayEnabled =
          (nextFields.manualAdaptiveCleanMarketDecayEnabled ?? '').trim() || 'true';
        nextFields.manualAdaptivePtbRelaxDecayPerMarketCent =
          (nextFields.manualAdaptivePtbRelaxDecayPerMarketCent ?? '').trim() || '5';
        nextFields.manualAdaptivePtbSlBumpDecayPerCleanMarketCent =
          (nextFields.manualAdaptivePtbSlBumpDecayPerCleanMarketCent ?? '').trim() || '5';
        nextFields.manualAdaptiveMaxPriceRelaxDecayPerMarketCent =
          (nextFields.manualAdaptiveMaxPriceRelaxDecayPerMarketCent ?? '').trim() || '1';
        nextFields.manualAdaptiveMaxPriceSlPenaltyDecayPerCleanMarketCent =
          (nextFields.manualAdaptiveMaxPriceSlPenaltyDecayPerCleanMarketCent ?? '').trim() || '2';
        nextFields.notifyOnManualAdaptiveRiskBlock =
          (nextFields.notifyOnManualAdaptiveRiskBlock ?? '').trim() || 'true';
        nextFields.notifyOnManualAdaptiveRiskStrict =
          (nextFields.notifyOnManualAdaptiveRiskStrict ?? '').trim() || 'true';
        nextFields.notifyOnManualAdaptiveRiskSlBump =
          (nextFields.notifyOnManualAdaptiveRiskSlBump ?? '').trim() || 'true';
        nextFields.notifyOnManualAdaptiveRiskSummary =
          (nextFields.notifyOnManualAdaptiveRiskSummary ?? '').trim() || 'true';
        nextFields.notifyOnManualAdaptiveCounterCap =
          (nextFields.notifyOnManualAdaptiveCounterCap ?? '').trim() || 'true';
        nextFields.manualAdaptiveCounterCapNotifyMinDeltaCent =
          (nextFields.manualAdaptiveCounterCapNotifyMinDeltaCent ?? '').trim() || '3';
        nextFields.manualAdaptiveNotifySummaryEveryMarkets =
          (nextFields.manualAdaptiveNotifySummaryEveryMarkets ?? '').trim() || '5';
        nextFields.manualAdaptiveNotifyMinIntervalSec =
          (nextFields.manualAdaptiveNotifyMinIntervalSec ?? '').trim() || '30';
        nextFields.manualAdaptiveNotifyIncludePayload =
          (nextFields.manualAdaptiveNotifyIncludePayload ?? '').trim() || 'false';
      } else if (pairLockStrategy === 'biased_hedge_v1') {
        nextFields.priceToBeatGuardEnabled = 'true';
        nextFields.priceToBeatMode = 'iv_mismatch_edge';
        nextFields.pairSizingMode = 'manual';
        nextFields.counterLegEnabled = 'true';
        nextFields.tpEnabled = 'false';
        nextFields.sizeMode = 'usdc';
        nextFields.sizeUsdc = (nextFields.sizeUsdc ?? '').trim() || '2';
        nextFields.maxPriceCent = (nextFields.maxPriceCent ?? '').trim() || '75';
        nextFields.pairProtectiveUnwindEnabled = 'true';
        nextFields.pairOrphanGraceMs = (nextFields.pairOrphanGraceMs ?? '').trim() || '1500';
        nextFields.reentryMaxAttempts = '0';
        nextFields.biasedHedgePrimaryBudgetUsdc = (nextFields.biasedHedgePrimaryBudgetUsdc ?? '').trim() || '2';
        nextFields.biasedHedgeHedgeBudgetUsdc = (nextFields.biasedHedgeHedgeBudgetUsdc ?? '').trim() || '0.5';
        nextFields.biasedHedgeMinDominantShare = (nextFields.biasedHedgeMinDominantShare ?? '').trim() || '0.75';
        nextFields.biasedHedgeMaxHedgeSpendRatio = (nextFields.biasedHedgeMaxHedgeSpendRatio ?? '').trim() || '0.25';
        nextFields.biasedHedgePrimaryMinEdge = (nextFields.biasedHedgePrimaryMinEdge ?? '').trim() || '0.08';
        nextFields.biasedHedgePrimaryMinFinalQ = (nextFields.biasedHedgePrimaryMinFinalQ ?? '').trim() || '0.72';
        nextFields.biasedHedgeMaxPriceCent = (nextFields.biasedHedgeMaxPriceCent ?? '').trim() || '75';
        nextFields.biasedHedgeHighPriceCent = (nextFields.biasedHedgeHighPriceCent ?? '').trim() || '70';
        nextFields.biasedHedgeHighPriceMinFinalQ = (nextFields.biasedHedgeHighPriceMinFinalQ ?? '').trim() || '0.82';
        nextFields.biasedHedgeHighPriceMinEdge = (nextFields.biasedHedgeHighPriceMinEdge ?? '').trim() || '0.10';
        nextFields.biasedHedgeHedgeOnlyIfPrimaryFilled = 'true';
        nextFields.biasedHedgeHedgeMinPriceCent = (nextFields.biasedHedgeHedgeMinPriceCent ?? '').trim() || '3';
        nextFields.biasedHedgeHedgeMaxPriceCent = (nextFields.biasedHedgeHedgeMaxPriceCent ?? '').trim() || '25';
        nextFields.biasedHedgeDisableNewPrimaryAfterSec = (nextFields.biasedHedgeDisableNewPrimaryAfterSec ?? '').trim() || '180';
        nextFields.biasedHedgeDisableAnyBuyAfterSec = (nextFields.biasedHedgeDisableAnyBuyAfterSec ?? '').trim() || '240';
        nextFields.biasedHedgeMaxSideSwitchCount = (nextFields.biasedHedgeMaxSideSwitchCount ?? '').trim() || '0';
        nextFields.biasedHedgeMaxPairedEffectiveCostCent = (nextFields.biasedHedgeMaxPairedEffectiveCostCent ?? '').trim() || '95';
        nextFields.biasedHedgeStopBiasInvalidationEnabled = 'true';
        nextFields.biasedHedgeStopMinQFinalToHold = (nextFields.biasedHedgeStopMinQFinalToHold ?? '').trim() || '0.55';
        nextFields.biasedHedgeStopMinEdgeToHold = (nextFields.biasedHedgeStopMinEdgeToHold ?? '').trim() || '0';
        nextFields.biasedHedgeStopExitPctOnInvalidation = (nextFields.biasedHedgeStopExitPctOnInvalidation ?? '').trim() || '100';
        nextFields.biasedHedgeStopPtbStopLossEnabled = 'true';
        nextFields.biasedHedgeStopPtbStopLossGapUsd = (nextFields.biasedHedgeStopPtbStopLossGapUsd ?? '').trim() || '-3';
        nextFields.biasedHedgeStopPtbStopLossTimeDecayMode = (nextFields.biasedHedgeStopPtbStopLossTimeDecayMode ?? '').trim() || 'tighten';
        nextFields.biasedHedgeStopTimeExitRulesJson = (nextFields.biasedHedgeStopTimeExitRulesJson ?? '').trim() || '[{"elapsedSec":90,"remainingPct":60},{"elapsedSec":150,"remainingPct":0}]';
      }
      return {
        ...next,
        fields: nextFields,
      };
    }
    if (
      nodeType === 'action.place_order' &&
      key === 'counterLegPriceToBeatGuardEnabled' &&
      value === 'true'
    ) {
      const counterLegPriceToBeatMode = normalizePairLockCounterPtbMode(
        next.fields.counterLegPriceToBeatMode ?? ''
      );
      return {
        ...next,
        fields: {
          ...next.fields,
          counterLegPriceToBeatMode,
          ...(counterLegPriceToBeatMode === 'manual'
            ? {
                counterLegPriceToBeatMaxDiffUnit: normalizePairLockCounterPtbUnit(
                  next.fields.counterLegPriceToBeatMaxDiffUnit ?? ''
                ),
              }
            : {}),
        },
      };
    }
    if (nodeType === 'action.place_order' && key === 'counterLegPriceToBeatMode') {
      const counterLegPriceToBeatMode = normalizePairLockCounterPtbMode(value);
      return {
        ...next,
        fields: {
          ...next.fields,
          counterLegPriceToBeatMode,
          ...(counterLegPriceToBeatMode === 'manual'
            ? {
                counterLegPriceToBeatMaxDiffUnit: normalizePairLockCounterPtbUnit(
                  next.fields.counterLegPriceToBeatMaxDiffUnit ?? ''
                ),
              }
            : {}),
        },
      };
    }
    if (nodeType === 'action.place_order' && key === 'counterLegSlEnabled' && value === 'true') {
      return {
        ...next,
        fields: {
          ...next.fields,
          counterLegSlTriggerPriceMode: normalizeStopLossTriggerPriceMode(
            next.fields.counterLegSlTriggerPriceMode ?? next.fields.slTriggerPriceMode ?? ''
          ),
        },
      };
    }
    if (nodeType === 'action.place_order' && key === 'ptbStopLossEnabled' && value === 'true') {
      return {
        ...next,
        fields: {
          ...next.fields,
          ptbStopLossGapUnit: normalizePtbStopLossGapUnit(next.fields.ptbStopLossGapUnit ?? ''),
          ptbStopLossTimeDecayMode:
            (next.fields.ptbStopLossTimeDecayMode ?? '').trim() || 'tighten',
        },
      };
    }
    if (
      nodeType === 'action.place_order' &&
      key === 'counterLegPtbStopLossEnabled' &&
      value === 'true'
    ) {
      return {
        ...next,
        fields: {
          ...next.fields,
          counterLegPtbStopLossGapUnit: normalizePtbStopLossGapUnit(
            next.fields.counterLegPtbStopLossGapUnit ?? '',
            normalizePtbStopLossGapUnit(next.fields.ptbStopLossGapUnit ?? '')
          ),
          counterLegPtbStopLossTimeDecayMode:
            (next.fields.counterLegPtbStopLossTimeDecayMode ?? '').trim() ||
            (next.fields.ptbStopLossTimeDecayMode ?? '').trim() ||
            'tighten',
        },
      };
    }
    if (nodeType === 'action.place_order' && key === 'pairSizingMode') {
      const pairSizingMode =
        value.trim().toLowerCase() === 'auto_remaining_budget'
          ? 'auto_remaining_budget'
          : 'manual';
      const nextFields: Record<string, string> = {
        ...next.fields,
        pairSizingMode,
      };
      if (
        pairSizingMode === 'auto_remaining_budget' &&
        !(nextFields.pairTotalBudgetUsdc ?? '').trim()
      ) {
        const primaryBudgetUsdc = Number(nextFields.sizeUsdc ?? '');
        const counterBudgetUsdc = Number(nextFields.counterLegSizeUsdc ?? '');
        if (Number.isFinite(primaryBudgetUsdc) && primaryBudgetUsdc > 0) {
          const totalBudgetUsdc =
            primaryBudgetUsdc +
            (Number.isFinite(counterBudgetUsdc) && counterBudgetUsdc > 0 ? counterBudgetUsdc : 0);
          nextFields.pairTotalBudgetUsdc = String(totalBudgetUsdc);
        }
      }
      return {
        ...next,
        fields: nextFields,
      };
    }
    return next;
  }

  const previousSource = getOutcomeSource(nodeType, prev.fields);
  const nextSource = getOutcomeSource(nodeType, nextFields);
  if (previousSource === nextSource) {
    return next;
  }

  if (nodeType === 'trigger.position_drawdown') {
    const tokenId = (next.fields.tokenId ?? '').trim();
    const outcomeLabel = (next.fields.outcomeLabel ?? '').trim();
    if (!tokenId && !outcomeLabel) {
      return next;
    }
    return {
      ...next,
      fields: {
        ...next.fields,
        tokenId: '',
        outcomeLabel: '',
      },
    };
  }

  if (next.outcomeConditionRows.length === 0) {
    return next;
  }

  next = {
    ...next,
    outcomeConditionRows: [],
  };
  return next;
}

export function updateTriggerSizeRowState(
  prev: NodeConfigFormState | null,
  index: number,
  value: string
): NodeConfigFormState | null {
  if (!prev) return prev;
  const nextRows = [...prev.triggerSizeRows];
  while (nextRows.length <= index) nextRows.push('');
  nextRows[index] = value;
  return { ...prev, triggerSizeRows: nextRows };
}

export function syncPlaceOrderTriggerRowsState(
  prev: NodeConfigFormState | null
): NodeConfigFormState | null {
  if (!prev) return prev;
  const parsedMax = Number(prev.fields.maxTriggers ?? '');
  const targetCount =
    Number.isFinite(parsedMax) && parsedMax > 1 ? Math.min(20, Math.floor(parsedMax)) : 0;
  const currentRows = prev.triggerSizeRows || [];
  const nextRows =
    targetCount > 0
      ? Array.from({ length: targetCount }, (_, i) => currentRows[i] ?? '')
      : [];
  const unchanged =
    nextRows.length === currentRows.length && nextRows.every((v, i) => v === currentRows[i]);
  if (unchanged) return prev;
  return { ...prev, triggerSizeRows: nextRows };
}

function sameDistinctValues(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

export function syncPlaceOrderInheritedMaxPriceState(
  prev: NodeConfigFormState | null,
  resolution: UpstreamMaxPriceResolution
): NodeConfigFormState | null {
  if (!prev) return prev;

  const currentMaxPriceCent = (prev.fields.maxPriceCent ?? '').trim();
  const wasInherited = prev.placeOrderMaxPriceUi?.isInheritedValue === true;
  let nextFields = prev.fields;
  let isInheritedValue = false;

  if (resolution.kind === 'single' && resolution.maxPriceCent) {
    if (wasInherited || currentMaxPriceCent.length === 0) {
      if (currentMaxPriceCent !== resolution.maxPriceCent) {
        nextFields = { ...prev.fields, maxPriceCent: resolution.maxPriceCent };
      }
      isInheritedValue = true;
    }
  } else if (wasInherited && currentMaxPriceCent.length > 0) {
    nextFields = { ...prev.fields, maxPriceCent: '' };
  }

  const nextUi = {
    isInheritedValue,
    upstreamKind: resolution.kind,
    upstreamMaxPriceCent: resolution.maxPriceCent,
    distinctUpstreamMaxPriceCents: resolution.distinctMaxPriceCents,
  } as const;

  const prevUi = prev.placeOrderMaxPriceUi;
  const fieldsUnchanged = nextFields === prev.fields;
  const uiUnchanged =
    prevUi != null &&
    prevUi.isInheritedValue === nextUi.isInheritedValue &&
    prevUi.upstreamKind === nextUi.upstreamKind &&
    prevUi.upstreamMaxPriceCent === nextUi.upstreamMaxPriceCent &&
    sameDistinctValues(prevUi.distinctUpstreamMaxPriceCents, nextUi.distinctUpstreamMaxPriceCents);

  if (fieldsUnchanged && uiUnchanged) {
    return prev;
  }

  return {
    ...prev,
    fields: nextFields,
    placeOrderMaxPriceUi: nextUi,
  };
}

export function syncPlaceOrderInheritedMarketState(
  prev: NodeConfigFormState | null,
  resolution: UpstreamFixedMarketResolution
): NodeConfigFormState | null {
  if (!prev) return prev;
  if (!isPresetPlaceOrderMarker(prev.fields.presetKind, prev.fields.refKey)) {
    return prev;
  }

  const currentMarketSlug = (prev.fields.marketSlug ?? '').trim();
  const currentTokenId = (prev.fields.tokenId ?? '').trim();
  const currentOutcomeLabel = (prev.fields.outcomeLabel ?? '').trim();
  const hasMarketSeedUi = prev.placeOrderMarketSeedUi != null;
  const wasInheritedMarketSlug = prev.placeOrderMarketSeedUi?.isInheritedMarketSlug === true;
  const wasInheritedTokenId = prev.placeOrderMarketSeedUi?.isInheritedTokenId === true;
  const wasInheritedOutcomeLabel = prev.placeOrderMarketSeedUi?.isInheritedOutcomeLabel === true;
  let nextFields = prev.fields;
  let isInheritedMarketSlug = false;
  let isInheritedTokenId = false;
  let isInheritedOutcomeLabel = false;

  if (resolution.kind === 'single' && resolution.marketSlug) {
    if (!hasMarketSeedUi || wasInheritedMarketSlug || currentMarketSlug.length === 0) {
      if (currentMarketSlug !== resolution.marketSlug) {
        nextFields = { ...nextFields, marketSlug: resolution.marketSlug };
      }
      isInheritedMarketSlug = true;
    }
  } else if (wasInheritedMarketSlug && currentMarketSlug.length > 0) {
    nextFields = { ...nextFields, marketSlug: '' };
  }

  if (
    resolution.kind === 'single' &&
    resolution.outcomeKind === 'single' &&
    resolution.tokenId &&
    resolution.outcomeLabel
  ) {
    if (!hasMarketSeedUi || wasInheritedTokenId || currentTokenId.length === 0) {
      if ((nextFields.tokenId ?? '').trim() !== resolution.tokenId) {
        nextFields = { ...nextFields, tokenId: resolution.tokenId };
      }
      isInheritedTokenId = true;
    }
    if (!hasMarketSeedUi || wasInheritedOutcomeLabel || currentOutcomeLabel.length === 0) {
      if ((nextFields.outcomeLabel ?? '').trim() !== resolution.outcomeLabel) {
        nextFields = { ...nextFields, outcomeLabel: resolution.outcomeLabel };
      }
      isInheritedOutcomeLabel = true;
    }
  } else {
    if (!hasMarketSeedUi && resolution.kind === 'single') {
      if (currentTokenId.length > 0 || currentOutcomeLabel.length > 0) {
        nextFields = { ...nextFields, tokenId: '', outcomeLabel: '' };
      }
    }
    if (wasInheritedTokenId && currentTokenId.length > 0) {
      nextFields = { ...nextFields, tokenId: '' };
    }
    if (wasInheritedOutcomeLabel && currentOutcomeLabel.length > 0) {
      nextFields = { ...nextFields, outcomeLabel: '' };
    }
  }

  const nextUi = {
    isInheritedMarketSlug,
    isInheritedTokenId,
    isInheritedOutcomeLabel,
    upstreamKind: resolution.kind,
    upstreamOutcomeKind: resolution.outcomeKind,
    upstreamMarketSlug: resolution.marketSlug,
    upstreamTokenId: resolution.tokenId,
    upstreamOutcomeLabel: resolution.outcomeLabel,
    distinctUpstreamMarketSlugs: resolution.distinctMarketSlugs,
    distinctUpstreamOutcomeLabels: resolution.distinctOutcomeLabels,
  } as const;

  const prevUi = prev.placeOrderMarketSeedUi;
  const fieldsUnchanged = nextFields === prev.fields;
  const uiUnchanged =
    prevUi != null &&
    prevUi.isInheritedMarketSlug === nextUi.isInheritedMarketSlug &&
    prevUi.isInheritedTokenId === nextUi.isInheritedTokenId &&
    prevUi.isInheritedOutcomeLabel === nextUi.isInheritedOutcomeLabel &&
    prevUi.upstreamKind === nextUi.upstreamKind &&
    prevUi.upstreamOutcomeKind === nextUi.upstreamOutcomeKind &&
    prevUi.upstreamMarketSlug === nextUi.upstreamMarketSlug &&
    prevUi.upstreamTokenId === nextUi.upstreamTokenId &&
    prevUi.upstreamOutcomeLabel === nextUi.upstreamOutcomeLabel &&
    sameDistinctValues(prevUi.distinctUpstreamMarketSlugs, nextUi.distinctUpstreamMarketSlugs) &&
    sameDistinctValues(prevUi.distinctUpstreamOutcomeLabels, nextUi.distinctUpstreamOutcomeLabels);

  if (fieldsUnchanged && uiUnchanged) {
    return prev;
  }

  return {
    ...prev,
    fields: nextFields,
    placeOrderMarketSeedUi: nextUi,
  };
}

export function updateExpressionRowState(
  prev: NodeConfigFormState | null,
  rowId: string,
  patch: Partial<ConditionDraft>
): NodeConfigFormState | null {
  if (!prev) return prev;
  return {
    ...prev,
    expressionRows: prev.expressionRows.map((r) => (r.id === rowId ? { ...r, ...patch } : r)),
  };
}

export function addExpressionRowState(prev: NodeConfigFormState | null): NodeConfigFormState | null {
  if (!prev) return prev;
  return { ...prev, expressionRows: [...prev.expressionRows, createEmptyConditionDraft()] };
}

export function removeExpressionRowState(
  prev: NodeConfigFormState | null,
  rowId: string
): NodeConfigFormState | null {
  if (!prev) return prev;
  const next = prev.expressionRows.filter((r) => r.id !== rowId);
  return { ...prev, expressionRows: next.length > 0 ? next : [createEmptyConditionDraft()] };
}

export function updateStatePatchRowState(
  prev: NodeConfigFormState | null,
  rowId: string,
  patch: Partial<{ key: string; value: string; valueType: PrimitiveValueType }>
): NodeConfigFormState | null {
  if (!prev) return prev;
  return {
    ...prev,
    statePatchRows: prev.statePatchRows.map((r) => (r.id === rowId ? { ...r, ...patch } : r)),
  };
}

export function addStatePatchRowState(prev: NodeConfigFormState | null): NodeConfigFormState | null {
  if (!prev) return prev;
  return { ...prev, statePatchRows: [...prev.statePatchRows, createEmptyKeyValueDraft()] };
}

export function removeStatePatchRowState(
  prev: NodeConfigFormState | null,
  rowId: string
): NodeConfigFormState | null {
  if (!prev) return prev;
  const next = prev.statePatchRows.filter((r) => r.id !== rowId);
  return { ...prev, statePatchRows: next.length > 0 ? next : [createEmptyKeyValueDraft()] };
}

export function addOutcomeConditionState(
  prev: NodeConfigFormState | null,
  tokenId: string,
  outcomeLabel: string
): NodeConfigFormState | null {
  if (!prev) return prev;
  const normalizedTokenId = tokenId.trim();
  const normalizedOutcomeLabel = outcomeLabel.trim();
  if (!normalizedTokenId || !normalizedOutcomeLabel) return prev;
  if (prev.outcomeConditionRows.some((r) => r.tokenId === normalizedTokenId)) return prev;
  const row: OutcomeConditionRow = {
    ...createEmptyOutcomeConditionRow(),
    tokenId: normalizedTokenId,
    outcomeLabel: normalizedOutcomeLabel,
  };
  return { ...prev, outcomeConditionRows: [...prev.outcomeConditionRows, row] };
}

export function removeOutcomeConditionState(
  prev: NodeConfigFormState | null,
  rowId: string
): NodeConfigFormState | null {
  if (!prev) return prev;
  return {
    ...prev,
    outcomeConditionRows: prev.outcomeConditionRows.filter((r) => r.id !== rowId),
  };
}

export function updateOutcomeConditionState(
  prev: NodeConfigFormState | null,
  rowId: string,
  patch: Partial<OutcomeConditionRow>
): NodeConfigFormState | null {
  if (!prev) return prev;
  const nextFields = isLevelTriggerCondition(patch.triggerCondition)
    ? { ...prev.fields, repeatMode: 'once', onceScope: 'market' }
    : prev.fields;
  return {
    ...prev,
    fields: nextFields,
    outcomeConditionRows: prev.outcomeConditionRows.map((r) => (r.id === rowId ? { ...r, ...patch } : r)),
  };
}

export function addDrawdownRuleState(prev: NodeConfigFormState | null): NodeConfigFormState | null {
  if (!prev) return prev;
  return {
    ...prev,
    drawdownRuleRows: [...(prev.drawdownRuleRows || []), createEmptyDrawdownRuleRow()],
  };
}

export function removeDrawdownRuleState(
  prev: NodeConfigFormState | null,
  rowId: string
): NodeConfigFormState | null {
  if (!prev) return prev;
  const next = (prev.drawdownRuleRows || []).filter((row) => row.id !== rowId);
  return {
    ...prev,
    drawdownRuleRows: next.length > 0 ? next : [createEmptyDrawdownRuleRow()],
  };
}

export function updateDrawdownRuleState(
  prev: NodeConfigFormState | null,
  rowId: string,
  patch: Partial<DrawdownRuleRow>
): NodeConfigFormState | null {
  if (!prev) return prev;
  return {
    ...prev,
    drawdownRuleRows: (prev.drawdownRuleRows || []).map((row) =>
      row.id === rowId ? { ...row, ...patch } : row
    ),
  };
}
