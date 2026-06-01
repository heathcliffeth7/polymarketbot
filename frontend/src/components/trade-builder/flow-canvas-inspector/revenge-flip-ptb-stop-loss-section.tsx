import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import {
  PTB_STOP_LOSS_CURRENT_PRICE_SOURCE_OPTIONS,
  normalizeOptionalPtbStopLossCurrentPriceSource,
} from '@/lib/trade-flow-config-mappers/ptb-modes';
import {
  REVENGE_FLIP_PTB_STOP_LOSS_CURRENT_SOURCE_FIELD,
  REVENGE_FLIP_PTB_STOP_LOSS_ENABLED_FIELD,
  REVENGE_FLIP_PTB_STOP_LOSS_GAP_FIELD,
  REVENGE_FLIP_PTB_STOP_LOSS_GAP_UNIT_FIELD,
  REVENGE_FLIP_PTB_STOP_LOSS_TIME_DECAY_FIELD,
} from '@/lib/trade-flow-config-mappers/revenge-flip';
import { EMPTY_SELECT_SENTINEL } from './shared';

interface RevengeFlipPtbStopLossSectionProps {
  fields: Record<string, string>;
  onUpdateField: (key: string, value: string) => void;
}

export function RevengeFlipPtbStopLossSection({
  fields,
  onUpdateField,
}: RevengeFlipPtbStopLossSectionProps) {
  const enabled = fields[REVENGE_FLIP_PTB_STOP_LOSS_ENABLED_FIELD] === 'true';
  const unit = fields[REVENGE_FLIP_PTB_STOP_LOSS_GAP_UNIT_FIELD] === 'cent' ? 'cent' : 'usd';
  const timeDecayRaw = fields[REVENGE_FLIP_PTB_STOP_LOSS_TIME_DECAY_FIELD] || 'tighten';
  const timeDecayMode =
    timeDecayRaw === 'relax' || timeDecayRaw === 'none' ? timeDecayRaw : 'tighten';
  const currentSource = normalizeOptionalPtbStopLossCurrentPriceSource(
    fields[REVENGE_FLIP_PTB_STOP_LOSS_CURRENT_SOURCE_FIELD],
  );

  return (
    <div className="space-y-2 rounded-md border border-rose-100 bg-white/80 p-2">
      <div className="flex items-center justify-between gap-2">
        <Label className="text-[11px] font-medium text-slate-600">PTB Gap Stop-Loss</Label>
        <Switch
          checked={enabled}
          onCheckedChange={(checked) =>
            onUpdateField(REVENGE_FLIP_PTB_STOP_LOSS_ENABLED_FIELD, checked ? 'true' : 'false')
          }
        />
      </div>
      {enabled && (
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-4">
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">Gap</Label>
            <Input
              type="number"
              step={0.01}
              value={fields[REVENGE_FLIP_PTB_STOP_LOSS_GAP_FIELD] || ''}
              onChange={(event) =>
                onUpdateField(REVENGE_FLIP_PTB_STOP_LOSS_GAP_FIELD, event.target.value)
              }
              className="h-8 border-slate-200 bg-white text-xs"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">Unit</Label>
            <Select
              value={unit}
              onValueChange={(value) => onUpdateField(REVENGE_FLIP_PTB_STOP_LOSS_GAP_UNIT_FIELD, value)}
            >
              <SelectTrigger className="h-8 border-slate-200 bg-white text-xs" size="sm">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="usd">USD</SelectItem>
                <SelectItem value="cent">Cent</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">Current Source</Label>
            <Select
              value={currentSource || EMPTY_SELECT_SENTINEL}
              onValueChange={(value) =>
                onUpdateField(
                  REVENGE_FLIP_PTB_STOP_LOSS_CURRENT_SOURCE_FIELD,
                  value === EMPTY_SELECT_SENTINEL ? '' : value,
                )
              }
            >
              <SelectTrigger className="h-8 border-slate-200 bg-white text-xs" size="sm">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value={EMPTY_SELECT_SENTINEL}>Entry PTB Source</SelectItem>
                {PTB_STOP_LOSS_CURRENT_PRICE_SOURCE_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">Time Mode</Label>
            <Select
              value={timeDecayMode}
              onValueChange={(value) =>
                onUpdateField(REVENGE_FLIP_PTB_STOP_LOSS_TIME_DECAY_FIELD, value)
              }
            >
              <SelectTrigger className="h-8 border-slate-200 bg-white text-xs" size="sm">
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
      )}
    </div>
  );
}
