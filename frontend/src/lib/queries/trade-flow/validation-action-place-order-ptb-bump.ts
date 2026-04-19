import type { TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { toBooleanish, toFiniteNumber } from './shared';
import { pushNodeError } from './validation-core';

export function validateActionPlaceOrderPtbStopLossBumpConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  side: string,
  config: Record<string, unknown>,
  priceToBeatGuardEnabled: boolean | null
) {
  const bumpEnabled = toBooleanish(config.priceToBeatStopLossBumpEnabled);
  const hasBumpAmount = config.priceToBeatStopLossBumpAmount != null;
  const hasBumpMaxValue = config.priceToBeatStopLossBumpMaxValue != null;
  const bumpUnit = String(config.priceToBeatStopLossBumpUnit ?? '').trim().toLowerCase();
  const hasBumpUnit = bumpUnit.length > 0;
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

    const bumpAmount = toFiniteNumber(config.priceToBeatStopLossBumpAmount);
    if (bumpAmount == null || bumpAmount <= 0) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_stop_loss_bump_amount',
        'action.place_order priceToBeatStopLossBumpAmount must be > 0.'
      );
    }
    const bumpMaxValue = toFiniteNumber(config.priceToBeatStopLossBumpMaxValue);
    if (hasBumpMaxValue && (bumpMaxValue == null || bumpMaxValue <= 0)) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_stop_loss_bump_max_value',
        'action.place_order priceToBeatStopLossBumpMaxValue must be > 0.'
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
    if (bumpUnit !== 'usd' && bumpUnit !== 'cent') {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_stop_loss_bump_unit',
        'action.place_order priceToBeatStopLossBumpUnit must be usd or cent.'
      );
    }
  } else if (hasBumpAmount || hasBumpMaxValue || hasBumpUnit) {
    pushNodeError(
      issues,
      node,
      'price_to_beat_stop_loss_bump_requires_toggle',
      'priceToBeatStopLossBumpAmount/max/unit require priceToBeatStopLossBumpEnabled=true.'
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
