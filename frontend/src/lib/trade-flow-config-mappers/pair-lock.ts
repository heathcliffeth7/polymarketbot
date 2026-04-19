import { toStringValue } from './utils';

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
] as const;

export const PAIR_LOCK_SUPPORTED_STOP_LOSS_FIELD_KEYS = [
  'slEnabled',
  'slPriceCent',
  'slTriggerPriceMode',
  'ptbStopLossEnabled',
  'ptbStopLossGapUsd',
  'ptbStopLossTimeDecayMode',
  'notifyOnSlHit',
  'reenterOnSlHit',
  'reentryMaxAttempts',
  'reentryCooldownSec',
] as const;

export const PAIR_LOCK_UNSUPPORTED_EXIT_FIELD_KEYS = [
  'tpEnabled',
  'tpPriceCent',
  'tpPrice',
  'tpRules',
  'slRules',
  'ptbStopLossRules',
  'timeExitRules',
  'notifyOnTpHit',
  'stagedSlReentryOnlyAfterAllStages',
  'reentryMinPriceCent',
  'reentryMaxPriceCent',
  'reentrySkipCurrentWindow',
  'reentryPriceToBeatMaxDiff',
  'reentryPriceToBeatMaxDiffUnit',
  'reentryThresholdDecay',
  'reentryMaxPriceTightenBps',
] as const;

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
    delete config.ptbStopLossTimeDecayMode;
  } else {
    const ptbStopLossTimeDecayModeRaw = toStringValue(config.ptbStopLossTimeDecayMode)
      .trim()
      .toLowerCase();
    config.ptbStopLossTimeDecayMode =
      ptbStopLossTimeDecayModeRaw === 'none' || ptbStopLossTimeDecayModeRaw === 'relax'
        ? ptbStopLossTimeDecayModeRaw
        : 'tighten';
  }

  if (!anyStopLossEnabled) {
    delete config.notifyOnSlHit;
    delete config.reenterOnSlHit;
    delete config.reentryMaxAttempts;
    delete config.reentryCooldownSec;
    return;
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
