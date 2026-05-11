import type { TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { isRecord, toFiniteNumber } from './shared';

export function pushNodeError(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  code: string,
  message: string
) {
  issues.push({ severity: 'error', code, message, nodeKey: node.key });
}

export function pushNodeWarning(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  code: string,
  message: string
) {
  issues.push({ severity: 'warning', code, message, nodeKey: node.key });
}

export function validateAuxiliaryNodeConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>
) {
  if (node.type === 'trigger.time_window') {
    const startAt = config.startAt == null ? null : String(config.startAt);
    const endAt = config.endAt == null ? null : String(config.endAt);
    if (startAt && Number.isNaN(new Date(startAt).getTime())) {
      pushNodeError(issues, node, 'invalid_start_at', 'trigger.time_window startAt must be RFC3339 datetime.');
    }
    if (endAt && Number.isNaN(new Date(endAt).getTime())) {
      pushNodeError(issues, node, 'invalid_end_at', 'trigger.time_window endAt must be RFC3339 datetime.');
    }
  }

  if (node.type === 'logic.if' && !isRecord(config.expression)) {
    pushNodeError(issues, node, 'missing_expression', 'logic.if requires expression object (JSONLogic).');
  }

  if (node.type === 'logic.switch' && config.expression === undefined) {
    pushNodeError(issues, node, 'missing_expression', 'logic.switch requires expression.');
  }

  if (node.type === 'logic.delay') {
    const delayMs = toFiniteNumber(config.delayMs ?? config.ms);
    if (delayMs != null && delayMs < 0) {
      pushNodeError(issues, node, 'invalid_delay', 'logic.delay delayMs must be >= 0.');
    }
  }

  if (node.type === 'logic.retry') {
    const maxAttempts = toFiniteNumber(config.maxAttempts);
    if (maxAttempts != null && maxAttempts < 1) {
      pushNodeError(issues, node, 'invalid_max_attempts', 'logic.retry maxAttempts must be >= 1.');
    }
  }
  if (node.type === 'action.cancel_order' || node.type === 'action.update_order') {
    const hasId = toFiniteNumber(config.builderOrderId) != null;
    const hasRef = String(config.targetRef ?? '').trim().length > 0;
    if (!hasId && !hasRef) {
      pushNodeError(
        issues,
        node,
        'missing_target_ref',
        `${node.type} requires builderOrderId or targetRef.`
      );
    }
  }

  if (node.type === 'action.set_state') {
    const patch = config.statePatch ?? config.state;
    if (patch !== undefined && !isRecord(patch)) {
      pushNodeError(
        issues,
        node,
        'invalid_state_patch',
        'action.set_state statePatch/state must be an object.'
      );
    }
  }

}
