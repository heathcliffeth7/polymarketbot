import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import type { EntryTimingProfileRow } from '@/lib/trade-flow-config-mappers';
import { Clock3, Plus, Trash2 } from 'lucide-react';

interface EntryTimingProfilesSectionProps {
  rows: EntryTimingProfileRow[];
  onAdd: () => void;
  onUpdate: (rowId: string, patch: Partial<EntryTimingProfileRow>) => void;
  onRemove: (rowId: string) => void;
}

export function EntryTimingProfilesSection({
  rows,
  onAdd,
  onUpdate,
  onRemove,
}: EntryTimingProfilesSectionProps) {
  return (
    <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
      <div className="flex items-center gap-1.5">
        <Clock3 className="h-3.5 w-3.5 text-emerald-500" />
        <p className="text-[11px] font-semibold text-slate-700">Entry Timing Profiles</p>
      </div>
      <p className="text-[10px] leading-relaxed text-slate-400 italic">
        Auto-scope + once modunda kalan saniyeye gore profil secer. Kural: `remainingSec &lt;= start`
        ve `remainingSec &gt; end`. PTB gap, maxPrice ve sizeUsdc bu profilden override edilebilir.
      </p>
      <div className="space-y-2">
        {rows.map((row, index) => (
          <div key={row.id} className="space-y-2 rounded-md border border-slate-200 bg-white p-2.5">
            <div className="flex items-center justify-between">
              <p className="text-[10px] font-semibold text-slate-600">Profil #{index + 1}</p>
              <Button
                type="button"
                size="sm"
                variant="ghost"
                className="h-6 w-6 p-0 text-red-400 hover:text-red-600"
                onClick={() => onRemove(row.id)}
              >
                <Trash2 className="h-3 w-3" />
              </Button>
            </div>
            <div className="grid gap-2 md:grid-cols-3">
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-600">Baslangic Kalan (sn)</Label>
                <Input
                  value={row.startRemainingSec}
                  onChange={(e) => onUpdate(row.id, { startRemainingSec: e.target.value })}
                  placeholder="90"
                  className="h-8 border-slate-300 bg-white text-[11px] text-slate-900"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-600">Bitis Kalan (sn)</Label>
                <Input
                  value={row.endRemainingSec}
                  onChange={(e) => onUpdate(row.id, { endRemainingSec: e.target.value })}
                  placeholder="45"
                  className="h-8 border-slate-300 bg-white text-[11px] text-slate-900"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-600">maxPrice (cent)</Label>
                <Input
                  value={row.maxPriceCent}
                  onChange={(e) => onUpdate(row.id, { maxPriceCent: e.target.value })}
                  placeholder="60"
                  className="h-8 border-slate-300 bg-white text-[11px] text-slate-900"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-600">PTB Min Gap</Label>
                <Input
                  value={row.priceToBeatTriggerMinGap}
                  onChange={(e) =>
                    onUpdate(row.id, { priceToBeatTriggerMinGap: e.target.value })
                  }
                  placeholder="10"
                  className="h-8 border-slate-300 bg-white text-[11px] text-slate-900"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-600">PTB Max Gap</Label>
                <Input
                  value={row.priceToBeatTriggerMaxGap}
                  onChange={(e) =>
                    onUpdate(row.id, { priceToBeatTriggerMaxGap: e.target.value })
                  }
                  placeholder="12"
                  className="h-8 border-slate-300 bg-white text-[11px] text-slate-900"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-600">sizeUsdc</Label>
                <Input
                  value={row.sizeUsdc}
                  onChange={(e) => onUpdate(row.id, { sizeUsdc: e.target.value })}
                  placeholder="1.5"
                  className="h-8 border-slate-300 bg-white text-[11px] text-slate-900"
                />
              </div>
            </div>
          </div>
        ))}
      </div>
      <Button
        type="button"
        size="sm"
        variant="outline"
        className="h-7 border-slate-300 px-2 text-[11px] text-slate-700"
        onClick={onAdd}
      >
        <Plus className="mr-1 h-3 w-3" />
        Profil Ekle
      </Button>
    </div>
  );
}
