import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  createEmptyPtbStopLossRuleRow,
  PTB_CURRENT_PRICE_SOURCE_OPTIONS,
  normalizeOptionalPtbCurrentPriceSource,
  type PtbCurrentPriceSource,
  type PtbGapUnit,
  type PtbStopLossRuleRow,
} from '@/lib/trade-flow-config-mappers';
import { PtbStopLossRuleSection } from './exit-sections';
import { EMPTY_SELECT_SENTINEL } from './shared';

interface PtbStopLossSectionProps {
  enabled: boolean;
  unit: PtbGapUnit;
  timeDecayMode: 'tighten' | 'relax' | 'none';
  currentSourceOverride: string;
  inheritedCurrentSource: PtbCurrentPriceSource;
  rows: PtbStopLossRuleRow[];
  onUpdateField: (key: string, value: string) => void;
  onUpdateRows: (updater: (rows: PtbStopLossRuleRow[]) => PtbStopLossRuleRow[]) => void;
}

function currentSourceLabel(source: PtbCurrentPriceSource): string {
  return PTB_CURRENT_PRICE_SOURCE_OPTIONS.find((option) => option.value === source)?.label ?? 'Chainlink';
}

function createDefaultPtbStopLossRuleRow(): PtbStopLossRuleRow {
  return { ...createEmptyPtbStopLossRuleRow(), sizePct: '100' };
}

export function PtbStopLossSection({
  enabled,
  unit,
  timeDecayMode,
  currentSourceOverride,
  inheritedCurrentSource,
  rows,
  onUpdateField,
  onUpdateRows,
}: PtbStopLossSectionProps) {
  const normalizedOverride = normalizeOptionalPtbCurrentPriceSource(currentSourceOverride);
  const selectedCurrentSourceText = normalizedOverride
    ? currentSourceLabel(normalizedOverride)
    : `Entry PTB kaynagi ile ayni (${currentSourceLabel(inheritedCurrentSource)})`;
  const visibleRows = enabled && rows.length === 0 ? [createDefaultPtbStopLossRuleRow()] : rows;
  const handleEnabledChange = (checked: boolean) => {
    onUpdateField('ptbStopLossEnabled', checked ? 'true' : 'false');
    if (checked && rows.length === 0) {
      onUpdateRows((currentRows) =>
        currentRows.length > 0 ? currentRows : [createDefaultPtbStopLossRuleRow()]
      );
    }
  };

  return (
    <div className="space-y-2 rounded-md border border-slate-200/80 bg-slate-50/80 p-3">
      <div className="flex items-center justify-between gap-2">
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">
            PTB Gap Stop-Loss
          </Label>
          <p className="text-[10px] leading-relaxed text-slate-400 italic">
            Master PTB toggle. Hard gap ve kademeli PTB satirlari bu ana switch ile
            acilip kapanir.
          </p>
        </div>
        <input
          type="checkbox"
          checked={enabled}
          onChange={(event) => handleEnabledChange(event.target.checked)}
          className="h-4 w-4 rounded border-slate-300"
        />
      </div>
      {enabled && (
        <>
          <p className="text-[10px] leading-relaxed text-slate-400 italic">
            Bu alan karsi token fiyati degil, underlying directional gap olarak calisir. Up/Yes
            icin secilen current kaynak - PTB, Down/No icin PTB - secilen current kaynak izlenir.
            -10, Up/Yes icin PTB referansinin 10 altini; Down/No icin PTB referansinin
            10 ustunu bekler. 0 / 100 tek satir eski hard PTB kapanisini staged sekilde
            temsil eder. Negatif esik, karsi yone overshoot bekler ve time decay
            uygulanmaz.
          </p>
          <div className="grid grid-cols-2 gap-2">
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">PTB Gap Birimi</Label>
              <Select value={unit} onValueChange={(value) => onUpdateField('ptbStopLossGapUnit', value)}>
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
              <Label className="text-[11px] font-medium text-slate-600">PTB SL Zaman Modu</Label>
              <Select
                value={timeDecayMode}
                onValueChange={(value) => onUpdateField('ptbStopLossTimeDecayMode', value)}
              >
                <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="tighten">tighten</SelectItem>
                  <SelectItem value="relax">relax</SelectItem>
                  <SelectItem value="none">none</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">PTB SL Current Kaynagi</Label>
            <Select
              value={normalizedOverride || EMPTY_SELECT_SENTINEL}
              onValueChange={(value) =>
                onUpdateField(
                  'ptbStopLossCurrentPriceSource',
                  value === EMPTY_SELECT_SENTINEL ? '' : value
                )
              }
            >
              <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={EMPTY_SELECT_SENTINEL}>
                  Entry PTB kaynagi ile ayni ({currentSourceLabel(inheritedCurrentSource)})
                </SelectItem>
                {PTB_CURRENT_PRICE_SOURCE_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <p className="text-[10px] leading-relaxed text-slate-400 italic">
            Secili PTB SL current: {selectedCurrentSourceText}.{' '}
            {unit === 'cent'
              ? 'Cent modu: 1 = $0.01. Bu unit staged satirlarin ve varsa legacy hard gap degerinin tamamina uygulanir.'
              : 'USD modu: 1 = $1.00. Bu unit staged satirlarin ve varsa legacy hard gap degerinin tamamina uygulanir.'}
            {' '}Bos birakilirsa yukaridaki PTB Current Kaynagi kullanilir.
          </p>
          <PtbStopLossRuleSection
            unit={unit}
            rows={visibleRows}
            onAdd={() =>
              onUpdateRows((currentRows) => [
                ...(currentRows.length > 0 ? currentRows : visibleRows),
                createEmptyPtbStopLossRuleRow(),
              ])
            }
            onUpdate={(rowId, patch) =>
              onUpdateRows((currentRows) =>
                (currentRows.length > 0 ? currentRows : visibleRows).map((row) =>
                  row.id === rowId ? { ...row, ...patch } : row
                )
              )
            }
            onRemove={(rowId) =>
              onUpdateRows((currentRows) =>
                (currentRows.length > 0 ? currentRows : visibleRows).filter((row) => row.id !== rowId)
              )
            }
          />
        </>
      )}
    </div>
  );
}
