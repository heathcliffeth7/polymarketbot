import type {
  EntryTimingProfileRow,
  NodeConfigFormState,
  PtbIvTimeRuleRow,
  PtbStopLossRuleRow,
  TimeExitRuleRow,
} from '@/lib/trade-flow-config-mappers';

export function updatePtbStopLossRuleRowsFormState(
  prev: NodeConfigFormState | null,
  updater: (rows: PtbStopLossRuleRow[]) => PtbStopLossRuleRow[]
): NodeConfigFormState | null {
  return prev
    ? {
        ...prev,
        ptbStopLossRuleRows: updater([...(prev.ptbStopLossRuleRows || [])]),
      }
    : prev;
}

export function updatePtbIvTimeRuleRowsFormState(
  prev: NodeConfigFormState | null,
  updater: (rows: PtbIvTimeRuleRow[]) => PtbIvTimeRuleRow[]
): NodeConfigFormState | null {
  return prev
    ? {
        ...prev,
        ptbIvTimeRuleRows: updater([...(prev.ptbIvTimeRuleRows || [])]),
      }
    : prev;
}

export function updateEntryTimingProfileRowsFormState(
  prev: NodeConfigFormState | null,
  updater: (rows: EntryTimingProfileRow[]) => EntryTimingProfileRow[]
): NodeConfigFormState | null {
  return prev
    ? {
        ...prev,
        entryTimingProfileRows: updater([...(prev.entryTimingProfileRows || [])]),
      }
    : prev;
}

export function updateTimeExitRuleRowsFormState(
  prev: NodeConfigFormState | null,
  updater: (rows: TimeExitRuleRow[]) => TimeExitRuleRow[]
): NodeConfigFormState | null {
  return prev
    ? {
        ...prev,
        timeExitRuleRows: updater([...(prev.timeExitRuleRows || [])]),
      }
    : prev;
}
