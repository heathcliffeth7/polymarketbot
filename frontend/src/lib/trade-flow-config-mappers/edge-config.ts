import { createEmptyConditionDraft } from './drafts';
import { buildSimpleCondition, parseSimpleCondition } from './expressions';
import type { EdgeConditionFormState } from './types';
import { createId, safeJsonStringify } from './utils';

export function parseEdgeConditionToForm(condition: unknown): EdgeConditionFormState {
  if (condition == null) {
    return {
      enabled: false,
      conditionRow: createEmptyConditionDraft(),
      conditionSupported: true,
      advancedJson: '',
    };
  }

  const parsed = parseSimpleCondition(condition);
  if (parsed) {
    return {
      enabled: true,
      conditionRow: { id: createId('edge_cond'), ...parsed },
      conditionSupported: true,
      advancedJson: safeJsonStringify(condition),
    };
  }

  return {
    enabled: true,
    conditionRow: createEmptyConditionDraft(),
    conditionSupported: false,
    advancedJson: safeJsonStringify(condition),
  };
}

export function buildEdgeConditionFromForm(form: EdgeConditionFormState): Record<string, unknown> | null {
  if (!form.enabled) return null;
  if (!form.conditionRow.leftVar.trim()) return null;
  return buildSimpleCondition(form.conditionRow);
}
