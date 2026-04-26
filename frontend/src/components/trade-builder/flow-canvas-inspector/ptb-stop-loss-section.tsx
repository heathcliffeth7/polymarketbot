import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  createEmptyPtbStopLossRuleRow,
  type PtbGapUnit,
  type PtbStopLossRuleRow,
} from '@/lib/trade-flow-config-mappers';
import { PtbStopLossRuleSection } from './exit-sections';

interface PtbStopLossSectionProps {
  enabled: boolean;
  unit: PtbGapUnit;
  timeDecayMode: 'tighten' | 'relax' | 'none';
  rows: PtbStopLossRuleRow[];
  onUpdateField: (key: string, value: string) => void;
  onUpdateRows: (updater: (rows: PtbStopLossRuleRow[]) => PtbStopLossRuleRow[]) => void;
}

export function PtbStopLossSection({
  enabled,
  unit,
  timeDecayMode,
  rows,
  onUpdateField,
  onUpdateRows,
}: PtbStopLossSectionProps) {
  return (
    <div className="space-y-2 rounded-md border border-slate-200/80 bg-slate-50/80 p-3">
      <div className="flex items-center justify-between gap-2">
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">
            PTB Gap Stop-Loss
          </Label>
          <p className="text-[10px] leading-relaxed text-slate-400 italic">
            Master PTB toggle. Hard gap ve kademeli PTB satirlari bu ana switch ile
            acilip kapanir.
          </p>
        </div>
        <input
          type="checkbox"
          checked={enabled}
          onChange={(event) =>
            onUpdateField('ptbStopLossEnabled', event.target.checked ? 'true' : 'false')
          }
          className="h-4 w-4 rounded border-slate-300"
        />
      </div>
      {enabled && (
        <>
          <p className="text-[10px] leading-relaxed text-slate-400 italic">
            Bu alan karsi token fiyati degil, underlying directional gap olarak calisir. Up/Yes
            icin current Chainlink - PTB, Down/No icin PTB - current Chainlink izlenir.
            -10, Up/Yes icin PTB referansinin 10 altini; Down/No icin PTB referansinin
            10 ustunu bekler. 0 / 100 tek satir eski hard PTB kapanisini staged sekilde
            temsil eder. Negatif esik, karsi yone overshoot bekler ve time decay
            uygulanmaz.
          </p>
          <div className="grid grid-cols-2 gap-2">
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">PTB Gap Birimi</Label>
              <Select value={unit} onValueChange={(value) => onUpdateField('ptbStopLossGapUnit', value)}>
                <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="usd">USD</SelectItem>
                  <SelectItem value="cent">Cent</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">PTB SL Zaman Modu</Label>
              <Select
                value={timeDecayMode}
                onValueChange={(value) => onUpdateField('ptbStopLossTimeDecayMode', value)}
              >
                <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="tighten">tighten</SelectItem>
                  <SelectItem value="relax">relax</SelectItem>
                  <SelectItem value="none">none</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
          <p className="text-[10px] leading-relaxed text-slate-400 italic">
            {unit === 'cent'
              ? 'Cent modu: 1 = $0.01. Bu unit staged satirlarin ve varsa legacy hard gap degerinin tamamina uygulanir.'
              : 'USD modu: 1 = $1.00. Bu unit staged satirlarin ve varsa legacy hard gap degerinin tamamina uygulanir.'}
          </p>
          <PtbStopLossRuleSection
            unit={unit}
            rows={rows}
            onAdd={() =>
              onUpdateRows((currentRows) => [...currentRows, createEmptyPtbStopLossRuleRow()])
            }
            onUpdate={(rowId, patch) =>
              onUpdateRows((currentRows) =>
                currentRows.map((row) => (row.id === rowId ? { ...row, ...patch } : row))
              )
            }
            onRemove={(rowId) =>
              onUpdateRows((currentRows) => currentRows.filter((row) => row.id !== rowId))
            }
          />
        </>
      )}
    </div>
  );
}
