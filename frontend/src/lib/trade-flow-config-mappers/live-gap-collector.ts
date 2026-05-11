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
  'liveGapHistoryPrewarmEnabled',
  'liveGapHistoryPrewarmSec',
  'liveGapHistoryPrewarmStartMode',
  'liveGapHistoryPrewarmSides',
  'liveGapHistorySampleMs',
  'liveGapHistoryRetentionMs',
  'notifyOnPreBuyHistoryWarning',
  'preBuyHistoryWarningMode',
  'liveGapCollectorBinanceMaxStaleMs',
  'liveGapCollectorLowCleanGapUsd',
  'liveGapCollectorNormalGapUsd',
  'liveGapCollectorHighGapUsd',
  'liveGapCollectorHighChopGapUsd',
  'liveGapCollectorLatencyBufferUsd',
  'liveGapCollectorStrongOnlyUnderSec',
  'liveGapCollectorNoNewEntryUnderSec',
  'liveGapCollectorStrongSignalExtraGapUsd',
  'liveGapStopLossEnabled',
  'liveGapStopLossEntryGapRatio',
  'liveGapStopLossGapUsd',
  'liveGapStopLossMinRemainingSec',
  'noReversalEntryGuardEnabled',
  'noReversalLookbackMode',
  'noReversalBaselineFloorPct',
  'noReversalDailyFallbackFloorPct',
  'noReversalSourceMismatchBufferUsd',
  'noReversalSourceMismatchBufferFloorRatio',
  'noReversalLateHighExtraBufferUsd',
  'noReversalFreezePerMarket',
  'noReversalCacheTtlSec',
  'noReversalProfileQueryTimeoutMs',
  'noReversalMaxRelaxPctPerWindow',
  'noReversalMaxTightenPctPerWindow',
  'noReversalSoftPassOnInsufficientData',
];

const DEFAULTS: Record<string, string> = {
  liveGapCollectorEnabled: 'true',
  liveGapCollectorWindowStartSec: '220',
  liveGapCollectorWindowEndSec: '285',
  liveGapCollectorRetryMs: '150',
  liveGapCollectorHardMaxPriceCent: '93',
  liveGapCollectorPtbTelemetryEnabled: 'true',
  notifyOnLiveGapCollectorDecision: 'true',
  liveGapHistoryPrewarmEnabled: 'true',
  liveGapHistoryPrewarmSec: '20',
  liveGapHistoryPrewarmStartMode: 'before_trigger_window',
  liveGapHistoryPrewarmSides: 'both',
  liveGapHistorySampleMs: '250',
  liveGapHistoryRetentionMs: '30000',
  notifyOnPreBuyHistoryWarning: 'true',
  preBuyHistoryWarningMode: 'smart',
  liveGapCollectorBinanceMaxStaleMs: '1500',
  liveGapCollectorLowCleanGapUsd: '22',
  liveGapCollectorNormalGapUsd: '32',
  liveGapCollectorHighGapUsd: '48',
  liveGapCollectorHighChopGapUsd: '55',
  liveGapCollectorLatencyBufferUsd: '2',
  liveGapCollectorStrongOnlyUnderSec: '20',
  liveGapCollectorNoNewEntryUnderSec: '15',
  liveGapCollectorStrongSignalExtraGapUsd: '8',
  liveGapStopLossEnabled: 'true',
  liveGapStopLossEntryGapRatio: '0.33',
  liveGapStopLossMinRemainingSec: '15',
  noReversalEntryGuardEnabled: 'false',
  noReversalLookbackMode: 'multi_window_adaptive',
  noReversalBaselineFloorPct: '0.80',
  noReversalDailyFallbackFloorPct: '0.70',
  noReversalSourceMismatchBufferFloorRatio: '0.15',
  noReversalFreezePerMarket: 'true',
  noReversalCacheTtlSec: '60',
  noReversalProfileQueryTimeoutMs: '500',
  noReversalMaxRelaxPctPerWindow: '0.20',
  noReversalMaxTightenPctPerWindow: '0.40',
  noReversalSoftPassOnInsufficientData: 'true',
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
  config.liveGapHistoryPrewarmEnabled = config.liveGapHistoryPrewarmEnabled !== false;
  config.notifyOnPreBuyHistoryWarning = config.notifyOnPreBuyHistoryWarning !== false;
  config.noReversalEntryGuardEnabled = config.noReversalEntryGuardEnabled === true;
  config.noReversalFreezePerMarket = config.noReversalFreezePerMarket !== false;
  config.noReversalSoftPassOnInsufficientData = config.noReversalSoftPassOnInsufficientData !== false;
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
