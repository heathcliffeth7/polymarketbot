import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { hasUpstreamTriggerWithTriggerPrice } from './graph';
import { toBooleanish, toFiniteNumber } from './shared';
import { pushNodeError } from './validation-core';

export function validateActionPlaceOrderExecutionFloorConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  graph: TradeFlowGraph,
  side: string,
  config: Record<string, unknown>
) {
  const executionFloorGuardEnabled = toBooleanish(config.executionFloorGuardEnabled);
  if (config.executionFloorGuardEnabled != null && executionFloorGuardEnabled == null) {
    pushNodeError(
      issues,
      node,
      'invalid_execution_floor_guard_enabled',
      'action.place_order executionFloorGuardEnabled must be boolean (true/false).'
    );
  }

  const executionFloorPriceCent = toFiniteNumber(config.executionFloorPriceCent);
  const hasConfiguredExecutionFloorPrice =
    executionFloorPriceCent != null &&
    executionFloorPriceCent > 0 &&
    executionFloorPriceCent <= 100;
  if (
    config.executionFloorPriceCent != null &&
    !hasConfiguredExecutionFloorPrice
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_execution_floor_price_cent',
      'action.place_order executionFloorPriceCent must be in (0, 100].'
    );
  }

  if (executionFloorGuardEnabled === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_execution_floor_guard_side',
      'action.place_order executionFloorGuardEnabled is only valid for side=buy.'
    );
  }
  if (
    executionFloorGuardEnabled === true &&
    !hasConfiguredExecutionFloorPrice &&
    !hasUpstreamTriggerWithTriggerPrice(node.key, graph)
  ) {
    pushNodeError(
      issues,
      node,
      'missing_upstream_execution_floor_trigger_price',
      'executionFloorGuardEnabled requires executionFloorPriceCent or an upstream trigger with configured triggerPrice.'
    );
  }

  const retryOnExecutionFloorGuardBlock = toBooleanish(config.retryOnExecutionFloorGuardBlock);
  if (
    config.retryOnExecutionFloorGuardBlock != null &&
    retryOnExecutionFloorGuardBlock == null
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_retry_on_execution_floor_guard_block',
      'action.place_order retryOnExecutionFloorGuardBlock must be boolean (true/false).'
    );
  }
  if (retryOnExecutionFloorGuardBlock === true && executionFloorGuardEnabled !== true) {
    pushNodeError(
      issues,
      node,
      'retry_on_execution_floor_guard_block_requires_guard',
      'retryOnExecutionFloorGuardBlock requires executionFloorGuardEnabled=true.'
    );
  }

  const notifyOnExecutionFloorBlocked = toBooleanish(config.notifyOnExecutionFloorBlocked);
  if (
    config.notifyOnExecutionFloorBlocked != null &&
    notifyOnExecutionFloorBlocked == null
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_execution_floor_blocked',
      'action.place_order notifyOnExecutionFloorBlocked must be boolean (true/false).'
    );
  }
  if (notifyOnExecutionFloorBlocked === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_execution_floor_blocked_side',
      'action.place_order notifyOnExecutionFloorBlocked is only valid for side=buy.'
    );
  }
  if (
    notifyOnExecutionFloorBlocked === true &&
    executionFloorGuardEnabled !== true
  ) {
    pushNodeError(
      issues,
      node,
      'notify_on_execution_floor_blocked_requires_guard',
      'notifyOnExecutionFloorBlocked requires executionFloorGuardEnabled=true.'
    );
  }
}
