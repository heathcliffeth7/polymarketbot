import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  createEmptyPtbStopLossBumpLossRuleRow,
  type PtbStopLossBumpLossRuleRow,
  type PtbStopLossBumpMode,
} from '@/lib/trade-flow-config-mappers';
import { Plus, Trash2 } from 'lucide-react';

interface PriceToBeatStopLossBumpSectionProps {
  enabled: boolean;
  mode: PtbStopLossBumpMode;
  amount: string;
  maxValue: string;
  decayWindows: string;
  scopeMode: 'global' | 'per_scope';
  unit: 'usd' | 'cent';
  defaultUnit: 'usd' | 'cent';
  lossRuleRows: PtbStopLossBumpLossRuleRow[];
  onUpdateField: (key: string, value: string) => void;
  onUpdateLossRuleRows: (
    updater: (rows: PtbStopLossBumpLossRuleRow[]) => PtbStopLossBumpLossRuleRow[]
  ) => void;
}

export function PriceToBeatStopLossBumpSection({
  enabled,
  mode,
  amount,
  maxValue,
  decayWindows,
  scopeMode,
  unit,
  defaultUnit,
  lossRuleRows,
  onUpdateField,
  onUpdateLossRuleRows,
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
            if (event.target.checked && !mode) {
              onUpdateField('priceToBeatStopLossBumpMode', 'fixed');
            }
          }}
          className="h-4 w-4 rounded border-slate-300"
        />
      </div>
      {enabled && (
        <div className="space-y-2">
          <div className="space-y-1">
            <Label className="text-[10px] font-medium text-slate-500">Bump Modu</Label>
            <Select
              value={mode}
              onValueChange={(value) => onUpdateField('priceToBeatStopLossBumpMode', value)}
            >
              <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="fixed">Sabit bump</SelectItem>
                <SelectItem value="loss_table">Zarar bazli tablo</SelectItem>
              </SelectContent>
            </Select>
          </div>
          {mode === 'fixed' ? (
            <div className="space-y-1">
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
            </div>
          ) : (
            <div className="space-y-2 rounded-md border border-slate-200/80 bg-white/80 p-2">
              <p className="text-[10px] leading-relaxed text-slate-400 italic">
                İlk finalized SL fill net zararına göre son uygun satır seçilir. Ornek:
                1 USD -&gt; 25 cent, 2 USD -&gt; 50 cent, 5 USD -&gt; 1 USD.
              </p>
              <div className="space-y-2">
                {lossRuleRows.length === 0 ? (
                  <p className="text-[10px] text-slate-400 italic">
                    Henüz zarar tablosu eklenmedi.
                  </p>
                ) : (
                  lossRuleRows.map((row, index) => (
                    <div
                      key={row.id}
                      className="space-y-1.5 rounded-md border border-slate-200 bg-white p-2"
                    >
                      <div className="flex items-center justify-between">
                        <p className="text-[10px] font-medium text-slate-600">
                          Kademe #{index + 1}
                        </p>
                        <Button
                          type="button"
                          size="sm"
                          variant="ghost"
                          className="h-6 w-6 p-0 text-red-400 hover:text-red-600"
                          onClick={() =>
                            onUpdateLossRuleRows((rows) =>
                              rows.filter((candidate) => candidate.id !== row.id)
                            )
                          }
                        >
                          <Trash2 className="h-3 w-3" />
                        </Button>
                      </div>
                      <div className="grid grid-cols-2 gap-2">
                        <div className="space-y-1">
                          <Label className="text-[10px] font-medium text-slate-500">
                            Zarar (USD)
                          </Label>
                          <Input
                            type="number"
                            step="any"
                            value={row.lossUsd}
                            onChange={(event) =>
                              onUpdateLossRuleRows((rows) =>
                                rows.map((candidate) =>
                                  candidate.id === row.id
                                    ? { ...candidate, lossUsd: event.target.value }
                                    : candidate
                                )
                              )
                            }
                            placeholder="1"
                            className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                          />
                        </div>
                        <div className="space-y-1">
                          <Label className="text-[10px] font-medium text-slate-500">
                            Bump ({unit === 'cent' ? 'Cent' : 'USD'})
                          </Label>
                          <Input
                            type="number"
                            step="any"
                            value={row.bumpValue}
                            onChange={(event) =>
                              onUpdateLossRuleRows((rows) =>
                                rows.map((candidate) =>
                                  candidate.id === row.id
                                    ? { ...candidate, bumpValue: event.target.value }
                                    : candidate
                                )
                              )
                            }
                            placeholder={unit === 'cent' ? '25' : '0.25'}
                            className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                          />
                        </div>
                      </div>
                    </div>
                  ))
                )}
              </div>
              <Button
                type="button"
                size="sm"
                variant="outline"
                className="h-7 border-slate-300 px-2 text-[11px] text-slate-700"
                onClick={() =>
                  onUpdateLossRuleRows((rows) => [
                    ...rows,
                    createEmptyPtbStopLossBumpLossRuleRow(),
                  ])
                }
              >
                <Plus className="mr-1 h-3 w-3" />
                Zarar Kademesi Ekle
              </Button>
            </div>
          )}
          <div className="grid grid-cols-2 gap-2">
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
          </div>
        </div>
      )}
    </div>
  );
}
