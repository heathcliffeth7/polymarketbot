import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { REVENGE_FLIP_STOP_LOSS_RULES_FIELD } from '@/lib/trade-flow-config-mappers/revenge-flip';
import { Plus, Trash2 } from 'lucide-react';

interface RevengeFlipStopLossRulesProps {
  value: string;
  onUpdateField: (key: string, value: string) => void;
}

interface StopLossRuleRow {
  minFlip: string;
  maxFlip: string;
  stopLossPct: string;
}

function toText(value: unknown): string {
  if (value == null) return '';
  return String(value);
}

function parseRows(value: string): StopLossRuleRow[] {
  try {
    const parsed = JSON.parse(value || '[]');
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((item): item is Record<string, unknown> => Boolean(item) && typeof item === 'object')
      .map((item) => ({
        minFlip: toText(item.minFlip ?? 0),
        maxFlip: toText(item.maxFlip ?? ''),
        stopLossPct: toText(item.stopLossPct ?? ''),
      }));
  } catch {
    return [];
  }
}

function finiteOrUndefined(value: string): number | undefined {
  if (!value.trim()) return undefined;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function serializeRows(rows: StopLossRuleRow[]): string {
  return JSON.stringify(
    rows.map((row) => {
      const next: Record<string, number> = {};
      next.minFlip = finiteOrUndefined(row.minFlip) ?? 0;
      const maxFlip = finiteOrUndefined(row.maxFlip);
      if (maxFlip != null) next.maxFlip = maxFlip;
      const stopLossPct = finiteOrUndefined(row.stopLossPct);
      if (stopLossPct != null) next.stopLossPct = stopLossPct;
      return next;
    }),
  );
}

export function RevengeFlipStopLossRules({
  value,
  onUpdateField,
}: RevengeFlipStopLossRulesProps) {
  const rows = parseRows(value);
  const updateRows = (nextRows: StopLossRuleRow[]) =>
    onUpdateField(REVENGE_FLIP_STOP_LOSS_RULES_FIELD, serializeRows(nextRows));
  const updateRow = (index: number, patch: Partial<StopLossRuleRow>) =>
    updateRows(rows.map((row, rowIndex) => (rowIndex === index ? { ...row, ...patch } : row)));

  return (
    <div className="space-y-2 rounded-md border border-rose-100 bg-white/80 p-2">
      <div className="flex items-center justify-between gap-2">
        <Label className="text-[11px] font-medium text-slate-600">Stop Loss Rules</Label>
        <Button
          type="button"
          size="sm"
          variant="outline"
          className="h-7 border-rose-200 px-2 text-[11px] text-slate-700"
          onClick={() =>
            updateRows([
              ...rows,
              { minFlip: String(rows.length), maxFlip: '', stopLossPct: '' },
            ])
          }
        >
          <Plus className="mr-1 h-3 w-3" />
          Kural Ekle
        </Button>
      </div>
      {rows.length === 0 ? (
        <p className="text-[10px] text-slate-400 italic">
          Bos birakilirsa sabit Stop Loss yuzdesi kullanilir.
        </p>
      ) : (
        <div className="space-y-2">
          {rows.map((row, index) => (
            <div
              key={`stop-loss-rule-${index}`}
              className="grid grid-cols-[1fr_1fr_1fr_32px] gap-2"
            >
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-500">Min Flip</Label>
                <Input
                  type="number"
                  min={0}
                  step={1}
                  value={row.minFlip}
                  onChange={(event) => updateRow(index, { minFlip: event.target.value })}
                  className="h-8 border-slate-200 bg-white text-xs"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-500">Max Flip</Label>
                <Input
                  type="number"
                  min={0}
                  step={1}
                  value={row.maxFlip}
                  onChange={(event) => updateRow(index, { maxFlip: event.target.value })}
                  placeholder="∞"
                  className="h-8 border-slate-200 bg-white text-xs"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-500">Stop Loss</Label>
                <Input
                  type="number"
                  min={0.01}
                  max={0.99}
                  step={0.01}
                  value={row.stopLossPct}
                  onChange={(event) => updateRow(index, { stopLossPct: event.target.value })}
                  className="h-8 border-slate-200 bg-white text-xs"
                />
              </div>
              <div className="flex items-end">
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  className="h-8 w-8 p-0 text-red-400 hover:text-red-600"
                  onClick={() => updateRows(rows.filter((_, rowIndex) => rowIndex !== index))}
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
