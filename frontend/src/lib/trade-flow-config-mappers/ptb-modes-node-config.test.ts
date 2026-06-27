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
    priceToBeatIvEntryQualityPolicy: true,
    priceToBeatIvNormalMaxPriceCent: 94,
    priceToBeatIvPremiumMaxPriceCent: 96,
    priceToBeatIvNoNewEntryBelowSeconds: 8,
    priceToBeatIvMinExpectedMoveBps: 2,
    priceToBeatIvMinExpectedMoveUsd: 0,
    priceToBeatIvMinExpectedMoveMode: 'adaptive',
    priceToBeatIvAdaptiveMinExpectedMoveBaseBps: 1.5,
    priceToBeatIvAdaptiveMinExpectedMoveMinBps: 1.5,
    priceToBeatIvAdaptiveMinExpectedMoveMaxBps: 2.75,
    priceToBeatIvAdaptiveDisagreementBpsAdd: 0.25,
    priceToBeatIvAdaptiveStrongDisagreementBpsAdd: 0.5,
    priceToBeatIvAdaptiveSpreadBpsAdd: 0.2,
    priceToBeatIvAdaptiveWideSpreadBpsAdd: 0.4,
    priceToBeatIvAdaptiveStaleBpsAdd: 0.2,
    priceToBeatIvAdaptiveNoiseBpsAdd: 0.25,
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
    priceToBeatIvCexAlignMaxBps: 5,
    priceToBeatIvCexMagnitudeGuardEnabled: true,
    priceToBeatIvCexMagnitudeShallowRatio: 0.5,
    priceToBeatIvCexMagnitudeModerateRatio: 1,
    priceToBeatIvLowQualityEdgeRecheckEnabled: true,
    priceToBeatIvLowQualityGapMargin: 0.1,
    priceToBeatIvLowQualityEdgeMarginCent: 5,
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
  });

  assert.equal(form.ptbIvTimeRuleRows?.length, 2);
  assert.equal(form.ptbIvTimeRuleRows?.[0]?.maxPriceCent, '65');
  assert.equal(form.ptbIvTimeRuleRows?.[0]?.minExpectedMoveUsd, '12');
  assert.equal(form.ptbIvTimeRuleRows?.[0]?.minGapStrengthMargin, '0.15');
  assert.equal(form.ptbIvTimeRuleRows?.[0]?.minGapUsdMargin, '2.5');
  assert.equal(form.fields.priceToBeatIvEntryQualityPolicy, 'true');
  assert.equal(form.fields.priceToBeatIvNormalMaxPriceCent, '94');
  assert.equal(form.fields.priceToBeatIvPremiumMaxPriceCent, '96');
  assert.equal(form.fields.priceToBeatIvMinExpectedMoveBps, '2');
  assert.equal(form.fields.priceToBeatIvMinExpectedMoveMode, 'adaptive');
  assert.equal(form.fields.priceToBeatIvAdaptiveMinExpectedMoveMaxBps, '2.75');
  assert.equal(form.fields.priceToBeatIvPremiumBufferRetain5s, '0.9');
  assert.equal(form.fields.priceToBeatIvSpikeMultiplier, '2.5');
  assert.equal(form.fields.priceToBeatIvCexAlignMaxBps, '5');
  assert.equal(form.fields.priceToBeatIvCexMagnitudeGuardEnabled, 'true');
  assert.equal(form.fields.priceToBeatIvCexMagnitudeShallowRatio, '0.5');
  assert.equal(form.fields.priceToBeatIvLowQualityEdgeRecheckEnabled, 'true');
  assert.equal(form.fields.priceToBeatIvLowQualityGapMargin, '0.1');
  assert.equal(form.fields.priceToBeatIvStaleGapStrengthPenalty, '0.1');
  assert.equal(form.fields.priceToBeatIvMinAdjustedMargin, '0.02');
  assert.equal(form.fields.priceToBeatIvMediumChopMinAdjMargin, '0.045');
  assert.equal(form.fields.priceToBeatIvMediumChopHighPriceRefCent, '82');
  assert.equal(form.fields.priceToBeatIvHighPriceEarlyReversalGuardEnabled, 'true');
  assert.equal(form.fields.priceToBeatIvHighPriceEarlyRefCent, '77');
  assert.equal(form.fields.priceToBeatIvHighPriceEarlyQExtremeCent, '98.5');
  assert.equal(form.fields.priceToBeatIvHighPriceEarlyQExtremeRequireBinanceQ, 'true');
  assert.equal(form.fields.priceToBeatIvMinFinalQ, '0.62');
  assert.equal(form.fields.priceToBeatIvProtectionMode, 'adaptive');
  assert.equal(form.fields.priceToBeatIvDepthMaxSlippage, '0.03');
  assert.equal(form.fields.priceToBeatIvModelBookGapWarn, '0.3');
  assert.equal(form.fields.priceToBeatIvBorderlinePumpBookLeadGuardEnabled, 'true');
  assert.equal(form.fields.priceToBeatIvBorderlineGapMarginEarly, '0.1');
  assert.equal(form.fields.priceToBeatIvBorderlineBookLeadQMinCent, '95');
  assert.equal(form.fields.priceToBeatIvPtbChopGuardEnabled, 'true');
  assert.equal(form.fields.priceToBeatIvPtbChopPathZBlock, '1.75');
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
  assert.equal(rebuilt.priceToBeatIvEntryQualityPolicy, true);
  assert.equal(rebuilt.priceToBeatIvNormalMaxPriceCent, 94);
  assert.equal(rebuilt.priceToBeatIvPremiumMaxPriceCent, 96);
  assert.equal(rebuilt.priceToBeatIvNoNewEntryBelowSeconds, 8);
  assert.equal(rebuilt.priceToBeatIvMinExpectedMoveBps, 2);
  assert.equal(rebuilt.priceToBeatIvMinExpectedMoveUsd, 0);
  assert.equal(rebuilt.priceToBeatIvMinExpectedMoveMode, 'adaptive');
  assert.equal(rebuilt.priceToBeatIvAdaptiveMinExpectedMoveBaseBps, 1.5);
  assert.equal(rebuilt.priceToBeatIvAdaptiveMinExpectedMoveMinBps, 1.5);
  assert.equal(rebuilt.priceToBeatIvAdaptiveMinExpectedMoveMaxBps, 2.75);
  assert.equal(rebuilt.priceToBeatIvAdaptiveDisagreementBpsAdd, 0.25);
  assert.equal(rebuilt.priceToBeatIvAdaptiveStrongDisagreementBpsAdd, 0.5);
  assert.equal(rebuilt.priceToBeatIvAdaptiveSpreadBpsAdd, 0.2);
  assert.equal(rebuilt.priceToBeatIvAdaptiveWideSpreadBpsAdd, 0.4);
  assert.equal(rebuilt.priceToBeatIvAdaptiveStaleBpsAdd, 0.2);
  assert.equal(rebuilt.priceToBeatIvAdaptiveNoiseBpsAdd, 0.25);
  assert.equal(rebuilt.priceToBeatIvGapStrengthMin60To45, 2.5);
  assert.equal(rebuilt.priceToBeatIvGapStrengthMin45To25, 2.2);
  assert.equal(rebuilt.priceToBeatIvGapStrengthMin25To10, 1.9);
  assert.equal(rebuilt.priceToBeatIvGapStrengthMin10To8, 2);
  assert.equal(rebuilt.priceToBeatIvBufferTrendGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatIvBufferRetain5s, 0.85);
  assert.equal(rebuilt.priceToBeatIvBufferRetain10s, 0.7);
  assert.equal(rebuilt.priceToBeatIvPremiumBufferRetain5s, 0.9);
  assert.equal(rebuilt.priceToBeatIvPremiumBufferRetain10s, 0.75);
  assert.equal(rebuilt.priceToBeatIvSpikeFadeGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatIvSpikeMultiplier, 2.5);
  assert.equal(rebuilt.priceToBeatIvSpikeRetraceRatio, 0.2);
  assert.equal(rebuilt.priceToBeatIvPremiumMaxSpreadCent, 2);
  assert.equal(rebuilt.priceToBeatIvPremiumMaxChainlinkAgeMs, 2500);
  assert.equal(rebuilt.priceToBeatIvCexAlignMaxBps, 5);
  assert.equal(rebuilt.priceToBeatIvCexMagnitudeGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatIvCexMagnitudeShallowRatio, 0.5);
  assert.equal(rebuilt.priceToBeatIvCexMagnitudeModerateRatio, 1);
  assert.equal(rebuilt.priceToBeatIvLowQualityEdgeRecheckEnabled, true);
  assert.equal(rebuilt.priceToBeatIvLowQualityGapMargin, 0.1);
  assert.equal(rebuilt.priceToBeatIvLowQualityEdgeMarginCent, 5);
  assert.equal(rebuilt.priceToBeatIvBinanceMissingAskThresholdCent, 65);
  assert.equal(rebuilt.priceToBeatIvBinanceMissingPenalty, 0.02);
  assert.equal(rebuilt.priceToBeatIvMinAdjustedMargin, 0.02);
  assert.equal(rebuilt.priceToBeatIvMediumChopMinAdjMargin, 0.045);
  assert.equal(rebuilt.priceToBeatIvMediumChopHighPriceMinAdjMargin, 0.05);
  assert.equal(rebuilt.priceToBeatIvMediumChopHighPriceRefCent, 82);
  assert.equal(rebuilt.priceToBeatIvMediumChopBinanceFailOpenMarginAdd, 0.005);
  assert.equal(rebuilt.priceToBeatIvMediumChopStaleMs, 1500);
  assert.equal(rebuilt.priceToBeatIvMediumChopStaleMarginAdd, 0.005);
  assert.equal(rebuilt.priceToBeatIvHighPriceEarlyReversalGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatIvHighPriceEarlyRefCent, 77);
  assert.equal(rebuilt.priceToBeatIvHighPriceEarlyRemainingSec, 120);
  assert.equal(rebuilt.priceToBeatIvHighPriceEarlyMaxStaleMs, 2000);
  assert.equal(rebuilt.priceToBeatIvHighPriceEarlyStaleGapAdd, 0.3);
  assert.equal(rebuilt.priceToBeatIvHighPriceEarlyBinanceMissingGapAdd, 0.35);
  assert.equal(rebuilt.priceToBeatIvHighPriceEarlyQExtremeCent, 98.5);
  assert.equal(rebuilt.priceToBeatIvHighPriceEarlyQExtremeMinGapStrength, 3.5);
  assert.equal(rebuilt.priceToBeatIvHighPriceEarlyQExtremeMaxStaleMs, 1500);
  assert.equal(rebuilt.priceToBeatIvHighPriceEarlyQExtremeRequireBinanceQ, true);
  assert.equal(rebuilt.priceToBeatIvHighPriceEarlyQExtremeRequireCleanStrongCex, true);
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
  assert.equal(rebuilt.priceToBeatIvBorderlinePumpBookLeadGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatIvBorderlineGapMarginEarly, 0.1);
  assert.equal(rebuilt.priceToBeatIvBorderlinePumpShockRatio, 1.25);
  assert.equal(rebuilt.priceToBeatIvBorderlineBookLeadQMinCent, 95);
  assert.equal(rebuilt.priceToBeatIvBorderlineBookLeadCheapTokenCent, 60);
  assert.equal(rebuilt.priceToBeatIvBorderlineBookLeadDislocationCent, 30);
  assert.equal(rebuilt.priceToBeatIvPtbChopGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatIvPtbChopLookbackSeconds, 10);
  assert.equal(rebuilt.priceToBeatIvPtbChopExtendedLookbackSeconds, 15);
  assert.equal(rebuilt.priceToBeatIvPtbChopDeadbandBps, 0.5);
  assert.equal(rebuilt.priceToBeatIvPtbChopDeadbandMinUsdBtc, 5);
  assert.equal(rebuilt.priceToBeatIvPtbChopDeadbandMinUsdEth, 0.3);
  assert.equal(rebuilt.priceToBeatIvPtbChopDeadbandMinUsdSol, 0.03);
  assert.equal(rebuilt.priceToBeatIvPtbChopZeroCrossBlock10s, 2);
  assert.equal(rebuilt.priceToBeatIvPtbChopZeroCrossBlock15s, 3);
  assert.equal(rebuilt.priceToBeatIvPtbChopPathZWarn, 1.25);
  assert.equal(rebuilt.priceToBeatIvPtbChopPathZBlock, 1.75);
  assert.equal(rebuilt.priceToBeatIvPtbChopEfficiencyWarn, 0.25);
  assert.equal(rebuilt.priceToBeatIvPtbChopEfficiencyBlock, 0.15);
  assert.equal(rebuilt.priceToBeatIvPtbChopOppositeDepthZWarn, 0.5);
  assert.equal(rebuilt.priceToBeatIvPtbChopOppositeDepthZBlock, 0.9);
  assert.equal(rebuilt.priceToBeatIvPtbChopMaxGapStrengthPenalty, 0.35);
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

test('action.place_order iv_mismatch_edge price band guard round-trips as JSON array', () => {
  const priceBands = [
    {
      name: 'certainty_88_90',
      minPriceCent: 88,
      maxPriceCent: 90,
      minQCent: 97.5,
      minFairEdgeCent: 7,
      maxSpreadCent: 1.5,
      requireCleanCex: true,
      requireCexWithDirection: true,
      requireBookConfirmation: true,
      requireNoChainlinkStalePenalty: true,
      requireNoMixedCex: true,
      timeRules: [
        {
          startRemainingSec: 240,
          endRemainingSec: 180,
          minGapStrength: 4.3,
          minQCent: 98.8,
          minFairEdgeCent: 9,
          maxSpreadCent: 1,
        },
      ],
    },
  ];
  const form = parseNodeConfigToForm('action.place_order', {
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
    priceToBeatIvPriceBands: priceBands,
  });

  assert.equal(form.fields.priceToBeatIvPriceBandGuardEnabled, 'true');
  assert.equal(form.fields.priceToBeatIvPriceBandSource, 'execution_vwap');
  assert.match(form.fields.priceToBeatIvPriceBands, /certainty_88_90/);

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.priceToBeatIvPriceBandGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatIvPriceBandSource, 'execution_vwap');
  assert.equal(rebuilt.priceToBeatIvPriceBandCombineMode, 'strictest');
  assert.deepEqual(rebuilt.priceToBeatIvPriceBands, priceBands);
});
