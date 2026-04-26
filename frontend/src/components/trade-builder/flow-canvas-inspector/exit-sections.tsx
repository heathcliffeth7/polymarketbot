import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import type {
  ExitLadderRuleRow,
  PtbGapUnit,
  PtbStopLossRuleRow,
} from '@/lib/trade-flow-config-mappers';
import { Plus, Trash2, Zap } from 'lucide-react';

export function ExitLadderSection({
  title,
  description,
  rows,
  addLabel,
  onAdd,
  onUpdate,
  onRemove,
}: {
  title: string;
  description: string;
  rows: ExitLadderRuleRow[];
  addLabel: string;
  onAdd: () => void;
  onUpdate: (rowId: string, patch: Partial<ExitLadderRuleRow>) => void;
  onRemove: (rowId: string) => void;
}) {
  return (
    <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
      <div className="flex items-center gap-1.5">
        <Zap className="h-3.5 w-3.5 text-sky-500" />
        <p className="text-[11px] font-semibold text-slate-700">{title}</p>
      </div>
      <p className="text-[10px] leading-relaxed text-slate-400 italic">{description}</p>
      {rows.length === 0 ? (
        <p className="text-[10px] text-slate-400 italic">Henüz kademe eklenmedi.</p>
      ) : (
        <div className="space-y-2">
          {rows.map((row, index) => (
            <div
              key={row.id}
              className="space-y-1.5 rounded-md border border-slate-200 bg-white p-2.5"
            >
              <div className="flex items-center justify-between">
                <p className="text-[10px] font-medium text-slate-600">Kademe #{index + 1}</p>
                <Button
                  size="sm"
                  variant="ghost"
                  className="h-6 w-6 p-0 text-red-400 hover:text-red-600"
                  onClick={() => onRemove(row.id)}
                >
                  <Trash2 className="h-3 w-3" />
                </Button>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div className="space-y-0.5">
                  <Label className="text-[10px] font-medium text-slate-600">Fiyat (cent)</Label>
                  <Input
                    type="number"
                    value={row.priceCent}
                    onChange={(e) => onUpdate(row.id, { priceCent: e.target.value })}
                    placeholder="ör: 72"
                    className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900"
                  />
                </div>
                <div className="space-y-0.5">
                  <Label className="text-[10px] font-medium text-slate-600">Boyut (%)</Label>
                  <Input
                    type="number"
                    value={row.sizePct}
                    onChange={(e) => onUpdate(row.id, { sizePct: e.target.value })}
                    placeholder="ör: 35"
                    className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900"
                  />
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
      <Button
        size="sm"
        variant="outline"
        className="h-7 border-slate-300 px-2 text-[11px] text-slate-700"
        onClick={onAdd}
      >
        <Plus className="mr-1 h-3 w-3" />
        {addLabel}
      </Button>
    </div>
  );
}

export function PtbStopLossRuleSection({
  unit,
  rows,
  onAdd,
  onUpdate,
  onRemove,
}: {
  unit: PtbGapUnit;
  rows: PtbStopLossRuleRow[];
  onAdd: () => void;
  onUpdate: (rowId: string, patch: Partial<PtbStopLossRuleRow>) => void;
  onRemove: (rowId: string) => void;
}) {
  const isCentUnit = unit === 'cent';
  return (
    <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
      <div className="flex items-center gap-1.5">
        <Zap className="h-3.5 w-3.5 text-sky-500" />
        <p className="text-[11px] font-semibold text-slate-700">Kademeli PTB Stop-Loss</p>
      </div>
      <p className="text-[10px] leading-relaxed text-slate-400 italic">
        Her kademe gapUsd + sizePct ile tanimlanir. Bu alan karsi token fiyati degil,
        directional gap esigidir. Satirlar genisten dara dogru azalmalidir; toplam satis
        yuzu 100 olmalidir. 0 parity demektir. -10, secili birime gore Up/Yes icin
        current &lt;= PTB - 10, Down/No icin current &gt;= PTB + 10 anlamina gelir. Ornek:
        20 &gt; 0 &gt; -20.
      </p>
      <p className="text-[10px] leading-relaxed text-slate-400 italic">
        {isCentUnit
          ? 'Cent modu: 1 = $0.01. Bu bolumdeki tum gap satirlari cent olarak yorumlanir.'
          : 'USD modu: 1 = $1.00. Bu bolumdeki tum gap satirlari USD olarak yorumlanir.'}
      </p>
      {rows.length === 0 ? (
        <p className="text-[10px] text-slate-400 italic">Henüz PTB kademesi eklenmedi.</p>
      ) : (
        <div className="space-y-2">
          {rows.map((row, index) => (
            <div
              key={row.id}
              className="space-y-1.5 rounded-md border border-slate-200 bg-white p-2.5"
            >
              <div className="flex items-center justify-between">
                <p className="text-[10px] font-medium text-slate-600">Kademe #{index + 1}</p>
                <Button
                  size="sm"
                  variant="ghost"
                  className="h-6 w-6 p-0 text-red-400 hover:text-red-600"
                  onClick={() => onRemove(row.id)}
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </Button>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <div className="space-y-1">
                  <Label className="text-[10px] font-medium text-slate-500">
                    Gap Eşiği ({isCentUnit ? 'Cent' : 'USD'})
                  </Label>
                  <Input
                    type="number"
                    step="any"
                    value={row.gapUsd}
                    onChange={(event) => onUpdate(row.id, { gapUsd: event.target.value })}
                    placeholder={isCentUnit ? '20 veya -20' : '12.5 veya -20'}
                    className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                  />
                </div>
                <div className="space-y-1">
                  <Label className="text-[10px] font-medium text-slate-500">
                    Satış Yüzdesi (%)
                  </Label>
                  <Input
                    type="number"
                    step="any"
                    value={row.sizePct}
                    onChange={(event) => onUpdate(row.id, { sizePct: event.target.value })}
                    placeholder="25"
                    className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                  />
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
      <Button type="button" size="sm" variant="outline" className="h-7 border-slate-300 px-2 text-[11px] text-slate-700" onClick={onAdd}>
        <Plus className="mr-1 h-3 w-3" />
        PTB Kademe Ekle
      </Button>
    </div>
  );
}
