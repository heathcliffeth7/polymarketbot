import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildNodeConfigFromForm,
  parseNodeConfigToForm,
} from '@/lib/trade-flow-config-mappers';

test('trigger.market_price iv_mismatch_edge round-trips and drops manual PTB fields', () => {
  const form = parseNodeConfigToForm('trigger.market_price', {
    marketMode: 'auto_scope',
    marketScope: 'eth_5m_updown',
    marketSelection: 'latest_by_slug',
    priceMode: 'composite',
    repeatMode: 'once',
    priceToBeatTriggerEnabled: true,
    priceToBeatMode: 'iv_mismatch_edge',
    priceToBeatTriggerMinGap: 2,
    priceToBeatTriggerUnit: 'usd',
  });

  assert.equal(form.fields.priceToBeatMode, 'iv_mismatch_edge');

  const rebuilt = buildNodeConfigFromForm('trigger.market_price', form);
  assert.equal(rebuilt.priceToBeatMode, 'iv_mismatch_edge');
  assert.equal('priceToBeatTriggerMinGap' in rebuilt, false);
  assert.equal('priceToBeatTriggerUnit' in rebuilt, false);
});

test('action.place_order iv_mismatch_edge round-trips for primary and counter PTB guards', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: 'pair_lock',
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'eth-updown-5m-1774013100',
    tokenId: 'eth-up-token',
    outcomeLabel: 'Up',
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'iv_mismatch_edge',
    priceToBeatMaxDiff: 2,
    priceToBeatMaxDiffUnit: 'usd',
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
    counterLegPriceToBeatMaxDiff: 2,
    counterLegPriceToBeatMaxDiffUnit: 'usd',
  });

  assert.equal(form.fields.priceToBeatMode, 'iv_mismatch_edge');
  assert.equal(form.fields.counterLegPriceToBeatMode, 'iv_mismatch_edge');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.priceToBeatMode, 'iv_mismatch_edge');
  assert.equal(rebuilt.counterLegPriceToBeatMode, 'iv_mismatch_edge');
  assert.equal('priceToBeatMaxDiff' in rebuilt, false);
  assert.equal('counterLegPriceToBeatMaxDiff' in rebuilt, false);
});

test('action.place_order iv_mismatch_edge time rules round-trip through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 10,
    marketSlug: 'btc-updown-5m-1774013100',
    tokenId: 'btc-up-token',
    outcomeLabel: 'Up',
    maxPriceCent: 75,
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
  });

  assert.equal(form.ptbIvTimeRuleRows?.length, 2);
  assert.equal(form.ptbIvTimeRuleRows?.[0]?.maxPriceCent, '65');
  assert.equal(form.ptbIvTimeRuleRows?.[0]?.minExpectedMoveUsd, '12');
  assert.equal(form.ptbIvTimeRuleRows?.[0]?.minGapStrengthMargin, '0.15');
  assert.equal(form.ptbIvTimeRuleRows?.[0]?.minGapUsdMargin, '2.5');
  assert.equal(form.fields.priceToBeatIvStaleGapStrengthPenalty, '0.1');
  assert.equal(form.fields.priceToBeatIvMinAdjustedMargin, '0.02');
  assert.equal(form.fields.priceToBeatIvMinFinalQ, '0.62');
  assert.equal(form.fields.priceToBeatIvProtectionMode, 'adaptive');
  assert.equal(form.fields.priceToBeatIvDepthMaxSlippage, '0.03');
  assert.equal(form.fields.priceToBeatIvModelBookGapWarn, '0.3');
  assert.equal(form.fields.priceToBeatIvLateHighPriceSelectedMidSoftCent, '75');
  assert.equal(form.fields.priceToBeatIvParticipationLongCredit, '0.02');
  assert.equal(form.fields.priceToBeatIvVolumeBaselineMode, 'hourly');
  assert.equal(form.fields.priceToBeatIvAdaptiveGreenEdgeDelta, '-0.01');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.deepEqual(rebuilt.priceToBeatIvTimeRules, [
    { startRemainingSec: 120, endRemainingSec: 60, maxPriceCent: 65, minEdge: 0.08, minGapStrength: 0.85, minExpectedMoveUsd: 12, minGapStrengthMargin: 0.15, minGapUsdMargin: 2.5 },
    { startRemainingSec: 60, endRemainingSec: 30, maxPriceCent: 70, minEdge: 0.09, minGapStrength: 0.9 },
  ]);
  assert.equal(rebuilt.priceToBeatIvStalePenaltyMs, 1500);
  assert.equal(rebuilt.priceToBeatIvStaleGapStrengthPenalty, 0.1);
  assert.equal(rebuilt.priceToBeatIvNegativeVelocityGapStrengthPenalty, 0.15);
  assert.equal(rebuilt.priceToBeatIvBinanceMissingAskThresholdCent, 65);
  assert.equal(rebuilt.priceToBeatIvBinanceMissingPenalty, 0.02);
  assert.equal(rebuilt.priceToBeatIvMinAdjustedMargin, 0.02);
  assert.equal(rebuilt.priceToBeatIvMinFinalQ, 0.62);
  assert.equal(rebuilt.priceToBeatIvBinanceDisagreementThreshold, 0.15);
  assert.equal(rebuilt.priceToBeatIvBinanceDisagreementPenalty, 0.02);
  assert.equal(rebuilt.priceToBeatIvLargeBinanceDisagreementThreshold, 0.2);
  assert.equal(rebuilt.priceToBeatIvLargeBinanceDisagreementPenalty, 0.04);
  assert.equal(rebuilt.priceToBeatIvProtectionMode, 'adaptive');
  assert.equal(rebuilt.priceToBeatIvBookLeadGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatIvBookLeadUnderSec, 120);
  assert.equal(rebuilt.priceToBeatIvBookLeadMinMidDiff, 0.2);
  assert.equal(rebuilt.priceToBeatIvOppositeMidBlockCent, 65);
  assert.equal(rebuilt.priceToBeatIvBlockOnOppositeBookLead, true);
  assert.equal(rebuilt.priceToBeatIvTooGoodToBeTrueGap, 0.45);
  assert.equal(rebuilt.priceToBeatIvModelBookGapWarn, 0.3);
  assert.equal(rebuilt.priceToBeatIvModelBookGapHard, 0.45);
  assert.equal(rebuilt.priceToBeatIvModelBookWarnThresholdPenalty, 0.02);
  assert.equal(rebuilt.priceToBeatIvModelBookWarnGapPenalty, 0.05);
  assert.equal(rebuilt.priceToBeatIvDepthGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatIvDepthMaxSlippage, 0.03);
  assert.equal(rebuilt.priceToBeatIvLateHighPriceSoftUnderSec, 60);
  assert.equal(rebuilt.priceToBeatIvLateHighPriceAskCent, 65);
  assert.equal(rebuilt.priceToBeatIvLateHighPriceSelectedMidSoftCent, 75);
  assert.equal(rebuilt.priceToBeatIvLateHighPriceThresholdPenalty, 0.03);
  assert.equal(rebuilt.priceToBeatIvLateHighPriceSelectedMidHardCent, 65);
  assert.equal(rebuilt.priceToBeatIvLateHighPriceMinGapUsd, 20);
  assert.equal(rebuilt.priceToBeatIvParticipationCreditEnabled, true);
  assert.equal(rebuilt.priceToBeatIvParticipationAfterMinutes, 60);
  assert.equal(rebuilt.priceToBeatIvParticipationLongAfterMinutes, 180);
  assert.equal(rebuilt.priceToBeatIvParticipationCredit, 0.01);
  assert.equal(rebuilt.priceToBeatIvParticipationLongCredit, 0.02);
  assert.equal(rebuilt.priceToBeatIvParticipationMinThreshold, 0.05);
  assert.equal(rebuilt.priceToBeatIvRequireBinanceFreshUnderSec, 60);
  assert.equal(rebuilt.priceToBeatIvBinanceMaxStaleMs, 2000);
  assert.equal(rebuilt.priceToBeatIvRequireBinanceSameDirection, true);
  assert.equal(rebuilt.priceToBeatIvMomentumProtectionEnabled, true);
  assert.equal(rebuilt.priceToBeatIvDropZBlockThreshold, 0.8);
  assert.equal(rebuilt.priceToBeatIvProtectionSoftThresholdPenalty, 0.03);
  assert.equal(rebuilt.priceToBeatIvProtectionSoftGapStrengthPenalty, 0.1);
  assert.equal(rebuilt.priceToBeatIvVolumeBaselineMode, 'hourly');
  assert.equal(rebuilt.priceToBeatIvVolumeBaselineLookbackDays, 7);
  assert.equal(rebuilt.priceToBeatIvVolumeWindowSec, 30);
  assert.equal(rebuilt.priceToBeatIvVolumeBaselineMinSamples, 20);
  assert.equal(rebuilt.priceToBeatIvLowHourlyVolumeRatio, 0.7);
  assert.equal(rebuilt.priceToBeatIvHighHourlyVolumeRatio, 1.5);
  assert.equal(rebuilt.priceToBeatIvExtremeHourlyVolumeRatio, 3);
  assert.equal(rebuilt.priceToBeatIvBookReliabilityThreshold, 0.6);
  assert.equal(rebuilt.priceToBeatIvAdaptiveGreenEdgeDelta, -0.01);
  assert.equal(rebuilt.priceToBeatIvAdaptiveGreenGapStrengthDelta, -0.03);
  assert.equal(rebuilt.priceToBeatIvAdaptiveOrangeEdgeDelta, 0.03);
  assert.equal(rebuilt.priceToBeatIvAdaptiveOrangeGapStrengthDelta, 0.15);
  assert.equal(rebuilt.priceToBeatIvAdaptiveOrangeGapUsdMarginDelta, 1);
  assert.equal(rebuilt.priceToBeatIvAdaptiveRedBlock, true);
});
