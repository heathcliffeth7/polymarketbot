import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';

interface PriceToBeatMaxPriceRelaxSectionProps {
  missCount: string;
  historyCount: string;
  minValue: string;
  minDepthUsd: string;
  minUnit: 'usd' | 'cent';
  stepMode: 'percent' | 'absolute';
  stepValue: string;
  stepUnit: 'usd' | 'cent';
  onUpdateField: (key: string, value: string) => void;
}

export function PriceToBeatMaxPriceRelaxSection({
  missCount,
  historyCount,
  minValue,
  minDepthUsd,
  minUnit,
  stepMode,
  stepValue,
  stepUnit,
  onUpdateField,
}: PriceToBeatMaxPriceRelaxSectionProps) {
  return (
    <div className="space-y-2 rounded-md border border-slate-200/80 bg-slate-50/70 p-2">
      <div className="grid grid-cols-2 gap-2">
        <div className="space-y-1">
          <Label className="text-[10px] font-medium text-slate-500">
            Relax Miss Sayisi
          </Label>
          <Input
            type="number"
            step="1"
            min="1"
            value={missCount}
            onChange={(event) =>
              onUpdateField('priceToBeatMaxPriceRelaxMissCount', event.target.value)
            }
            placeholder="5"
            className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
          />
        </div>
        <div className="space-y-1">
          <Label className="text-[10px] font-medium text-slate-500">
            Relax History Sayisi
          </Label>
          <Input
            type="number"
            step="1"
            min="1"
            value={historyCount}
            onChange={(event) =>
              onUpdateField('priceToBeatMaxPriceRelaxHistoryCount', event.target.value)
            }
            placeholder="5"
            className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
          />
        </div>
      </div>
      <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
        <div className="space-y-1">
          <Label className="text-[10px] font-medium text-slate-500">
            Relax Min Deger
          </Label>
          <Input
            type="number"
            step="any"
            value={minValue}
            onChange={(event) =>
              onUpdateField('priceToBeatMaxPriceRelaxMinValue', event.target.value)
            }
            placeholder="bos = tampon fallback"
            className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
          />
        </div>
        <div className="space-y-1">
          <Label className="text-[10px] font-medium text-slate-500">
            Relax Min Birimi
          </Label>
          <Select
            value={minUnit}
            onValueChange={(value) =>
              onUpdateField('priceToBeatMaxPriceRelaxMinUnit', value)
            }
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
          <Label className="text-[10px] font-medium text-slate-500">
            Min Tradable Depth (USDC)
          </Label>
          <Input
            type="number"
            step="any"
            value={minDepthUsd}
            onChange={(event) =>
              onUpdateField('priceToBeatMaxPriceRelaxMinDepthUsd', event.target.value)
            }
            placeholder="5"
            className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
          />
        </div>
        <div className="space-y-1">
          <Label className="text-[10px] font-medium text-slate-500">
            Relax Step Modu
          </Label>
          <Select
            value={stepMode}
            onValueChange={(value) => onUpdateField('priceToBeatMaxPriceRelaxStepMode', value)}
          >
            <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="percent">Percent</SelectItem>
              <SelectItem value="absolute">Absolute</SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div className="space-y-1">
          <Label className="text-[10px] font-medium text-slate-500">
            Relax Step Degeri
          </Label>
          <Input
            type="number"
            step="any"
            value={stepValue}
            onChange={(event) =>
              onUpdateField('priceToBeatMaxPriceRelaxStepValue', event.target.value)
            }
            placeholder={stepMode === 'percent' ? '25' : '0.10'}
            className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
          />
        </div>
        {stepMode === 'absolute' && (
          <div className="space-y-1">
            <Label className="text-[10px] font-medium text-slate-500">
              Relax Step Birimi
            </Label>
            <Select
              value={stepUnit}
              onValueChange={(value) =>
                onUpdateField('priceToBeatMaxPriceRelaxStepUnit', value)
              }
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
        )}
      </div>
      <p className="text-[10px] leading-relaxed text-slate-400 italic">
        Miss sayisi kadar ardisk completed markette PTB gecildigi halde maxPrice ve
        fillability yuzunden kaliteli firsat kacarsa relax calisir. History sayisi kadar
        market taranir. Qualified miss sayilmasi icin best ask maxPrice altinda olmali,
        en az iki tradeable saniye gorulmeli ve depth bu USDC esigini karsilamali.
        Her ekstra miss&apos;te secilen step kadar ek gevseme uygulanir. Final PTB,
        relax sonrasinda bu min degerin altina inmez; bos birakirsan tampon fallback
        kullanilir.
      </p>
    </div>
  );
}
