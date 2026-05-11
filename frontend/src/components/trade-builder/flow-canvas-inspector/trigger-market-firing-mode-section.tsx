import { Label } from '@/components/ui/label';
import type { OutcomeConditionRow } from '@/lib/trade-flow-config-mappers';

type TriggerFiringMode = 'once_market' | 'once_run' | 'loop';

interface TriggerMarketFiringModeSectionProps {
  fields: Record<string, string>;
  outcomeConditionRows: OutcomeConditionRow[];
  onUpdateField: (key: string, value: string) => void;
}

function hasLevelTrigger(rows: OutcomeConditionRow[]): boolean {
  return rows.some((row) => {
    const condition = row.triggerCondition.trim().toLowerCase();
    return condition === 'level_above' || condition === 'level_below';
  });
}

function resolveFiringMode(fields: Record<string, string>): TriggerFiringMode {
  const repeatMode = (fields.repeatMode ?? '').trim().toLowerCase();
  const onceScope = (fields.onceScope ?? '').trim().toLowerCase();
  if (repeatMode === 'once') {
    return onceScope === 'run' ? 'once_run' : 'once_market';
  }
  return 'loop';
}

function modeButtonClass(active: boolean): string {
  const base =
    'rounded-md border px-2.5 py-1.5 text-[10px] font-medium transition disabled:cursor-not-allowed disabled:opacity-50';
  if (active) return `${base} border-sky-300 bg-sky-50 text-sky-700`;
  return `${base} border-slate-300 bg-white text-slate-600 hover:border-sky-300 hover:bg-sky-50`;
}

export function TriggerMarketFiringModeSection({
  fields,
  outcomeConditionRows,
  onUpdateField,
}: TriggerMarketFiringModeSectionProps) {
  const levelTriggerActive = hasLevelTrigger(outcomeConditionRows);
  const mode = resolveFiringMode(fields);
  const setMode = (nextMode: TriggerFiringMode) => {
    if (nextMode === 'loop') {
      if (levelTriggerActive) return;
      onUpdateField('repeatMode', 'loop');
      onUpdateField('onceScope', '');
      return;
    }
    onUpdateField('repeatMode', 'once');
    onUpdateField('onceScope', nextMode === 'once_run' ? 'run' : 'market');
  };

  return (
    <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
      <div className="space-y-1">
        <Label className="text-[11px] font-semibold text-slate-700">Tetik Calisma Modu</Label>
        <p className="text-[10px] leading-relaxed text-slate-400 italic">
          Re-entry ve `level_above/level_below` icin dogru ayar genelde her markette bir kezdir.
        </p>
      </div>
      <div className="grid grid-cols-1 gap-1.5 sm:grid-cols-3">
        <button
          type="button"
          className={modeButtonClass(mode === 'once_market')}
          onClick={() => setMode('once_market')}
        >
          Her markette bir kez
        </button>
        <button
          type="button"
          className={modeButtonClass(mode === 'once_run')}
          onClick={() => setMode('once_run')}
        >
          Run boyunca bir kez
        </button>
        <button
          type="button"
          className={modeButtonClass(mode === 'loop')}
          disabled={levelTriggerActive}
          onClick={() => setMode('loop')}
        >
          Dongu
        </button>
      </div>
      {levelTriggerActive && (
        <p className="text-[10px] leading-relaxed text-sky-600">
          `level_above/level_below` kosulu dongu modunda publish edilemez. Bu yuzden `Dongu`
          kapali tutulur.
        </p>
      )}
      {mode === 'once_market' && (
        <p className="text-[10px] leading-relaxed text-slate-400 italic">
          Config olarak `repeatMode=once`, `onceScope=market` yazilir; her yeni auto-scope
          markette tekrar tetiklenebilir.
        </p>
      )}
    </div>
  );
}
