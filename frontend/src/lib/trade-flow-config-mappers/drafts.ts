import type {
  ConditionDraft,
  DrawdownRuleRow,
  EntryTimingProfileRow,
  ExitLadderRuleRow,
  KeyValueDraft,
  OutcomeConditionRow,
  PtbIvTimeRuleRow,
  PtbStopLossBumpLossRuleRow,
  PtbStopLossRuleRow,
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

export function createEmptyPtbStopLossRuleRow(): PtbStopLossRuleRow {
  return { id: createId('pr'), gapUsd: '', sizePct: '' };
}

export function createEmptyPtbStopLossBumpLossRuleRow(): PtbStopLossBumpLossRuleRow {
  return { id: createId('pbl'), lossUsd: '', bumpValue: '' };
}

export function createEmptyPtbIvTimeRuleRow(): PtbIvTimeRuleRow {
  return {
    id: createId('piv'),
    startRemainingSec: '',
    endRemainingSec: '',
    maxPriceCent: '',
    minEdge: '',
    minGapStrength: '',
    minExpectedMoveUsd: '',
    minGapStrengthMargin: '',
    minGapUsdMargin: '',
  };
}

export function createEmptyEntryTimingProfileRow(): EntryTimingProfileRow {
  return {
    id: createId('etp'),
    startRemainingSec: '',
    endRemainingSec: '',
    maxPriceCent: '',
    priceToBeatTriggerMinGap: '',
    priceToBeatTriggerMaxGap: '',
    sizeUsdc: '',
  };
}

export function createEmptyTimeExitRuleRow(): TimeExitRuleRow {
  return { id: createId('tr'), elapsedMinutes: '', remainingPct: '' };
}
