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
import { REVENGE_FLIP_ENTRY_PTB_RULES_FIELD } from '@/lib/trade-flow-config-mappers/revenge-flip';
import {
  PTB_CURRENT_PRICE_SOURCE_OPTIONS,
  normalizePtbCurrentPriceSource,
} from '@/lib/trade-flow-config-mappers/ptb-modes';
import { Plus, Trash2 } from 'lucide-react';

interface RevengeFlipEntryPtbRulesProps {
  value: string;
  currentSource: string;
  onUpdateField: (key: string, value: string) => void;
}

interface EntryPtbRuleRow {
  minFlip: string;
  maxFlip: string;
  sideMode: 'any' | 'same' | 'opposite' | 'up' | 'down';
  minRemainingSec: string;
  maxRemainingSec: string;
  priceToBeatMinDiff: string;
  priceToBeatMinDiffUnit: 'usd' | 'cent';
  maxPriceCent: string;
}

function toText(value: unknown): string {
  if (value == null) return '';
  return String(value);
}

function parseUnit(value: unknown): 'usd' | 'cent' {
  const text = toText(value).trim().toLowerCase();
  return text === 'usd' ? 'usd' : 'cent';
}

function parseSideMode(value: unknown): EntryPtbRuleRow['sideMode'] {
  const text = toText(value).trim().toLowerCase();
  if (text === 'same' || text === 'opposite' || text === 'up' || text === 'down') return text;
  return 'any';
}

function parseRows(value: string): EntryPtbRuleRow[] {
  try {
    const parsed = JSON.parse(value || '[]');
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((item): item is Record<string, unknown> => Boolean(item) && typeof item === 'object')
      .map((item) => ({
        minFlip: toText(item.minFlip ?? 0),
        maxFlip: toText(item.maxFlip ?? ''),
        sideMode: parseSideMode(item.sideMode ?? item.entrySideMode),
        minRemainingSec: toText(item.minRemainingSec ?? item.remainingSecMin ?? ''),
        maxRemainingSec: toText(item.maxRemainingSec ?? item.remainingSecMax ?? ''),
        priceToBeatMinDiff: toText(
          item.priceToBeatMinDiff ?? item.ptbMinDiff ?? item.priceToBeatMaxDiff ?? item.ptbMaxDiff ?? '',
        ),
        priceToBeatMinDiffUnit: parseUnit(
          item.priceToBeatMinDiffUnit ?? item.priceToBeatMaxDiffUnit ?? item.ptbDiffUnit,
        ),
        maxPriceCent: toText(item.maxPriceCent ?? item.entryMaxPriceCent ?? ''),
      }));
  } catch {
    return [];
  }
}

function finiteOrUndefined(value: string): number | undefined {
  if (!value.trim()) return undefined;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function serializeRows(rows: EntryPtbRuleRow[]): string {
  return JSON.stringify(
    rows.map((row) => {
      const next: Record<string, number | string> = {
        minFlip: finiteOrUndefined(row.minFlip) ?? 0,
        sideMode: row.sideMode,
        priceToBeatMinDiffUnit: row.priceToBeatMinDiffUnit,
      };
      const maxFlip = finiteOrUndefined(row.maxFlip);
      if (maxFlip != null) next.maxFlip = maxFlip;
      const minRemainingSec = finiteOrUndefined(row.minRemainingSec);
      if (minRemainingSec != null) next.minRemainingSec = minRemainingSec;
      const maxRemainingSec = finiteOrUndefined(row.maxRemainingSec);
      if (maxRemainingSec != null) next.maxRemainingSec = maxRemainingSec;
      const priceToBeatMinDiff = finiteOrUndefined(row.priceToBeatMinDiff);
      if (priceToBeatMinDiff != null) next.priceToBeatMinDiff = priceToBeatMinDiff;
      const maxPriceCent = finiteOrUndefined(row.maxPriceCent);
      if (maxPriceCent != null) next.maxPriceCent = maxPriceCent;
      return next;
    }),
  );
}

export function RevengeFlipEntryPtbRules({
  value,
  currentSource,
  onUpdateField,
}: RevengeFlipEntryPtbRulesProps) {
  const rows = parseRows(value);
  const normalizedCurrentSource = normalizePtbCurrentPriceSource(currentSource);
  const updateRows = (nextRows: EntryPtbRuleRow[]) =>
    onUpdateField(REVENGE_FLIP_ENTRY_PTB_RULES_FIELD, serializeRows(nextRows));
  const updateRow = (index: number, patch: Partial<EntryPtbRuleRow>) =>
    updateRows(rows.map((row, rowIndex) => (rowIndex === index ? { ...row, ...patch } : row)));

  return (
    <div className="space-y-2 rounded-md border border-rose-100 bg-white/80 p-2 text-slate-900">
      <div className="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <Label className="text-[11px] font-medium text-slate-600">Entry PTB Rules</Label>
          <p className="mt-0.5 text-[10px] text-slate-400">
            Ilk alimda ilk uygun rule kazanir; stop sonrasi re-entry kurallari ayrica uygulanir.
          </p>
        </div>
        <div className="flex flex-wrap items-end gap-2">
          <div className="w-32 space-y-1">
            <Label className="text-[10px] font-medium text-slate-500">PTB Source</Label>
            <Select
              value={normalizedCurrentSource}
              onValueChange={(nextSource) =>
                onUpdateField('priceToBeatCurrentPriceSource', nextSource)
              }
            >
              <SelectTrigger className="h-7 border-slate-200 bg-white text-xs" size="sm">
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
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-7 border-rose-200 px-2 text-[11px] text-slate-700"
            onClick={() =>
              updateRows([
                ...rows,
                {
                  minFlip: String(rows.length),
                  maxFlip: '',
                  sideMode: 'any',
                  minRemainingSec: '',
                  maxRemainingSec: '',
                  priceToBeatMinDiff: '',
                  priceToBeatMinDiffUnit: 'cent',
                  maxPriceCent: '',
                },
              ])
            }
          >
            <Plus className="mr-1 h-3 w-3" />
            PTB Kurali Ekle
          </Button>
        </div>
      </div>
      {rows.length === 0 ? (
        <p className="text-[10px] text-slate-400 italic">
          Bos birakilirsa once Time Rules, sonra global min PTB farki kullanilir.
        </p>
      ) : (
        <div className="space-y-2">
          {rows.map((row, index) => (
            <div
              key={`entry-ptb-rule-${index}`}
              className="grid grid-cols-2 gap-2 sm:grid-cols-3"
            >
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-500">Min Flip</Label>
                <Input
                  type="number"
                  min={0}
                  step={1}
                  value={row.minFlip}
                  onChange={(event) => updateRow(index, { minFlip: event.target.value })}
                  className="h-8 border-slate-200 bg-white text-xs"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-500">Max Flip</Label>
                <Input
                  type="number"
                  min={0}
                  step={1}
                  value={row.maxFlip}
                  onChange={(event) => updateRow(index, { maxFlip: event.target.value })}
                  placeholder="all"
                  className="h-8 border-slate-200 bg-white text-xs"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-500">Side</Label>
                <Select
                  value={row.sideMode}
                  onValueChange={(value) =>
                    updateRow(index, { sideMode: value as EntryPtbRuleRow['sideMode'] })
                  }
                >
                  <SelectTrigger className="h-8 border-slate-200 bg-white text-xs" size="sm">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="any">Any</SelectItem>
                    <SelectItem value="same">Same</SelectItem>
                    <SelectItem value="opposite">Opposite</SelectItem>
                    <SelectItem value="up">Up</SelectItem>
                    <SelectItem value="down">Down</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-500">Min Rem Sec</Label>
                <Input
                  type="number"
                  min={0}
                  step={1}
                  value={row.minRemainingSec}
                  onChange={(event) => updateRow(index, { minRemainingSec: event.target.value })}
                  className="h-8 border-slate-200 bg-white text-xs"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-500">Max Rem Sec</Label>
                <Input
                  type="number"
                  min={0}
                  step={1}
                  value={row.maxRemainingSec}
                  onChange={(event) => updateRow(index, { maxRemainingSec: event.target.value })}
                  placeholder="all"
                  className="h-8 border-slate-200 bg-white text-xs"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-500">Min PTB Diff</Label>
                <Input
                  type="number"
                  min={0}
                  step={0.01}
                  value={row.priceToBeatMinDiff}
                  onChange={(event) =>
                    updateRow(index, { priceToBeatMinDiff: event.target.value })
                  }
                  className="h-8 border-slate-200 bg-white text-xs"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-500">PTB Unit</Label>
                <Select
                  value={row.priceToBeatMinDiffUnit}
                  onValueChange={(value) =>
                    updateRow(index, { priceToBeatMinDiffUnit: value as 'usd' | 'cent' })
                  }
                >
                  <SelectTrigger className="h-8 border-slate-200 bg-white text-xs" size="sm">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="cent">Cent</SelectItem>
                    <SelectItem value="usd">USD</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-1">
                <Label className="text-[10px] font-medium text-slate-500">Max Price</Label>
                <Input
                  type="number"
                  min={0}
                  max={100}
                  step={0.01}
                  value={row.maxPriceCent}
                  onChange={(event) => updateRow(index, { maxPriceCent: event.target.value })}
                  placeholder="global"
                  className="h-8 border-slate-200 bg-white text-xs"
                />
              </div>
              <div className="flex items-end">
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  className="h-8 w-8 p-0 text-red-400 hover:text-red-600"
                  onClick={() => updateRows(rows.filter((_, rowIndex) => rowIndex !== index))}
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
