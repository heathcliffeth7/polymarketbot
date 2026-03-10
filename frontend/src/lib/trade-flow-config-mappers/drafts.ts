import type { ConditionDraft, DrawdownRuleRow, KeyValueDraft, OutcomeConditionRow } from './types';
import { createId } from './utils';

export function createEmptyConditionDraft(): ConditionDraft {
  return {
    id: createId('cond'),
    leftVar: 'market_price',
    operator: '<=',
    rightType: 'number',
    rightValue: '50',
  };
}

export function createEmptyKeyValueDraft(): KeyValueDraft {
  return {
    id: createId('kv'),
    key: '',
    value: '',
    valueType: 'string',
  };
}

export function createEmptyOutcomeConditionRow(): OutcomeConditionRow {
  return {
    id: createId('oc'),
    tokenId: '',
    outcomeLabel: '',
    triggerCondition: '',
    triggerPriceCent: '',
    maxPriceCent: '',
  };
}

export function createEmptyDrawdownRuleRow(): DrawdownRuleRow {
  return { id: createId('dr'), direction: 'down', lossPct: '', durationValue: '' };
}
