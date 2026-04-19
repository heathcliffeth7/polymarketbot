import { Button } from '@/components/ui/button';
import type { NodeConfigFormState } from '@/lib/trade-flow-config-mappers';
import { PAIR_LOCK_UNSUPPORTED_EXIT_FIELD_KEYS } from '@/lib/trade-flow-config-mappers/pair-lock';

const PAIR_LOCK_STALE_FIELD_KEYS = [
  ...PAIR_LOCK_UNSUPPORTED_EXIT_FIELD_KEYS,
  'notifyOnTriggerPriceBlocked',
  'notifyOnExecutionFloorBlocked',
] as const;

function shouldHidePairLockStaleEntry(key: string, value: string): boolean {
  if (key !== 'reentryCooldownSec' && key !== 'reentryMaxPriceTightenBps') {
    return false;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed === 0;
}

function collectPairLockStaleEntries(form: NodeConfigFormState): string[] {
  const entries = PAIR_LOCK_STALE_FIELD_KEYS
    .map((key) => {
      const value = (form.fields[key] ?? '').trim();
      if (shouldHidePairLockStaleEntry(key, value)) return null;
      return value ? `${key}=${value}` : null;
    })
    .filter((value): value is string => !!value);

  if (form.tpRuleRows.length > 0) entries.push(`tpRules=${form.tpRuleRows.length}`);
  if (form.slRuleRows.length > 0) entries.push(`slRules=${form.slRuleRows.length}`);
  if (form.ptbStopLossRuleRows.length > 0) {
    entries.push(`ptbStopLossRules=${form.ptbStopLossRuleRows.length}`);
  }
  if (form.timeExitRuleRows.length > 0) {
    entries.push(`timeExitRules=${form.timeExitRuleRows.length}`);
  }

  return entries;
}

interface PairLockStaleConfigSectionProps {
  visible: boolean;
  form: NodeConfigFormState;
  onFormChange: React.Dispatch<React.SetStateAction<NodeConfigFormState | null>>;
}

export function PairLockStaleConfigSection({
  visible,
  form,
  onFormChange,
}: PairLockStaleConfigSectionProps) {
  if (!visible) return null;
  const staleEntries = collectPairLockStaleEntries(form);
  if (staleEntries.length === 0) return null;

  return (
    <div className="space-y-2 rounded-md border border-amber-200 bg-amber-50 px-2 py-2 text-[10px] leading-relaxed text-amber-800">
      <p>
        Pair lock modunda desteklenmeyen ama draft config&apos;te kalan alanlar bulundu. Bu alanlar
        basic UI&apos;da normalde gizlenir ama validation&apos;ı bozabilir.
      </p>
      <div className="flex flex-wrap gap-1">
        {staleEntries.map((entry) => (
          <span
            key={entry}
            className="rounded border border-amber-300 bg-white px-1.5 py-0.5 text-[10px] text-amber-900"
          >
            {entry}
          </span>
        ))}
      </div>
      <Button
        size="sm"
        variant="outline"
        className="h-7 border-amber-300 px-2 text-[11px] text-amber-900"
        onClick={() =>
          onFormChange((prev) => {
            if (!prev) return prev;
            const nextFields = { ...prev.fields };
            for (const key of PAIR_LOCK_STALE_FIELD_KEYS) {
              nextFields[key] = '';
            }
            return {
              ...prev,
              fields: nextFields,
              tpRuleRows: [],
              slRuleRows: [],
              ptbStopLossRuleRows: [],
              timeExitRuleRows: [],
            };
          })
        }
      >
        Uyumsuz Alanlari Temizle
      </Button>
    </div>
  );
}
