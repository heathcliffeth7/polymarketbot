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
      <div className="space-y-2 border-t border-slate-200 pt-2">
        <Label className="text-[11px] font-medium text-slate-600">Expected Move Floor</Label>
        <div className="grid grid-cols-2 gap-2">
          <IvRuleSelect
            label="Floor modu"
            value={fields.priceToBeatIvMinExpectedMoveMode || 'fixed'}
            onChange={(value) => onUpdateField('priceToBeatIvMinExpectedMoveMode', value)}
            options={[
              { label: 'Fixed', value: 'fixed' },
              { label: 'Adaptive', value: 'adaptive' },
            ]}
          />
          <IvRuleInput
            label="Base bps"
            value={fields.priceToBeatIvAdaptiveMinExpectedMoveBaseBps ?? ''}
            placeholder="1.5"
            onChange={(value) =>
              onUpdateField('priceToBeatIvAdaptiveMinExpectedMoveBaseBps', value)
            }
          />
          <IvRuleInput
            label="Min bps"
            value={fields.priceToBeatIvAdaptiveMinExpectedMoveMinBps ?? ''}
            placeholder="1.5"
            onChange={(value) => onUpdateField('priceToBeatIvAdaptiveMinExpectedMoveMinBps', value)}
          />
          <IvRuleInput
            label="Max bps"
            value={fields.priceToBeatIvAdaptiveMinExpectedMoveMaxBps ?? ''}
            placeholder="2.75"
            onChange={(value) => onUpdateField('priceToBeatIvAdaptiveMinExpectedMoveMaxBps', value)}
          />
          <IvRuleInput
            label="Disagree add"
            value={fields.priceToBeatIvAdaptiveDisagreementBpsAdd ?? ''}
            placeholder="0.25"
            onChange={(value) => onUpdateField('priceToBeatIvAdaptiveDisagreementBpsAdd', value)}
          />
          <IvRuleInput
            label="Strong add"
            value={fields.priceToBeatIvAdaptiveStrongDisagreementBpsAdd ?? ''}
            placeholder="0.50"
            onChange={(value) =>
              onUpdateField('priceToBeatIvAdaptiveStrongDisagreementBpsAdd', value)
            }
          />
          <IvRuleInput
            label="Spread add"
            value={fields.priceToBeatIvAdaptiveSpreadBpsAdd ?? ''}
            placeholder="0.20"
            onChange={(value) => onUpdateField('priceToBeatIvAdaptiveSpreadBpsAdd', value)}
          />
          <IvRuleInput
            label="Wide spread add"
            value={fields.priceToBeatIvAdaptiveWideSpreadBpsAdd ?? ''}
            placeholder="0.40"
            onChange={(value) => onUpdateField('priceToBeatIvAdaptiveWideSpreadBpsAdd', value)}
          />
          <IvRuleInput
            label="Stale add"
            value={fields.priceToBeatIvAdaptiveStaleBpsAdd ?? ''}
            placeholder="0.20"
            onChange={(value) => onUpdateField('priceToBeatIvAdaptiveStaleBpsAdd', value)}
          />
          <IvRuleInput
            label="Noise add"
            value={fields.priceToBeatIvAdaptiveNoiseBpsAdd ?? ''}
            placeholder="0.25"
            onChange={(value) => onUpdateField('priceToBeatIvAdaptiveNoiseBpsAdd', value)}
          />
        </div>
      </div>
      <div className="space-y-2 border-t border-slate-200 pt-2">
        <Label className="text-[11px] font-medium text-slate-600">EQ77 Risk Cap</Label>
        <div className="grid grid-cols-2 gap-2">
          <IvRuleCheckbox
            label="Risk cap aktif"
            value={fields.priceToBeatIvEq77RiskCapEnabled}
            onChange={(value) => onUpdateField('priceToBeatIvEq77RiskCapEnabled', value)}
          />
          <IvRuleCheckbox
            label="Recheck submit"
            value={fields.priceToBeatIvRecheckBeforeSubmit}
            onChange={(value) => onUpdateField('priceToBeatIvRecheckBeforeSubmit', value)}
          />
          <IvRuleCheckbox
            label="Wait for price"
            value={fields.priceToBeatIvWaitForPriceEnabled}
            onChange={(value) => onUpdateField('priceToBeatIvWaitForPriceEnabled', value)}
          />
          <IvRuleCheckbox
            label="Passive bid"
            value={fields.priceToBeatIvPassiveBidEnabled}
            onChange={(value) => onUpdateField('priceToBeatIvPassiveBidEnabled', value)}
          />
          <IvRuleInput
            label="Clean score max"
            value={fields.priceToBeatIvRiskScoreCleanMax ?? ''}
            placeholder="20"
            onChange={(value) => onUpdateField('priceToBeatIvRiskScoreCleanMax', value)}
          />
          <IvRuleInput
            label="Moderate score max"
            value={fields.priceToBeatIvRiskScoreModerateMax ?? ''}
            placeholder="45"
            onChange={(value) => onUpdateField('priceToBeatIvRiskScoreModerateMax', value)}
          />
          <IvRuleInput
            label="High score max"
            value={fields.priceToBeatIvRiskScoreHighMax ?? ''}
            placeholder="70"
            onChange={(value) => onUpdateField('priceToBeatIvRiskScoreHighMax', value)}
          />
          <IvRuleInput
            label="Moderate cap c"
            value={fields.priceToBeatIvModerateRiskMaxPriceCent ?? ''}
            placeholder="74"
            onChange={(value) => onUpdateField('priceToBeatIvModerateRiskMaxPriceCent', value)}
          />
          <IvRuleInput
            label="High cap c"
            value={fields.priceToBeatIvHighRiskMaxPriceCent ?? ''}
            placeholder="70"
            onChange={(value) => onUpdateField('priceToBeatIvHighRiskMaxPriceCent', value)}
          />
          <IvRuleInput
            label="Deep value c"
            value={fields.priceToBeatIvDeepValueMaxPriceCent ?? ''}
            placeholder="64"
            onChange={(value) => onUpdateField('priceToBeatIvDeepValueMaxPriceCent', value)}
          />
          <IvRuleInput
            label="Max haircut c"
            value={fields.priceToBeatIvMaxRiskHaircutCent ?? ''}
            placeholder="8"
            onChange={(value) => onUpdateField('priceToBeatIvMaxRiskHaircutCent', value)}
          />
          <IvRuleInput
            label="Odds spread c"
            value={fields.priceToBeatIvOddsMaxSpreadCent ?? ''}
            placeholder="5"
            onChange={(value) => onUpdateField('priceToBeatIvOddsMaxSpreadCent', value)}
          />
          <IvRuleInput
            label="CEX missing pts"
            value={fields.priceToBeatIvCexUnconfirmedRiskPoints ?? ''}
            placeholder="10"
            onChange={(value) => onUpdateField('priceToBeatIvCexUnconfirmedRiskPoints', value)}
          />
          <IvRuleInput
            label="CEX conflict pts"
            value={fields.priceToBeatIvCexConflictRiskPoints ?? ''}
            placeholder="10"
            onChange={(value) => onUpdateField('priceToBeatIvCexConflictRiskPoints', value)}
          />
          <IvRuleInput
            label="Passive TTL ms"
            value={fields.priceToBeatIvPassiveBidTtlMs ?? ''}
            placeholder="1500"
            onChange={(value) => onUpdateField('priceToBeatIvPassiveBidTtlMs', value)}
          />
        </div>
        <div className="mt-2 border-t border-slate-200 pt-2">
          <Label className="text-[11px] font-medium text-slate-500">Wait Reprice Guard</Label>
          <div className="mt-2 grid grid-cols-2 gap-2">
            <IvRuleCheckbox
              label="Wait reprice"
              value={fields.priceToBeatIvWaitRepriceGuardEnabled}
              onChange={(value) => onUpdateField('priceToBeatIvWaitRepriceGuardEnabled', value)}
            />
            <IvRuleCheckbox
              label="Falling cap"
              value={fields.priceToBeatIvFallingIntoCapGuardEnabled}
              onChange={(value) => onUpdateField('priceToBeatIvFallingIntoCapGuardEnabled', value)}
            />
            <IvRuleInput
              label="Wait early ms"
              value={fields.priceToBeatIvWaitMaxAgeMsEarly ?? ''}
              placeholder="8000"
              onChange={(value) => onUpdateField('priceToBeatIvWaitMaxAgeMsEarly', value)}
            />
            <IvRuleInput
              label="Wait mid ms"
              value={fields.priceToBeatIvWaitMaxAgeMsMid ?? ''}
              placeholder="5000"
              onChange={(value) => onUpdateField('priceToBeatIvWaitMaxAgeMsMid', value)}
            />
            <IvRuleInput
              label="Wait late ms"
              value={fields.priceToBeatIvWaitMaxAgeMsLate ?? ''}
              placeholder="3000"
              onChange={(value) => onUpdateField('priceToBeatIvWaitMaxAgeMsLate', value)}
            />
            <IvRuleInput
              label="Ask over cap c"
              value={fields.priceToBeatIvWaitInitialAskMaxOverCapCent ?? ''}
              placeholder="10"
              onChange={(value) => onUpdateField('priceToBeatIvWaitInitialAskMaxOverCapCent', value)}
            />
            <IvRuleInput
              label="Drop early c"
              value={fields.priceToBeatIvFallingIntoCapDropCentEarly ?? ''}
              placeholder="15"
              onChange={(value) => onUpdateField('priceToBeatIvFallingIntoCapDropCentEarly', value)}
            />
            <IvRuleInput
              label="Drop mid c"
              value={fields.priceToBeatIvFallingIntoCapDropCentMid ?? ''}
              placeholder="12"
              onChange={(value) => onUpdateField('priceToBeatIvFallingIntoCapDropCentMid', value)}
            />
            <IvRuleInput
              label="Drop late c"
              value={fields.priceToBeatIvFallingIntoCapDropCentLate ?? ''}
              placeholder="8"
              onChange={(value) => onUpdateField('priceToBeatIvFallingIntoCapDropCentLate', value)}
            />
            <IvRuleCheckbox
              label="Late expensive"
              value={fields.priceToBeatIvLateExpensiveEntryGuardEnabled}
              onChange={(value) => onUpdateField('priceToBeatIvLateExpensiveEntryGuardEnabled', value)}
            />
            <IvRuleInput
              label="Late seconds"
              value={fields.priceToBeatIvLateExpensiveSeconds ?? ''}
              placeholder="45"
              onChange={(value) => onUpdateField('priceToBeatIvLateExpensiveSeconds', value)}
            />
            <IvRuleInput
              label="Late VWAP c"
              value={fields.priceToBeatIvLateExpensiveVwapCent ?? ''}
              placeholder="70"
              onChange={(value) => onUpdateField('priceToBeatIvLateExpensiveVwapCent', value)}
            />
            <IvRuleInput
              label="Late q c"
              value={fields.priceToBeatIvLateExpensiveMinQCent ?? ''}
              placeholder="92"
              onChange={(value) => onUpdateField('priceToBeatIvLateExpensiveMinQCent', value)}
            />
            <IvRuleInput
              label="Late gap extra"
              value={fields.priceToBeatIvLateExpensiveMinGapStrengthExtra ?? ''}
              placeholder="0.50"
              onChange={(value) => onUpdateField('priceToBeatIvLateExpensiveMinGapStrengthExtra', value)}
            />
            <IvRuleCheckbox
              label="Mixed gap block"
              value={fields.priceToBeatIvGapFailMixedCexGuardEnabled}
              onChange={(value) => onUpdateField('priceToBeatIvGapFailMixedCexGuardEnabled', value)}
            />
            <IvRuleInput
              label="Mixed max sec"
              value={fields.priceToBeatIvGapFailMixedCexMaxSeconds ?? ''}
              placeholder="120"
              onChange={(value) => onUpdateField('priceToBeatIvGapFailMixedCexMaxSeconds', value)}
            />
            <IvRuleCheckbox
              label="Late mixed"
              value={fields.priceToBeatIvLateExpensiveMixedCexGuardEnabled}
              onChange={(value) => onUpdateField('priceToBeatIvLateExpensiveMixedCexGuardEnabled', value)}
            />
            <IvRuleInput
              label="Late mixed sec"
              value={fields.priceToBeatIvLateExpensiveMixedCexSeconds ?? ''}
              placeholder="45"
              onChange={(value) => onUpdateField('priceToBeatIvLateExpensiveMixedCexSeconds', value)}
            />
            <IvRuleInput
              label="Late mixed VWAP c"
              value={fields.priceToBeatIvLateExpensiveMixedCexMinVwapCent ?? ''}
              placeholder="70"
              onChange={(value) => onUpdateField('priceToBeatIvLateExpensiveMixedCexMinVwapCent', value)}
            />
            <IvRuleCheckbox
              label="Late needs fail"
              value={fields.priceToBeatIvLateExpensiveMixedCexRequireGapFailOrLagHigh}
              onChange={(value) => onUpdateField('priceToBeatIvLateExpensiveMixedCexRequireGapFailOrLagHigh', value)}
            />
          </div>
        </div>
        <div className="mt-2 border-t border-slate-200 pt-2">
          <Label className="text-[11px] font-medium text-slate-500">
            Oracle Lag / Pump Shock
          </Label>
          <div className="mt-2 grid grid-cols-2 gap-2">
            <IvRuleCheckbox
              label="Oracle lag"
              value={fields.priceToBeatIvOracleLagBookLeadGuardEnabled}
              onChange={(value) => onUpdateField('priceToBeatIvOracleLagBookLeadGuardEnabled', value)}
            />
            <IvRuleCheckbox
              label="Best ask fallback"
              value={fields.priceToBeatIvOracleLagUseBestAskFallback}
              onChange={(value) => onUpdateField('priceToBeatIvOracleLagUseBestAskFallback', value)}
            />
            <IvRuleInput
              label="High q c"
              value={fields.priceToBeatIvOracleLagQHighCent ?? ''}
              placeholder="70"
              onChange={(value) => onUpdateField('priceToBeatIvOracleLagQHighCent', value)}
            />
            <IvRuleInput
              label="Cheap high c"
              value={fields.priceToBeatIvOracleLagCheapTokenCent ?? ''}
              placeholder="70"
              onChange={(value) => onUpdateField('priceToBeatIvOracleLagCheapTokenCent', value)}
            />
            <IvRuleInput
              label="High disloc c"
              value={fields.priceToBeatIvModelBookDislocationHighCent ?? ''}
              placeholder="20"
              onChange={(value) => onUpdateField('priceToBeatIvModelBookDislocationHighCent', value)}
            />
            <IvRuleCheckbox
              label="Lag no-book"
              value={fields.priceToBeatIvChainlinkCexLagNoBookGuardEnabled}
              onChange={(value) => onUpdateField('priceToBeatIvChainlinkCexLagNoBookGuardEnabled', value)}
            />
            <IvRuleInput
              label="No-book sec"
              value={fields.priceToBeatIvChainlinkCexLagNoBookMaxSeconds ?? ''}
              placeholder="45"
              onChange={(value) => onUpdateField('priceToBeatIvChainlinkCexLagNoBookMaxSeconds', value)}
            />
            <IvRuleCheckbox
              label="No-book non-strong"
              value={fields.priceToBeatIvChainlinkCexLagNoBookRequireNonStrongCex}
              onChange={(value) => onUpdateField('priceToBeatIvChainlinkCexLagNoBookRequireNonStrongCex', value)}
            />
            <IvRuleInput
              label="Extreme q c"
              value={fields.priceToBeatIvOracleLagQExtremeCent ?? ''}
              placeholder="98"
              onChange={(value) => onUpdateField('priceToBeatIvOracleLagQExtremeCent', value)}
            />
            <IvRuleInput
              label="Cheap token c"
              value={fields.priceToBeatIvOracleLagCheapTokenExtremeCent ?? ''}
              placeholder="72"
              onChange={(value) => onUpdateField('priceToBeatIvOracleLagCheapTokenExtremeCent', value)}
            />
            <IvRuleInput
              label="Red disloc c"
              value={fields.priceToBeatIvModelBookDislocationRedCent ?? ''}
              placeholder="25"
              onChange={(value) => onUpdateField('priceToBeatIvModelBookDislocationRedCent', value)}
            />
            <IvRuleCheckbox
              label="VWAP required"
              value={fields.priceToBeatIvExecutionVwapRequiredOnHighDislocation}
              onChange={(value) => onUpdateField('priceToBeatIvExecutionVwapRequiredOnHighDislocation', value)}
            />
            <IvRuleCheckbox
              label="Borderline block"
              value={fields.priceToBeatIvBorderlinePumpBookLeadGuardEnabled}
              onChange={(value) => onUpdateField('priceToBeatIvBorderlinePumpBookLeadGuardEnabled', value)}
            />
            <IvRuleInput
              label="Border margin"
              value={fields.priceToBeatIvBorderlineGapMarginEarly ?? ''}
              placeholder="0.10"
              onChange={(value) => onUpdateField('priceToBeatIvBorderlineGapMarginEarly', value)}
            />
            <IvRuleInput
              label="Border pump"
              value={fields.priceToBeatIvBorderlinePumpShockRatio ?? ''}
              placeholder="1.25"
              onChange={(value) => onUpdateField('priceToBeatIvBorderlinePumpShockRatio', value)}
            />
            <IvRuleInput
              label="Border q c"
              value={fields.priceToBeatIvBorderlineBookLeadQMinCent ?? ''}
              placeholder="95"
              onChange={(value) => onUpdateField('priceToBeatIvBorderlineBookLeadQMinCent', value)}
            />
            <IvRuleInput
              label="Border cheap c"
              value={fields.priceToBeatIvBorderlineBookLeadCheapTokenCent ?? ''}
              placeholder="60"
              onChange={(value) => onUpdateField('priceToBeatIvBorderlineBookLeadCheapTokenCent', value)}
            />
            <IvRuleInput
              label="Border disloc c"
              value={fields.priceToBeatIvBorderlineBookLeadDislocationCent ?? ''}
              placeholder="30"
              onChange={(value) => onUpdateField('priceToBeatIvBorderlineBookLeadDislocationCent', value)}
            />
            <IvRuleCheckbox
              label="Pump shock"
              value={fields.priceToBeatIvPumpShockGuardEnabled}
              onChange={(value) => onUpdateField('priceToBeatIvPumpShockGuardEnabled', value)}
            />
            <IvRuleInput
              label="Pump ratio"
              value={fields.priceToBeatIvPumpShockGapGrowthRatio ?? ''}
              placeholder="1.25"
              onChange={(value) => onUpdateField('priceToBeatIvPumpShockGapGrowthRatio', value)}
            />
            <IvRuleInput
              label="Pump hard"
              value={fields.priceToBeatIvPumpShockHardRatio ?? ''}
              placeholder="1.50"
              onChange={(value) => onUpdateField('priceToBeatIvPumpShockHardRatio', value)}
            />
            <IvRuleInput
              label="Pump hold ms"
              value={fields.priceToBeatIvPumpShockMinHoldMs ?? ''}
              placeholder="3000"
              onChange={(value) => onUpdateField('priceToBeatIvPumpShockMinHoldMs', value)}
            />
            <IvRuleInput
              label="Pump retain"
              value={fields.priceToBeatIvPumpShockMinBufferRetain ?? ''}
              placeholder="0.80"
              onChange={(value) => onUpdateField('priceToBeatIvPumpShockMinBufferRetain', value)}
            />
          </div>
          <div className="mt-2 border-t border-slate-200 pt-2">
            <Label className="text-[11px] font-medium text-slate-500">PTB Chop</Label>
            <div className="mt-2 grid grid-cols-2 gap-2">
              <IvRuleCheckbox
                label="Chop guard"
                value={fields.priceToBeatIvPtbChopGuardEnabled}
                onChange={(value) => onUpdateField('priceToBeatIvPtbChopGuardEnabled', value)}
              />
              <IvRuleInput
                label="Lookback sn"
                value={fields.priceToBeatIvPtbChopLookbackSeconds ?? ''}
                placeholder="10"
                onChange={(value) => onUpdateField('priceToBeatIvPtbChopLookbackSeconds', value)}
              />
              <IvRuleInput
                label="Ext lookback sn"
                value={fields.priceToBeatIvPtbChopExtendedLookbackSeconds ?? ''}
                placeholder="15"
                onChange={(value) =>
                  onUpdateField('priceToBeatIvPtbChopExtendedLookbackSeconds', value)
                }
              />
              <IvRuleInput
                label="Deadband bps"
                value={fields.priceToBeatIvPtbChopDeadbandBps ?? ''}
                placeholder="0.5"
                onChange={(value) => onUpdateField('priceToBeatIvPtbChopDeadbandBps', value)}
              />
              <IvRuleInput
                label="BTC floor USD"
                value={fields.priceToBeatIvPtbChopDeadbandMinUsdBtc ?? ''}
                placeholder="5"
                onChange={(value) =>
                  onUpdateField('priceToBeatIvPtbChopDeadbandMinUsdBtc', value)
                }
              />
              <IvRuleInput
                label="ETH floor USD"
                value={fields.priceToBeatIvPtbChopDeadbandMinUsdEth ?? ''}
                placeholder="0.30"
                onChange={(value) =>
                  onUpdateField('priceToBeatIvPtbChopDeadbandMinUsdEth', value)
                }
              />
              <IvRuleInput
                label="SOL floor USD"
                value={fields.priceToBeatIvPtbChopDeadbandMinUsdSol ?? ''}
                placeholder="0.03"
                onChange={(value) =>
                  onUpdateField('priceToBeatIvPtbChopDeadbandMinUsdSol', value)
                }
              />
              <IvRuleInput
                label="Cross 10s block"
                value={fields.priceToBeatIvPtbChopZeroCrossBlock10s ?? ''}
                placeholder="2"
                onChange={(value) => onUpdateField('priceToBeatIvPtbChopZeroCrossBlock10s', value)}
              />
              <IvRuleInput
                label="Cross 15s block"
                value={fields.priceToBeatIvPtbChopZeroCrossBlock15s ?? ''}
                placeholder="3"
                onChange={(value) => onUpdateField('priceToBeatIvPtbChopZeroCrossBlock15s', value)}
              />
              <IvRuleInput
                label="Path z warn"
                value={fields.priceToBeatIvPtbChopPathZWarn ?? ''}
                placeholder="1.25"
                onChange={(value) => onUpdateField('priceToBeatIvPtbChopPathZWarn', value)}
              />
              <IvRuleInput
                label="Path z block"
                value={fields.priceToBeatIvPtbChopPathZBlock ?? ''}
                placeholder="1.75"
                onChange={(value) => onUpdateField('priceToBeatIvPtbChopPathZBlock', value)}
              />
              <IvRuleInput
                label="Efficiency warn"
                value={fields.priceToBeatIvPtbChopEfficiencyWarn ?? ''}
                placeholder="0.25"
                onChange={(value) => onUpdateField('priceToBeatIvPtbChopEfficiencyWarn', value)}
              />
              <IvRuleInput
                label="Efficiency block"
                value={fields.priceToBeatIvPtbChopEfficiencyBlock ?? ''}
                placeholder="0.15"
                onChange={(value) => onUpdateField('priceToBeatIvPtbChopEfficiencyBlock', value)}
              />
              <IvRuleInput
                label="Opp depth warn"
                value={fields.priceToBeatIvPtbChopOppositeDepthZWarn ?? ''}
                placeholder="0.50"
                onChange={(value) =>
                  onUpdateField('priceToBeatIvPtbChopOppositeDepthZWarn', value)
                }
              />
              <IvRuleInput
                label="Opp depth block"
                value={fields.priceToBeatIvPtbChopOppositeDepthZBlock ?? ''}
                placeholder="0.90"
                onChange={(value) =>
                  onUpdateField('priceToBeatIvPtbChopOppositeDepthZBlock', value)
                }
              />
              <IvRuleInput
                label="Penalty cap"
                value={fields.priceToBeatIvPtbChopMaxGapStrengthPenalty ?? ''}
                placeholder="0.35"
                onChange={(value) =>
                  onUpdateField('priceToBeatIvPtbChopMaxGapStrengthPenalty', value)
                }
              />
            </div>
          </div>
        </div>
      </div>
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
            label="Depth hard block"
            value={fields.priceToBeatIvDepthGuardHardBlockEnabled}
            onChange={(value) => onUpdateField('priceToBeatIvDepthGuardHardBlockEnabled', value)}
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
