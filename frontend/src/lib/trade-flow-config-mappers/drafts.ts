import type {
  ConditionDraft,
  DrawdownRuleRow,
  ExitLadderRuleRow,
  KeyValueDraft,
  OutcomeConditionRow,
  TimeExitRuleRow,
} from './types';
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

export function createEmptyExitLadderRuleRow(): ExitLadderRuleRow {
  return { id: createId('er'), priceCent: '', sizePct: '' };
}

export function createEmptyTimeExitRuleRow(): TimeExitRuleRow {
  return { id: createId('tr'), elapsedMinutes: '', remainingPct: '' };
}
