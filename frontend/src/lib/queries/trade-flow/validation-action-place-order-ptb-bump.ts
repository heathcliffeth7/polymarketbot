import type { TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { toBooleanish, toFiniteNumber } from './shared';
import { pushNodeError } from './validation-core';

interface ParsedPtbStopLossBumpLossRule {
  lossUsd: number;
  bumpValue: number;
}

function normalizePriceToBeatStopLossBumpModeValue(
  value: unknown
): 'fixed' | 'loss_table' | null {
  const normalized = String(value ?? '').trim().toLowerCase();
  if (!normalized) return null;
  if (normalized === 'fixed' || normalized === 'loss_table') {
    return normalized;
  }
  return null;
}

function parsePtbStopLossBumpLossRules(raw: unknown): {
  isArray: boolean;
  validRules: ParsedPtbStopLossBumpLossRule[];
  invalidItem: boolean;
} {
  if (!Array.isArray(raw)) {
    return { isArray: false, validRules: [], invalidItem: false };
  }

  const validRules: ParsedPtbStopLossBumpLossRule[] = [];
  let invalidItem = false;
  for (const item of raw) {
    if (item == null || typeof item !== 'object' || Array.isArray(item)) {
      invalidItem = true;
      continue;
    }
    const lossUsd = toFiniteNumber((item as Record<string, unknown>).lossUsd);
    const bumpValue = toFiniteNumber((item as Record<string, unknown>).bumpValue);
    if (lossUsd == null || lossUsd <= 0 || bumpValue == null || bumpValue <= 0) {
      invalidItem = true;
      continue;
    }
    validRules.push({ lossUsd, bumpValue });
  }

  if (raw.length > 0 && validRules.length === 0) {
    invalidItem = true;
  }

  return { isArray: true, validRules, invalidItem };
}

export function validateActionPlaceOrderPtbStopLossBumpConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  side: string,
  config: Record<string, unknown>,
  priceToBeatGuardEnabled: boolean | null
) {
  const bumpEnabled = toBooleanish(config.priceToBeatStopLossBumpEnabled);
  const bumpMode = normalizePriceToBeatStopLossBumpModeValue(
    config.priceToBeatStopLossBumpMode
  );
  const hasBumpAmount = config.priceToBeatStopLossBumpAmount != null;
  const hasBumpMaxValue = config.priceToBeatStopLossBumpMaxValue != null;
  const hasBumpMode = config.priceToBeatStopLossBumpMode != null;
  const bumpUnit = String(config.priceToBeatStopLossBumpUnit ?? '').trim().toLowerCase();
  const hasBumpUnit = bumpUnit.length > 0;
  const parsedLossRules = parsePtbStopLossBumpLossRules(
    config.priceToBeatStopLossBumpLossRules
  );
  const hasBumpLossRules = config.priceToBeatStopLossBumpLossRules != null;
  const relaxEnabled = toBooleanish(config.priceToBeatMaxPriceRelaxEnabled);
  const hasRelaxEnabled = config.priceToBeatMaxPriceRelaxEnabled != null;
  const relaxMissCountRaw = String(config.priceToBeatMaxPriceRelaxMissCount ?? '').trim();
  const hasRelaxMissCount = relaxMissCountRaw.length > 0;
  const relaxHistoryCountRaw = String(config.priceToBeatMaxPriceRelaxHistoryCount ?? '').trim();
  const hasRelaxHistoryCount = relaxHistoryCountRaw.length > 0;
  const relaxMinValueRaw = String(config.priceToBeatMaxPriceRelaxMinValue ?? '').trim();
  const hasRelaxMinValue = relaxMinValueRaw.length > 0;
  const relaxMinUnit = String(config.priceToBeatMaxPriceRelaxMinUnit ?? '')
    .trim()
    .toLowerCase();
  const hasRelaxMinUnit = relaxMinUnit.length > 0;
  const relaxStepMode = String(config.priceToBeatMaxPriceRelaxStepMode ?? '')
    .trim()
    .toLowerCase();
  const hasRelaxStepMode = relaxStepMode.length > 0;
  const relaxStepValueRaw = String(config.priceToBeatMaxPriceRelaxStepValue ?? '').trim();
  const hasRelaxStepValue = relaxStepValueRaw.length > 0;
  const relaxStepUnit = String(config.priceToBeatMaxPriceRelaxStepUnit ?? '')
    .trim()
    .toLowerCase();
  const hasRelaxStepUnit = relaxStepUnit.length > 0;
  const hasRelaxConfig =
    hasRelaxEnabled ||
    hasRelaxMissCount ||
    hasRelaxHistoryCount ||
    hasRelaxMinValue ||
    hasRelaxMinUnit ||
    hasRelaxStepMode ||
    hasRelaxStepValue ||
    hasRelaxStepUnit;

  if (config.priceToBeatStopLossBumpEnabled != null && bumpEnabled == null) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_stop_loss_bump_enabled',
      'action.place_order priceToBeatStopLossBumpEnabled must be boolean (true/false).'
    );
  }

  if (hasRelaxEnabled && relaxEnabled == null) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_max_price_relax_enabled',
      'action.place_order priceToBeatMaxPriceRelaxEnabled must be boolean (true/false).'
    );
  }

  if (hasBumpMode && bumpMode == null) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_stop_loss_bump_mode',
      'action.place_order priceToBeatStopLossBumpMode must be fixed or loss_table.'
    );
  }

  if (bumpEnabled === true) {
    if (side !== 'buy') {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_stop_loss_bump_side',
        'action.place_order priceToBeatStopLossBumpEnabled is only valid for side=buy.'
      );
    }
    if (priceToBeatGuardEnabled !== true) {
      pushNodeError(
        issues,
        node,
        'price_to_beat_stop_loss_bump_requires_guard',
        'priceToBeatStopLossBumpEnabled requires priceToBeatGuardEnabled=true.'
      );
    }

    const effectiveBumpMode = bumpMode ?? 'fixed';
    const bumpAmount = toFiniteNumber(config.priceToBeatStopLossBumpAmount);
    const bumpMaxValue = toFiniteNumber(config.priceToBeatStopLossBumpMaxValue);
    if (hasBumpMaxValue && (bumpMaxValue == null || bumpMaxValue <= 0)) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_stop_loss_bump_max_value',
        'action.place_order priceToBeatStopLossBumpMaxValue must be > 0.'
      );
    }
    if (bumpUnit !== 'usd' && bumpUnit !== 'cent') {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_stop_loss_bump_unit',
        'action.place_order priceToBeatStopLossBumpUnit must be usd or cent.'
      );
    }
    if (effectiveBumpMode === 'fixed') {
      if (bumpAmount == null || bumpAmount <= 0) {
        pushNodeError(
          issues,
          node,
          'invalid_price_to_beat_stop_loss_bump_amount',
          'action.place_order priceToBeatStopLossBumpAmount must be > 0.'
        );
      }
      if (
        bumpAmount != null &&
        bumpAmount > 0 &&
        bumpMaxValue != null &&
        bumpMaxValue < bumpAmount
      ) {
        pushNodeError(
          issues,
          node,
          'invalid_price_to_beat_stop_loss_bump_max_value_range',
          'action.place_order priceToBeatStopLossBumpMaxValue must be >= priceToBeatStopLossBumpAmount.'
        );
      }
      if (hasBumpLossRules || parsedLossRules.isArray) {
        pushNodeError(
          issues,
          node,
          'price_to_beat_stop_loss_bump_loss_rules_require_loss_table_mode',
          'action.place_order priceToBeatStopLossBumpLossRules requires priceToBeatStopLossBumpMode=loss_table.'
        );
      }
    } else {
      if (hasBumpAmount) {
        pushNodeError(
          issues,
          node,
          'price_to_beat_stop_loss_bump_amount_only_in_fixed_mode',
          'action.place_order priceToBeatStopLossBumpAmount is only valid when priceToBeatStopLossBumpMode=fixed.'
        );
      }
      if (!parsedLossRules.isArray || parsedLossRules.validRules.length === 0) {
        pushNodeError(
          issues,
          node,
          'missing_price_to_beat_stop_loss_bump_loss_rules',
          'action.place_order priceToBeatStopLossBumpMode=loss_table requires priceToBeatStopLossBumpLossRules.'
        );
      }
      if (parsedLossRules.invalidItem) {
        pushNodeError(
          issues,
          node,
          'invalid_price_to_beat_stop_loss_bump_loss_rules',
          'action.place_order priceToBeatStopLossBumpLossRules entries must provide positive lossUsd and bumpValue.'
        );
      }
      for (let index = 1; index < parsedLossRules.validRules.length; index += 1) {
        if (
          parsedLossRules.validRules[index - 1].lossUsd >=
          parsedLossRules.validRules[index].lossUsd
        ) {
          pushNodeError(
            issues,
            node,
            'invalid_price_to_beat_stop_loss_bump_loss_rules_order',
            'action.place_order priceToBeatStopLossBumpLossRules lossUsd values must be strictly increasing.'
          );
          break;
        }
      }
    }
  } else if (hasBumpAmount || hasBumpMaxValue || hasBumpUnit || hasBumpMode || hasBumpLossRules) {
    pushNodeError(
      issues,
      node,
      'price_to_beat_stop_loss_bump_requires_toggle',
      'priceToBeatStopLossBump mode/amount/lossRules/max/unit require priceToBeatStopLossBumpEnabled=true.'
    );
  }

  if (hasRelaxConfig && priceToBeatGuardEnabled !== true) {
    pushNodeError(
      issues,
      node,
      'price_to_beat_max_price_relax_requires_guard',
      'MaxPrice relax config requires priceToBeatGuardEnabled=true.'
    );
  }

  if (hasRelaxMissCount) {
    const relaxMissCount = toFiniteNumber(config.priceToBeatMaxPriceRelaxMissCount);
    if (
      relaxMissCount == null ||
      !Number.isInteger(relaxMissCount) ||
      relaxMissCount <= 0
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_max_price_relax_miss_count',
        'action.place_order priceToBeatMaxPriceRelaxMissCount must be an integer > 0.'
      );
    }
  }

  if (hasRelaxHistoryCount) {
    const relaxHistoryCount = toFiniteNumber(config.priceToBeatMaxPriceRelaxHistoryCount);
    if (
      relaxHistoryCount == null ||
      !Number.isInteger(relaxHistoryCount) ||
      relaxHistoryCount <= 0
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_max_price_relax_history_count',
        'action.place_order priceToBeatMaxPriceRelaxHistoryCount must be an integer > 0.'
      );
    }
  }

  if (hasRelaxMinValue) {
    const relaxMinValue = toFiniteNumber(config.priceToBeatMaxPriceRelaxMinValue);
    if (relaxMinValue == null || relaxMinValue <= 0) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_max_price_relax_min_value',
        'action.place_order priceToBeatMaxPriceRelaxMinValue must be > 0.'
      );
    }
  }

  if (hasRelaxMinUnit && !hasRelaxMinValue) {
    pushNodeError(
      issues,
      node,
      'price_to_beat_max_price_relax_min_unit_requires_value',
      'action.place_order priceToBeatMaxPriceRelaxMinUnit requires priceToBeatMaxPriceRelaxMinValue.'
    );
  }
  if (hasRelaxMinValue && relaxMinUnit !== 'usd' && relaxMinUnit !== 'cent') {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_max_price_relax_min_unit',
      'action.place_order priceToBeatMaxPriceRelaxMinUnit must be usd or cent.'
    );
  }

  if (
    hasRelaxStepMode &&
    relaxStepMode !== 'percent' &&
    relaxStepMode !== 'absolute'
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_max_price_relax_step_mode',
      'action.place_order priceToBeatMaxPriceRelaxStepMode must be percent or absolute.'
    );
  }

  if (hasRelaxStepValue) {
    const relaxStepValue = toFiniteNumber(config.priceToBeatMaxPriceRelaxStepValue);
    if (relaxStepValue == null || relaxStepValue <= 0) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_max_price_relax_step_value',
        'action.place_order priceToBeatMaxPriceRelaxStepValue must be > 0.'
      );
    } else if (
      (relaxStepMode === '' || relaxStepMode === 'percent') &&
      relaxStepValue > 100
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_max_price_relax_step_percent_value',
        'action.place_order priceToBeatMaxPriceRelaxStepValue must be <= 100 in percent mode.'
      );
    }
  }

  if (relaxStepMode === 'absolute' && !hasRelaxStepUnit) {
    pushNodeError(
      issues,
      node,
      'price_to_beat_max_price_relax_step_unit_required',
      'action.place_order priceToBeatMaxPriceRelaxStepUnit is required when priceToBeatMaxPriceRelaxStepMode=absolute.'
    );
  }
  if (
    hasRelaxStepUnit &&
    relaxStepMode === 'absolute' &&
    relaxStepUnit !== 'usd' &&
    relaxStepUnit !== 'cent'
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_max_price_relax_step_unit',
      'action.place_order priceToBeatMaxPriceRelaxStepUnit must be usd or cent.'
    );
  }
}
