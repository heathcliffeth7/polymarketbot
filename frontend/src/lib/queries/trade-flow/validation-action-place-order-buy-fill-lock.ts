import type { TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { toBooleanish, toTrimmedString } from './shared';
import { pushNodeError } from './validation-core';

export function validateActionPlaceOrderBuyFillLockConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>,
  side: string
) {
  const buyFillLockEnabled = toBooleanish(config.buyFillLockEnabled);
  const releaseBuyFillLockOnStopLoss = toBooleanish(config.releaseBuyFillLockOnStopLoss);
  const buyFillLockGroup = toTrimmedString(config.buyFillLockGroup);

  if (config.buyFillLockEnabled != null && buyFillLockEnabled == null) {
    pushNodeError(
      issues,
      node,
      'invalid_buy_fill_lock_enabled',
      'action.place_order buyFillLockEnabled must be boolean (true/false).'
    );
  }
  if (
    config.releaseBuyFillLockOnStopLoss != null &&
    releaseBuyFillLockOnStopLoss == null
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_release_buy_fill_lock_on_stop_loss',
      'action.place_order releaseBuyFillLockOnStopLoss must be boolean (true/false).'
    );
  }
  if (buyFillLockEnabled === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_buy_fill_lock_side',
      'action.place_order buyFillLockEnabled is only valid for side=buy.'
    );
  }
  if (buyFillLockEnabled === true && !buyFillLockGroup) {
    pushNodeError(
      issues,
      node,
      'missing_buy_fill_lock_group',
      'action.place_order buyFillLockEnabled=true requires buyFillLockGroup.'
    );
  }
  if (
    (buyFillLockGroup || releaseBuyFillLockOnStopLoss === true) &&
    buyFillLockEnabled !== true
  ) {
    pushNodeError(
      issues,
      node,
      'buy_fill_lock_toggle_required',
      'action.place_order buyFillLockGroup and releaseBuyFillLockOnStopLoss require buyFillLockEnabled=true.'
    );
  }
}
