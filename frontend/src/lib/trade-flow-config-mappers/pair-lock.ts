import { toStringValue } from './utils';
import { normalizePtbStopLossGapUnit } from './ptb-stop-loss';

export const PAIR_LOCK_CONFIG_KEYS = [
  'pairMaxTotalCent',
  'pairTargetTotalCent',
  'pairSizingMode',
  'pairTotalBudgetUsdc',
  'pairMinNetProfitUsdc',
  'pairProfitSafetyBufferUsdc',
  'pairOrphanGraceMs',
  'notifyOnPairLocked',
  'notifyOnPairUnwind',
  'notifyOnPairNoEdge',
  'counterLegEnabled',
  'counterLegSizeUsdc',
  'counterLegOutcomeLabel',
  'counterLegTriggerCondition',
  'counterLegTriggerPriceCent',
  'counterLegMaxPriceCent',
  'counterLegPriceToBeatGuardEnabled',
  'counterLegPriceToBeatMode',
  'counterLegPriceToBeatMaxDiff',
  'counterLegPriceToBeatMaxDiffUnit',
  'counterLegExecutionFloorGuardEnabled',
  'counterLegExecutionFloorPriceCent',
  'counterLegRetryOnPriceToBeatGuardBlock',
  'counterLegRetryOnExecutionFloorGuardBlock',
  'counterLegRetryOnMaxPriceBlock',
  'counterLegTpEnabled',
  'counterLegTpPriceCent',
  'counterLegTpRules',
  'counterLegNotifyOnTpHit',
  'counterLegSlEnabled',
  'counterLegSlPriceCent',
  'counterLegSlTriggerPriceMode',
  'counterLegPtbStopLossEnabled',
  'counterLegPtbStopLossGapUsd',
  'counterLegPtbStopLossGapUnit',
  'counterLegPtbStopLossTimeDecayMode',
  'counterLegNotifyOnSlHit',
] as const;

export const PAIR_LOCK_SUPPORTED_STOP_LOSS_FIELD_KEYS = [
  'slEnabled',
  'slPriceCent',
  'slTriggerPriceMode',
  'ptbStopLossEnabled',
  'ptbStopLossGapUsd',
  'ptbStopLossGapUnit',
  'ptbStopLossTimeDecayMode',
  'notifyOnSlHit',
  'reenterOnSlHit',
  'reentryMaxAttempts',
  'reentryCooldownSec',
  'counterLegSlEnabled',
  'counterLegSlPriceCent',
  'counterLegSlTriggerPriceMode',
  'counterLegPtbStopLossEnabled',
  'counterLegPtbStopLossGapUsd',
  'counterLegPtbStopLossGapUnit',
  'counterLegPtbStopLossTimeDecayMode',
  'counterLegNotifyOnSlHit',
] as const;

export const PAIR_LOCK_UNSUPPORTED_EXIT_FIELD_KEYS = [
  'slRules',
  'timeExitRules',
  'stagedSlReentryOnlyAfterAllStages',
  'reentryMinPriceCent',
  'reentryMaxPriceCent',
  'reentrySkipCurrentWindow',
  'reentryPriceToBeatMaxDiff',
  'reentryPriceToBeatMaxDiffUnit',
  'reentryThresholdDecay',
  'reentryMaxPriceTightenBps',
  'counterLegSlRules',
  'counterLegPtbStopLossRules',
  'counterLegReenterOnSlHit',
  'counterLegReentryMaxAttempts',
  'counterLegReentryCooldownSec',
] as const;

function applyExplicitCounterStopLossFormDefaults(fields: Record<string, string>): void {
  if (
    (fields.counterLegSlEnabled ?? '').trim().toLowerCase() === 'true' &&
    !(fields.counterLegSlTriggerPriceMode ?? '').trim()
  ) {
    fields.counterLegSlTriggerPriceMode = 'best_bid';
  }
  if (
    (fields.counterLegPtbStopLossEnabled ?? '').trim().toLowerCase() === 'true' &&
    !(fields.counterLegPtbStopLossTimeDecayMode ?? '').trim()
  ) {
    fields.counterLegPtbStopLossTimeDecayMode = 'tighten';
  }
}

export function isPairLockSupportedStopLossField(key: string): boolean {
  return (
    PAIR_LOCK_SUPPORTED_STOP_LOSS_FIELD_KEYS as readonly string[]
  ).includes(key);
}

export function normalizePairLockSizingMode(value: string): 'manual' | 'auto_remaining_budget' {
  return value.trim().toLowerCase() === 'auto_remaining_budget'
    ? 'auto_remaining_budget'
    : 'manual';
}

export function applyPairLockFormDefaults(
  fields: Record<string, string>,
  cfg: Record<string, unknown>
): void {
  const modeRaw = toStringValue(cfg.mode).trim().toLowerCase();
  fields.mode = modeRaw === 'pair_lock' ? 'pair_lock' : 'single';
  if (fields.mode !== 'pair_lock') {
    return;
  }

  if (!(fields.pairMaxTotalCent ?? '').trim()) {
    fields.pairMaxTotalCent = toStringValue(cfg.pairMaxTotalCent ?? cfg.pairTargetTotalCent).trim();
  }
  if (!(fields.counterLegOutcomeLabel ?? '').trim()) {
    fields.counterLegOutcomeLabel = 'opposite';
  }
  if (!(fields.pairOrphanGraceMs ?? '').trim()) {
    fields.pairOrphanGraceMs = '1500';
  }
  fields.pairSizingMode = normalizePairLockSizingMode(toStringValue(cfg.pairSizingMode));
  if (
    fields.pairSizingMode === 'auto_remaining_budget' &&
    !(fields.pairTotalBudgetUsdc ?? '').trim()
  ) {
    fields.pairTotalBudgetUsdc = toStringValue(cfg.pairTotalBudgetUsdc).trim();
  }
  applyExplicitCounterStopLossFormDefaults(fields);
}

export function normalizePairLockBuildConfig(config: Record<string, unknown>): void {
  const pairLockMode = toStringValue(config.mode).trim().toLowerCase() === 'pair_lock';
  if (!pairLockMode) {
    delete config.mode;
    for (const key of PAIR_LOCK_CONFIG_KEYS) {
      delete config[key];
    }
    return;
  }

  const pairMaxTotalCent = Number(toStringValue(config.pairMaxTotalCent ?? config.pairTargetTotalCent).trim());
  if (Number.isFinite(pairMaxTotalCent) && pairMaxTotalCent > 0 && pairMaxTotalCent < 100) {
    config.pairMaxTotalCent = pairMaxTotalCent;
  } else {
    delete config.pairMaxTotalCent;
  }
  delete config.pairTargetTotalCent;
  delete config.pairMinNetProfitUsdc;
  delete config.pairProfitSafetyBufferUsdc;
  delete config.notifyOnPairNoEdge;

  const pairOrphanGraceMs = Number(toStringValue(config.pairOrphanGraceMs).trim());
  if (Number.isFinite(pairOrphanGraceMs) && pairOrphanGraceMs >= 0) {
    config.pairOrphanGraceMs = Math.floor(pairOrphanGraceMs);
  } else {
    delete config.pairOrphanGraceMs;
  }

  if (!toStringValue(config.counterLegOutcomeLabel).trim()) {
    config.counterLegOutcomeLabel = 'opposite';
  }

  config.pairSizingMode = normalizePairLockSizingMode(toStringValue(config.pairSizingMode));
  if (config.pairSizingMode === 'auto_remaining_budget') {
    const pairTotalBudgetUsdc = Number(toStringValue(config.pairTotalBudgetUsdc).trim());
    if (Number.isFinite(pairTotalBudgetUsdc) && pairTotalBudgetUsdc > 0) {
      config.pairTotalBudgetUsdc = pairTotalBudgetUsdc;
    } else {
      delete config.pairTotalBudgetUsdc;
    }
    delete config.counterLegSizeUsdc;
  } else {
    delete config.pairTotalBudgetUsdc;
  }

  if (config.counterLegPriceToBeatGuardEnabled === true) {
    const counterLegPriceToBeatModeRaw = toStringValue(config.counterLegPriceToBeatMode).trim().toLowerCase();
    config.counterLegPriceToBeatMode =
      counterLegPriceToBeatModeRaw === 'auto_last_3_avg_excursion'
        ? 'auto_last_3_avg_excursion'
        : counterLegPriceToBeatModeRaw === 'auto_vol_pct'
          ? 'auto_vol_pct'
          : 'manual';
    if (config.counterLegPriceToBeatMode === 'manual') {
      const counterLegPriceToBeatUnitRaw = toStringValue(config.counterLegPriceToBeatMaxDiffUnit).trim().toLowerCase();
      config.counterLegPriceToBeatMaxDiffUnit = counterLegPriceToBeatUnitRaw === 'cent' ? 'cent' : 'usd';
    } else {
      delete config.counterLegPriceToBeatMaxDiff;
      delete config.counterLegPriceToBeatMaxDiffUnit;
    }
  } else {
    delete config.counterLegPriceToBeatMode;
    delete config.counterLegPriceToBeatMaxDiff;
    delete config.counterLegPriceToBeatMaxDiffUnit;
  }
}

export function normalizePairLockStopLossBuildConfig(
  config: Record<string, unknown>
): void {
  const slEnabled = config.slEnabled === true;
  const ptbStopLossEnabled = config.ptbStopLossEnabled === true;
  const anyStopLossEnabled = slEnabled || ptbStopLossEnabled;
  const counterSlEnabled = config.counterLegSlEnabled === true;
  const counterPtbStopLossEnabled = config.counterLegPtbStopLossEnabled === true;
  const anyCounterStopLossEnabled = counterSlEnabled || counterPtbStopLossEnabled;

  for (const key of PAIR_LOCK_UNSUPPORTED_EXIT_FIELD_KEYS) {
    delete config[key];
  }

  if (!slEnabled) {
    delete config.slEnabled;
    delete config.slPriceCent;
    delete config.slPrice;
    delete config.slTriggerPriceMode;
  }

  if (!ptbStopLossEnabled) {
    delete config.ptbStopLossEnabled;
    delete config.ptbStopLossGapUsd;
    delete config.ptbStopLossGapUnit;
    delete config.ptbStopLossRules;
    delete config.ptbStopLossTimeDecayMode;
  } else {
    config.ptbStopLossGapUnit = normalizePtbStopLossGapUnit(config.ptbStopLossGapUnit);
    const ptbStopLossTimeDecayModeRaw = toStringValue(config.ptbStopLossTimeDecayMode)
      .trim()
      .toLowerCase();
    config.ptbStopLossTimeDecayMode =
      ptbStopLossTimeDecayModeRaw === 'none' || ptbStopLossTimeDecayModeRaw === 'relax'
        ? ptbStopLossTimeDecayModeRaw
        : 'tighten';
  }

  if (!counterSlEnabled) {
    delete config.counterLegSlPriceCent;
    delete config.counterLegSlTriggerPriceMode;
  } else {
    const counterLegSlTriggerPriceModeRaw = toStringValue(config.counterLegSlTriggerPriceMode)
      .trim()
      .toLowerCase();
    config.counterLegSlTriggerPriceMode =
      counterLegSlTriggerPriceModeRaw === 'composite' ||
      counterLegSlTriggerPriceModeRaw === 'composite_safe' ||
      counterLegSlTriggerPriceModeRaw === 'composite_fast' ||
      counterLegSlTriggerPriceModeRaw === 'last_trade'
        ? counterLegSlTriggerPriceModeRaw
        : 'best_bid';
  }

  if (!counterPtbStopLossEnabled) {
    delete config.counterLegPtbStopLossGapUsd;
    delete config.counterLegPtbStopLossGapUnit;
    delete config.counterLegPtbStopLossTimeDecayMode;
  } else {
    config.counterLegPtbStopLossGapUnit = normalizePtbStopLossGapUnit(
      config.counterLegPtbStopLossGapUnit,
      config.ptbStopLossGapUnit === 'cent' ? 'cent' : 'usd'
    );
    const counterLegPtbStopLossTimeDecayModeRaw = toStringValue(
      config.counterLegPtbStopLossTimeDecayMode
    )
      .trim()
      .toLowerCase();
    config.counterLegPtbStopLossTimeDecayMode =
      counterLegPtbStopLossTimeDecayModeRaw === 'none' ||
      counterLegPtbStopLossTimeDecayModeRaw === 'relax'
        ? counterLegPtbStopLossTimeDecayModeRaw
        : 'tighten';
  }

  if (!anyStopLossEnabled) {
    delete config.notifyOnSlHit;
    delete config.reenterOnSlHit;
    delete config.reentryMaxAttempts;
    delete config.reentryCooldownSec;
    return;
  }

  if (!anyCounterStopLossEnabled) {
    delete config.counterLegNotifyOnSlHit;
  }

  if (config.reenterOnSlHit !== true) {
    delete config.reentryMaxAttempts;
    delete config.reentryCooldownSec;
    return;
  }

  const reentryMaxAttempts = Number(toStringValue(config.reentryMaxAttempts).trim());
  if (
    Number.isInteger(reentryMaxAttempts) &&
    reentryMaxAttempts >= 1 &&
    reentryMaxAttempts <= 10
  ) {
    config.reentryMaxAttempts = reentryMaxAttempts;
  } else {
    delete config.reentryMaxAttempts;
  }

  const reentryCooldownSec = Number(toStringValue(config.reentryCooldownSec).trim());
  if (Number.isInteger(reentryCooldownSec) && reentryCooldownSec >= 0) {
    config.reentryCooldownSec = reentryCooldownSec;
  } else {
    delete config.reentryCooldownSec;
  }
}

export function normalizePairLockTakeProfitBuildConfig(
  config: Record<string, unknown>
): void {
  const primaryTpEnabled =
    config.tpEnabled === true ||
    (Array.isArray(config.tpRules) && config.tpRules.length > 0);
  const counterTpEnabled =
    config.counterLegEnabled === true &&
    (config.counterLegTpEnabled === true ||
      (Array.isArray(config.counterLegTpRules) && config.counterLegTpRules.length > 0));

  if (!primaryTpEnabled) {
    delete config.tpEnabled;
    delete config.tpPriceCent;
    delete config.tpPrice;
    delete config.tpRules;
    delete config.notifyOnTpHit;
  }

  if (!counterTpEnabled) {
    delete config.counterLegTpEnabled;
    delete config.counterLegTpPriceCent;
    delete config.counterLegTpRules;
    delete config.counterLegNotifyOnTpHit;
  }
}
