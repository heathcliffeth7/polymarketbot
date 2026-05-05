import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import {
  normalizePtbCurrentPriceSource,
  normalizePtbMode,
  PTB_CURRENT_PRICE_SOURCE_OPTIONS,
  PTB_MODE_OPTIONS,
  type PtbCurrentPriceSource,
  type PtbGapUnit,
  type PtbIvTimeRuleRow,
  type PtbMode,
  type PtbStopLossBumpMode,
  type PtbStopLossBumpLossRuleRow,
} from '@/lib/trade-flow-config-mappers';
import { PriceToBeatIvTimeRulesSection } from './price-to-beat-iv-time-rules-section';
import { PriceToBeatMaxPriceRelaxSection } from './price-to-beat-max-price-relax-section';
import { PriceToBeatStopLossBumpSection } from './price-to-beat-stop-loss-bump-section';

interface PriceToBeatGuardSectionProps {
  checked: boolean;
  retryChecked: boolean;
  mode: PtbMode;
  unit: PtbGapUnit;
  currentSource: PtbCurrentPriceSource;
  currentSourceVisible: boolean;
  fields: Record<string, string>;
  stopLossBumpUi: {
    checked: boolean;
    mode: PtbStopLossBumpMode;
    scope: 'global' | 'per_scope';
    unit: 'usd' | 'cent';
  };
  stopLossBumpLossRuleRows: PtbStopLossBumpLossRuleRow[];
  ivTimeRuleRows: PtbIvTimeRuleRow[];
  maxPriceRelaxMinUnit: 'usd' | 'cent';
  maxPriceRelaxStepMode: 'absolute' | 'percent';
  maxPriceRelaxStepUnit: 'usd' | 'cent';
  onUpdateField: (key: string, value: string) => void;
  onUpdateStopLossBumpLossRuleRows: (
    updater: (rows: PtbStopLossBumpLossRuleRow[]) => PtbStopLossBumpLossRuleRow[]
  ) => void;
  onUpdateIvTimeRuleRows: (updater: (rows: PtbIvTimeRuleRow[]) => PtbIvTimeRuleRow[]) => void;
}

export function PriceToBeatGuardSection({
  checked,
  retryChecked,
  mode,
  unit,
  currentSource,
  currentSourceVisible,
  fields,
  stopLossBumpUi,
  stopLossBumpLossRuleRows,
  ivTimeRuleRows,
  maxPriceRelaxMinUnit,
  maxPriceRelaxStepMode,
  maxPriceRelaxStepUnit,
  onUpdateField,
  onUpdateStopLossBumpLossRuleRows,
  onUpdateIvTimeRuleRows,
}: PriceToBeatGuardSectionProps) {
  const notifyChecked =
    (fields.notifyOnPriceToBeatGapBlocked ?? '').toString().trim().toLowerCase() === 'true';
  const showCurrentSource = checked || currentSourceVisible;

  return (
    <>
      <div className="mt-2 flex items-center justify-between gap-2 border-t border-slate-200 pt-2">
        <Label className="text-[11px] font-medium text-slate-600">
          Price to Beat Korumasi
        </Label>
        <input
          type="checkbox"
          checked={checked}
          onChange={(e) => {
            onUpdateField('priceToBeatGuardEnabled', e.target.checked ? 'true' : 'false');
            if (e.target.checked) {
              onUpdateField('priceToBeatMode', normalizePtbMode(fields.priceToBeatMode));
              onUpdateField(
                'priceToBeatCurrentPriceSource',
                normalizePtbCurrentPriceSource(fields.priceToBeatCurrentPriceSource)
              );
            }
            if (
              e.target.checked &&
              !['usd', 'cent'].includes(
                (fields.priceToBeatMaxDiffUnit ?? '').toString().trim().toLowerCase()
              )
            ) {
              onUpdateField('priceToBeatMaxDiffUnit', 'usd');
            }
            if (e.target.checked && !(fields.notifyOnPriceToBeatGapBlocked ?? '').toString().trim()) {
              onUpdateField('notifyOnPriceToBeatGapBlocked', 'true');
            }
          }}
          className="h-4 w-4 rounded border-slate-300"
        />
      </div>
      <p className="text-[10px] leading-relaxed text-slate-400 italic">
        PTB referansi ayni kalir; current underlying fiyati secilen kaynaktan gelir. Market BTC ise
        Binance BTCUSDT, Coinbase BTC-USD takip edilir. Veri eksik/stale ise Chainlink fallback
        yapilmaz.
      </p>
      {showCurrentSource && (
        <div className="mt-2 space-y-1 border-t border-slate-200 pt-2">
          <Label className="text-[11px] font-medium text-slate-600">PTB Current Kaynagi</Label>
          <Select
            value={currentSource}
            onValueChange={(value) => onUpdateField('priceToBeatCurrentPriceSource', value)}
          >
            <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {PTB_CURRENT_PRICE_SOURCE_OPTIONS.map((option) => (
                <SelectItem key={option.value} value={option.value}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      )}
      {checked && (
        <div className="mt-2 space-y-2 border-t border-slate-200 pt-2">
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">PTB Modu</Label>
            <Select value={mode} onValueChange={(value) => onUpdateField('priceToBeatMode', value)}>
              <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {PTB_MODE_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          {mode === 'manual' ? (
            <>
              <div className="space-y-1">
                <Label className="text-[11px] font-medium text-slate-600">Minimum Fark</Label>
                <Input
                  type="number"
                  step="any"
                  value={fields.priceToBeatMaxDiff ?? ''}
                  onChange={(event) => onUpdateField('priceToBeatMaxDiff', event.target.value)}
                  placeholder={unit === 'cent' ? '1' : '5'}
                  className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-[11px] font-medium text-slate-600">Birim</Label>
                <Select value={unit} onValueChange={(value) => onUpdateField('priceToBeatMaxDiffUnit', value)}>
                  <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="usd">USD</SelectItem>
                    <SelectItem value="cent">Cent</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <p className="text-[10px] leading-relaxed text-slate-400 italic">
                {unit === 'cent'
                  ? 'Cent modu: 1 = $0.01. Fark bu minimum degerin altinda kalirsa bloklanir.'
                  : 'USD modu: 5 = $5.00. Fark bu minimum degerin altinda kalirsa bloklanir.'}
              </p>
            </>
          ) : (
            <p className="text-[10px] leading-relaxed text-slate-400 italic">
              Dinamik modda esik elle girilmez. Ayni asset/timeframe icin otomatik PTB esigi
              kullanilir.
            </p>
          )}
          {mode === 'iv_mismatch_edge' && (
            <PriceToBeatIvTimeRulesSection
              rows={ivTimeRuleRows}
              fields={fields}
              onUpdateField={onUpdateField}
              onUpdateRows={onUpdateIvTimeRuleRows}
            />
          )}
          <PriceToBeatStopLossBumpSection
            enabled={stopLossBumpUi.checked}
            mode={stopLossBumpUi.mode}
            amount={fields.priceToBeatStopLossBumpAmount ?? ''}
            maxValue={fields.priceToBeatStopLossBumpMaxValue ?? ''}
            decayWindows={fields.priceToBeatStopLossBumpDecayWindows ?? ''}
            scopeMode={stopLossBumpUi.scope}
            unit={stopLossBumpUi.unit}
            defaultUnit={mode === 'manual' ? unit : 'usd'}
            lossRuleRows={stopLossBumpLossRuleRows}
            onUpdateField={onUpdateField}
            onUpdateLossRuleRows={onUpdateStopLossBumpLossRuleRows}
          />
          <PriceToBeatMaxPriceRelaxSection
            enabled={fields.priceToBeatMaxPriceRelaxEnabled ?? ''}
            missCount={fields.priceToBeatMaxPriceRelaxMissCount ?? ''}
            historyCount={fields.priceToBeatMaxPriceRelaxHistoryCount ?? ''}
            minValue={fields.priceToBeatMaxPriceRelaxMinValue ?? ''}
            minDepthUsd={fields.priceToBeatMaxPriceRelaxMinDepthUsd ?? ''}
            minUnit={maxPriceRelaxMinUnit}
            stepMode={maxPriceRelaxStepMode}
            stepValue={fields.priceToBeatMaxPriceRelaxStepValue ?? ''}
            stepUnit={maxPriceRelaxStepUnit}
            onUpdateField={onUpdateField}
          />
          <div className="flex items-center justify-between gap-2">
            <Label className="text-[11px] font-medium text-slate-600">
              Price to Beat Engel Bildirimi
            </Label>
            <input
              type="checkbox"
              checked={notifyChecked}
              onChange={(e) =>
                onUpdateField('notifyOnPriceToBeatGapBlocked', e.target.checked ? 'true' : 'false')
              }
              className="h-4 w-4 rounded border-slate-300"
            />
          </div>
          <div className="flex items-center justify-between gap-2">
            <Label className="text-[11px] font-medium text-slate-600">Iyilesince Tekrar Dene</Label>
            <input
              type="checkbox"
              checked={retryChecked}
              onChange={(e) =>
                onUpdateField('retryOnPriceToBeatGuardBlock', e.target.checked ? 'true' : 'false')
              }
              className="h-4 w-4 rounded border-slate-300"
            />
          </div>
          <p className="text-[10px] leading-relaxed text-slate-400 italic">
            Guard fail olursa node hata verir ama bekleme modunda yeniden denenir; kosullar
            duzelince order akisina devam eder.
          </p>
        </div>
      )}
    </>
  );
}
