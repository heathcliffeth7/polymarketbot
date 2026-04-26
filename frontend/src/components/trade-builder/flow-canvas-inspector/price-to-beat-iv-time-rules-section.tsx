import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import {
  createEmptyPtbIvTimeRuleRow,
  type PtbIvTimeRuleRow,
} from '@/lib/trade-flow-config-mappers';
import { Plus, Trash2 } from 'lucide-react';

interface PriceToBeatIvTimeRulesSectionProps {
  rows: PtbIvTimeRuleRow[];
  fields: Record<string, string>;
  onUpdateField: (key: string, value: string) => void;
  onUpdateRows: (updater: (rows: PtbIvTimeRuleRow[]) => PtbIvTimeRuleRow[]) => void;
}

export function PriceToBeatIvTimeRulesSection({
  rows,
  fields,
  onUpdateField,
  onUpdateRows,
}: PriceToBeatIvTimeRulesSectionProps) {
  return (
    <div className="space-y-2 rounded-md border border-slate-200/80 bg-slate-50/70 p-2">
      <div className="space-y-1">
        <Label className="text-[11px] font-medium text-slate-600">IV Gap Strength Kurallari</Label>
        <p className="text-[10px] leading-relaxed text-slate-400 italic">
          Kalan sure araligina gore max fiyat, edge ve gap/expected_move esigini belirler.
        </p>
      </div>
      <div className="space-y-2">
        {rows.length === 0 ? (
          <p className="text-[10px] text-slate-400 italic">Henuz IV time rule eklenmedi.</p>
        ) : (
          rows.map((row, index) => (
            <div key={row.id} className="space-y-2 rounded-md border border-slate-200 bg-white p-2">
              <div className="flex items-center justify-between">
                <p className="text-[10px] font-medium text-slate-600">Aralik #{index + 1}</p>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  className="h-6 w-6 p-0 text-red-400 hover:text-red-600"
                  onClick={() =>
                    onUpdateRows((currentRows) =>
                      currentRows.filter((candidate) => candidate.id !== row.id)
                    )
                  }
                >
                  <Trash2 className="h-3 w-3" />
                </Button>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <IvRuleInput
                  label="Start rem sn"
                  value={row.startRemainingSec}
                  placeholder="120"
                  onChange={(value) =>
                    onUpdateRows((currentRows) =>
                      updateRuleRow(currentRows, row.id, { startRemainingSec: value })
                    )
                  }
                />
                <IvRuleInput
                  label="End rem sn"
                  value={row.endRemainingSec}
                  placeholder="60"
                  onChange={(value) =>
                    onUpdateRows((currentRows) =>
                      updateRuleRow(currentRows, row.id, { endRemainingSec: value })
                    )
                  }
                />
                <IvRuleInput
                  label="Max fiyat (cent)"
                  value={row.maxPriceCent}
                  placeholder="65"
                  onChange={(value) =>
                    onUpdateRows((currentRows) =>
                      updateRuleRow(currentRows, row.id, { maxPriceCent: value })
                    )
                  }
                />
                <IvRuleInput
                  label="Min edge"
                  value={row.minEdge}
                  placeholder="0.08"
                  onChange={(value) =>
                    onUpdateRows((currentRows) =>
                      updateRuleRow(currentRows, row.id, { minEdge: value })
                    )
                  }
                />
                <IvRuleInput
                  label="Min gap strength"
                  value={row.minGapStrength}
                  placeholder="0.90"
                  onChange={(value) =>
                    onUpdateRows((currentRows) =>
                      updateRuleRow(currentRows, row.id, { minGapStrength: value })
                    )
                  }
                />
                <IvRuleInput
                  label="Min exp move USD"
                  value={row.minExpectedMoveUsd}
                  placeholder="12"
                  onChange={(value) =>
                    onUpdateRows((currentRows) =>
                      updateRuleRow(currentRows, row.id, { minExpectedMoveUsd: value })
                    )
                  }
                />
                <IvRuleInput
                  label="Min gap str margin"
                  value={row.minGapStrengthMargin}
                  placeholder="0.15"
                  onChange={(value) =>
                    onUpdateRows((currentRows) =>
                      updateRuleRow(currentRows, row.id, { minGapStrengthMargin: value })
                    )
                  }
                />
                <IvRuleInput
                  label="Min gap USD margin"
                  value={row.minGapUsdMargin}
                  placeholder="2.5"
                  onChange={(value) =>
                    onUpdateRows((currentRows) =>
                      updateRuleRow(currentRows, row.id, { minGapUsdMargin: value })
                    )
                  }
                />
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
        onClick={() => onUpdateRows((currentRows) => [...currentRows, createEmptyPtbIvTimeRuleRow()])}
      >
        <Plus className="mr-1 h-3 w-3" />
        IV Aralik Ekle
      </Button>
      <div className="grid grid-cols-2 gap-2 border-t border-slate-200 pt-2">
        <IvRuleInput
          label="Stale ms"
          value={fields.priceToBeatIvStalePenaltyMs ?? ''}
          placeholder="1500"
          onChange={(value) => onUpdateField('priceToBeatIvStalePenaltyMs', value)}
        />
        <IvRuleInput
          label="Stale ceza"
          value={fields.priceToBeatIvStaleGapStrengthPenalty ?? ''}
          placeholder="0.10"
          onChange={(value) => onUpdateField('priceToBeatIvStaleGapStrengthPenalty', value)}
        />
        <IvRuleInput
          label="Momentum ceza"
          value={fields.priceToBeatIvNegativeVelocityGapStrengthPenalty ?? ''}
          placeholder="0.15"
          onChange={(value) => onUpdateField('priceToBeatIvNegativeVelocityGapStrengthPenalty', value)}
        />
        <IvRuleInput
          label="Binance ask esigi"
          value={fields.priceToBeatIvBinanceMissingAskThresholdCent ?? ''}
          placeholder="65"
          onChange={(value) => onUpdateField('priceToBeatIvBinanceMissingAskThresholdCent', value)}
        />
        <IvRuleInput
          label="Binance eksik ceza"
          value={fields.priceToBeatIvBinanceMissingPenalty ?? ''}
          placeholder="0.02"
          onChange={(value) => onUpdateField('priceToBeatIvBinanceMissingPenalty', value)}
        />
        <IvRuleInput
          label="Min adj margin"
          value={fields.priceToBeatIvMinAdjustedMargin ?? ''}
          placeholder="0.02"
          onChange={(value) => onUpdateField('priceToBeatIvMinAdjustedMargin', value)}
        />
        <IvRuleInput
          label="Min final q"
          value={fields.priceToBeatIvMinFinalQ ?? ''}
          placeholder="0.62"
          onChange={(value) => onUpdateField('priceToBeatIvMinFinalQ', value)}
        />
        <IvRuleInput
          label="Disagree esik"
          value={fields.priceToBeatIvBinanceDisagreementThreshold ?? ''}
          placeholder="0.15"
          onChange={(value) => onUpdateField('priceToBeatIvBinanceDisagreementThreshold', value)}
        />
        <IvRuleInput
          label="Disagree ceza"
          value={fields.priceToBeatIvBinanceDisagreementPenalty ?? ''}
          placeholder="0.02"
          onChange={(value) => onUpdateField('priceToBeatIvBinanceDisagreementPenalty', value)}
        />
        <IvRuleInput
          label="Large disagree"
          value={fields.priceToBeatIvLargeBinanceDisagreementThreshold ?? ''}
          placeholder="0.20"
          onChange={(value) =>
            onUpdateField('priceToBeatIvLargeBinanceDisagreementThreshold', value)
          }
        />
        <IvRuleInput
          label="Large ceza"
          value={fields.priceToBeatIvLargeBinanceDisagreementPenalty ?? ''}
          placeholder="0.04"
          onChange={(value) => onUpdateField('priceToBeatIvLargeBinanceDisagreementPenalty', value)}
        />
      </div>
      <div className="space-y-2 border-t border-slate-200 pt-2">
        <Label className="text-[11px] font-medium text-slate-600">Protection Mode</Label>
        <div className="grid grid-cols-2 gap-2">
          <IvRuleSelect
            label="Koruma modu"
            value={fields.priceToBeatIvProtectionMode || 'off'}
            onChange={(value) => onUpdateField('priceToBeatIvProtectionMode', value)}
            options={[
              { label: 'Off', value: 'off' },
              { label: 'Soft', value: 'soft' },
              { label: 'Hard', value: 'hard' },
              { label: 'Adaptive', value: 'adaptive' },
            ]}
          />
          <IvRuleInput
            label="Book lead sn"
            value={fields.priceToBeatIvBookLeadUnderSec ?? ''}
            placeholder="120"
            onChange={(value) => onUpdateField('priceToBeatIvBookLeadUnderSec', value)}
          />
          <IvRuleInput
            label="Book mid diff"
            value={fields.priceToBeatIvBookLeadMinMidDiff ?? ''}
            placeholder="0.20"
            onChange={(value) => onUpdateField('priceToBeatIvBookLeadMinMidDiff', value)}
          />
          <IvRuleInput
            label="Ters mid cent"
            value={fields.priceToBeatIvOppositeMidBlockCent ?? ''}
            placeholder="65"
            onChange={(value) => onUpdateField('priceToBeatIvOppositeMidBlockCent', value)}
          />
          <IvRuleInput
            label="Model-book warn"
            value={fields.priceToBeatIvModelBookGapWarn ?? ''}
            placeholder="0.30"
            onChange={(value) => onUpdateField('priceToBeatIvModelBookGapWarn', value)}
          />
          <IvRuleInput
            label="Model-book hard"
            value={fields.priceToBeatIvModelBookGapHard ?? fields.priceToBeatIvTooGoodToBeTrueGap ?? ''}
            placeholder="0.45"
            onChange={(value) => {
              onUpdateField('priceToBeatIvModelBookGapHard', value);
              onUpdateField('priceToBeatIvTooGoodToBeTrueGap', value);
            }}
          />
          <IvRuleInput
            label="MB edge ceza"
            value={fields.priceToBeatIvModelBookWarnThresholdPenalty ?? ''}
            placeholder="0.02"
            onChange={(value) => onUpdateField('priceToBeatIvModelBookWarnThresholdPenalty', value)}
          />
          <IvRuleInput
            label="MB gap ceza"
            value={fields.priceToBeatIvModelBookWarnGapPenalty ?? ''}
            placeholder="0.05"
            onChange={(value) => onUpdateField('priceToBeatIvModelBookWarnGapPenalty', value)}
          />
          <IvRuleInput
            label="Legacy hard gap"
            value={fields.priceToBeatIvTooGoodToBeTrueGap ?? ''}
            placeholder="0.45"
            onChange={(value) => onUpdateField('priceToBeatIvTooGoodToBeTrueGap', value)}
          />
          <IvRuleInput
            label="Depth slip"
            value={fields.priceToBeatIvDepthMaxSlippage ?? ''}
            placeholder="0.03"
            onChange={(value) => onUpdateField('priceToBeatIvDepthMaxSlippage', value)}
          />
          <IvRuleInput
            label="Late soft sn"
            value={fields.priceToBeatIvLateHighPriceSoftUnderSec ?? ''}
            placeholder="60"
            onChange={(value) => onUpdateField('priceToBeatIvLateHighPriceSoftUnderSec', value)}
          />
          <IvRuleInput
            label="Late ask cent"
            value={fields.priceToBeatIvLateHighPriceAskCent ?? ''}
            placeholder="65"
            onChange={(value) => onUpdateField('priceToBeatIvLateHighPriceAskCent', value)}
          />
          <IvRuleInput
            label="Late soft mid"
            value={fields.priceToBeatIvLateHighPriceSelectedMidSoftCent ?? ''}
            placeholder="75"
            onChange={(value) => onUpdateField('priceToBeatIvLateHighPriceSelectedMidSoftCent', value)}
          />
          <IvRuleInput
            label="Late edge ceza"
            value={fields.priceToBeatIvLateHighPriceThresholdPenalty ?? ''}
            placeholder="0.03"
            onChange={(value) => onUpdateField('priceToBeatIvLateHighPriceThresholdPenalty', value)}
          />
          <IvRuleInput
            label="Late hard mid"
            value={fields.priceToBeatIvLateHighPriceSelectedMidHardCent ?? ''}
            placeholder="65"
            onChange={(value) => onUpdateField('priceToBeatIvLateHighPriceSelectedMidHardCent', value)}
          />
          <IvRuleInput
            label="Late min gap USD"
            value={fields.priceToBeatIvLateHighPriceMinGapUsd ?? ''}
            placeholder="20"
            onChange={(value) => onUpdateField('priceToBeatIvLateHighPriceMinGapUsd', value)}
          />
          <IvRuleInput
            label="Part min"
            value={fields.priceToBeatIvParticipationAfterMinutes ?? ''}
            placeholder="60"
            onChange={(value) => onUpdateField('priceToBeatIvParticipationAfterMinutes', value)}
          />
          <IvRuleInput
            label="Part long min"
            value={fields.priceToBeatIvParticipationLongAfterMinutes ?? ''}
            placeholder="180"
            onChange={(value) => onUpdateField('priceToBeatIvParticipationLongAfterMinutes', value)}
          />
          <IvRuleInput
            label="Part credit"
            value={fields.priceToBeatIvParticipationCredit ?? ''}
            placeholder="0.01"
            onChange={(value) => onUpdateField('priceToBeatIvParticipationCredit', value)}
          />
          <IvRuleInput
            label="Part long credit"
            value={fields.priceToBeatIvParticipationLongCredit ?? ''}
            placeholder="0.02"
            onChange={(value) => onUpdateField('priceToBeatIvParticipationLongCredit', value)}
          />
          <IvRuleInput
            label="Part floor"
            value={fields.priceToBeatIvParticipationMinThreshold ?? ''}
            placeholder="0.05"
            onChange={(value) => onUpdateField('priceToBeatIvParticipationMinThreshold', value)}
          />
          <IvRuleInput
            label="Binance zor sn"
            value={fields.priceToBeatIvRequireBinanceFreshUnderSec ?? ''}
            placeholder="60"
            onChange={(value) => onUpdateField('priceToBeatIvRequireBinanceFreshUnderSec', value)}
          />
          <IvRuleInput
            label="Binance stale ms"
            value={fields.priceToBeatIvBinanceMaxStaleMs ?? ''}
            placeholder="2000"
            onChange={(value) => onUpdateField('priceToBeatIvBinanceMaxStaleMs', value)}
          />
          <IvRuleInput
            label="Drop z koruma"
            value={fields.priceToBeatIvDropZBlockThreshold ?? ''}
            placeholder="0.80"
            onChange={(value) => onUpdateField('priceToBeatIvDropZBlockThreshold', value)}
          />
          <IvRuleInput
            label="Soft edge ceza"
            value={fields.priceToBeatIvProtectionSoftThresholdPenalty ?? ''}
            placeholder="0.03"
            onChange={(value) => onUpdateField('priceToBeatIvProtectionSoftThresholdPenalty', value)}
          />
          <IvRuleInput
            label="Soft gap ceza"
            value={fields.priceToBeatIvProtectionSoftGapStrengthPenalty ?? ''}
            placeholder="0.10"
            onChange={(value) => onUpdateField('priceToBeatIvProtectionSoftGapStrengthPenalty', value)}
          />
        </div>
        <div className="grid grid-cols-2 gap-2">
          <IvRuleCheckbox
            label="Book lead"
            value={fields.priceToBeatIvBookLeadGuardEnabled}
            onChange={(value) => onUpdateField('priceToBeatIvBookLeadGuardEnabled', value)}
          />
          <IvRuleCheckbox
            label="Ters book block"
            value={fields.priceToBeatIvBlockOnOppositeBookLead}
            onChange={(value) => onUpdateField('priceToBeatIvBlockOnOppositeBookLead', value)}
          />
          <IvRuleCheckbox
            label="Depth"
            value={fields.priceToBeatIvDepthGuardEnabled}
            onChange={(value) => onUpdateField('priceToBeatIvDepthGuardEnabled', value)}
          />
          <IvRuleCheckbox
            label="Participation"
            value={fields.priceToBeatIvParticipationCreditEnabled}
            onChange={(value) => onUpdateField('priceToBeatIvParticipationCreditEnabled', value)}
          />
          <IvRuleCheckbox
            label="Binance yon"
            value={fields.priceToBeatIvRequireBinanceSameDirection}
            onChange={(value) => onUpdateField('priceToBeatIvRequireBinanceSameDirection', value)}
          />
          <IvRuleCheckbox
            label="Momentum"
            value={fields.priceToBeatIvMomentumProtectionEnabled}
            onChange={(value) => onUpdateField('priceToBeatIvMomentumProtectionEnabled', value)}
          />
        </div>
      </div>
      <div className="space-y-2 border-t border-slate-200 pt-2">
        <Label className="text-[11px] font-medium text-slate-600">Adaptive Volume</Label>
        <div className="grid grid-cols-2 gap-2">
          <IvRuleSelect
            label="Baseline"
            value={fields.priceToBeatIvVolumeBaselineMode || 'off'}
            onChange={(value) => onUpdateField('priceToBeatIvVolumeBaselineMode', value)}
            options={[
              { label: 'Off', value: 'off' },
              { label: 'Hourly', value: 'hourly' },
            ]}
          />
          <IvRuleInput
            label="Lookback gun"
            value={fields.priceToBeatIvVolumeBaselineLookbackDays ?? ''}
            placeholder="7"
            onChange={(value) => onUpdateField('priceToBeatIvVolumeBaselineLookbackDays', value)}
          />
          <IvRuleInput
            label="Volume window sn"
            value={fields.priceToBeatIvVolumeWindowSec ?? ''}
            placeholder="30"
            onChange={(value) => onUpdateField('priceToBeatIvVolumeWindowSec', value)}
          />
          <IvRuleInput
            label="Min sample"
            value={fields.priceToBeatIvVolumeBaselineMinSamples ?? ''}
            placeholder="20"
            onChange={(value) => onUpdateField('priceToBeatIvVolumeBaselineMinSamples', value)}
          />
          <IvRuleInput
            label="Low vol ratio"
            value={fields.priceToBeatIvLowHourlyVolumeRatio ?? ''}
            placeholder="0.7"
            onChange={(value) => onUpdateField('priceToBeatIvLowHourlyVolumeRatio', value)}
          />
          <IvRuleInput
            label="High vol ratio"
            value={fields.priceToBeatIvHighHourlyVolumeRatio ?? ''}
            placeholder="1.5"
            onChange={(value) => onUpdateField('priceToBeatIvHighHourlyVolumeRatio', value)}
          />
          <IvRuleInput
            label="Extreme vol ratio"
            value={fields.priceToBeatIvExtremeHourlyVolumeRatio ?? ''}
            placeholder="3.0"
            onChange={(value) => onUpdateField('priceToBeatIvExtremeHourlyVolumeRatio', value)}
          />
          <IvRuleInput
            label="Book reliability"
            value={fields.priceToBeatIvBookReliabilityThreshold ?? ''}
            placeholder="0.60"
            onChange={(value) => onUpdateField('priceToBeatIvBookReliabilityThreshold', value)}
          />
          <IvRuleInput
            label="Green edge delta"
            value={fields.priceToBeatIvAdaptiveGreenEdgeDelta ?? ''}
            placeholder="-0.01"
            onChange={(value) => onUpdateField('priceToBeatIvAdaptiveGreenEdgeDelta', value)}
          />
          <IvRuleInput
            label="Green gap delta"
            value={fields.priceToBeatIvAdaptiveGreenGapStrengthDelta ?? ''}
            placeholder="-0.03"
            onChange={(value) =>
              onUpdateField('priceToBeatIvAdaptiveGreenGapStrengthDelta', value)
            }
          />
          <IvRuleInput
            label="Orange edge delta"
            value={fields.priceToBeatIvAdaptiveOrangeEdgeDelta ?? ''}
            placeholder="0.03"
            onChange={(value) => onUpdateField('priceToBeatIvAdaptiveOrangeEdgeDelta', value)}
          />
          <IvRuleInput
            label="Orange gap delta"
            value={fields.priceToBeatIvAdaptiveOrangeGapStrengthDelta ?? ''}
            placeholder="0.15"
            onChange={(value) =>
              onUpdateField('priceToBeatIvAdaptiveOrangeGapStrengthDelta', value)
            }
          />
          <IvRuleInput
            label="Orange USD margin"
            value={fields.priceToBeatIvAdaptiveOrangeGapUsdMarginDelta ?? ''}
            placeholder="1.0"
            onChange={(value) =>
              onUpdateField('priceToBeatIvAdaptiveOrangeGapUsdMarginDelta', value)
            }
          />
          <IvRuleCheckbox
            label="Red block"
            value={fields.priceToBeatIvAdaptiveRedBlock}
            onChange={(value) => onUpdateField('priceToBeatIvAdaptiveRedBlock', value)}
          />
        </div>
      </div>
    </div>
  );
}

function IvRuleInput({
  label,
  value,
  placeholder,
  onChange,
}: {
  label: string;
  value: string;
  placeholder: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="space-y-1">
      <Label className="text-[10px] font-medium text-slate-500">{label}</Label>
      <Input
        type="number"
        step="any"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
      />
    </div>
  );
}

function IvRuleSelect({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: Array<{ label: string; value: string }>;
  onChange: (value: string) => void;
}) {
  return (
    <div className="space-y-1">
      <Label className="text-[10px] font-medium text-slate-500">{label}</Label>
      <Select value={value} onValueChange={onChange}>
        <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

function IvRuleCheckbox({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string | undefined;
  onChange: (value: string) => void;
}) {
  return (
    <label className="flex h-8 items-center gap-2 rounded border border-slate-200 bg-white px-2 text-[10px] font-medium text-slate-600">
      <input
        type="checkbox"
        className="h-3.5 w-3.5 rounded border-slate-300"
        checked={value === 'true'}
        onChange={(event) => onChange(event.currentTarget.checked ? 'true' : 'false')}
      />
      {label}
    </label>
  );
}

function updateRuleRow(
  rows: PtbIvTimeRuleRow[],
  rowId: string,
  patch: Partial<PtbIvTimeRuleRow>
): PtbIvTimeRuleRow[] {
  return rows.map((row) => (row.id === rowId ? { ...row, ...patch } : row));
}
