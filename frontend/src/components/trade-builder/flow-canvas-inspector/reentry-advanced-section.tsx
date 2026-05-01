import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';

interface ReentryAdvancedSectionProps {
  visible: boolean;
  fields: Record<string, string>;
  priceToBeatGuardChecked: boolean;
  priceToBeatGuardMode: string;
  priceToBeatGuardUnit: 'usd' | 'cent';
  onUpdateField: (key: string, value: string) => void;
}

export function ReentryAdvancedSection({
  visible,
  fields,
  priceToBeatGuardChecked,
  priceToBeatGuardMode,
  priceToBeatGuardUnit,
  onUpdateField,
}: ReentryAdvancedSectionProps) {
  if (!visible) return null;

  const reentryPriceToBeatUnitRaw = (fields.reentryPriceToBeatMaxDiffUnit ?? '')
    .toString()
    .trim()
    .toLowerCase();
  const reentryPriceToBeatUnit =
    reentryPriceToBeatUnitRaw === 'usd' || reentryPriceToBeatUnitRaw === 'cent'
      ? reentryPriceToBeatUnitRaw
      : priceToBeatGuardMode === 'manual'
        ? priceToBeatGuardUnit
        : '';

  return (
    <div className="space-y-2 rounded-md border border-slate-200/80 bg-slate-50/70 p-2">
      <Label className="text-[11px] font-semibold text-slate-700">
        SL Sonrasi Re-entry Detaylari
      </Label>
      {priceToBeatGuardChecked && (
        <>
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">
              Re-entry PTB Min Fark
            </Label>
            <Input
              type="number"
              step="any"
              value={fields.reentryPriceToBeatMaxDiff ?? ''}
              onChange={(event) => onUpdateField('reentryPriceToBeatMaxDiff', event.target.value)}
              placeholder={priceToBeatGuardMode === 'manual' ? '2' : 'bos birak'}
              className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">
              Re-entry PTB Birimi
            </Label>
            <Select
              value={reentryPriceToBeatUnit || undefined}
              onValueChange={(value) => onUpdateField('reentryPriceToBeatMaxDiffUnit', value)}
            >
              <SelectTrigger
                className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900"
                size="sm"
              >
                <SelectValue placeholder="Birim sec" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="usd">USD</SelectItem>
                <SelectItem value="cent">Cent</SelectItem>
              </SelectContent>
            </Select>
          </div>
          {priceToBeatGuardMode === 'manual' ? (
            <p className="text-[10px] leading-relaxed text-slate-400 italic">
              Bu override yalniz re-entry denemelerinde uygulanir. Birim secilmezse ana PTB
              birimi kullanilir: `{priceToBeatGuardUnit}`.
            </p>
          ) : (
            <p className="text-[10px] leading-relaxed text-slate-400 italic">
              Ana PTB auto modda kalsa bile re-entry denemesinde bu deger manual override olarak
              kullanilir. Bu modda birim secimi zorunludur.
            </p>
          )}
        </>
      )}
      <div className="grid grid-cols-2 gap-2">
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">Cooldown (sn)</Label>
          <Input
            type="number"
            step="1"
            min="0"
            value={fields.reentryCooldownSec ?? ''}
            onChange={(event) => onUpdateField('reentryCooldownSec', event.target.value)}
            placeholder="0"
            className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
          />
        </div>
        {priceToBeatGuardChecked && (
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">PTB Decay</Label>
            <Input
              type="number"
              step="any"
              value={fields.reentryThresholdDecay ?? ''}
              onChange={(event) => onUpdateField('reentryThresholdDecay', event.target.value)}
              placeholder="0.8"
              className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
            />
          </div>
        )}
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">
            MaxPrice Tighten (bps)
          </Label>
          <Input
            type="number"
            step="1"
            min="0"
            value={fields.reentryMaxPriceTightenBps ?? ''}
            onChange={(event) => onUpdateField('reentryMaxPriceTightenBps', event.target.value)}
            placeholder="500"
            className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
          />
        </div>
        <div className="flex items-center justify-between gap-2 rounded-md border border-slate-200 bg-white px-2 py-2">
          <Label className="text-[11px] font-medium text-slate-600">Ayni pencereyi atla</Label>
          <input
            type="checkbox"
            checked={(fields.reentrySkipCurrentWindow ?? '').toString().trim().toLowerCase() === 'true'}
            onChange={(event) =>
              onUpdateField('reentrySkipCurrentWindow', event.target.checked ? 'true' : 'false')
            }
            className="h-4 w-4 rounded border-slate-300"
          />
        </div>
      </div>
    </div>
  );
}
