import type {
  NodeConfigFormState,
  PtbStopLossBumpLossRuleRow,
  PtbStopLossBumpMode,
} from '@/lib/trade-flow-config-mappers';

export function resolvePriceToBeatStopLossBumpUiState(
  fields: Record<string, string>,
  fallbackUnit: 'usd' | 'cent'
): {
  checked: boolean;
  mode: PtbStopLossBumpMode;
  unit: 'usd' | 'cent';
  scope: 'global' | 'per_scope';
} {
  const checked =
    (fields.priceToBeatStopLossBumpEnabled ?? '').toString().trim().toLowerCase() === 'true';
  const mode =
    (fields.priceToBeatStopLossBumpMode ?? '').toString().trim().toLowerCase() === 'loss_table'
      ? 'loss_table'
      : 'fixed';
  const unitRaw = (fields.priceToBeatStopLossBumpUnit ?? '').toString().trim().toLowerCase();
  const unit = unitRaw === 'usd' || unitRaw === 'cent' ? unitRaw : fallbackUnit;
  const scope =
    (fields.priceToBeatStopLossBumpScope ?? '').toString().trim().toLowerCase() === 'global'
      ? 'global'
      : 'per_scope';

  return { checked, mode, unit, scope };
}

export function updatePtbStopLossBumpLossRuleRowsFormState(
  prev: NodeConfigFormState | null,
  updater: (rows: PtbStopLossBumpLossRuleRow[]) => PtbStopLossBumpLossRuleRow[]
): NodeConfigFormState | null {
  return prev
    ? {
        ...prev,
        ptbStopLossBumpLossRuleRows: updater([...(prev.ptbStopLossBumpLossRuleRows || [])]),
      }
    : prev;
}
