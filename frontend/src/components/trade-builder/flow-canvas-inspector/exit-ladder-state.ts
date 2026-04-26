import {
  createEmptyExitLadderRuleRow,
  type ExitLadderRuleRow,
} from '@/lib/trade-flow-config-mappers';

export function appendPrimaryTakeProfitRuleRow(
  rows: ExitLadderRuleRow[]
): ExitLadderRuleRow[] {
  const nextRow = createEmptyExitLadderRuleRow();
  if (rows.length === 0) {
    return [...rows, { ...nextRow, sizePct: '100' }];
  }

  return [...rows, nextRow];
}
