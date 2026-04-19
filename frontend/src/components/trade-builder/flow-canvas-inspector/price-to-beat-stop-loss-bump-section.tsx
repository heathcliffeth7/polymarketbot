import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';

interface PriceToBeatStopLossBumpSectionProps {
  enabled: boolean;
  amount: string;
  maxValue: string;
  decayWindows: string;
  scopeMode: 'global' | 'per_scope';
  unit: 'usd' | 'cent';
  defaultUnit: 'usd' | 'cent';
  onUpdateField: (key: string, value: string) => void;
}

export function PriceToBeatStopLossBumpSection({
  enabled,
  amount,
  maxValue,
  decayWindows,
  scopeMode,
  unit,
  defaultUnit,
  onUpdateField,
}: PriceToBeatStopLossBumpSectionProps) {
  return (
    <div className="space-y-2 rounded-md border border-slate-200/80 bg-slate-50/70 p-2">
      <div className="flex items-center justify-between gap-2">
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">
            Auto/Manual PTB Ayari
          </Label>
          <p className="text-[10px] leading-relaxed text-slate-400 italic">
            Manual modda stop-loss gelen marketlerden sonra minimum farki yukari iter. Auto modda
            maxPrice altinda kalan son miss marketleri baz alip PTB gevseme tamponu olarak
            kullanilir.
          </p>
        </div>
        <input
          type="checkbox"
          checked={enabled}
          onChange={(event) => {
            onUpdateField('priceToBeatStopLossBumpEnabled', event.target.checked ? 'true' : 'false');
            if (event.target.checked && !unit) {
              onUpdateField('priceToBeatStopLossBumpUnit', defaultUnit);
            }
          }}
          className="h-4 w-4 rounded border-slate-300"
        />
      </div>
      {enabled && (
        <div className="grid grid-cols-2 gap-2">
          <div className="space-y-1">
            <Label className="text-[10px] font-medium text-slate-500">Tampon / Kademe</Label>
            <Input
              type="number"
              step="any"
              value={amount}
              onChange={(event) =>
                onUpdateField('priceToBeatStopLossBumpAmount', event.target.value)
              }
              placeholder={unit === 'cent' ? '10' : '1'}
              className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-[10px] font-medium text-slate-500">Max Limit</Label>
            <Input
              type="number"
              step="any"
              value={maxValue}
              onChange={(event) =>
                onUpdateField('priceToBeatStopLossBumpMaxValue', event.target.value)
              }
              placeholder={unit === 'cent' ? '30' : '3'}
              className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-[10px] font-medium text-slate-500">Tampon Birimi</Label>
            <Select
              value={unit}
              onValueChange={(value) => onUpdateField('priceToBeatStopLossBumpUnit', value)}
            >
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
            <Label className="text-[10px] font-medium text-slate-500">Decay Penceresi</Label>
            <Input
              type="number"
              step="1"
              min="1"
              value={decayWindows}
              onChange={(event) =>
                onUpdateField('priceToBeatStopLossBumpDecayWindows', event.target.value)
              }
              placeholder="örn. 3"
              className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-[10px] font-medium text-slate-500">Scope</Label>
            <Select
              value={scopeMode}
              onValueChange={(value) =>
                onUpdateField('priceToBeatStopLossBumpScope', value)
              }
            >
              <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="per_scope">Asset + timeframe + yon</SelectItem>
                <SelectItem value="global">Global</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>
      )}
    </div>
  );
}
