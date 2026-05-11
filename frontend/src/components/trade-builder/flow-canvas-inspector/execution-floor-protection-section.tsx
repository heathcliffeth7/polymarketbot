import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

interface ExecutionFloorProtectionSectionProps {
  checked: boolean;
  retryChecked: boolean;
  disabled: boolean;
  hasUpstreamTriggerPrice: boolean;
  hasConfiguredFloorPrice: boolean;
  floorPriceCent: string;
  onUpdateField: (key: string, value: string) => void;
}

export function ExecutionFloorProtectionSection({
  checked,
  retryChecked,
  disabled,
  hasUpstreamTriggerPrice,
  hasConfiguredFloorPrice,
  floorPriceCent,
  onUpdateField,
}: ExecutionFloorProtectionSectionProps) {
  const missingSource = !hasConfiguredFloorPrice && !hasUpstreamTriggerPrice;

  return (
    <div className="mt-2 space-y-1 border-t border-slate-200 pt-2">
      <div className="flex items-center justify-between gap-2">
        <Label className="text-[11px] font-medium text-slate-600">
          Execution Floor Korumasi
        </Label>
        <input
          type="checkbox"
          checked={checked}
          disabled={disabled}
          onChange={(e) =>
            onUpdateField('executionFloorGuardEnabled', e.target.checked ? 'true' : 'false')
          }
          className="h-4 w-4 rounded border-slate-300 disabled:cursor-not-allowed disabled:opacity-50"
        />
      </div>
      <p className="text-[10px] leading-relaxed text-slate-400 italic">
        Best ask effective floor seviyesinin altindaysa buy emrini engelle. `Execution Floor Fiyat
        (cent)` bossa upstream tetik fiyati fallback olarak kullanilir.
      </p>
      <div className="space-y-0.5 pt-1">
        <Label className="text-[10px] font-medium text-slate-600">
          Execution Floor Fiyat (cent)
        </Label>
        <Input
          type="number"
          value={floorPriceCent}
          onChange={(e) => onUpdateField('executionFloorPriceCent', e.target.value)}
          placeholder="or: 82"
          className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900"
        />
      </div>
      {hasConfiguredFloorPrice && !hasUpstreamTriggerPrice && (
        <p className="text-[10px] leading-relaxed text-sky-600">
          Manual floor kullaniliyor; upstream tetik fiyati gerekmiyor.
        </p>
      )}
      {missingSource && !checked && (
        <p className="text-[10px] leading-relaxed text-amber-600">
          Bu koruma yalnizca manual `Execution Floor Fiyat (cent)` veya upstream tetik fiyati
          varsa acilabilir.
        </p>
      )}
      {missingSource && checked && (
        <p className="text-[10px] leading-relaxed text-amber-600">
          Mevcut ayar artik effective floor bulamiyor. Manual floor gir ya da korumayi kapat.
        </p>
      )}
      {checked && (
        <div className="mt-2 flex items-center justify-between gap-2 border-t border-slate-200 pt-2">
          <Label className="text-[11px] font-medium text-slate-600">
            Iyilesince Tekrar Dene
          </Label>
          <input
            type="checkbox"
            checked={retryChecked}
            onChange={(e) =>
              onUpdateField('retryOnExecutionFloorGuardBlock', e.target.checked ? 'true' : 'false')
            }
            className="h-4 w-4 rounded border-slate-300"
          />
        </div>
      )}
      {checked && (
        <p className="text-[10px] leading-relaxed text-slate-400 italic">
          Best ask floor&apos;un altindaysa bu toggle bekleme/iptal kararini belirler. Best ask
          yoksa mevcut runtime semantigi korunur.
        </p>
      )}
    </div>
  );
}
