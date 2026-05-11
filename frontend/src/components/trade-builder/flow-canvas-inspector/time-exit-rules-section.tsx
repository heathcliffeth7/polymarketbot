import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import type { TimeExitRuleRow } from '@/lib/trade-flow-config-mappers';
import { Plus, Settings2, Trash2 } from 'lucide-react';

interface TimeExitRulesSectionProps {
  rows: TimeExitRuleRow[];
  onAdd: () => void;
  onUpdate: (rowId: string, patch: Partial<TimeExitRuleRow>) => void;
  onRemove: (rowId: string) => void;
}

export function TimeExitRulesSection({
  rows,
  onAdd,
  onUpdate,
  onRemove,
}: TimeExitRulesSectionProps) {
  return (
    <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
      <div className="flex items-center gap-1.5">
        <Settings2 className="h-3.5 w-3.5 text-amber-500" />
        <p className="text-[11px] font-semibold text-slate-700">Zaman Bazli Cikis</p>
      </div>
      <p className="text-[10px] leading-relaxed text-slate-400 italic">
        Sayac buy fill aninda baslar. Her kural, tetiklendigi andaki kalan pozisyonun belirli bir yuzdesini satar.
      </p>
      {rows.length === 0 ? (
        <p className="text-[10px] text-slate-400 italic">Henüz süre kademesi eklenmedi.</p>
      ) : (
        <div className="space-y-2">
          {rows.map((row, index) => (
            <div key={row.id} className="space-y-1.5 rounded-md border border-slate-200 bg-white p-2.5">
              <div className="flex items-center justify-between">
                <p className="text-[10px] font-medium text-slate-600">Sure #{index + 1}</p>
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
                  <Label className="text-[10px] font-medium text-slate-600">Dakika</Label>
                  <Input
                    type="number"
                    value={row.elapsedMinutes}
                    onChange={(e) => onUpdate(row.id, { elapsedMinutes: e.target.value })}
                    placeholder="ör: 12"
                    className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900"
                  />
                </div>
                <div className="space-y-0.5">
                  <Label className="text-[10px] font-medium text-slate-600">Kalan (%)</Label>
                  <Input
                    type="number"
                    value={row.remainingPct}
                    onChange={(e) => onUpdate(row.id, { remainingPct: e.target.value })}
                    placeholder="ör: 30"
                    className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900"
                  />
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
      <Button size="sm" variant="outline" className="h-7 border-slate-300 px-2 text-[11px] text-slate-700" onClick={onAdd}>
        <Plus className="mr-1 h-3 w-3" />
        Sure Kademesi Ekle
      </Button>
    </div>
  );
}
