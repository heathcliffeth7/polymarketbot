import { toStringValue } from './utils';

export const LIVE_GAP_COLLECTOR_MODE = 'live_gap_collector_v1';

export const LIVE_GAP_COLLECTOR_CONFIG_KEYS = [
  'liveGapCollectorEnabled',
  'liveGapCollectorWindowStartSec',
  'liveGapCollectorWindowEndSec',
  'liveGapCollectorRetryMs',
  'liveGapCollectorHardMaxPriceCent',
  'liveGapCollectorPtbTelemetryEnabled',
  'notifyOnLiveGapCollectorDecision',
  'notifyOnLiveGapAdaptiveLowGapChange',
  'notifyOnLiveGapAdaptiveLowGapNearMissChange',
  'liveGapHistoryPrewarmEnabled',
  'liveGapHistoryPrewarmSec',
  'liveGapHistoryPrewarmStartMode',
  'liveGapHistoryPrewarmSides',
  'liveGapHistorySampleMs',
  'liveGapHistoryRetentionMs',
  'notifyOnPreBuyHistoryWarning',
  'preBuyHistoryWarningMode',
  'liveGapCollectorBinanceMaxStaleMs',
  'liveGapCollectorDetailedGapBandsEnabled',
  'liveGapCollectorGapBandMode',
  'liveGapCollectorUltraCleanGapUsd',
  'liveGapCollectorLowCleanGapUsd',
  'liveGapCollectorMildCleanGapUsd',
  'liveGapCollectorNormalGapUsd',
  'liveGapCollectorActiveGapUsd',
  'liveGapCollectorHighGapUsd',
  'liveGapCollectorHighChopGapUsd',
  'liveGapCollectorExtremeChopGapUsd',
  'liveGapAdaptiveLowGapEnabled',
  'liveGapAdaptiveLowGapMode',
  'liveGapAdaptiveLowGapTriggerCount',
  'liveGapAdaptiveLowGapStepPct',
  'liveGapAdaptiveLowGapMaxRelaxPct',
  'liveGapAdaptiveLowGapMaxShortfallPct',
  'liveGapAdaptiveLowGapMaxFillCent',
  'liveGapAdaptiveLowGapMinRemainingSec',
  'liveGapAdaptiveLowGapRequireLocalPathClean',
  'liveGapCollectorLatencyBufferUsd',
  'liveGapCollectorStrongOnlyUnderSec',
  'liveGapCollectorNoNewEntryUnderSec',
  'liveGapCollectorStrongSignalExtraGapUsd',
  'liveGapStopLossEnabled',
  'liveGapStopLossEntryGapRatio',
  'liveGapStopLossGapUsd',
  'liveGapStopLossMinRemainingSec',
  'noReversalEntryGuardEnabled',
  'noReversalDecisionMode',
  'noReversalLookbackMode',
  'noReversalBaselineFloorPct',
  'noReversalDailyFallbackFloorPct',
  'noReversalSourceMismatchBufferUsd',
  'noReversalSourceMismatchBufferFloorRatio',
  'noReversalLateHighExtraBufferUsd',
  'noReversalFreezePerMarket',
  'noReversalCacheTtlSec',
  'noReversalProfileQueryTimeoutMs',
  'noReversalProfileLookupTimeoutMs',
  'noReversalPrewarmQueryTimeoutMs',
  'noReversalPrecomputedProfilesEnabled',
  'noReversalAllowColdProfileQuery',
  'noReversalUseLocalPathFallbackOnMissingProfile',
  'noReversalLocalPathFallbackEnabled',
  'noReversalLocalPathLookbackMs',
  'noReversalLocalPathMinHistoryMs',
  'noReversalLocalPathGateMode',
  'noReversalLocalPathFreshRetraceWindowMs',
  'noReversalLocalPathFreshMaxDropUsd',
  'noReversalLocalPathFreshMinHistoryMs',
  'noReversalBlockIfProfileMissingAndLocalPathInsufficient',
  'noReversalProfileMissingEmergencyMarginEnabled',
  'noReversalProfileMissingEmergencyMarginFloorRatio',
  'noReversalMaxRelaxPctPerWindow',
  'noReversalMaxTightenPctPerWindow',
];

const DEFAULTS: Record<string, string> = {
  liveGapCollectorEnabled: 'true',
  liveGapCollectorWindowStartSec: '220',
  liveGapCollectorWindowEndSec: '285',
  liveGapCollectorRetryMs: '150',
  liveGapCollectorHardMaxPriceCent: '93',
  liveGapCollectorPtbTelemetryEnabled: 'true',
  notifyOnLiveGapCollectorDecision: 'true',
  notifyOnLiveGapAdaptiveLowGapChange: 'true',
  notifyOnLiveGapAdaptiveLowGapNearMissChange: 'true',
  liveGapHistoryPrewarmEnabled: 'true',
  liveGapHistoryPrewarmSec: '35',
  liveGapHistoryPrewarmStartMode: 'before_trigger_window',
  liveGapHistoryPrewarmSides: 'both',
  liveGapHistorySampleMs: '250',
  liveGapHistoryRetentionMs: '300000',
  notifyOnPreBuyHistoryWarning: 'true',
  preBuyHistoryWarningMode: 'smart',
  liveGapCollectorBinanceMaxStaleMs: '1500',
  liveGapCollectorDetailedGapBandsEnabled: 'false',
  liveGapCollectorGapBandMode: 'volume_volatility_v2',
  liveGapCollectorUltraCleanGapUsd: '18',
  liveGapCollectorLowCleanGapUsd: '22',
  liveGapCollectorMildCleanGapUsd: '23',
  liveGapCollectorNormalGapUsd: '32',
  liveGapCollectorActiveGapUsd: '31',
  liveGapCollectorHighGapUsd: '48',
  liveGapCollectorHighChopGapUsd: '55',
  liveGapCollectorExtremeChopGapUsd: '55',
  liveGapAdaptiveLowGapEnabled: 'false',
  liveGapAdaptiveLowGapMode: 'active_direct_market_once_v1',
  liveGapAdaptiveLowGapTriggerCount: '1',
  liveGapAdaptiveLowGapStepPct: '0.05',
  liveGapAdaptiveLowGapMaxRelaxPct: '0.05',
  liveGapAdaptiveLowGapMaxShortfallPct: '0.20',
  liveGapAdaptiveLowGapMaxFillCent: '90',
  liveGapAdaptiveLowGapMinRemainingSec: '35',
  liveGapAdaptiveLowGapRequireLocalPathClean: 'true',
  liveGapCollectorLatencyBufferUsd: '2',
  liveGapCollectorStrongOnlyUnderSec: '20',
  liveGapCollectorNoNewEntryUnderSec: '15',
  liveGapCollectorStrongSignalExtraGapUsd: '8',
  liveGapStopLossEnabled: 'true',
  liveGapStopLossEntryGapRatio: '0.33',
  liveGapStopLossMinRemainingSec: '15',
  noReversalEntryGuardEnabled: 'false',
  noReversalDecisionMode: 'historical_adaptive',
  noReversalLookbackMode: 'multi_window_adaptive',
  noReversalBaselineFloorPct: '0.80',
  noReversalDailyFallbackFloorPct: '0.70',
  noReversalSourceMismatchBufferFloorRatio: '0.15',
  noReversalFreezePerMarket: 'true',
  noReversalCacheTtlSec: '60',
  noReversalProfileQueryTimeoutMs: '500',
  noReversalProfileLookupTimeoutMs: '500',
  noReversalPrewarmQueryTimeoutMs: '30000',
  noReversalPrecomputedProfilesEnabled: 'true',
  noReversalAllowColdProfileQuery: 'false',
  noReversalUseLocalPathFallbackOnMissingProfile: 'true',
  noReversalLocalPathFallbackEnabled: 'true',
  noReversalLocalPathLookbackMs: '300000',
  noReversalLocalPathMinHistoryMs: '30000',
  noReversalLocalPathGateMode: 'clean_floor',
  noReversalLocalPathFreshRetraceWindowMs: '10000',
  noReversalLocalPathFreshMaxDropUsd: '5',
  noReversalLocalPathFreshMinHistoryMs: '1000',
  noReversalBlockIfProfileMissingAndLocalPathInsufficient: 'true',
  noReversalProfileMissingEmergencyMarginEnabled: 'true',
  noReversalProfileMissingEmergencyMarginFloorRatio: '0.9',
  noReversalMaxRelaxPctPerWindow: '0.20',
  noReversalMaxTightenPctPerWindow: '0.40',
};

export function applyLiveGapCollectorFormDefaults(fields: Record<string, string>) {
  if (toStringValue(fields.mode).trim().toLowerCase() !== LIVE_GAP_COLLECTOR_MODE) return;
  if (!toStringValue(fields.side).trim()) fields.side = 'buy';
  if (!toStringValue(fields.executionMode).trim()) fields.executionMode = 'market';
  if (!toStringValue(fields.tpEnabled).trim()) fields.tpEnabled = 'true';
  if (!toStringValue(fields.tpPriceCent).trim()) fields.tpPriceCent = '98';
  if (!toStringValue(fields.maxPriceCent).trim()) fields.maxPriceCent = '93';
  for (const [key, value] of Object.entries(DEFAULTS)) {
    if (!toStringValue(fields[key]).trim()) fields[key] = value;
  }
}

export function normalizeLiveGapCollectorBuildConfig(config: Record<string, unknown>): boolean {
  const enabled = toStringValue(config.mode).trim().toLowerCase() === LIVE_GAP_COLLECTOR_MODE;
  if (!enabled) {
    for (const key of LIVE_GAP_COLLECTOR_CONFIG_KEYS) delete config[key];
    return false;
  }
  config.mode = LIVE_GAP_COLLECTOR_MODE;
  config.liveGapCollectorEnabled = config.liveGapCollectorEnabled !== false;
  config.notifyOnLiveGapCollectorDecision = config.notifyOnLiveGapCollectorDecision !== false;
  config.notifyOnLiveGapAdaptiveLowGapChange =
    config.notifyOnLiveGapAdaptiveLowGapChange !== false;
  config.notifyOnLiveGapAdaptiveLowGapNearMissChange =
    config.notifyOnLiveGapAdaptiveLowGapNearMissChange !== false;
  config.liveGapHistoryPrewarmEnabled = config.liveGapHistoryPrewarmEnabled !== false;
  config.notifyOnPreBuyHistoryWarning = config.notifyOnPreBuyHistoryWarning !== false;
  config.noReversalEntryGuardEnabled = config.noReversalEntryGuardEnabled === true;
  const noReversalDecisionMode =
    toStringValue(config.noReversalDecisionMode).trim() === 'local_path_only'
      ? 'local_path_only'
      : 'historical_adaptive';
  config.noReversalDecisionMode = noReversalDecisionMode;
  config.noReversalFreezePerMarket = config.noReversalFreezePerMarket !== false;
  config.noReversalSoftPassOnInsufficientData = false;
  config.noReversalPrecomputedProfilesEnabled = noReversalDecisionMode !== 'local_path_only';
  config.noReversalAllowColdProfileQuery = false;
  config.noReversalUseLocalPathFallbackOnMissingProfile = true;
  config.noReversalLocalPathFallbackEnabled = true;
  config.noReversalBlockIfProfileMissingAndLocalPathInsufficient = true;
  config.noReversalProfileMissingEmergencyMarginEnabled = true;
  config.liveGapCollectorDetailedGapBandsEnabled =
    config.liveGapCollectorDetailedGapBandsEnabled === true;
  config.liveGapCollectorGapBandMode =
    toStringValue(config.liveGapCollectorGapBandMode).trim() || 'volume_volatility_v2';
  config.liveGapAdaptiveLowGapEnabled = config.liveGapAdaptiveLowGapEnabled === true;
  config.liveGapAdaptiveLowGapMode =
    toStringValue(config.liveGapAdaptiveLowGapMode).trim() || 'active_direct_market_once_v1';
  config.liveGapAdaptiveLowGapRequireLocalPathClean =
    config.liveGapAdaptiveLowGapRequireLocalPathClean !== false;
  config.noReversalLocalPathLookbackMs = Number(config.noReversalLocalPathLookbackMs) || 300000;
  config.noReversalLocalPathMinHistoryMs = Number(config.noReversalLocalPathMinHistoryMs) || 30000;
  config.noReversalLocalPathGateMode =
    toStringValue(config.noReversalLocalPathGateMode).trim() === 'fresh_floor_touch'
      ? 'fresh_floor_touch'
      : 'clean_floor';
  config.noReversalLocalPathFreshRetraceWindowMs =
    Number(config.noReversalLocalPathFreshRetraceWindowMs) || 10000;
  config.noReversalLocalPathFreshMaxDropUsd =
    Number(config.noReversalLocalPathFreshMaxDropUsd) || 5;
  config.noReversalLocalPathFreshMinHistoryMs =
    Number(config.noReversalLocalPathFreshMinHistoryMs) || 1000;
  config.noReversalProfileLookupTimeoutMs = Number(config.noReversalProfileLookupTimeoutMs) || 500;
  config.noReversalPrewarmQueryTimeoutMs = Number(config.noReversalPrewarmQueryTimeoutMs) || 30000;
  config.noReversalProfileMissingEmergencyMarginFloorRatio =
    Number(config.noReversalProfileMissingEmergencyMarginFloorRatio) || 0.9;
  config.liveGapHistoryRetentionMs = Math.max(Number(config.liveGapHistoryRetentionMs) || 300000, 300000);
  config.side = 'buy';
  if (!toStringValue(config.executionMode).trim()) config.executionMode = 'market';
  config.tpEnabled = true;
  if (config.tpPriceCent == null) config.tpPriceCent = 98;
  const hardMax = Math.min(Number(config.liveGapCollectorHardMaxPriceCent) || 93, 93);
  config.liveGapCollectorHardMaxPriceCent = hardMax;
  const maxPrice = Number(config.maxPriceCent);
  config.maxPriceCent = Number.isFinite(maxPrice) && maxPrice > 0 ? Math.min(maxPrice, hardMax) : hardMax;
  return true;
}
