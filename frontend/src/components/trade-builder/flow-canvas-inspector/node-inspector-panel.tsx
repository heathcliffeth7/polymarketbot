import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Separator } from '@/components/ui/separator';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { NODE_FIELD_SCHEMAS, createEmptyExitLadderRuleRow, createEmptyPtbStopLossRuleRow, createEmptyTimeExitRuleRow, type ExitLadderRuleRow, isPresetBuySellPlaceOrderMarker, isPresetPlaceOrderMarker, type PtbStopLossRuleRow, type TimeExitRuleRow } from '@/lib/trade-flow-config-mappers';
import { NODE_FIELD_HELP_CONTENT, NODE_TYPE_OPTIONS } from '../flow-canvas-constants';
import { normalizeDateTimeInput } from '../flow-canvas-utils';
import { Settings2, Trash2, Plus, Zap } from 'lucide-react';
import { EMPTY_SELECT_SENTINEL } from './shared';
import { ExitLadderSection, PtbStopLossRuleSection } from './exit-sections';
import { ExecutionFloorProtectionSection } from './execution-floor-protection-section';
import { MaxPriceProtectionSection } from './max-price-protection-section';
import { PairLockAutoPreviewSection } from './pair-lock-auto-preview-section';
import { PriceToBeatMaxPriceRelaxSection } from './price-to-beat-max-price-relax-section';
import { PriceToBeatStopLossBumpSection } from './price-to-beat-stop-loss-bump-section';
import { TimeExitRulesSection } from './time-exit-rules-section';
import { PairLockSummarySection, TriggerPairLockHint } from './pair-lock-binding-section';
import { PairLockStaleConfigSection } from './pair-lock-stale-config-section';
import {
  isPairLockField,
  isPairLockIncompatibleField,
  resolvePairLockCounterPtbVisibility,
  resolvePairLockSizingFieldVisibility,
  resolvePairLockStopLossFieldVisibility,
  resolvePairLockUiState,
} from './pair-lock-inspector';
import { DrawdownRulesSection, ExpressionSection, OpenPositionsSection, OutcomeConditionsSection, StatePatchSection } from './sections';
import type { NodeInspectorPanelProps } from './types';

export function NodeInspectorPanel({
  form,
  nodeKeyDraft,
  nodeTypeDraft,
  tab,
  openPositions,
  openPositionsMeta,
  openPositionsLoading,
  openPositionApplyingKey,
  canApplyOpenPosition,
  marketOutcomes,
  marketOutcomesLoading,
  upstreamAutoScope,
  upstreamHasTriggerPrice,
  upstreamMaxPriceResolution,
  upstreamPairLockTrigger,
  userTelegramBotTokenMasked,
  userTelegramDefaultChatId,
  actions,
}: NodeInspectorPanelProps) {
  const [openFieldHelpState, setOpenFieldHelpState] = useState<{ nodeType: string; key: string } | null>(null);
  const nodeSchema = NODE_FIELD_SCHEMAS[nodeTypeDraft] || [];
  const nodeFieldHelp = NODE_FIELD_HELP_CONTENT[nodeTypeDraft] || {};
  const placeOrderSizeMode = (form.fields.sizeMode ?? '').trim().toLowerCase();
  const dualDcaBaseSizing = (form.fields.baseSizing ?? '').trim().toLowerCase();
  const triggerMarketMode = (form.fields.marketMode ?? '').trim().toLowerCase();
  const { triggerBindingMode, placeOrderPairLockEnabled, placeOrderCounterOutcomePreview } =
    resolvePairLockUiState(nodeTypeDraft, form.fields);
  const triggerRepeatMode = (form.fields.repeatMode ?? '').trim().toLowerCase();
  const triggerCycleWindowMode = (form.fields.cycleWindowMode ?? '').trim().toLowerCase();
  const triggerPriceToBeatEnabled = (form.fields.priceToBeatTriggerEnabled ?? '').toString().trim().toLowerCase() === 'true';
  const triggerPriceToBeatModeRaw = (form.fields.priceToBeatMode ?? '').toString().trim().toLowerCase();
  const triggerPriceToBeatMode = triggerPriceToBeatModeRaw === 'auto_last_3_avg_excursion' || triggerPriceToBeatModeRaw === 'auto_vol_pct' ? triggerPriceToBeatModeRaw : 'manual';
  const placeOrderMaxTriggersRaw = Number(form.fields.maxTriggers ?? '');
  const placeOrderMaxTriggers = Number.isFinite(placeOrderMaxTriggersRaw) && placeOrderMaxTriggersRaw > 0 ? Math.min(20, Math.floor(placeOrderMaxTriggersRaw)) : 1;
  const placeOrderTriggerRows = form.triggerSizeRows || [];
  const placeOrderTriggerNumericRows = placeOrderTriggerRows.map((raw) => {
    const trimmed = raw.trim();
    if (!trimmed) return null;
    const parsed = Number(trimmed);
    return Number.isFinite(parsed) ? parsed : null;
  });
  const placeOrderTriggerSum = placeOrderTriggerNumericRows.reduce<number>(
    (sum, value) => (typeof value === 'number' ? sum + value : sum),
    0
  );
  const placeOrderTriggerRowInvalid = placeOrderTriggerNumericRows.some((value, index) => {
    const raw = placeOrderTriggerRows[index]?.trim() ?? '';
    if (!raw) return false;
    return value == null || value <= 0;
  });
  const placeOrderTriggerSumInvalid =
    nodeTypeDraft === 'action.place_order' &&
    placeOrderSizeMode === 'pct' &&
    placeOrderTriggerRows.some((row) => row.trim().length > 0) &&
    placeOrderTriggerSum > 100.000001;
  const isPresetPlaceOrder =
    nodeTypeDraft === 'action.place_order' &&
    isPresetPlaceOrderMarker(
      form.fields.presetKind,
      form.fields.refKey
    );
  const isPresetBuySellPlaceOrder =
    nodeTypeDraft === 'action.place_order' &&
    isPresetBuySellPlaceOrderMarker(
      form.fields.presetKind,
      form.fields.refKey
    );
  const placeOrderSide = nodeTypeDraft === 'action.place_order' ? (form.fields.side ?? '').toString().trim().toLowerCase() : '';
  const placeOrderTpEnabled = nodeTypeDraft === 'action.place_order' ? (form.fields.tpEnabled ?? '').toString().trim().toLowerCase() === 'true' : false;
  const placeOrderSlEnabled = nodeTypeDraft === 'action.place_order' ? (form.fields.slEnabled ?? '').toString().trim().toLowerCase() === 'true' : false;
  const placeOrderTpRuleRows = form.tpRuleRows || [], placeOrderSlRuleRows = form.slRuleRows || [], placeOrderPtbStopLossRuleRows = form.ptbStopLossRuleRows || [], placeOrderTimeExitRuleRows = form.timeExitRuleRows || [];
  const ptbStopLossChecked = nodeTypeDraft === 'action.place_order' ? (form.fields.ptbStopLossEnabled ?? '').toString().trim().toLowerCase() === 'true' : false;
  const placeOrderHasAnyStopLossProtection =
    nodeTypeDraft === 'action.place_order' &&
    placeOrderSide === 'buy' &&
    (placeOrderSlEnabled ||
      ptbStopLossChecked ||
      placeOrderPtbStopLossRuleRows.length > 0);
  const showTpLadderSection = nodeTypeDraft === 'action.place_order' && !placeOrderPairLockEnabled && placeOrderSide === 'buy' && (placeOrderTpEnabled || placeOrderTpRuleRows.length > 0);
  const showSlLadderSection = nodeTypeDraft === 'action.place_order' && !placeOrderPairLockEnabled && placeOrderSide === 'buy' && (placeOrderSlEnabled || placeOrderSlRuleRows.length > 0);
  const showPtbStopLossSection = nodeTypeDraft === 'action.place_order' && placeOrderSide === 'buy' && !placeOrderPairLockEnabled;
  const showTimeExitSection = nodeTypeDraft === 'action.place_order' && placeOrderSide === 'buy' && !placeOrderPairLockEnabled;
  const placeOrderMaxPriceCentValue =
    nodeTypeDraft === 'action.place_order' ? (form.fields.maxPriceCent ?? '').toString().trim() : '';
  const placeOrderExecutionFloorPriceCentValue = nodeTypeDraft === 'action.place_order' ? (form.fields.executionFloorPriceCent ?? '').toString().trim() : '';
  const placeOrderMaxPriceUi = form.placeOrderMaxPriceUi;
  const placeOrderMarketSeedUi = form.placeOrderMarketSeedUi;
  const placeOrderHasInheritedMaxPrice = placeOrderMaxPriceUi?.isInheritedValue === true;
  const placeOrderHasAmbiguousUpstreamMaxPrice = nodeTypeDraft === 'action.place_order' && upstreamMaxPriceResolution.kind === 'multiple';
  const placeOrderHasStaleLocalMaxPrice = nodeTypeDraft === 'action.place_order' && !placeOrderHasInheritedMaxPrice && placeOrderMaxPriceCentValue.length > 0 && upstreamMaxPriceResolution.kind === 'single' && upstreamMaxPriceResolution.maxPriceCent != null && upstreamMaxPriceResolution.maxPriceCent !== placeOrderMaxPriceCentValue;
  const placeOrderReentryChecked =
    (form.fields.reenterOnSlHit ?? '').toString().trim().toLowerCase() === 'true';
  const placeOrderReentryMinPriceCentValue =
    nodeTypeDraft === 'action.place_order'
      ? (form.fields.reentryMinPriceCent ?? '').toString().trim()
      : '';
  const placeOrderReentryMaxPriceCentValue =
    nodeTypeDraft === 'action.place_order'
      ? (form.fields.reentryMaxPriceCent ?? '').toString().trim()
      : '';
  const placeOrderReentryPriceToBeatMaxDiffValue = nodeTypeDraft === 'action.place_order' ? (form.fields.reentryPriceToBeatMaxDiff ?? '').toString().trim() : '';
  const placeOrderTriggerGuardChecked =
    (form.fields.triggerPriceGuardEnabled ?? '').toString().trim().toLowerCase() === 'true';
  const reentryTriggerGuardActive =
    nodeTypeDraft === 'action.place_order' &&
    placeOrderSide === 'buy' &&
    placeOrderReentryChecked &&
    placeOrderReentryMinPriceCentValue.length > 0;
  const triggerGuardProtectionActive =
    placeOrderTriggerGuardChecked || reentryTriggerGuardActive;
  const triggerGuardRetryChecked =
    (form.fields.retryOnTriggerPriceGuardBlock ?? '').toString().trim().toLowerCase() === 'true';
  const executionFloorGuardChecked =
    (form.fields.executionFloorGuardEnabled ?? '').toString().trim().toLowerCase() === 'true';
  const executionFloorRetryChecked =
    (form.fields.retryOnExecutionFloorGuardBlock ?? '').toString().trim().toLowerCase() === 'true';
  const parsedExecutionFloorPriceCent = Number(placeOrderExecutionFloorPriceCentValue);
  const hasManualExecutionFloorPrice = nodeTypeDraft === 'action.place_order' && Number.isFinite(parsedExecutionFloorPriceCent) && parsedExecutionFloorPriceCent > 0 && parsedExecutionFloorPriceCent <= 100;
  const reentryMaxPriceProtectionActive =
    nodeTypeDraft === 'action.place_order' &&
    placeOrderSide === 'buy' &&
    placeOrderReentryChecked &&
    placeOrderReentryMaxPriceCentValue.length > 0;
  const maxPriceProtectionActive = nodeTypeDraft === 'action.place_order' && placeOrderSide === 'buy' && (placeOrderMaxPriceCentValue.length > 0 || (placeOrderMaxPriceUi?.upstreamKind === 'single' && placeOrderMaxPriceUi.upstreamMaxPriceCent != null) || reentryMaxPriceProtectionActive);
  const maxPriceNotifyChecked =
    (form.fields.notifyOnMaxPriceBlocked ?? '').toString().trim().toLowerCase() === 'true';
  const maxPriceRetryChecked =
    (form.fields.retryOnMaxPriceBlock ?? '').toString().trim().toLowerCase() === 'true';
  const priceToBeatGuardChecked =
    (form.fields.priceToBeatGuardEnabled ?? '').toString().trim().toLowerCase() === 'true';
  const priceToBeatRetryChecked =
    (form.fields.retryOnPriceToBeatGuardBlock ?? '').toString().trim().toLowerCase() === 'true';
  const priceToBeatGuardModeRaw = (form.fields.priceToBeatMode ?? '').toString().trim().toLowerCase();
  const priceToBeatGuardMode =
    priceToBeatGuardModeRaw === 'auto_last_3_avg_excursion' ||
    priceToBeatGuardModeRaw === 'auto_vol_pct'
      ? priceToBeatGuardModeRaw
      : 'manual';
  const priceToBeatGuardUnit =
    (form.fields.priceToBeatMaxDiffUnit ?? '').toString().trim().toLowerCase() === 'cent'
      ? 'cent'
      : 'usd';
  const priceToBeatStopLossBumpChecked =
    (form.fields.priceToBeatStopLossBumpEnabled ?? '').toString().trim().toLowerCase() === 'true';
  const priceToBeatStopLossBumpUnitRaw =
    (form.fields.priceToBeatStopLossBumpUnit ?? '').toString().trim().toLowerCase();
  const priceToBeatStopLossBumpUnit =
    priceToBeatStopLossBumpUnitRaw === 'usd' || priceToBeatStopLossBumpUnitRaw === 'cent'
      ? priceToBeatStopLossBumpUnitRaw
      : priceToBeatGuardUnit;
  const priceToBeatStopLossBumpScope =
    (form.fields.priceToBeatStopLossBumpScope ?? '').toString().trim().toLowerCase() === 'global'
      ? 'global'
      : 'per_scope';
  const priceToBeatMaxPriceRelaxMinUnitRaw =
    (form.fields.priceToBeatMaxPriceRelaxMinUnit ?? '').toString().trim().toLowerCase();
  const priceToBeatMaxPriceRelaxMinUnit =
    priceToBeatMaxPriceRelaxMinUnitRaw === 'usd' || priceToBeatMaxPriceRelaxMinUnitRaw === 'cent'
      ? priceToBeatMaxPriceRelaxMinUnitRaw
      : 'usd';
  const priceToBeatMaxPriceRelaxStepModeRaw =
    (form.fields.priceToBeatMaxPriceRelaxStepMode ?? '').toString().trim().toLowerCase();
  const priceToBeatMaxPriceRelaxStepMode =
    priceToBeatMaxPriceRelaxStepModeRaw === 'absolute' ? 'absolute' : 'percent';
  const priceToBeatMaxPriceRelaxStepUnitRaw =
    (form.fields.priceToBeatMaxPriceRelaxStepUnit ?? '').toString().trim().toLowerCase();
  const priceToBeatMaxPriceRelaxStepUnit =
    priceToBeatMaxPriceRelaxStepUnitRaw === 'cent' ? 'cent' : 'usd';
  const ptbStopLossTimeDecayModeRaw =
    (form.fields.ptbStopLossTimeDecayMode ?? '').toString().trim().toLowerCase();
  const ptbStopLossTimeDecayMode =
    ptbStopLossTimeDecayModeRaw === 'none' || ptbStopLossTimeDecayModeRaw === 'relax'
      ? ptbStopLossTimeDecayModeRaw
      : 'tighten';
  const reentryPriceToBeatOverrideUnitRaw = nodeTypeDraft === 'action.place_order'
    ? (form.fields.reentryPriceToBeatMaxDiffUnit ?? '').toString().trim().toLowerCase()
    : '';
  const reentryPriceToBeatOverrideUnit =
    reentryPriceToBeatOverrideUnitRaw === 'usd' || reentryPriceToBeatOverrideUnitRaw === 'cent'
      ? reentryPriceToBeatOverrideUnitRaw
      : priceToBeatGuardMode === 'manual'
        ? priceToBeatGuardUnit
        : '';
  const showDedicatedTriggerGuard =
    nodeTypeDraft === 'action.place_order' && placeOrderSide === 'buy';
  const triggerGuardDisabled =
    !upstreamHasTriggerPrice && !placeOrderTriggerGuardChecked;
  const executionFloorGuardDisabled = !upstreamHasTriggerPrice && !hasManualExecutionFloorPrice && !executionFloorGuardChecked;
  const hideAutoScopePlaceOrderOutcomeFields =
    isPresetPlaceOrder && upstreamAutoScope && placeOrderSide === 'buy';
  const supportsOpenPositionPicker =
    nodeTypeDraft === 'trigger.open_positions' || nodeTypeDraft === 'action.place_order';
  const telegramLegacyBotToken = (form.fields.botToken ?? '').trim();
  const telegramUserBotToken = (userTelegramBotTokenMasked ?? '').trim();
  const telegramNodeChatId = (form.fields.chatId ?? '').trim();
  const telegramUserDefaultChatId = (userTelegramDefaultChatId ?? '').trim();
  const telegramBotTokenMasked = telegramUserBotToken;
  const telegramBotTokenSource = telegramUserBotToken
    ? 'user'
    : telegramLegacyBotToken
      ? 'legacy_ignored'
      : 'missing';
  const updateTpRuleRows = (updater: (rows: ExitLadderRuleRow[]) => ExitLadderRuleRow[]) => {
    actions.onFormChange((prev) =>
      prev
        ? {
            ...prev,
            fields: { ...prev.fields, tpEnabled: 'true' },
            tpRuleRows: updater([...(prev.tpRuleRows || [])]),
          }
        : prev
    );
  };
  const updateSlRuleRows = (updater: (rows: ExitLadderRuleRow[]) => ExitLadderRuleRow[]) => {
    actions.onFormChange((prev) =>
      prev
        ? {
            ...prev,
            fields: { ...prev.fields, slEnabled: 'true' },
            slRuleRows: updater([...(prev.slRuleRows || [])]),
          }
        : prev
    );
  };
  const updatePtbStopLossRuleRows = (
    updater: (rows: PtbStopLossRuleRow[]) => PtbStopLossRuleRow[]
  ) => {
    actions.onFormChange((prev) =>
      prev
        ? {
            ...prev,
            ptbStopLossRuleRows: updater([...(prev.ptbStopLossRuleRows || [])]),
          }
        : prev
    );
  };
  const updateTimeExitRuleRows = (updater: (rows: TimeExitRuleRow[]) => TimeExitRuleRow[]) => {
    actions.onFormChange((prev) =>
      prev
        ? {
            ...prev,
            timeExitRuleRows: updater([...(prev.timeExitRuleRows || [])]),
          }
        : prev
    );
  };
  const visibleNodeSchema = nodeSchema.filter((field) => {
    if (nodeTypeDraft === 'action.place_order') {
      if (field.key === 'sizeMode') return !placeOrderPairLockEnabled;
      if (field.key === 'sizePct') return !placeOrderPairLockEnabled && placeOrderSizeMode === 'pct';
      if (field.key === 'sizeUsdc') {
        return placeOrderPairLockEnabled || placeOrderSizeMode !== 'pct';
      }
      const pairLockSizingVisibility = resolvePairLockSizingFieldVisibility(
        field.key,
        placeOrderPairLockEnabled,
        form.fields
      );
      if (pairLockSizingVisibility != null) {
        return pairLockSizingVisibility;
      }
      const pairLockCounterPtbVisibility = resolvePairLockCounterPtbVisibility(
        field.key,
        placeOrderPairLockEnabled,
        form.fields
      );
      if (pairLockCounterPtbVisibility != null) {
        return pairLockCounterPtbVisibility;
      }
      const pairLockStopLossVisibility = resolvePairLockStopLossFieldVisibility(field.key, placeOrderPairLockEnabled, form.fields);
      if (pairLockStopLossVisibility != null) return pairLockStopLossVisibility;
      if (isPairLockField(field.key)) {
        return placeOrderPairLockEnabled;
      }
      if (placeOrderPairLockEnabled && isPairLockIncompatibleField(field.key)) {
        return false;
      }
      if (
        isPresetPlaceOrder &&
        (field.key === 'kind' ||
          field.key === 'triggerCondition' ||
          field.key === 'triggerPrice' ||
          field.key === 'triggerPriceCent')
      ) {
        return false;
      }
      if (
        hideAutoScopePlaceOrderOutcomeFields &&
        (field.key === 'marketSlug' || field.key === 'tokenId' || field.key === 'outcomeLabel')
      ) {
        return false;
      }
      if (field.key === 'tpEnabled') {
        return placeOrderSide === 'buy';
      }
      if (field.key === 'tpPriceCent') {
        const tpEnabled = (form.fields.tpEnabled ?? '').toString().trim().toLowerCase();
        return placeOrderSide === 'buy' && tpEnabled === 'true';
      }
      if (field.key === 'slEnabled') {
        return placeOrderSide === 'buy';
      }
      if (field.key === 'slPriceCent') {
        const slEnabled = (form.fields.slEnabled ?? '').toString().trim().toLowerCase();
        return placeOrderSide === 'buy' && slEnabled === 'true';
      }
      if (field.key === 'slTriggerPriceMode') {
        const slEnabled = (form.fields.slEnabled ?? '').toString().trim().toLowerCase();
        return placeOrderSide === 'buy' && slEnabled === 'true';
      }
      if (field.key === 'reenterOnSlHit') {
        return placeOrderHasAnyStopLossProtection;
      }
      if (field.key === 'stagedSlReentryOnlyAfterAllStages') {
        return (
          placeOrderSide === 'buy' &&
          (placeOrderSlRuleRows.length > 0 ||
            placeOrderPtbStopLossRuleRows.length > 0) &&
          placeOrderReentryChecked
        );
      }
      if (field.key === 'reentryMaxAttempts') {
        const reenterOnSlHit = (form.fields.reenterOnSlHit ?? '').toString().trim().toLowerCase();
        return placeOrderHasAnyStopLossProtection && reenterOnSlHit === 'true';
      }
      if (field.key === 'reentryMinPriceCent' || field.key === 'reentryMaxPriceCent') {
        const reenterOnSlHit = (form.fields.reenterOnSlHit ?? '').toString().trim().toLowerCase();
        return placeOrderHasAnyStopLossProtection && reenterOnSlHit === 'true';
      }
      if (field.key === 'notifyOnTriggerPriceBlocked') {
        return placeOrderSide === 'buy' && triggerGuardProtectionActive;
      }
      if (field.key === 'notifyOnExecutionFloorBlocked') {
        return placeOrderSide === 'buy' && executionFloorGuardChecked;
      }
      if (field.key === 'notifyOnMaxPriceBlocked' || field.key === 'retryOnMaxPriceBlock') {
        return false;
      }
      if (field.key === 'notifyOnTpHit') {
        const tpEnabled = (form.fields.tpEnabled ?? '').toString().trim().toLowerCase();
        return placeOrderSide === 'buy' && tpEnabled === 'true';
      }
      if (field.key === 'notifyOnSlHit') {
        return placeOrderHasAnyStopLossProtection;
      }
      if (
        field.key === 'ptbStopLossEnabled' ||
        field.key === 'ptbStopLossGapUsd' ||
        field.key === 'triggerPriceGuardEnabled' ||
        field.key === 'retryOnTriggerPriceGuardBlock' ||
        field.key === 'executionFloorGuardEnabled' ||
        field.key === 'executionFloorPriceCent' ||
        field.key === 'retryOnExecutionFloorGuardBlock' ||
        field.key === 'priceToBeatGuardEnabled' ||
        field.key === 'priceToBeatMode' ||
        field.key === 'priceToBeatMaxDiff' ||
        field.key === 'priceToBeatMaxDiffUnit' ||
        field.key === 'priceToBeatStopLossBumpEnabled' ||
        field.key === 'priceToBeatStopLossBumpAmount' ||
        field.key === 'priceToBeatStopLossBumpUnit' ||
        field.key === 'priceToBeatStopLossBumpScope' ||
        field.key === 'priceToBeatStopLossBumpDecayWindows' ||
        field.key === 'priceToBeatMaxPriceRelaxMissCount' ||
        field.key === 'priceToBeatMaxPriceRelaxHistoryCount' ||
        field.key === 'priceToBeatMaxPriceRelaxMinValue' ||
        field.key === 'priceToBeatMaxPriceRelaxMinUnit' ||
        field.key === 'priceToBeatMaxPriceRelaxMinDepthUsd' ||
        field.key === 'priceToBeatMaxPriceRelaxStepMode' ||
        field.key === 'priceToBeatMaxPriceRelaxStepValue' ||
        field.key === 'priceToBeatMaxPriceRelaxStepUnit' ||
        field.key === 'reentryCooldownSec' ||
        field.key === 'reentrySkipCurrentWindow' ||
        field.key === 'reentryThresholdDecay' ||
        field.key === 'reentryMaxPriceTightenBps' ||
        field.key === 'reentryPriceToBeatMaxDiff' ||
        field.key === 'reentryPriceToBeatMaxDiffUnit' ||
        field.key === 'ptbStopLossTimeDecayMode' ||
        field.key === 'notifyOnPriceToBeatGapBlocked' ||
        field.key === 'retryOnPriceToBeatGuardBlock'
      ) {
        return false;
      }
    }
    if (nodeTypeDraft === 'action.dual_dca') {
      if (field.key === 'baseShares') return dualDcaBaseSizing !== 'usdc';
      if (field.key === 'baseUsdc') return dualDcaBaseSizing === 'usdc';
    }
    if (nodeTypeDraft === 'trigger.market_price') {
      if (field.key === 'marketScope' || field.key === 'marketSelection') {
        return triggerMarketMode === 'auto_scope';
      }
      if (field.key === 'protectionMode') {
        return triggerMarketMode === 'auto_scope';
      }
      if (field.key === 'protectionPreset') {
        const protectionMode = (form.fields.protectionMode ?? '').trim().toLowerCase();
        return triggerMarketMode === 'auto_scope' && protectionMode === 'underlying_confirm';
      }
      if (field.key === 'marketSlug') {
        return triggerMarketMode !== 'auto_scope';
      }
      if (field.key === 'onceScope') {
        return triggerRepeatMode === 'once';
      }
      if (field.key === 'bindingMode') {
        return true;
      }
      if (
        triggerBindingMode === 'pair_lock_only' &&
        (
          field.key === 'priceToBeatTriggerEnabled' ||
          field.key === 'priceToBeatMode' ||
          field.key === 'priceToBeatTriggerUnit' ||
          field.key === 'priceToBeatTriggerMinGap' ||
          field.key === 'priceToBeatTriggerMaxGap'
        )
      ) {
        return false;
      }
      if (field.key === 'cycleWindowMode') {
        return triggerMarketMode === 'auto_scope';
      }
      if (field.key === 'cycleWindowSecs') {
        return triggerMarketMode === 'auto_scope' &&
          (triggerCycleWindowMode === 'first' || triggerCycleWindowMode === 'last');
      }
      if (field.key === 'cycleWindowStartSec' || field.key === 'cycleWindowEndSec') {
        return triggerMarketMode === 'auto_scope' && triggerCycleWindowMode === 'custom_range';
      }
      if (field.key === 'autoSellOnWindowEnd') {
        return triggerMarketMode === 'auto_scope' && triggerCycleWindowMode === 'custom_range';
      }
      if (field.key === 'priceToBeatTriggerEnabled') {
        return triggerMarketMode === 'auto_scope';
      }
      if (field.key === 'priceToBeatMode') {
        return triggerMarketMode === 'auto_scope' && triggerPriceToBeatEnabled;
      }
      if (
        field.key === 'priceToBeatTriggerUnit' ||
        field.key === 'priceToBeatTriggerMinGap' ||
        field.key === 'priceToBeatTriggerMaxGap'
      ) {
        return (
          triggerMarketMode === 'auto_scope' &&
          triggerPriceToBeatEnabled &&
          triggerPriceToBeatMode === 'manual'
        );
      }
    }
    return true;
  });
  const openFieldHelpKey =
    openFieldHelpState?.nodeType === nodeTypeDraft &&
    visibleNodeSchema.some((field) => field.key === openFieldHelpState.key)
      ? openFieldHelpState.key
      : null;

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex items-center gap-2 pb-1">
        <Settings2 className="h-4 w-4 text-sky-500" />
        <h3 className="text-sm font-semibold text-slate-800">Node Ayarlari</h3>
      </div>
      <Separator className="mb-2" />

      <Tabs
        value={tab}
        onValueChange={(v) => actions.onTabChange(v as 'basic' | 'advanced')}
        className="flex min-h-0 flex-1 flex-col"
      >
        <TabsList className="bg-slate-100">
          <TabsTrigger value="basic">Form</TabsTrigger>
          <TabsTrigger value="advanced">Advanced</TabsTrigger>
        </TabsList>

        <div className="min-h-0 flex-1 overflow-y-auto">
          <TabsContent value="basic" className="space-y-3 pt-2">
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">Node Key</Label>
              <Input
                value={nodeKeyDraft}
                onChange={(e) => actions.onNodeKeyChange(e.target.value)}
                className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
              />
            </div>

            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">Node Type</Label>
              <Select value={nodeTypeDraft} onValueChange={(v) => actions.onNodeTypeChange(v)}>
                <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {NODE_TYPE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            {nodeTypeDraft === 'action.telegram_notify' && (
              <div className="space-y-1">
                <Label className="text-[11px] font-medium text-slate-600">Bot Token</Label>
                <Input
                  value={telegramBotTokenMasked}
                  disabled
                  placeholder="Settings -> Telegram"
                  className="h-8 border-slate-200 bg-slate-50 text-xs text-slate-500"
                />
                <p className="text-[10px] leading-relaxed text-slate-400 italic">
                  {telegramBotTokenSource === 'user'
                    ? 'Bu token mevcut kullanicinin Telegram ayarindan gelir ve workflow icinde tekrar saklanmaz.'
                    : telegramBotTokenSource === 'legacy_ignored'
                      ? 'Bu workflow eski inline token ile acildi, fakat artik kullanilmaz. Settings -> Telegram ekranindan mevcut kullanici tokenini kaydet.'
                      : 'Telegram bot token henuz tanimli degil. Settings -> Telegram ekranindan ekle.'}
                </p>
              </div>
            )}

            {nodeTypeDraft === 'action.telegram_notify' && (
              <div className="space-y-1">
                <Label className="text-[11px] font-medium text-slate-600">
                  Default Chat ID (Fallback)
                </Label>
                <Input
                  value={telegramUserDefaultChatId}
                  disabled
                  placeholder="Settings -> Telegram"
                  className="h-8 border-slate-200 bg-slate-50 text-xs text-slate-500"
                />
                <p className="text-[10px] leading-relaxed text-slate-400 italic">
                  {telegramNodeChatId
                    ? 'Node Chat ID doluysa runtime onu kullanir. Varsayilan Chat ID sadece node bos oldugunda fallback olur.'
                    : telegramUserDefaultChatId
                      ? 'Node Chat ID bos. Runtime bu kullanicinin varsayilan Chat ID degerini kullanir.'
                      : 'Varsayilan Chat ID opsiyoneldir. Burasi da bossa node icinde Chat ID doldurman gerekir.'}
                </p>
              </div>
            )}

            {visibleNodeSchema.map((field) => {
              const selectOptions = field.input === 'select'
                ? isPresetBuySellPlaceOrder && field.key === 'executionMode'
                  ? [{ label: 'market (IOC)', value: 'market' }]
                  : (field.options || [])
                : [];
              const selectValue =
                isPresetBuySellPlaceOrder && field.key === 'executionMode' ? 'market' : (form.fields[field.key] ?? '');
              return (
                <div key={field.key} className="space-y-1">
                <div className="flex items-center gap-1">
                  <Label className="text-[11px] font-medium text-slate-600">{field.label}</Label>
                  {nodeTypeDraft === 'action.dual_dca' && nodeFieldHelp[field.key] && (
                    <button
                      type="button"
                      className="inline-flex h-4 w-4 items-center justify-center rounded-full border border-sky-300 text-sky-700 transition hover:bg-sky-100"
                      aria-label={`${field.label} alan bilgisi`}
                      aria-expanded={openFieldHelpKey === field.key}
                      aria-controls={`dual-dca-field-help-${field.key}`}
                      onClick={() =>
                        setOpenFieldHelpState((prev) =>
                          prev?.nodeType === nodeTypeDraft && prev.key === field.key
                            ? null
                            : { nodeType: nodeTypeDraft, key: field.key }
                        )
                      }
                    >
                      <span className="h-1.5 w-1.5 rounded-full bg-sky-600" />
                    </button>
                  )}
                </div>
                {field.key === 'outcomeLabel' &&
                  (nodeTypeDraft === 'trigger.open_positions' ||
                    nodeTypeDraft === 'trigger.market_price' ||
                    nodeTypeDraft === 'trigger.position_drawdown') ? (
                  <Select
                    value={(() => {
                      const selectedTokenId = (form.fields.tokenId ?? '').trim();
                      if (selectedTokenId && marketOutcomes.some((o) => o.token_id === selectedTokenId)) {
                        return selectedTokenId;
                      }
                      const selectedLabel = (form.fields[field.key] ?? '').trim();
                      return (
                        marketOutcomes.find((o) => o.label === selectedLabel)?.token_id ||
                        EMPTY_SELECT_SENTINEL
                      );
                    })()}
                    onValueChange={(v) => {
                      const tokenId = v === EMPTY_SELECT_SENTINEL ? '' : v;
                      if (!tokenId) {
                        actions.onUpdateField('tokenId', '');
                        actions.onUpdateField(field.key, '');
                        return;
                      }
                      const matched = marketOutcomes.find((o) => o.token_id === tokenId);
                      actions.onUpdateField('tokenId', tokenId);
                      actions.onUpdateField(field.key, matched?.label || '');
                    }}
                  >
                    <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value={EMPTY_SELECT_SENTINEL}>Sec...</SelectItem>
                      {marketOutcomes.map((o) => (
                        <SelectItem key={o.token_id} value={o.token_id}>
                          {o.label}{o.price != null ? ` ($${o.price.toFixed(2)})` : ''}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                ) : field.input === 'checkbox' ? (
                  <input
                    type="checkbox"
                    checked={(form.fields[field.key] ?? '').toString().trim().toLowerCase() === 'true'}
                    onChange={(e) => {
                      actions.onUpdateField(field.key, e.target.checked ? 'true' : 'false');
                      if (
                        field.key === 'priceToBeatTriggerEnabled' &&
                        e.target.checked &&
                        !['manual', 'auto_last_3_avg_excursion', 'auto_vol_pct'].includes(
                          (form.fields.priceToBeatMode ?? '')
                            .toString()
                            .trim()
                            .toLowerCase()
                        )
                      ) {
                        actions.onUpdateField('priceToBeatMode', 'manual');
                      }
                      if (
                        field.key === 'priceToBeatTriggerEnabled' &&
                        e.target.checked &&
                        !['usd', 'cent'].includes(
                          (form.fields.priceToBeatTriggerUnit ?? '')
                            .toString()
                            .trim()
                            .toLowerCase()
                        )
                      ) {
                        actions.onUpdateField('priceToBeatTriggerUnit', 'usd');
                      }
                    }}
                    className="h-4 w-4 rounded border-slate-300"
                  />
                ) : field.input === 'select' ? (
                  <Select
                    value={selectValue || EMPTY_SELECT_SENTINEL}
                    onValueChange={(v) =>
                      actions.onUpdateField(field.key, v === EMPTY_SELECT_SENTINEL ? '' : v)
                    }
                  >
                    <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {selectOptions.map((option) => (
                        <SelectItem
                          key={option.value || EMPTY_SELECT_SENTINEL}
                          value={option.value || EMPTY_SELECT_SENTINEL}
                        >
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                ) : field.input === 'textarea' ? (
                  <textarea
                    value={form.fields[field.key] ?? ''}
                    onChange={(e) => actions.onUpdateField(field.key, e.target.value)}
                    className="min-h-20 w-full rounded-md border border-slate-200 bg-white p-2 text-[11px] text-slate-900 focus-visible:ring-sky-300"
                  />
                ) : (
                  <Input
                    type={field.input}
                    value={
                      field.input === 'datetime-local'
                        ? normalizeDateTimeInput(form.fields[field.key] ?? '')
                        : form.fields[field.key] ?? ''
                    }
                    onChange={(e) => actions.onUpdateField(field.key, e.target.value)}
                    placeholder={field.placeholder}
                    className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                  />
                )}
                {nodeTypeDraft === 'action.dual_dca' &&
                  openFieldHelpKey === field.key &&
                  nodeFieldHelp[field.key] && (
                    <div
                      id={`dual-dca-field-help-${field.key}`}
                      className="rounded-lg border border-sky-200/60 border-l-2 border-l-sky-400 bg-gradient-to-br from-sky-50 to-indigo-50/50 p-2.5 shadow-sm"
                    >
                      {/* Baslik */}
                      <p className="text-[11px] font-semibold text-slate-800">
                        {nodeFieldHelp[field.key].title}
                      </p>
                      <p className="mt-0.5 text-[10px] leading-relaxed text-slate-600">
                        {nodeFieldHelp[field.key].description}
                      </p>

                      {/* Etki */}
                      {nodeFieldHelp[field.key].effect && (
                        <div className="mt-1.5 flex items-start gap-1.5">
                          <span className="mt-px inline-block rounded bg-sky-100 px-1 py-px text-[9px] font-semibold text-sky-700 whitespace-nowrap">Etki</span>
                          <p className="text-[10px] leading-relaxed text-slate-700">{nodeFieldHelp[field.key].effect}</p>
                        </div>
                      )}

                      {/* Ornek */}
                      {nodeFieldHelp[field.key].example && (
                        <div className="mt-1.5 flex items-start gap-1.5">
                          <span className="mt-px inline-block rounded bg-emerald-100 px-1 py-px text-[9px] font-semibold text-emerald-700 whitespace-nowrap">Ornek</span>
                          <p className="text-[10px] font-mono leading-relaxed text-slate-600 bg-white/60 rounded px-1">{nodeFieldHelp[field.key].example}</p>
                        </div>
                      )}

                      {/* Dusuk/Yuksek Etki */}
                      {(nodeFieldHelp[field.key].whatHappensIfLowHigh || []).length > 0 && (
                        <div className="mt-1.5">
                          <span className="inline-block rounded bg-amber-100 px-1 py-px text-[9px] font-semibold text-amber-700">Deger Etkisi</span>
                          <div className="mt-1 grid grid-cols-1 gap-0.5">
                            {(nodeFieldHelp[field.key].whatHappensIfLowHigh || []).map((item) => (
                              <p key={item} className="text-[10px] leading-relaxed text-slate-600 pl-1 border-l border-amber-200">
                                {item}
                              </p>
                            ))}
                          </div>
                        </div>
                      )}

                      {/* Basit Ornekler */}
                      {(nodeFieldHelp[field.key].simpleExamples || []).length > 0 && (
                        <div className="mt-1.5">
                          <span className="inline-block rounded bg-violet-100 px-1 py-px text-[9px] font-semibold text-violet-700">Ornekler</span>
                          <div className="mt-1 space-y-0.5">
                            {(nodeFieldHelp[field.key].simpleExamples || []).map((simple) => (
                              <p key={simple} className="text-[10px] leading-relaxed text-slate-600 pl-1 border-l border-violet-200">
                                {simple}
                              </p>
                            ))}
                          </div>
                        </div>
                      )}

                      {/* Ipuclari */}
                      {(nodeFieldHelp[field.key].tips || []).length > 0 && (
                        <div className="mt-1.5 rounded bg-slate-100/80 px-1.5 py-1">
                          {(nodeFieldHelp[field.key].tips || []).map((tip) => (
                            <p key={tip} className="text-[10px] leading-relaxed text-slate-500">
                              ⚡ {tip}
                            </p>
                          ))}
                        </div>
                      )}
                    </div>
                  )}
                {field.help && (
                  <p className="text-[10px] leading-relaxed text-slate-400 italic">{field.help}</p>
                )}
                <TriggerPairLockHint
                  visible={
                    field.key === 'bindingMode' &&
                    nodeTypeDraft === 'trigger.market_price' &&
                    triggerBindingMode === 'pair_lock_only'
                  }
                />
                <PairLockSummarySection
                  visible={field.key === 'mode' && placeOrderPairLockEnabled}
                  primaryOutcomeLabel={(form.fields.outcomeLabel ?? '').trim()}
                  counterOutcomePreview={placeOrderCounterOutcomePreview}
                  upstreamPairLockTrigger={upstreamPairLockTrigger}
                />
                <PairLockStaleConfigSection
                  visible={field.key === 'mode' && placeOrderPairLockEnabled}
                  form={form}
                  onFormChange={actions.onFormChange}
                />
                <PairLockAutoPreviewSection
                  visible={field.key === 'pairTotalBudgetUsdc' && placeOrderPairLockEnabled}
                  fields={form.fields}
                  marketOutcomes={marketOutcomes}
                  marketOutcomesLoading={marketOutcomesLoading}
                />
                {field.key === 'marketSlug' && placeOrderMarketSeedUi?.isInheritedMarketSlug && <p className="text-[10px] leading-relaxed text-sky-600">Bagli upstream `trigger.market_price` bilgisinden otomatik dolduruldu. Config&apos;e yazmak icin `Node Guncelle` kullan.</p>}
                {field.key === 'marketSlug' && placeOrderMarketSeedUi?.upstreamKind === 'multiple' && <p className="text-[10px] leading-relaxed text-amber-600">Birden fazla bagli upstream fixed market bulundu{placeOrderMarketSeedUi.distinctUpstreamMarketSlugs.length > 0 ? ` (${placeOrderMarketSeedUi.distinctUpstreamMarketSlugs.join(', ')})` : ''}. Bu yuzden otomatik doldurma yapilmadi.</p>}
                {field.key === 'tokenId' && placeOrderMarketSeedUi?.upstreamKind === 'single' && placeOrderMarketSeedUi.upstreamOutcomeKind === 'multiple' && <p className="text-[10px] leading-relaxed text-amber-600">Bagli upstream market bulundu ama outcome belirsiz{placeOrderMarketSeedUi.distinctUpstreamOutcomeLabels.length > 0 ? ` (${placeOrderMarketSeedUi.distinctUpstreamOutcomeLabels.join(', ')})` : ''}. Bu yuzden sadece market slug dolduruldu.</p>}
                {field.key === 'tokenId' && (placeOrderMarketSeedUi?.isInheritedTokenId || placeOrderMarketSeedUi?.isInheritedOutcomeLabel) && <p className="text-[10px] leading-relaxed text-sky-600">Bagli upstream `trigger.market_price` outcome bilgisinden otomatik dolduruldu. Config&apos;e yazmak icin `Node Guncelle` kullan.</p>}
                {field.key === 'reentryMinPriceCent' && placeOrderReentryChecked && (
                  <p className="text-[10px] leading-relaxed text-sky-600">
                    Bu alt limit yalniz re-entry denemelerinde current price icin uygulanir.
                    Trigger guard bildirim ve bekleme ayarlari burada da kullanilir.
                  </p>
                )}
                {field.key === 'reentryMaxPriceCent' && placeOrderReentryChecked && (
                  <p className="text-[10px] leading-relaxed text-sky-600">
                    Bu tavan yalniz re-entry denemelerinde kullanilir. Max price koruma ayarlari
                    bu alan icin de gecerlidir.
                  </p>
                )}
                {field.key === 'maxPriceCent' && placeOrderHasInheritedMaxPrice && (
                  <p className="text-[10px] leading-relaxed text-sky-600">
                    Upstream `trigger.market_price` tavanindan otomatik dolduruldu. Config&apos;e
                    yazmak icin `Node Guncelle` kullan.
                  </p>
                )}
                {field.key === 'maxPriceCent' && placeOrderHasStaleLocalMaxPrice && (
                  <p className="text-[10px] leading-relaxed text-amber-600">
                    Bu node icindeki kayitli tavan `{placeOrderMaxPriceCentValue}`c. Upstream tetik
                    artik `{upstreamMaxPriceResolution.maxPriceCent}`c tasiyor; otomatik
                    guncellenmez.
                  </p>
                )}
                {field.key === 'maxPriceCent' && placeOrderHasAmbiguousUpstreamMaxPrice && (
                  <p className="text-[10px] leading-relaxed text-amber-600">
                    Birden fazla veya belirsiz upstream tavan bulundu
                    {upstreamMaxPriceResolution.distinctMaxPriceCents.length > 0
                      ? ` (${upstreamMaxPriceResolution.distinctMaxPriceCents.join(', ')}c)`
                      : ''}.
                    Bu yuzden otomatik doldurma yapilmadi.
                  </p>
                )}
                {field.key === 'maxPriceCent' && showDedicatedTriggerGuard && (
                  <div className="space-y-1 rounded-md border border-slate-200/80 bg-slate-50/80 p-2">
                    <div className="flex items-center justify-between gap-2">
                      <Label className="text-[11px] font-medium text-slate-600">Tetik Fiyat Korumasi</Label>
                      <input
                        type="checkbox"
                        checked={placeOrderTriggerGuardChecked}
                        disabled={triggerGuardDisabled}
                        onChange={(e) =>
                          actions.onUpdateField(
                            'triggerPriceGuardEnabled',
                            e.target.checked ? 'true' : 'false'
                          )
                        }
                        className="h-4 w-4 rounded border-slate-300 disabled:cursor-not-allowed disabled:opacity-50"
                      />
                    </div>
                    <p className="text-[10px] leading-relaxed text-slate-400 italic">
                      Upstream tetik fiyatinin altina dusulurse buy emrini engelle.
                    </p>
                    {reentryTriggerGuardActive && (
                      <p className="text-[10px] leading-relaxed text-sky-600">
                        `Re-entry Min Fiyat` ayarli oldugu icin asagidaki bildirim ve bekleme
                        toggle&apos;lari re-entry alt fiyat korumasi icin de kullanilir.
                      </p>
                    )}
                    {!upstreamHasTriggerPrice && !placeOrderTriggerGuardChecked && !reentryTriggerGuardActive && (
                      <p className="text-[10px] leading-relaxed text-amber-600">
                        Bu koruma yalnizca upstream tetikte `triggerPrice` veya `triggerPriceCent`
                        varsa acilabilir.
                      </p>
                    )}
                    {!upstreamHasTriggerPrice && placeOrderTriggerGuardChecked && (
                      <p className="text-[10px] leading-relaxed text-amber-600">
                        Mevcut ayar upstream tetik fiyatini artik bulamiyor. Istersen kapatabilirsin.
                      </p>
                    )}
                    {triggerGuardProtectionActive && (
                      <div className="mt-2 flex items-center justify-between gap-2 border-t border-slate-200 pt-2">
                        <Label className="text-[11px] font-medium text-slate-600">
                          Iyilesince Tekrar Dene
                        </Label>
                        <input
                          type="checkbox"
                          checked={triggerGuardRetryChecked}
                          onChange={(e) =>
                            actions.onUpdateField(
                              'retryOnTriggerPriceGuardBlock',
                              e.target.checked ? 'true' : 'false'
                            )
                          }
                          className="h-4 w-4 rounded border-slate-300"
                        />
                      </div>
                    )}
                    {triggerGuardProtectionActive && (
                      <p className="text-[10px] leading-relaxed text-slate-400 italic">
                        Guard bloklarsa order iptal olmaz; bekleme moduna alinip kosullar
                        duzelince yeniden denenir.
                      </p>
                    )}
                    <ExecutionFloorProtectionSection
                      checked={executionFloorGuardChecked}
                      retryChecked={executionFloorRetryChecked}
                      disabled={executionFloorGuardDisabled}
                      hasUpstreamTriggerPrice={upstreamHasTriggerPrice}
                      hasConfiguredFloorPrice={hasManualExecutionFloorPrice}
                      floorPriceCent={placeOrderExecutionFloorPriceCentValue}
                      onUpdateField={actions.onUpdateField}
                    />
                    <div className="mt-2 flex items-center justify-between gap-2 border-t border-slate-200 pt-2">
                      <Label className="text-[11px] font-medium text-slate-600">
                        Price to Beat Korumasi
                      </Label>
                      <input
                        type="checkbox"
                        checked={priceToBeatGuardChecked}
                        onChange={(e) =>
                          {
                            actions.onUpdateField(
                              'priceToBeatGuardEnabled',
                              e.target.checked ? 'true' : 'false'
                            );
                            if (
                              e.target.checked &&
                              !['manual', 'auto_last_3_avg_excursion', 'auto_vol_pct'].includes(
                                (form.fields.priceToBeatMode ?? '')
                                  .toString()
                                  .trim()
                                  .toLowerCase()
                              )
                            ) {
                              actions.onUpdateField('priceToBeatMode', 'manual');
                            }
                            if (
                              e.target.checked &&
                              !['usd', 'cent'].includes(
                                (form.fields.priceToBeatMaxDiffUnit ?? '')
                                  .toString()
                                  .trim()
                                  .toLowerCase()
                              )
                            ) {
                              actions.onUpdateField('priceToBeatMaxDiffUnit', 'usd');
                            }
                            if (
                              e.target.checked &&
                              !(form.fields.notifyOnPriceToBeatGapBlocked ?? '')
                                .toString()
                                .trim()
                            ) {
                              actions.onUpdateField('notifyOnPriceToBeatGapBlocked', 'true');
                            }
                          }
                        }
                        className="h-4 w-4 rounded border-slate-300"
                      />
                    </div>
                    <p className="text-[10px] leading-relaxed text-slate-400 italic">
                      Price to Beat ile ayni Polymarket/Chainlink current price feedi kullanilir.
                      Fark belirlenen minimum seviyenin altindaysa buy emrini engelle. 5m ve 15m
                      updown marketlerde calisir.
                    </p>
                    {priceToBeatGuardChecked && (
                      <div className="mt-2 space-y-2 border-t border-slate-200 pt-2">
                        <div className="space-y-1">
                          <Label className="text-[11px] font-medium text-slate-600">
                            PTB Modu
                          </Label>
                          <Select
                            value={priceToBeatGuardMode}
                            onValueChange={(value) =>
                              actions.onUpdateField('priceToBeatMode', value)
                            }
                          >
                            <SelectTrigger
                              className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900"
                              size="sm"
                            >
                              <SelectValue />
                            </SelectTrigger>
                            <SelectContent>
                              <SelectItem value="manual">Manual</SelectItem>
                              <SelectItem value="auto_last_3_avg_excursion">
                                Auto: son 3 market excursion ort.
                              </SelectItem>
                              <SelectItem value="auto_vol_pct">
                                Auto: volatility bazli yuzde
                              </SelectItem>
                            </SelectContent>
                          </Select>
                        </div>
                        {priceToBeatGuardMode === 'manual' ? (
                          <>
                            <div className="space-y-1">
                              <Label className="text-[11px] font-medium text-slate-600">
                                Minimum Fark
                              </Label>
                              <Input
                                type="number"
                                step="any"
                                value={form.fields.priceToBeatMaxDiff ?? ''}
                                onChange={(event) =>
                                  actions.onUpdateField('priceToBeatMaxDiff', event.target.value)
                                }
                                placeholder={priceToBeatGuardUnit === 'cent' ? '1' : '5'}
                                className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                              />
                            </div>
                            <div className="space-y-1">
                              <Label className="text-[11px] font-medium text-slate-600">
                                Birim
                              </Label>
                              <Select
                                value={priceToBeatGuardUnit}
                                onValueChange={(value) =>
                                  actions.onUpdateField('priceToBeatMaxDiffUnit', value)
                                }
                              >
                                <SelectTrigger
                                  className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900"
                                  size="sm"
                                >
                                  <SelectValue />
                                </SelectTrigger>
                                <SelectContent>
                                  <SelectItem value="usd">USD</SelectItem>
                                  <SelectItem value="cent">Cent</SelectItem>
                                </SelectContent>
                              </Select>
                            </div>
                            <p className="text-[10px] leading-relaxed text-slate-400 italic">
                              {priceToBeatGuardUnit === 'cent'
                                ? 'Cent modu: 1 = $0.01. Fark bu minimum degerin altinda kalirsa bloklanir.'
                                : 'USD modu: 5 = $5.00. Fark bu minimum degerin altinda kalirsa bloklanir.'}
                            </p>
                          </>
                        ) : (
                          <p className="text-[10px] leading-relaxed text-slate-400 italic">
                            Dinamik modda esik elle girilmez. Ayni asset/timeframe icin son 3
                            tamamlanmis marketin yonlu excursion ortalamasi otomatik kullanilir.
                            Asagidaki relax ayariyla miss esigi gecildikten sonra maxPrice altinda
                            gorulen uygun gap seviyesine kademeli olarak gevseyebilir.
                          </p>
                        )}
                        <PriceToBeatStopLossBumpSection
                          enabled={priceToBeatStopLossBumpChecked}
                          amount={form.fields.priceToBeatStopLossBumpAmount ?? ''}
                          maxValue={form.fields.priceToBeatStopLossBumpMaxValue ?? ''}
                          decayWindows={form.fields.priceToBeatStopLossBumpDecayWindows ?? ''}
                          scopeMode={priceToBeatStopLossBumpScope}
                          unit={priceToBeatStopLossBumpUnit}
                          defaultUnit={priceToBeatGuardMode === 'manual' ? priceToBeatGuardUnit : 'usd'}
                          onUpdateField={actions.onUpdateField}
                        />
                        <PriceToBeatMaxPriceRelaxSection
                          missCount={form.fields.priceToBeatMaxPriceRelaxMissCount ?? ''}
                          historyCount={form.fields.priceToBeatMaxPriceRelaxHistoryCount ?? ''}
                          minValue={form.fields.priceToBeatMaxPriceRelaxMinValue ?? ''}
                          minDepthUsd={form.fields.priceToBeatMaxPriceRelaxMinDepthUsd ?? ''}
                          minUnit={priceToBeatMaxPriceRelaxMinUnit}
                          stepMode={priceToBeatMaxPriceRelaxStepMode}
                          stepValue={form.fields.priceToBeatMaxPriceRelaxStepValue ?? ''}
                          stepUnit={priceToBeatMaxPriceRelaxStepUnit}
                          onUpdateField={actions.onUpdateField}
                        />
                        {placeOrderReentryChecked && (
                          <div className="space-y-2 rounded-md border border-slate-200/80 bg-slate-50/70 p-2">
                            <div className="space-y-1">
                              <Label className="text-[11px] font-medium text-slate-600">Re-entry PTB Min Fark</Label>
                              <Input
                                type="number"
                                step="any"
                                value={placeOrderReentryPriceToBeatMaxDiffValue}
                                onChange={(event) => actions.onUpdateField('reentryPriceToBeatMaxDiff', event.target.value)}
                                placeholder={priceToBeatGuardMode === 'manual' ? '2' : 'bos birak'}
                                className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                              />
                            </div>
                            <div className="space-y-1">
                              <Label className="text-[11px] font-medium text-slate-600">Re-entry PTB Birimi</Label>
                              <Select value={reentryPriceToBeatOverrideUnit || undefined} onValueChange={(value) => actions.onUpdateField('reentryPriceToBeatMaxDiffUnit', value)}>
                                <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                                  <SelectValue placeholder="Birim sec" />
                                </SelectTrigger>
                                <SelectContent>
                                  <SelectItem value="usd">USD</SelectItem>
                                  <SelectItem value="cent">Cent</SelectItem>
                                </SelectContent>
                              </Select>
                            </div>
                            {priceToBeatGuardMode === 'manual' ? (
                              <p className="text-[10px] leading-relaxed text-slate-400 italic">Bu override yalniz re-entry denemelerinde uygulanir. Birim secilmezse ana PTB birimi kullanilir: `{priceToBeatGuardUnit}`.</p>
                            ) : (
                              <p className="text-[10px] leading-relaxed text-slate-400 italic">Ana PTB auto modda kalsa bile re-entry denemesinde bu deger manual override olarak kullanilir. Bu modda birim secimi zorunludur.</p>
                            )}
                            <div className="grid grid-cols-2 gap-2">
                              <div className="space-y-1">
                                <Label className="text-[11px] font-medium text-slate-600">Cooldown (sn)</Label>
                                <Input type="number" step="1" min="0" value={form.fields.reentryCooldownSec ?? ''} onChange={(event) => actions.onUpdateField('reentryCooldownSec', event.target.value)} placeholder="0" className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300" />
                              </div>
                              <div className="space-y-1">
                                <Label className="text-[11px] font-medium text-slate-600">PTB Decay</Label>
                                <Input type="number" step="any" value={form.fields.reentryThresholdDecay ?? ''} onChange={(event) => actions.onUpdateField('reentryThresholdDecay', event.target.value)} placeholder="0.8" className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300" />
                              </div>
                              <div className="space-y-1">
                                <Label className="text-[11px] font-medium text-slate-600">MaxPrice Tighten (bps)</Label>
                                <Input type="number" step="1" min="0" value={form.fields.reentryMaxPriceTightenBps ?? ''} onChange={(event) => actions.onUpdateField('reentryMaxPriceTightenBps', event.target.value)} placeholder="500" className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300" />
                              </div>
                              <div className="flex items-center justify-between gap-2 rounded-md border border-slate-200 bg-white px-2 py-2">
                                <Label className="text-[11px] font-medium text-slate-600">Ayni pencereyi atla</Label>
                                <input type="checkbox" checked={(form.fields.reentrySkipCurrentWindow ?? '').toString().trim().toLowerCase() === 'true'} onChange={(event) => actions.onUpdateField('reentrySkipCurrentWindow', event.target.checked ? 'true' : 'false')} className="h-4 w-4 rounded border-slate-300" />
                              </div>
                            </div>
                          </div>
                        )}
                        <div className="flex items-center justify-between gap-2">
                          <Label className="text-[11px] font-medium text-slate-600">
                            Price to Beat Engel Bildirimi
                          </Label>
                          <input
                            type="checkbox"
                            checked={
                              (form.fields.notifyOnPriceToBeatGapBlocked ?? '')
                                .toString()
                                .trim()
                                .toLowerCase() === 'true'
                            }
                            onChange={(e) =>
                              actions.onUpdateField(
                                'notifyOnPriceToBeatGapBlocked',
                                e.target.checked ? 'true' : 'false'
                              )
                            }
                            className="h-4 w-4 rounded border-slate-300"
                          />
                        </div>
                        <div className="flex items-center justify-between gap-2">
                          <Label className="text-[11px] font-medium text-slate-600">
                            Iyilesince Tekrar Dene
                          </Label>
                          <input
                            type="checkbox"
                            checked={priceToBeatRetryChecked}
                            onChange={(e) =>
                              actions.onUpdateField(
                                'retryOnPriceToBeatGuardBlock',
                                e.target.checked ? 'true' : 'false'
                              )
                            }
                            className="h-4 w-4 rounded border-slate-300"
                          />
                        </div>
                        <p className="text-[10px] leading-relaxed text-slate-400 italic">
                          Guard fail olursa node hata verir ama bekleme modunda yeniden denenir;
                          kosullar duzelince order akisina devam eder.
                        </p>
                      </div>
                    )}
                  </div>
                )}
                {field.key === 'maxPriceCent' && showDedicatedTriggerGuard && (
                  <MaxPriceProtectionSection
                    hasConfiguredMaxPrice={maxPriceProtectionActive}
                    notifyChecked={maxPriceNotifyChecked}
                    retryChecked={maxPriceRetryChecked}
                    onUpdateField={actions.onUpdateField}
                  />
                )}
                </div>
              );
            })}
            {isPresetPlaceOrder && (
              <p className="text-[10px] leading-relaxed text-slate-400 italic">
                Bu preset node tetik gelince calisir; node ici tetik kosulu kullanmaz. Al/Sat preset
                node&apos;lar market (IOC) modunda sabittir.
              </p>
            )}
            {isPresetPlaceOrder && upstreamAutoScope && (
              <p className="text-[10px] leading-relaxed text-slate-400 italic">
                {placeOrderSide === 'buy'
                  ? 'Buy: market/token auto-scope tetikten runtime’da cozulur; sourceTradeId yoksa backend usdc sizing ile local source trade uretebilir.'
                  : placeOrderSide === 'sell'
                    ? 'Sell: mevcut sourceTradeId veya pozisyon baglami gerekir; auto-scope tek basina yeterli degildir.'
                    : 'Auto-scope zincirinde buy runtime binding kullanabilir; sell tarafi mevcut sourceTradeId/pozisyon ister.'}
              </p>
            )}

            {nodeTypeDraft === 'action.place_order' && placeOrderMaxTriggers > 1 && (
              <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
                <div className="flex items-center gap-1.5">
                  <Zap className="h-3.5 w-3.5 text-sky-500" />
                  <p className="text-[11px] font-semibold text-slate-700">Tetik Bazli Tutar Plani</p>
                </div>
                <p className="text-[10px] leading-relaxed text-slate-400 italic">
                  Her tetik icin ayri {placeOrderSizeMode === 'pct' ? '%' : 'USDC'} degeri girebilirsin.
                </p>
                <p className="text-[10px] leading-relaxed text-slate-400 italic">
                  maxTriggers: {placeOrderMaxTriggers} (satir biterse order tamamlanir).
                </p>
                <div className="space-y-2">
                  {placeOrderTriggerRows.map((value, index) => (
                    <div key={`trigger-size-row-${index}`} className="space-y-1">
                      <Label className="text-[10px] font-medium text-slate-600">
                        Tetik #{index + 1} {placeOrderSizeMode === 'pct' ? '(%)' : '(USDC)'}
                      </Label>
                      <Input
                        type="number"
                        value={value}
                        onChange={(event) => actions.onUpdateTriggerSizeRow(index, event.target.value)}
                        placeholder={placeOrderSizeMode === 'pct' ? '25' : '10'}
                        className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                      />
                    </div>
                  ))}
                </div>
                {placeOrderSizeMode === 'pct' && (
                  <p
                    className={`text-[10px] ${
                      placeOrderTriggerSumInvalid ? 'text-red-500' : 'text-slate-500'
                    }`}
                  >
                    Toplam: {placeOrderTriggerSum.toFixed(2)}%
                  </p>
                )}
                {placeOrderTriggerRowInvalid && (
                  <p className="text-[10px] text-red-500">
                    Satir degerleri 0&apos;dan buyuk sayi olmali.
                  </p>
                )}
                {placeOrderTriggerSumInvalid && (
                  <p className="text-[10px] text-red-500">Yuzde toplami 100&apos;u gecemez.</p>
                )}
              </div>
            )}

            {!supportsOpenPositionPicker && (
              <p className="text-[10px] leading-relaxed text-slate-400 italic">
                Acik pozisyon listesi yalnizca{' '}
                <span className="text-slate-700">Tetik: Mevcut Pozisyonlar</span> veya{' '}
                <span className="text-slate-700">Aksiyon: Place Order</span> node&apos;lari
                secildiginde gorunur.
              </p>
            )}

            {supportsOpenPositionPicker && (
              <OpenPositionsSection
                openPositions={openPositions}
                openPositionsMeta={openPositionsMeta}
                openPositionsLoading={openPositionsLoading}
                openPositionApplyingKey={openPositionApplyingKey}
                canApplyOpenPosition={canApplyOpenPosition}
                actions={actions}
              />
            )}

            {nodeTypeDraft === 'trigger.position_drawdown' && (
              <DrawdownRulesSection rows={form.drawdownRuleRows || []} actions={actions} />
            )}

            {showTpLadderSection && (
              <ExitLadderSection
                title="Take Profit Kademeleri"
                description="Fiyat seviyeleri strict artar; boyut yüzdeleri orijinal buy fill üzerinden düşünülür."
                rows={placeOrderTpRuleRows}
                addLabel="TP Kademesi Ekle"
                onAdd={() => updateTpRuleRows((rows) => [...rows, createEmptyExitLadderRuleRow()])}
                onUpdate={(rowId, patch) =>
                  updateTpRuleRows((rows) =>
                    rows.map((row) => (row.id === rowId ? { ...row, ...patch } : row))
                  )
                }
                onRemove={(rowId) =>
                  updateTpRuleRows((rows) => rows.filter((row) => row.id !== rowId))
                }
              />
            )}

            {showSlLadderSection && (
              <ExitLadderSection
                title="Stop Loss Kademeleri"
                description="Fiyat seviyeleri strict azalır; node seviyesindeki SL trigger mode tum kademelere ortak uygulanir."
                rows={placeOrderSlRuleRows}
                addLabel="SL Kademesi Ekle"
                onAdd={() => updateSlRuleRows((rows) => [...rows, createEmptyExitLadderRuleRow()])}
                onUpdate={(rowId, patch) =>
                  updateSlRuleRows((rows) =>
                    rows.map((row) => (row.id === rowId ? { ...row, ...patch } : row))
                  )
                }
                onRemove={(rowId) =>
                  updateSlRuleRows((rows) => rows.filter((row) => row.id !== rowId))
                }
              />
            )}

            {showPtbStopLossSection && (
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
                    checked={ptbStopLossChecked}
                    onChange={(e) =>
                      actions.onUpdateField(
                        'ptbStopLossEnabled',
                        e.target.checked ? 'true' : 'false'
                      )
                    }
                    className="h-4 w-4 rounded border-slate-300"
                  />
                </div>
                {ptbStopLossChecked && (
                  <>
                    <p className="text-[10px] leading-relaxed text-slate-400 italic">
                      Underlying directional gap izlenir. Up/Yes icin `current Chainlink - PTB`,
                      Down/No icin `PTB - current Chainlink`. Buradan staged PTB satirlari
                      tanimlanir; `0 / 100` tek satir, eski hard PTB ile ayni kapanis mantigini verir.
                      Negatif gap, karsi yone ek overshoot bekler. Negatif esiklerde zaman decay uygulanmaz.
                    </p>
                    <div className="space-y-1">
                      <Label className="text-[11px] font-medium text-slate-600">PTB SL Zaman Modu</Label>
                      <Select value={ptbStopLossTimeDecayMode} onValueChange={(value) => actions.onUpdateField('ptbStopLossTimeDecayMode', value)}>
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
                    <PtbStopLossRuleSection
                      rows={placeOrderPtbStopLossRuleRows}
                      onAdd={() =>
                        updatePtbStopLossRuleRows((rows) => [
                          ...rows,
                          createEmptyPtbStopLossRuleRow(),
                        ])
                      }
                      onUpdate={(rowId, patch) =>
                        updatePtbStopLossRuleRows((rows) =>
                          rows.map((row) => (row.id === rowId ? { ...row, ...patch } : row))
                        )
                      }
                      onRemove={(rowId) =>
                        updatePtbStopLossRuleRows((rows) =>
                          rows.filter((row) => row.id !== rowId)
                        )
                      }
                    />
                  </>
                )}
              </div>
            )}

            {showTimeExitSection && (
              <TimeExitRulesSection
                rows={placeOrderTimeExitRuleRows}
                onAdd={() =>
                  updateTimeExitRuleRows((rows) => [...rows, createEmptyTimeExitRuleRow()])
                }
                onUpdate={(rowId, patch) =>
                  updateTimeExitRuleRows((rows) =>
                    rows.map((row) => (row.id === rowId ? { ...row, ...patch } : row))
                  )
                }
                onRemove={(rowId) =>
                  updateTimeExitRuleRows((rows) => rows.filter((row) => row.id !== rowId))
                }
              />
            )}

            {(nodeTypeDraft === 'trigger.open_positions' ||
              (nodeTypeDraft === 'trigger.market_price' &&
                triggerBindingMode !== 'pair_lock_only')) && (
              <OutcomeConditionsSection
                rows={form.outcomeConditionRows}
                marketOutcomes={marketOutcomes}
                marketOutcomesLoading={marketOutcomesLoading}
                actions={actions}
                nodeType={nodeTypeDraft}
              />
            )}
            {nodeTypeDraft === 'trigger.market_price' && triggerBindingMode === 'pair_lock_only' && (
              <div className="rounded-lg border border-sky-200/80 bg-sky-50/80 p-3 text-[10px] leading-relaxed text-sky-700">
                Bu modda trigger outcome secmez; marketi pair_lock node’una baglar. Up/Down secimi ve fiyat/PTB/max price mantigi pair_lock node’unda kalir.
              </div>
            )}

            {(nodeTypeDraft === 'logic.if' || nodeTypeDraft === 'logic.switch') && (
              <ExpressionSection form={form} actions={actions} />
            )}

            {nodeTypeDraft === 'action.set_state' && (
              <StatePatchSection rows={form.statePatchRows} actions={actions} />
            )}

            <p className="text-[10px] leading-relaxed text-slate-400 italic">
              Yeni node icin <span className="text-slate-700">Node Ekle</span>, secili node icin{' '}
              <span className="text-slate-700">Node Guncelle</span> kullan.
            </p>
          </TabsContent>

          <TabsContent value="advanced" className="space-y-2 pt-2">
            <p className="text-[11px] text-amber-400">
              Gelismis mod JSON icindir. Yanlis JSON flow dogrulamasini bozabilir.
            </p>
            <textarea
              value={form.advancedJson}
              onChange={(e) =>
                actions.onFormChange((prev) =>
                  prev ? { ...prev, advancedJson: e.target.value } : prev
                )
              }
              className="min-h-60 w-full rounded-md border border-slate-200 bg-white p-2 text-[11px] text-slate-900 focus-visible:ring-sky-300"
            />
            <p className="text-[10px] leading-relaxed text-slate-400 italic">
              JSON ile yeni node ekleyebilir veya secili node&apos;u guncelleyebilirsin.
            </p>
          </TabsContent>
        </div>
      </Tabs>

      <div className="shrink-0 border-t bg-white py-2 flex gap-2">
        {tab === 'basic' ? (
          <>
            <Button size="sm" className="flex-1" onClick={actions.onCreateNode}>
              <Plus className="mr-1 h-3.5 w-3.5" /> Node Ekle
            </Button>
            <Button
              size="sm"
              variant="secondary"
              className="flex-1"
              onClick={actions.onUpdateNode}
            >
              Node Guncelle
            </Button>
          </>
        ) : (
          <>
            <Button size="sm" className="flex-1" onClick={actions.onCreateFromAdvanced}>
              <Plus className="mr-1 h-3.5 w-3.5" /> JSON ile Ekle
            </Button>
            <Button
              size="sm"
              variant="secondary"
              className="flex-1"
              onClick={actions.onUpdateFromAdvanced}
            >
              JSON ile Guncelle
            </Button>
          </>
        )}
        <Button
          size="sm"
          variant="outline"
          className="border-red-200 text-red-600 hover:bg-red-50 hover:text-red-700"
          onClick={actions.onDeleteNode}
        >
          <Trash2 className="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>
  );
}
