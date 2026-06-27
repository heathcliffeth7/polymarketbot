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
  REVENGE_FLIP_CLASSIC_STOP_LOSS_ENABLED_FIELD,
  REVENGE_FLIP_CLOSE_ONLY_SEC_FIELD,
  REVENGE_FLIP_ENTRY_PTB_RULES_FIELD,
  REVENGE_FLIP_INITIAL_ORDER_USDC_FIELD,
  REVENGE_FLIP_LOT_LIMIT_PCT_FIELD,
  REVENGE_FLIP_MAX_FLIP_FIELD,
  REVENGE_FLIP_MIN_REENTRY_SHARES_FIELD,
  REVENGE_FLIP_POST_STOP_LOSS_IV_MISMATCH_ENABLED_FIELD,
  REVENGE_FLIP_PROFIT_TARGET_USDC_FIELD,
  REVENGE_FLIP_PTB_BUMP_AMOUNT_FIELD,
  REVENGE_FLIP_PTB_BUMP_ENABLED_FIELD,
  REVENGE_FLIP_PTB_BUMP_MAX_FIELD,
  REVENGE_FLIP_PTB_BUMP_MAX_UNIT_FIELD,
  REVENGE_FLIP_PTB_BUMP_UNIT_FIELD,
  REVENGE_FLIP_REENTRY_SIDE_MODE_FIELD,
  REVENGE_FLIP_STOP_LOSS_RULES_FIELD,
  REVENGE_FLIP_STOP_LOSS_PCT_FIELD,
  REVENGE_FLIP_TIME_RULES_FIELD,
  REVENGE_FLIP_TRIGGER_ENABLED_FIELD,
  REVENGE_FLIP_TRIGGER_MAX_CENT_FIELD,
  REVENGE_FLIP_TRIGGER_MIN_CENT_FIELD,
} from '@/lib/trade-flow-config-mappers/revenge-flip';
import { RevengeFlipEntryPtbRules } from './revenge-flip-entry-ptb-rules';
import { RevengeFlipPtbStopLossSection } from './revenge-flip-ptb-stop-loss-section';
import { RevengeFlipStopLossRules } from './revenge-flip-stop-loss-rules';

interface RevengeFlipSectionProps {
  visible: boolean;
  fields: Record<string, string>;
  onUpdateField: (key: string, value: string) => void;
}

function UnitSelect({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <Select value={value || 'usd'} onValueChange={onChange}>
      <SelectTrigger className="h-8 border-slate-200 bg-white text-xs" size="sm">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="usd">USD</SelectItem>
        <SelectItem value="cent">Cent</SelectItem>
      </SelectContent>
    </Select>
  );
}

export function RevengeFlipSection({
  visible,
  fields,
  onUpdateField,
}: RevengeFlipSectionProps) {
  if (!visible) return null;
  const triggerEnabled = fields[REVENGE_FLIP_TRIGGER_ENABLED_FIELD] === 'true';
  const bumpEnabled = fields[REVENGE_FLIP_PTB_BUMP_ENABLED_FIELD] === 'true';
  const classicStopLossEnabled =
    fields[REVENGE_FLIP_CLASSIC_STOP_LOSS_ENABLED_FIELD] !== 'false';

  return (
    <div className="space-y-3 rounded-md border border-rose-200/80 bg-rose-50/70 p-3 text-slate-900">
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">Initial USDC</Label>
          <Input
            type="number"
            min={0.01}
            step={0.01}
            value={fields[REVENGE_FLIP_INITIAL_ORDER_USDC_FIELD] || '5'}
            onChange={(event) =>
              onUpdateField(REVENGE_FLIP_INITIAL_ORDER_USDC_FIELD, event.target.value)
            }
            className="h-8 border-slate-200 bg-white text-xs"
          />
        </div>
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">Target PnL USDC</Label>
          <Input
            type="number"
            step={0.01}
            value={fields[REVENGE_FLIP_PROFIT_TARGET_USDC_FIELD] || '0.25'}
            onChange={(event) =>
              onUpdateField(REVENGE_FLIP_PROFIT_TARGET_USDC_FIELD, event.target.value)
            }
            className="h-8 border-slate-200 bg-white text-xs"
          />
        </div>
        <div className="flex items-center justify-between gap-2 rounded border border-rose-100 bg-white/80 px-2 py-1.5">
          <Label className="text-[11px] font-medium text-slate-600">Classic Stop-Loss</Label>
          <Switch
            checked={classicStopLossEnabled}
            onCheckedChange={(checked) =>
              onUpdateField(
                REVENGE_FLIP_CLASSIC_STOP_LOSS_ENABLED_FIELD,
                checked ? 'true' : 'false',
              )
            }
          />
        </div>
        {classicStopLossEnabled && (
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">Stop Loss</Label>
            <Input
              type="number"
              min={0.01}
              max={0.99}
              step={0.01}
              value={fields[REVENGE_FLIP_STOP_LOSS_PCT_FIELD] || '0.2'}
              onChange={(event) =>
                onUpdateField(REVENGE_FLIP_STOP_LOSS_PCT_FIELD, event.target.value)
              }
              className="h-8 border-slate-200 bg-white text-xs"
            />
          </div>
        )}
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">Max Flip</Label>
          <Input
            type="number"
            min={0}
            step={1}
            value={fields[REVENGE_FLIP_MAX_FLIP_FIELD] || '0'}
            onChange={(event) => onUpdateField(REVENGE_FLIP_MAX_FLIP_FIELD, event.target.value)}
            className="h-8 border-slate-200 bg-white text-xs"
          />
        </div>
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">Lot Limit</Label>
          <Input
            type="number"
            min={0.01}
            max={1}
            step={0.01}
            value={fields[REVENGE_FLIP_LOT_LIMIT_PCT_FIELD] || '0.98'}
            onChange={(event) =>
              onUpdateField(REVENGE_FLIP_LOT_LIMIT_PCT_FIELD, event.target.value)
            }
            className="h-8 border-slate-200 bg-white text-xs"
          />
        </div>
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">Close Only Sec</Label>
          <Input
            type="number"
            min={0}
            step={1}
            value={fields[REVENGE_FLIP_CLOSE_ONLY_SEC_FIELD] || '10'}
            onChange={(event) =>
              onUpdateField(REVENGE_FLIP_CLOSE_ONLY_SEC_FIELD, event.target.value)
            }
            className="h-8 border-slate-200 bg-white text-xs"
          />
        </div>
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">Min Re-entry Shares</Label>
          <Input
            type="number"
            min={0}
            step={0.01}
            value={fields[REVENGE_FLIP_MIN_REENTRY_SHARES_FIELD] || '0'}
            onChange={(event) =>
              onUpdateField(REVENGE_FLIP_MIN_REENTRY_SHARES_FIELD, event.target.value)
            }
            className="h-8 border-slate-200 bg-white text-xs"
          />
        </div>
        <div className="flex items-center justify-between gap-2 rounded border border-rose-100 bg-white/80 px-2 py-1.5">
          <Label className="text-[11px] font-medium text-slate-600">
            Stop-loss sonrası IV mismatch
          </Label>
          <Switch
            checked={fields[REVENGE_FLIP_POST_STOP_LOSS_IV_MISMATCH_ENABLED_FIELD] !== 'false'}
            onCheckedChange={(checked) =>
              onUpdateField(
                REVENGE_FLIP_POST_STOP_LOSS_IV_MISMATCH_ENABLED_FIELD,
                checked ? 'true' : 'false',
              )
            }
          />
        </div>
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">Re-entry Mode</Label>
          <Select
            value={fields[REVENGE_FLIP_REENTRY_SIDE_MODE_FIELD] || 'opposite'}
            onValueChange={(value) => onUpdateField(REVENGE_FLIP_REENTRY_SIDE_MODE_FIELD, value)}
          >
            <SelectTrigger className="h-8 border-slate-200 bg-white text-xs" size="sm">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="opposite">Opposite</SelectItem>
              <SelectItem value="rule_match">Rule Match</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      {classicStopLossEnabled && (
        <RevengeFlipStopLossRules
          value={fields[REVENGE_FLIP_STOP_LOSS_RULES_FIELD] || '[]'}
          onUpdateField={onUpdateField}
        />
      )}

      <RevengeFlipEntryPtbRules
        value={fields[REVENGE_FLIP_ENTRY_PTB_RULES_FIELD] || '[]'}
        currentSource={fields.priceToBeatCurrentPriceSource || 'chainlink'}
        onUpdateField={onUpdateField}
      />

      <RevengeFlipPtbStopLossSection fields={fields} onUpdateField={onUpdateField} />

      <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
        <div className="flex items-center justify-between gap-2 rounded border border-rose-100 bg-white/80 px-2 py-1.5">
          <Label className="text-[11px] font-medium text-slate-600">Trigger Range</Label>
          <Switch
            checked={triggerEnabled}
            onCheckedChange={(checked) =>
              onUpdateField(REVENGE_FLIP_TRIGGER_ENABLED_FIELD, checked ? 'true' : 'false')
            }
          />
        </div>
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">Min Cent</Label>
          <Input
            type="number"
            min={0}
            max={100}
            step={0.01}
            value={fields[REVENGE_FLIP_TRIGGER_MIN_CENT_FIELD] || '0'}
            onChange={(event) =>
              onUpdateField(REVENGE_FLIP_TRIGGER_MIN_CENT_FIELD, event.target.value)
            }
            className="h-8 border-slate-200 bg-white text-xs"
          />
        </div>
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">Max Cent</Label>
          <Input
            type="number"
            min={0}
            max={100}
            step={0.01}
            value={fields[REVENGE_FLIP_TRIGGER_MAX_CENT_FIELD] || '100'}
            onChange={(event) =>
              onUpdateField(REVENGE_FLIP_TRIGGER_MAX_CENT_FIELD, event.target.value)
            }
            className="h-8 border-slate-200 bg-white text-xs"
          />
        </div>
      </div>

      <div className="grid grid-cols-1 gap-2 sm:grid-cols-4">
        <div className="flex items-center justify-between gap-2 rounded border border-rose-100 bg-white/80 px-2 py-1.5">
          <Label className="text-[11px] font-medium text-slate-600">SL PTB Bump</Label>
          <Switch
            checked={bumpEnabled}
            onCheckedChange={(checked) =>
              onUpdateField(REVENGE_FLIP_PTB_BUMP_ENABLED_FIELD, checked ? 'true' : 'false')
            }
          />
        </div>
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">Bump</Label>
          <Input
            type="number"
            min={0}
            step={0.01}
            value={fields[REVENGE_FLIP_PTB_BUMP_AMOUNT_FIELD] || ''}
            onChange={(event) =>
              onUpdateField(REVENGE_FLIP_PTB_BUMP_AMOUNT_FIELD, event.target.value)
            }
            className="h-8 border-slate-200 bg-white text-xs"
          />
        </div>
        <div className="space-y-1">
          <Label className="text-[11px] font-medium text-slate-600">Unit</Label>
          <UnitSelect
            value={fields[REVENGE_FLIP_PTB_BUMP_UNIT_FIELD] || 'usd'}
            onChange={(value) => onUpdateField(REVENGE_FLIP_PTB_BUMP_UNIT_FIELD, value)}
          />
        </div>
        <div className="grid grid-cols-[1fr_88px] gap-2">
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">Max</Label>
            <Input
              type="number"
              min={0}
              step={0.01}
              value={fields[REVENGE_FLIP_PTB_BUMP_MAX_FIELD] || ''}
              onChange={(event) =>
                onUpdateField(REVENGE_FLIP_PTB_BUMP_MAX_FIELD, event.target.value)
              }
              className="h-8 border-slate-200 bg-white text-xs"
            />
          </div>
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">Unit</Label>
            <UnitSelect
              value={fields[REVENGE_FLIP_PTB_BUMP_MAX_UNIT_FIELD] || 'usd'}
              onChange={(value) => onUpdateField(REVENGE_FLIP_PTB_BUMP_MAX_UNIT_FIELD, value)}
            />
          </div>
        </div>
      </div>

      <div className="space-y-1">
        <Label className="text-[11px] font-medium text-slate-600">Time Rules JSON</Label>
        <textarea
          value={fields[REVENGE_FLIP_TIME_RULES_FIELD] || '[]'}
          onChange={(event) => onUpdateField(REVENGE_FLIP_TIME_RULES_FIELD, event.target.value)}
          className="min-h-20 w-full rounded-md border border-slate-200 bg-white px-2 py-1.5 font-mono text-xs text-slate-900 outline-none focus-visible:ring-2 focus-visible:ring-rose-300"
          spellCheck={false}
        />
      </div>
    </div>
  );
}
