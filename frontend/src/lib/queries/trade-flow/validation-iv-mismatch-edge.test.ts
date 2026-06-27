import assert from 'node:assert/strict';
import test from 'node:test';

import { normalizeTradeFlowGraph } from '@/lib/queries/trade-flow/graph';
import { validateActionPlaceOrderConfig } from '@/lib/queries/trade-flow/validation-action-place-order';
import { validateTriggerMarketPriceNodeConfig } from '@/lib/queries/trade-flow/validation-trigger-market-price';
import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';

function collectTriggerIssues(graph: TradeFlowGraph, nodeKey: string): TradeFlowValidationIssue[] {
  const node = graph.nodes.find((item) => item.key === nodeKey);
  assert.ok(node, `node ${nodeKey} should exist`);
  const issues: TradeFlowValidationIssue[] = [];
  validateTriggerMarketPriceNodeConfig(issues, node, graph);
  return issues;
}

function collectActionIssues(graph: TradeFlowGraph, nodeKey: string): TradeFlowValidationIssue[] {
  const node = graph.nodes.find((item) => item.key === nodeKey);
  assert.ok(node, `node ${nodeKey} should exist`);
  const issues: TradeFlowValidationIssue[] = [];
  validateActionPlaceOrderConfig(issues, node, graph);
  return issues;
}

function buildAutoScopeTrigger(key: string): TradeFlowNode {
  return {
    key,
    type: 'trigger.market_price',
    positionX: 0,
    positionY: 0,
    config: {
      marketMode: 'auto_scope',
      marketScope: 'btc_5m_updown',
      marketSelection: 'latest_by_slug',
      bindingMode: 'pair_lock_only',
      priceMode: 'composite',
      repeatMode: 'once',
      priceToBeatTriggerEnabled: true,
      priceToBeatMode: 'iv_mismatch_edge',
    },
  };
}

test('validateTriggerMarketPriceNodeConfig accepts iv_mismatch_edge PTB mode', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [buildAutoScopeTrigger('trigger_iv')],
    edges: [],
  });

  const issues = collectTriggerIssues(graph, 'trigger_iv');
  assert.equal(issues.some((issue) => issue.code === 'invalid_price_to_beat_mode'), false);
});

test('validateActionPlaceOrderConfig accepts iv_mismatch_edge primary PTB guard', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'iv_buy',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'xrp-updown-5m-1774013100',
          tokenId: 'xrp-up-token',
          outcomeLabel: 'Up',
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'iv_mismatch_edge',
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'iv_buy');
  assert.equal(issues.some((issue) => issue.code === 'invalid_price_to_beat_mode'), false);
  assert.equal(
    issues.some((issue) => issue.code === 'unsupported_price_to_beat_auto_vol_pct_asset'),
    false
  );
});

test('validateActionPlaceOrderConfig accepts iv_mismatch_edge PTB time rules', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'iv_buy_rules',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'btc-updown-5m-1774013100',
          tokenId: 'btc-up-token',
          outcomeLabel: 'Up',
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'iv_mismatch_edge',
          priceToBeatIvTimeRules: [
            { startRemainingSec: 120, endRemainingSec: 60, maxPriceCent: 65, minEdge: 0.08, minGapStrength: 0.85, minExpectedMoveUsd: 12, minGapStrengthMargin: 0.15, minGapUsdMargin: 2.5 },
            { startRemainingSec: 60, endRemainingSec: 30, maxPriceCent: 70, minEdge: 0.09, minGapStrength: 0.9 },
          ],
          priceToBeatIvEntryQualityPolicy: true,
          priceToBeatIvNormalMaxPriceCent: 94,
          priceToBeatIvPremiumMaxPriceCent: 96,
          priceToBeatIvNoNewEntryBelowSeconds: 8,
          priceToBeatIvMinExpectedMoveBps: 2,
          priceToBeatIvMinExpectedMoveUsd: 0,
          priceToBeatIvGapStrengthMin60To45: 2.5,
          priceToBeatIvGapStrengthMin45To25: 2.2,
          priceToBeatIvGapStrengthMin25To10: 1.9,
          priceToBeatIvGapStrengthMin10To8: 2,
          priceToBeatIvBufferTrendGuardEnabled: true,
          priceToBeatIvBufferRetain5s: 0.85,
          priceToBeatIvBufferRetain10s: 0.7,
          priceToBeatIvPremiumBufferRetain5s: 0.9,
          priceToBeatIvPremiumBufferRetain10s: 0.75,
          priceToBeatIvSpikeFadeGuardEnabled: true,
          priceToBeatIvSpikeMultiplier: 2.5,
          priceToBeatIvSpikeRetraceRatio: 0.2,
          priceToBeatIvPremiumMaxSpreadCent: 2,
          priceToBeatIvPremiumMaxChainlinkAgeMs: 2500,
          priceToBeatIvChainlinkStaleMs: 3500,
          priceToBeatIvEntryQualityChainlinkMaxAgeMs: 3500,
          priceToBeatIvCexAlignMaxBps: 5,
          priceToBeatIvCexLeadOverrideEnabled: true,
          priceToBeatIvOracleCexDivergenceBlockZ: 2.25,
          priceToBeatIvOracleTickJumpCooldownMs: 12000,
          priceToBeatIvCexMagnitudeGuardEnabled: true,
          priceToBeatIvCexMagnitudeShallowRatio: 0.5,
          priceToBeatIvCexMagnitudeModerateRatio: 1,
          priceToBeatIvWaitRepriceGuardEnabled: true,
          priceToBeatIvLowQualityEdgeRecheckEnabled: true,
          priceToBeatIvLowQualityGapMargin: 0.1,
          priceToBeatIvLowQualityEdgeMarginCent: 5,
          priceToBeatIvWaitMaxAgeMsEarly: 8000,
          priceToBeatIvWaitMaxAgeMsMid: 5000,
          priceToBeatIvWaitMaxAgeMsLate: 3000,
          priceToBeatIvWaitInitialAskMaxOverCapCent: 10,
          priceToBeatIvFallingIntoCapGuardEnabled: true,
          priceToBeatIvFallingIntoCapDropCentEarly: 15,
          priceToBeatIvFallingIntoCapDropCentMid: 12,
          priceToBeatIvFallingIntoCapDropCentLate: 8,
          priceToBeatIvLateExpensiveEntryGuardEnabled: true,
          priceToBeatIvLateExpensiveSeconds: 45,
          priceToBeatIvLateExpensiveVwapCent: 70,
          priceToBeatIvLateExpensiveMinQCent: 92,
          priceToBeatIvLateExpensiveMinGapStrengthExtra: 0.5,
          priceToBeatIvStalePenaltyMs: 1500,
          priceToBeatIvStaleGapStrengthPenalty: 0.1,
          priceToBeatIvNegativeVelocityGapStrengthPenalty: 0.15,
          priceToBeatIvBinanceMissingAskThresholdCent: 65,
          priceToBeatIvBinanceMissingPenalty: 0.02,
          priceToBeatIvMinAdjustedMargin: 0.02,
          priceToBeatIvMediumChopMinAdjMargin: 0.045,
          priceToBeatIvMediumChopHighPriceMinAdjMargin: 0.05,
          priceToBeatIvMediumChopHighPriceRefCent: 82,
          priceToBeatIvMediumChopBinanceFailOpenMarginAdd: 0.005,
          priceToBeatIvMediumChopStaleMs: 1500,
          priceToBeatIvMediumChopStaleMarginAdd: 0.005,
          priceToBeatIvHighPriceEarlyReversalGuardEnabled: true,
          priceToBeatIvHighPriceEarlyRefCent: 77,
          priceToBeatIvHighPriceEarlyRemainingSec: 120,
          priceToBeatIvHighPriceEarlyMaxStaleMs: 2000,
          priceToBeatIvHighPriceEarlyStaleGapAdd: 0.3,
          priceToBeatIvHighPriceEarlyBinanceMissingGapAdd: 0.35,
          priceToBeatIvHighPriceEarlyQExtremeCent: 98.5,
          priceToBeatIvHighPriceEarlyQExtremeMinGapStrength: 3.5,
          priceToBeatIvHighPriceEarlyQExtremeMaxStaleMs: 1500,
          priceToBeatIvHighPriceEarlyQExtremeRequireBinanceQ: true,
          priceToBeatIvHighPriceEarlyQExtremeRequireCleanStrongCex: true,
          priceToBeatIvMinFinalQ: 0.62,
          priceToBeatIvBinanceDisagreementThreshold: 0.15,
          priceToBeatIvBinanceDisagreementPenalty: 0.02,
          priceToBeatIvLargeBinanceDisagreementThreshold: 0.2,
          priceToBeatIvLargeBinanceDisagreementPenalty: 0.04,
          priceToBeatIvProtectionMode: 'adaptive',
          priceToBeatIvBookLeadGuardEnabled: true,
          priceToBeatIvBookLeadUnderSec: 120,
          priceToBeatIvBookLeadMinMidDiff: 0.2,
          priceToBeatIvOppositeMidBlockCent: 65,
          priceToBeatIvBlockOnOppositeBookLead: true,
          priceToBeatIvTooGoodToBeTrueGap: 0.45,
          priceToBeatIvModelBookGapWarn: 0.3,
          priceToBeatIvModelBookGapHard: 0.45,
          priceToBeatIvModelBookWarnThresholdPenalty: 0.02,
          priceToBeatIvModelBookWarnGapPenalty: 0.05,
          priceToBeatIvBorderlinePumpBookLeadGuardEnabled: true,
          priceToBeatIvBorderlineGapMarginEarly: 0.1,
          priceToBeatIvBorderlinePumpShockRatio: 1.25,
          priceToBeatIvBorderlineBookLeadQMinCent: 95,
          priceToBeatIvBorderlineBookLeadCheapTokenCent: 60,
          priceToBeatIvBorderlineBookLeadDislocationCent: 30,
          priceToBeatIvPtbChopGuardEnabled: true,
          priceToBeatIvPtbChopLookbackSeconds: 10,
          priceToBeatIvPtbChopExtendedLookbackSeconds: 15,
          priceToBeatIvPtbChopDeadbandBps: 0.5,
          priceToBeatIvPtbChopDeadbandMinUsdBtc: 5,
          priceToBeatIvPtbChopDeadbandMinUsdEth: 0.3,
          priceToBeatIvPtbChopDeadbandMinUsdSol: 0.03,
          priceToBeatIvPtbChopZeroCrossBlock10s: 2,
          priceToBeatIvPtbChopZeroCrossBlock15s: 3,
          priceToBeatIvPtbChopPathZWarn: 1.25,
          priceToBeatIvPtbChopPathZBlock: 1.75,
          priceToBeatIvPtbChopEfficiencyWarn: 0.25,
          priceToBeatIvPtbChopEfficiencyBlock: 0.15,
          priceToBeatIvPtbChopOppositeDepthZWarn: 0.5,
          priceToBeatIvPtbChopOppositeDepthZBlock: 0.9,
          priceToBeatIvPtbChopMaxGapStrengthPenalty: 0.35,
          priceToBeatIvDepthGuardEnabled: true,
          priceToBeatIvDepthMaxSlippage: 0.03,
          priceToBeatIvLateHighPriceSoftUnderSec: 60,
          priceToBeatIvLateHighPriceAskCent: 65,
          priceToBeatIvLateHighPriceSelectedMidSoftCent: 75,
          priceToBeatIvLateHighPriceThresholdPenalty: 0.03,
          priceToBeatIvLateHighPriceSelectedMidHardCent: 65,
          priceToBeatIvLateHighPriceMinGapUsd: 20,
          priceToBeatIvParticipationCreditEnabled: true,
          priceToBeatIvParticipationAfterMinutes: 60,
          priceToBeatIvParticipationLongAfterMinutes: 180,
          priceToBeatIvParticipationCredit: 0.01,
          priceToBeatIvParticipationLongCredit: 0.02,
          priceToBeatIvParticipationMinThreshold: 0.05,
          priceToBeatIvRequireBinanceFreshUnderSec: 60,
          priceToBeatIvBinanceMaxStaleMs: 2000,
          priceToBeatIvRequireBinanceSameDirection: true,
          priceToBeatIvMomentumProtectionEnabled: true,
          priceToBeatIvDropZBlockThreshold: 0.8,
          priceToBeatIvProtectionSoftThresholdPenalty: 0.03,
          priceToBeatIvProtectionSoftGapStrengthPenalty: 0.1,
          priceToBeatIvVolumeBaselineMode: 'hourly',
          priceToBeatIvVolumeBaselineLookbackDays: 7,
          priceToBeatIvVolumeWindowSec: 30,
          priceToBeatIvVolumeBaselineMinSamples: 20,
          priceToBeatIvLowHourlyVolumeRatio: 0.7,
          priceToBeatIvHighHourlyVolumeRatio: 1.5,
          priceToBeatIvExtremeHourlyVolumeRatio: 3,
          priceToBeatIvBookReliabilityThreshold: 0.6,
          priceToBeatIvAdaptiveGreenEdgeDelta: -0.01,
          priceToBeatIvAdaptiveGreenGapStrengthDelta: -0.03,
          priceToBeatIvAdaptiveOrangeEdgeDelta: 0.03,
          priceToBeatIvAdaptiveOrangeGapStrengthDelta: 0.15,
          priceToBeatIvAdaptiveOrangeGapUsdMarginDelta: 1,
          priceToBeatIvAdaptiveRedBlock: true,
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'iv_buy_rules');
  assert.equal(
    issues.some((issue) => issue.code.includes('price_to_beat_iv')),
    false
  );
});

test('validateActionPlaceOrderConfig rejects invalid iv_mismatch_edge quality guards', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'iv_buy_quality',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'btc-updown-5m-1774013100',
          tokenId: 'btc-up-token',
          outcomeLabel: 'Up',
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'iv_mismatch_edge',
          priceToBeatIvMinAdjustedMargin: -0.01,
          priceToBeatIvMediumChopMinAdjMargin: -0.01,
          priceToBeatIvMediumChopHighPriceMinAdjMargin: 1.2,
          priceToBeatIvMediumChopHighPriceRefCent: 101,
          priceToBeatIvMediumChopBinanceFailOpenMarginAdd: 1.2,
          priceToBeatIvMediumChopStaleMs: -1,
          priceToBeatIvMediumChopStaleMarginAdd: -0.01,
          priceToBeatIvHighPriceEarlyReversalGuardEnabled: 'true',
          priceToBeatIvHighPriceEarlyRefCent: 101,
          priceToBeatIvHighPriceEarlyRemainingSec: -1,
          priceToBeatIvHighPriceEarlyMaxStaleMs: 1.5,
          priceToBeatIvHighPriceEarlyStaleGapAdd: -0.1,
          priceToBeatIvHighPriceEarlyBinanceMissingGapAdd: -0.1,
          priceToBeatIvHighPriceEarlyQExtremeCent: -1,
          priceToBeatIvHighPriceEarlyQExtremeMinGapStrength: -0.1,
          priceToBeatIvHighPriceEarlyQExtremeMaxStaleMs: 1.5,
          priceToBeatIvHighPriceEarlyQExtremeRequireBinanceQ: 'true',
          priceToBeatIvHighPriceEarlyQExtremeRequireCleanStrongCex: 'true',
          priceToBeatIvMinFinalQ: 1.2,
          priceToBeatIvBinanceDisagreementThreshold: -0.2,
          priceToBeatIvLargeBinanceDisagreementPenalty: 1.2,
          priceToBeatIvProtectionMode: 'panic',
          priceToBeatIvBookLeadGuardEnabled: 'true',
          priceToBeatIvBookLeadMinMidDiff: 1.2,
          priceToBeatIvOppositeMidBlockCent: 101,
          priceToBeatIvModelBookGapWarn: 1.2,
          priceToBeatIvModelBookWarnGapPenalty: -0.1,
          priceToBeatIvBorderlinePumpBookLeadGuardEnabled: 'true',
          priceToBeatIvBorderlineGapMarginEarly: -0.1,
          priceToBeatIvBorderlinePumpShockRatio: -1,
          priceToBeatIvBorderlineBookLeadQMinCent: 101,
          priceToBeatIvBorderlineBookLeadCheapTokenCent: 0,
          priceToBeatIvBorderlineBookLeadDislocationCent: 101,
          priceToBeatIvPtbChopGuardEnabled: 'maybe',
          priceToBeatIvPtbChopLookbackSeconds: 1.5,
          priceToBeatIvPtbChopPathZBlock: -0.1,
          priceToBeatIvPtbChopEfficiencyWarn: 1.2,
          priceToBeatIvDepthGuardEnabled: 'true',
          priceToBeatIvDepthMaxSlippage: 1.2,
          priceToBeatIvLateHighPriceSoftUnderSec: -1,
          priceToBeatIvLateHighPriceAskCent: 101,
          priceToBeatIvLateHighPriceSelectedMidSoftCent: 0,
          priceToBeatIvLateHighPriceThresholdPenalty: 1.2,
          priceToBeatIvLateHighPriceSelectedMidHardCent: 101,
          priceToBeatIvLateHighPriceMinGapUsd: -1,
          priceToBeatIvParticipationCreditEnabled: 'true',
          priceToBeatIvParticipationAfterMinutes: -1,
          priceToBeatIvParticipationCredit: 1.2,
          priceToBeatIvBinanceMaxStaleMs: 1.5,
          priceToBeatIvChainlinkStaleMs: 1.5,
          priceToBeatIvProtectionSoftThresholdPenalty: 1.2,
          priceToBeatIvVolumeBaselineMode: 'daily',
          priceToBeatIvVolumeWindowSec: 1.5,
          priceToBeatIvBookReliabilityThreshold: 1.2,
          priceToBeatIvAdaptiveRedBlock: 'true',
          priceToBeatIvNormalMaxPriceCent: 96,
          priceToBeatIvPremiumMaxPriceCent: 94,
          priceToBeatIvMinExpectedMoveBps: -1,
          priceToBeatIvBufferTrendGuardEnabled: 'true',
          priceToBeatIvBufferRetain5s: 0.9,
          priceToBeatIvPremiumBufferRetain5s: 0.8,
          priceToBeatIvSpikeFadeGuardEnabled: 'true',
          priceToBeatIvSpikeMultiplier: 1,
          priceToBeatIvSpikeRetraceRatio: 1.2,
          priceToBeatIvPremiumMaxSpreadCent: -1,
          priceToBeatIvPremiumMaxChainlinkAgeMs: 0,
          priceToBeatIvCexAlignMaxBps: -1,
          priceToBeatIvCexLeadOverrideEnabled: 'true',
          priceToBeatIvOracleCexDivergenceBlockZ: -0.1,
          priceToBeatIvOracleTickJumpCooldownMs: 1.5,
          priceToBeatIvCexMagnitudeGuardEnabled: 'true',
          priceToBeatIvCexMagnitudeShallowRatio: -0.1,
          priceToBeatIvCexMagnitudeModerateRatio: -1,
          priceToBeatIvWaitRepriceGuardEnabled: 'true',
          priceToBeatIvLowQualityEdgeRecheckEnabled: 'true',
          priceToBeatIvLowQualityGapMargin: -0.1,
          priceToBeatIvLowQualityEdgeMarginCent: -1,
          priceToBeatIvWaitMaxAgeMsEarly: 0,
          priceToBeatIvWaitInitialAskMaxOverCapCent: -1,
          priceToBeatIvFallingIntoCapGuardEnabled: 'true',
          priceToBeatIvFallingIntoCapDropCentLate: 101,
          priceToBeatIvLateExpensiveEntryGuardEnabled: 'true',
          priceToBeatIvLateExpensiveSeconds: -1,
          priceToBeatIvLateExpensiveVwapCent: 101,
          priceToBeatIvLateExpensiveMinGapStrengthExtra: -0.1,
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'iv_buy_quality');
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_min_adjusted_margin')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_medium_chop_min_adj_margin')
  );
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_medium_chop_high_price_min_adj_margin'
    )
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_medium_chop_high_price_ref')
  );
  assert.ok(
    issues.some(
      (issue) =>
        issue.code === 'invalid_price_to_beat_iv_medium_chop_binance_fail_open_margin_add'
    )
  );
  assert.ok(issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_medium_chop_stale_ms'));
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_medium_chop_stale_margin_add')
  );
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_high_price_early_reversal_guard_enabled'
    )
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_high_price_early_ref_cent')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_high_price_early_remaining_sec')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_high_price_early_max_stale_ms')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_high_price_early_stale_gap_add')
  );
  assert.ok(
    issues.some(
      (issue) =>
        issue.code === 'invalid_price_to_beat_iv_high_price_early_binance_missing_gap_add'
    )
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_high_price_early_q_extreme_cent')
  );
  assert.ok(
    issues.some(
      (issue) =>
        issue.code === 'invalid_price_to_beat_iv_high_price_early_q_extreme_min_gap_strength'
    )
  );
  assert.ok(
    issues.some(
      (issue) =>
        issue.code === 'invalid_price_to_beat_iv_high_price_early_q_extreme_max_stale_ms'
    )
  );
  assert.ok(
    issues.some(
      (issue) =>
        issue.code === 'invalid_price_to_beat_iv_high_price_early_q_extreme_require_binance_q'
    )
  );
  assert.ok(
    issues.some(
      (issue) =>
        issue.code === 'invalid_price_to_beat_iv_high_price_early_q_extreme_require_clean_strong_cex'
    )
  );
  assert.ok(issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_min_final_q'));
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_binance_disagreement_threshold')
  );
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_large_binance_disagreement_penalty'
    )
  );
  assert.ok(issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_protection_mode'));
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_book_lead_guard_enabled')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_book_lead_min_mid_diff')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_opposite_mid_block')
  );
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_borderline_pump_book_lead_guard_enabled'
    )
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_borderline_gap_margin_early')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_borderline_pump_shock_ratio')
  );
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_borderline_book_lead_q_min_cent'
    )
  );
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_borderline_book_lead_cheap_token_cent'
    )
  );
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_borderline_book_lead_dislocation_cent'
    )
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_ptb_chop_guard_enabled')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_ptb_chop_lookback_seconds')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_ptb_chop_path_z_block')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_ptb_chop_efficiency_warn')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_depth_guard_enabled')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_depth_max_slippage')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_model_book_gap_warn')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_model_book_warn_gap_penalty')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_late_high_price_soft_under_sec')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_late_high_price_ask')
  );
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_late_high_price_selected_mid_soft'
    )
  );
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_late_high_price_threshold_penalty'
    )
  );
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_late_high_price_selected_mid_hard'
    )
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_late_high_price_min_gap_usd')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_participation_credit_enabled')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_participation_after_minutes')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_participation_credit')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_binance_max_stale_ms')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_chainlink_stale_ms')
  );
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_protection_soft_threshold_penalty'
    )
  );
  assert.ok(issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_volume_baseline_mode'));
  assert.ok(issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_volume_window_sec'));
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_book_reliability_threshold')
  );
  assert.ok(issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_adaptive_red_block'));
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_price_caps_relation')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_min_expected_move_bps')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_buffer_trend_guard_enabled')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_premium_retain_5s_relation')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_spike_fade_guard_enabled')
  );
  assert.ok(issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_spike_multiplier'));
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_spike_retrace_ratio')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_premium_max_spread')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_premium_max_chainlink_age')
  );
  assert.ok(issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_cex_align_max_bps'));
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_cex_lead_override_enabled')
  );
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_oracle_cex_divergence_block_z'
    )
  );
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_oracle_tick_jump_cooldown_ms'
    )
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_cex_magnitude_guard_enabled')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_cex_magnitude_shallow_ratio')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_cex_magnitude_moderate_ratio')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_wait_reprice_guard_enabled')
  );
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_low_quality_edge_recheck_enabled'
    )
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_low_quality_gap_margin')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_low_quality_edge_margin_cent')
  );
  assert.ok(issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_wait_max_age_ms_early'));
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_wait_initial_ask_max_over_cap_cent'
    )
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_falling_into_cap_guard_enabled')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_falling_into_cap_drop_cent_late')
  );
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_late_expensive_entry_guard_enabled'
    )
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_late_expensive_seconds')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_late_expensive_vwap_cent')
  );
  assert.ok(
    issues.some(
      (issue) =>
        issue.code === 'invalid_price_to_beat_iv_late_expensive_min_gap_strength_extra'
    )
  );
});

test('validateActionPlaceOrderConfig warns when iv chainlink stale thresholds are split', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'iv_buy_split_threshold',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'btc-updown-5m-1774013100',
          tokenId: 'btc-up-token',
          outcomeLabel: 'Up',
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'iv_mismatch_edge',
          priceToBeatIvChainlinkStaleMs: 3500,
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'iv_buy_split_threshold');
  assert.ok(
    issues.some(
      (issue) =>
        issue.code === 'price_to_beat_iv_chainlink_stale_threshold_pair_incomplete' &&
        issue.severity === 'warning'
    )
  );
});

test('validateActionPlaceOrderConfig accepts paired 3500ms iv chainlink stale thresholds', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'iv_buy_paired_chainlink_stale',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'btc-updown-5m-1774013100',
          tokenId: 'btc-up-token',
          outcomeLabel: 'Up',
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'iv_mismatch_edge',
          priceToBeatIvChainlinkStaleMs: 3500,
          priceToBeatIvEntryQualityChainlinkMaxAgeMs: 3500,
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'iv_buy_paired_chainlink_stale');
  assert.ok(
    !issues.some(
      (issue) => issue.code === 'price_to_beat_iv_chainlink_stale_threshold_pair_incomplete'
    )
  );
});

test('validateActionPlaceOrderConfig rejects negative iv chainlink stale threshold', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'iv_buy_negative_chainlink_stale',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'btc-updown-5m-1774013100',
          tokenId: 'btc-up-token',
          outcomeLabel: 'Up',
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'iv_mismatch_edge',
          priceToBeatIvChainlinkStaleMs: -1,
          priceToBeatIvEntryQualityChainlinkMaxAgeMs: 3500,
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'iv_buy_negative_chainlink_stale');
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_chainlink_stale_ms')
  );
});

test('validateActionPlaceOrderConfig rejects overlapping iv_mismatch_edge PTB time rules', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'iv_buy_overlap',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'btc-updown-5m-1774013100',
          tokenId: 'btc-up-token',
          outcomeLabel: 'Up',
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'iv_mismatch_edge',
          priceToBeatIvTimeRules: [
            { startRemainingSec: 120, endRemainingSec: 50, maxPriceCent: 65, minEdge: 0.08, minGapStrength: 0.85 },
            { startRemainingSec: 60, endRemainingSec: 30, maxPriceCent: 70, minEdge: 0.09, minGapStrength: 0.9 },
          ],
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'iv_buy_overlap');
  assert.equal(
    issues.some((issue) => issue.code === 'overlapping_price_to_beat_iv_time_rules'),
    true
  );
});

test('validateActionPlaceOrderConfig accepts iv_mismatch_edge price band guard config', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'iv_buy_price_band_guard',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'btc-updown-5m-1774013100',
          tokenId: 'btc-up-token',
          outcomeLabel: 'Up',
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'iv_mismatch_edge',
          priceToBeatIvPriceBandGuardEnabled: true,
          priceToBeatIvPriceBandSource: 'execution_vwap',
          priceToBeatIvPriceBandCombineMode: 'strictest',
          priceToBeatIvPriceBands: [
            {
              name: 'premium_lite_77_82',
              minPriceCent: 77,
              maxPriceCent: 82,
              minQCent: 86,
              minFairEdgeCent: 3.5,
              maxSpreadCent: 4,
              requireCleanCex: true,
              requireCexWithDirection: true,
              timeRules: [
                { startRemainingSec: 240, endRemainingSec: 180, minGapStrength: 2.75 },
                { startRemainingSec: 180, endRemainingSec: 120, minGapStrength: 2.5 },
              ],
            },
          ],
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'iv_buy_price_band_guard');
  assert.equal(
    issues.some((issue) => issue.code.includes('price_to_beat_iv_price_band')),
    false
  );
});

test('validateActionPlaceOrderConfig rejects invalid iv_mismatch_edge price band guard config', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'iv_buy_invalid_price_band_guard',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'btc-updown-5m-1774013100',
          tokenId: 'btc-up-token',
          outcomeLabel: 'Up',
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'iv_mismatch_edge',
          priceToBeatIvPriceBandGuardEnabled: true,
          priceToBeatIvPriceBandSource: 'best_ask',
          priceToBeatIvPriceBandCombineMode: 'loose',
          priceToBeatIvPriceBands: [
            {
              name: 'one',
              minPriceCent: 77,
              maxPriceCent: 82,
              timeRules: [
                { startRemainingSec: 240, endRemainingSec: 120, minGapStrength: 2.75 },
                { startRemainingSec: 180, endRemainingSec: 60, minGapStrength: 2.5 },
              ],
            },
            { name: 'two', minPriceCent: 81, maxPriceCent: 86, timeRules: [] },
          ],
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'iv_buy_invalid_price_band_guard');
  assert.equal(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_price_band_source'),
    true
  );
  assert.equal(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_price_band_combine_mode'),
    true
  );
  assert.equal(
    issues.some((issue) => issue.code === 'overlapping_price_to_beat_iv_price_bands'),
    true
  );
  assert.equal(
    issues.some((issue) => issue.code === 'overlapping_price_to_beat_iv_price_band_time_rules'),
    true
  );
});

test('validateActionPlaceOrderConfig rejects invalid iv_mismatch_edge expected move floor', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'iv_buy_invalid_floor',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'btc-updown-5m-1774013100',
          tokenId: 'btc-up-token',
          outcomeLabel: 'Up',
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'iv_mismatch_edge',
          priceToBeatIvTimeRules: [
            { startRemainingSec: 120, endRemainingSec: 60, maxPriceCent: 65, minEdge: 0.08, minGapStrength: 0.85, minExpectedMoveUsd: -1 },
          ],
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'iv_buy_invalid_floor');
  assert.equal(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_time_rule_min_expected_move'),
    true
  );
});

test('validateActionPlaceOrderConfig rejects invalid adaptive expected move floor config', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'iv_buy_invalid_adaptive_floor',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'btc-updown-5m-1774013100',
          tokenId: 'btc-up-token',
          outcomeLabel: 'Up',
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'iv_mismatch_edge',
          priceToBeatIvMinExpectedMoveMode: 'dynamic',
          priceToBeatIvAdaptiveMinExpectedMoveMinBps: 2,
          priceToBeatIvAdaptiveMinExpectedMoveBaseBps: 1.5,
          priceToBeatIvAdaptiveMinExpectedMoveMaxBps: 5.5,
          priceToBeatIvAdaptiveSpreadBpsAdd: -0.1,
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'iv_buy_invalid_adaptive_floor');
  assert.equal(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_min_expected_move_mode'),
    true
  );
  assert.equal(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_adaptive_min_expected_move_max_bps'
    ),
    true
  );
  assert.equal(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_adaptive_min_expected_move_bps_order'
    ),
    true
  );
  assert.equal(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_adaptive_spread_bps_add'),
    true
  );
});

test('validateActionPlaceOrderConfig rejects invalid iv_mismatch_edge gap margins', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'iv_buy_invalid_margin',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'btc-updown-5m-1774013100',
          tokenId: 'btc-up-token',
          outcomeLabel: 'Up',
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'iv_mismatch_edge',
          priceToBeatIvTimeRules: [
            { startRemainingSec: 120, endRemainingSec: 60, maxPriceCent: 65, minEdge: 0.08, minGapStrength: 0.85, minGapStrengthMargin: -0.1, minGapUsdMargin: -1 },
          ],
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'iv_buy_invalid_margin');
  assert.equal(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_iv_time_rule_min_gap_strength_margin'
    ),
    true
  );
});

test('validateActionPlaceOrderConfig accepts iv_mismatch_edge pair_lock counter guard', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair'),
      {
        key: 'pair_buy',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'iv_mismatch_edge',
          pairMaxTotalCent: 90,
          pairOrphanGraceMs: 1500,
          pairSizingMode: 'manual',
          counterLegEnabled: true,
          counterLegSizeUsdc: 5,
          counterLegOutcomeLabel: 'opposite',
          counterLegTriggerCondition: 'level_below',
          counterLegTriggerPriceCent: 20,
          counterLegMaxPriceCent: 42,
          counterLegPriceToBeatGuardEnabled: true,
          counterLegPriceToBeatMode: 'iv_mismatch_edge',
        },
      },
    ],
    edges: [{ key: 'edge_pair', source: 'trigger_pair', target: 'pair_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy');
  assert.equal(issues.some((issue) => issue.code === 'invalid_price_to_beat_mode'), false);
  assert.equal(
    issues.some((issue) => issue.code === 'invalid_counter_leg_price_to_beat_mode'),
    false
  );
});
