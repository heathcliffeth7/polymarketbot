import { Label } from '@/components/ui/label';

interface MaxPriceProtectionSectionProps {
  hasConfiguredMaxPrice: boolean;
  notifyChecked: boolean;
  retryChecked: boolean;
  onUpdateField: (key: string, value: string) => void;
}

export function MaxPriceProtectionSection({
  hasConfiguredMaxPrice,
  notifyChecked,
  retryChecked,
  onUpdateField,
}: MaxPriceProtectionSectionProps) {
  const disabled = !hasConfiguredMaxPrice;

  return (
    <div className="mt-2 space-y-1 rounded-md border border-slate-200/80 bg-slate-50/80 p-2">
      <Label className="text-[11px] font-medium text-slate-600">Max Fiyat Korumasi</Label>
      {!hasConfiguredMaxPrice && (
        <p className="text-[10px] leading-relaxed text-slate-400 italic">
          Tavan fiyat tanimlanmadan bu koruma aktif edilemez.
        </p>
      )}
      <div className="flex items-center justify-between gap-2">
        <Label className="text-[11px] font-medium text-slate-600">Max Fiyat Engel Bildirimi</Label>
        <input
          type="checkbox"
          checked={hasConfiguredMaxPrice && notifyChecked}
          disabled={disabled}
          onChange={(e) => onUpdateField('notifyOnMaxPriceBlocked', e.target.checked ? 'true' : 'false')}
          className="h-4 w-4 rounded border-slate-300 disabled:cursor-not-allowed disabled:opacity-50"
        />
      </div>
      <p className="text-[10px] leading-relaxed text-slate-400 italic">
        Max price guard emri engelleyince bildirim gonder.
      </p>
      <div className="mt-2 flex items-center justify-between gap-2 border-t border-slate-200 pt-2">
        <Label className="text-[11px] font-medium text-slate-600">Iyilesince Tekrar Dene</Label>
        <input
          type="checkbox"
          checked={hasConfiguredMaxPrice && retryChecked}
          disabled={disabled}
          onChange={(e) => onUpdateField('retryOnMaxPriceBlock', e.target.checked ? 'true' : 'false')}
          className="h-4 w-4 rounded border-slate-300 disabled:cursor-not-allowed disabled:opacity-50"
        />
      </div>
      <p className="text-[10px] leading-relaxed text-slate-400 italic">
        Guard bloklarsa order iptal olmaz; bekleme moduna alinip fiyat dusunce yeniden denenir.
      </p>
    </div>
  );
}
