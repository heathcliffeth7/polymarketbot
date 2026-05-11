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
          priceToBeatIvStalePenaltyMs: 1500,
          priceToBeatIvStaleGapStrengthPenalty: 0.1,
          priceToBeatIvNegativeVelocityGapStrengthPenalty: 0.15,
          priceToBeatIvBinanceMissingAskThresholdCent: 65,
          priceToBeatIvBinanceMissingPenalty: 0.02,
          priceToBeatIvMinAdjustedMargin: 0.02,
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
          priceToBeatIvMinFinalQ: 1.2,
          priceToBeatIvBinanceDisagreementThreshold: -0.2,
          priceToBeatIvLargeBinanceDisagreementPenalty: 1.2,
          priceToBeatIvProtectionMode: 'panic',
          priceToBeatIvBookLeadGuardEnabled: 'true',
          priceToBeatIvBookLeadMinMidDiff: 1.2,
          priceToBeatIvOppositeMidBlockCent: 101,
          priceToBeatIvModelBookGapWarn: 1.2,
          priceToBeatIvModelBookWarnGapPenalty: -0.1,
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
          priceToBeatIvProtectionSoftThresholdPenalty: 1.2,
          priceToBeatIvVolumeBaselineMode: 'daily',
          priceToBeatIvVolumeWindowSec: 1.5,
          priceToBeatIvBookReliabilityThreshold: 1.2,
          priceToBeatIvAdaptiveRedBlock: 'true',
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'iv_buy_quality');
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_iv_min_adjusted_margin')
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
