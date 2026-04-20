import { BOOLEAN_KEYS, NUMERIC_KEYS, RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME } from './constants';
import { buildObjectFromKeyValueDrafts, buildExpression, nestedExprGroupToJsonLogic, objectToRows, parseExpressionDraft, parseNumberArrayToStringRows } from './expressions';
import { createEmptyDrawdownRuleRow, createEmptyExitLadderRuleRow, createEmptyKeyValueDraft, createEmptyTimeExitRuleRow } from './drafts';
import { NODE_FIELD_SCHEMAS } from './schemas';
import { isPresetBuySellPlaceOrderMarker, isPresetPlaceOrderMarker, normalizeResolveMarketScope, resolveTriggerMarketOnceScope, toResolveMarketScope } from './presets';
import { applyPairLockFormDefaults, normalizePairLockBuildConfig, normalizePairLockStopLossBuildConfig, normalizePairLockTakeProfitBuildConfig, PAIR_LOCK_CONFIG_KEYS } from './pair-lock';
import { applyPtbStopLossFormDefaults, buildPtbStopLossRules, normalizePtbStopLossGapUnit, parsePtbStopLossRuleRows, shouldEnablePtbStopLossFromConfig } from './ptb-stop-loss';
import { normalizeTriggerMarketPriceCycleWindowConfig, readTriggerMarketPriceCycleWindowFields } from './cycle-window';
import { TRIGGER_MARKET_ONCE_SCOPE_VERSION } from './constants';
import type { DrawdownRuleRow, ExitLadderRuleRow, NodeConfigFormState, OutcomeConditionRow, PtbStopLossRuleRow, TimeExitRuleRow } from './types';
import {
  createId,
  isRecord,
  safeJsonStringify,
  toCentStringValue,
  toDateTimeLocalString,
  toStringValue,
  validateOutcomeConditionRow,
} from './utils';

export function parseNodeConfigToForm(nodeType: string, config: unknown): NodeConfigFormState {
  const cfg = isRecord(config) ? config : {};
  const fields: Record<string, string> = {};
  let triggerSizeRows: string[] = [];
  const tpRuleRows: ExitLadderRuleRow[] = [];
  const counterLegTpRuleRows: ExitLadderRuleRow[] = [];
  const slRuleRows: ExitLadderRuleRow[] = [];
  const ptbStopLossRuleRows: PtbStopLossRuleRow[] = [];
  const timeExitRuleRows: TimeExitRuleRow[] = [];
  for (const field of NODE_FIELD_SCHEMAS[nodeType] || []) {
    fields[field.key] =
      field.input === 'datetime-local'
      ? toDateTimeLocalString(cfg[field.key])
      : toStringValue(cfg[field.key]);
  }
  if (nodeType === 'action.telegram_notify') {
    fields.botToken = toStringValue(cfg.botToken);
  }
  if (nodeType === 'action.place_order') {
    if (!fields.tpPriceCent.trim()) {
      const legacyTpPrice = Number(cfg.tpPrice);
      if (Number.isFinite(legacyTpPrice) && legacyTpPrice > 0 && legacyTpPrice <= 1) {
        fields.tpPriceCent = String(Math.round(legacyTpPrice * 100));
      }
    }
    if (!fields.slPriceCent.trim()) {
      const legacySlPrice = Number(cfg.slPrice);
      if (Number.isFinite(legacySlPrice) && legacySlPrice > 0 && legacySlPrice <= 1) {
        fields.slPriceCent = String(Math.round(legacySlPrice * 100));
      }
    }
    if (!fields.slTriggerPriceMode || !fields.slTriggerPriceMode.trim()) {
      fields.slTriggerPriceMode = 'best_bid';
    }
    if (
      (fields.reenterOnSlHit ?? '').trim().toLowerCase() === 'true' &&
      !(fields.reentryMaxAttempts ?? '').trim()
    ) {
      fields.reentryMaxAttempts = '1';
    }
    if (
      (fields.priceToBeatGuardEnabled ?? '').trim().toLowerCase() === 'true' &&
      !['manual', 'auto_last_3_avg_excursion', 'auto_vol_pct'].includes(
        (fields.priceToBeatMode ?? '').trim().toLowerCase()
      )
    ) {
      fields.priceToBeatMode = 'manual';
    }
    if (
      (fields.priceToBeatGuardEnabled ?? '').trim().toLowerCase() === 'true' &&
      !(fields.notifyOnPriceToBeatGapBlocked ?? '').trim()
    ) {
      fields.notifyOnPriceToBeatGapBlocked = 'true';
    }
    if (
      (fields.priceToBeatGuardEnabled ?? '').trim().toLowerCase() === 'true' &&
      (fields.priceToBeatMode ?? '').trim().toLowerCase() === 'manual' &&
      !['usd', 'cent'].includes((fields.priceToBeatMaxDiffUnit ?? '').trim().toLowerCase())
    ) {
      fields.priceToBeatMaxDiffUnit = 'usd';
    }
    if (
      (fields.counterLegPriceToBeatGuardEnabled ?? '').trim().toLowerCase() === 'true' &&
      !['manual', 'auto_last_3_avg_excursion', 'auto_vol_pct'].includes((fields.counterLegPriceToBeatMode ?? '').trim().toLowerCase())
    ) {
      fields.counterLegPriceToBeatMode = 'manual';
    }
    if (
      (fields.counterLegPriceToBeatGuardEnabled ?? '').trim().toLowerCase() === 'true' &&
      (fields.counterLegPriceToBeatMode ?? '').trim().toLowerCase() === 'manual' &&
      !['usd', 'cent'].includes((fields.counterLegPriceToBeatMaxDiffUnit ?? '').trim().toLowerCase())
    ) {
      fields.counterLegPriceToBeatMaxDiffUnit = 'usd';
    }
    if (
      (fields.priceToBeatStopLossBumpEnabled ?? '').trim().toLowerCase() === 'true' &&
      !['usd', 'cent'].includes((fields.priceToBeatStopLossBumpUnit ?? '').trim().toLowerCase())
    ) {
      fields.priceToBeatStopLossBumpUnit = ['usd', 'cent'].includes(
        (fields.priceToBeatMaxDiffUnit ?? '').trim().toLowerCase()
      )
        ? fields.priceToBeatMaxDiffUnit
        : 'cent';
    }
    if ((fields.priceToBeatGuardEnabled ?? '').trim().toLowerCase() === 'true') {
      if (!(fields.priceToBeatMaxPriceRelaxMissCount ?? '').trim()) {
        fields.priceToBeatMaxPriceRelaxMissCount = '5';
      }
      if (!(fields.priceToBeatMaxPriceRelaxHistoryCount ?? '').trim()) {
        fields.priceToBeatMaxPriceRelaxHistoryCount = '5';
      }
      if (!(fields.priceToBeatMaxPriceRelaxMinDepthUsd ?? '').trim()) {
        fields.priceToBeatMaxPriceRelaxMinDepthUsd = '5';
      }
      if (!(fields.priceToBeatMaxPriceRelaxStepMode ?? '').trim()) {
        fields.priceToBeatMaxPriceRelaxStepMode = 'percent';
      }
      if (!(fields.priceToBeatMaxPriceRelaxStepValue ?? '').trim()) {
        fields.priceToBeatMaxPriceRelaxStepValue = '25';
      }
      if (
        (fields.priceToBeatMaxPriceRelaxMinValue ?? '').trim() &&
        !['usd', 'cent'].includes((fields.priceToBeatMaxPriceRelaxMinUnit ?? '').trim().toLowerCase())
      ) {
        fields.priceToBeatMaxPriceRelaxMinUnit = 'usd';
      }
      if (
        (fields.priceToBeatMaxPriceRelaxStepMode ?? '').trim().toLowerCase() === 'absolute' &&
        !['usd', 'cent'].includes((fields.priceToBeatMaxPriceRelaxStepUnit ?? '').trim().toLowerCase())
      ) {
        fields.priceToBeatMaxPriceRelaxStepUnit = 'usd';
      }
    }
    if (
      (fields.priceToBeatStopLossBumpEnabled ?? '').trim().toLowerCase() === 'true' &&
      !['global', 'per_scope'].includes(
        (fields.priceToBeatStopLossBumpScope ?? '').trim().toLowerCase()
      )
    ) {
      fields.priceToBeatStopLossBumpScope = 'per_scope';
    }
    if (
      (fields.ptbStopLossEnabled ?? '').trim().toLowerCase() === 'true' &&
      !['none', 'tighten', 'relax'].includes(
        (fields.ptbStopLossTimeDecayMode ?? '').trim().toLowerCase()
      )
    ) {
      fields.ptbStopLossTimeDecayMode = 'tighten';
    }
    if (!fields.sizePct.trim()) {
      fields.sizePct = toStringValue(cfg.sizePercent);
    }
    applyPairLockFormDefaults(fields, cfg);
    applyPtbStopLossFormDefaults(fields, cfg);
    const pairLockMode = fields.mode === 'pair_lock';
    if (Array.isArray(cfg.tpRules)) {
      for (const item of cfg.tpRules as Record<string, unknown>[]) {
        if (!isRecord(item)) continue;
        tpRuleRows.push({
          ...createEmptyExitLadderRuleRow(),
          priceCent: toCentStringValue(item.priceCent, item.price),
          sizePct: toStringValue(item.sizePct),
        });
      }
    }
    if (tpRuleRows.length > 0) {
      fields.tpEnabled = 'true';
    }
    if (Array.isArray(cfg.counterLegTpRules)) {
      for (const item of cfg.counterLegTpRules as Record<string, unknown>[]) {
        if (!isRecord(item)) continue;
        counterLegTpRuleRows.push({
          ...createEmptyExitLadderRuleRow(),
          priceCent: toCentStringValue(item.priceCent, item.price),
          sizePct: toStringValue(item.sizePct),
        });
      }
    }
    if (counterLegTpRuleRows.length > 0) {
      fields.counterLegTpEnabled = 'true';
    }

    if (Array.isArray(cfg.slRules)) {
      for (const item of cfg.slRules as Record<string, unknown>[]) {
        if (!isRecord(item)) continue;
        slRuleRows.push({
          ...createEmptyExitLadderRuleRow(),
          priceCent: toCentStringValue(item.priceCent, item.price),
          sizePct: toStringValue(item.sizePct),
        });
      }
    }
    if (slRuleRows.length > 0) {
      fields.slEnabled = 'true';
    }

    ptbStopLossRuleRows.push(...parsePtbStopLossRuleRows(cfg, pairLockMode));
    if (shouldEnablePtbStopLossFromConfig(cfg, ptbStopLossRuleRows)) {
      fields.ptbStopLossEnabled = 'true';
    }

    if (Array.isArray(cfg.timeExitRules)) {
      for (const item of cfg.timeExitRules as Record<string, unknown>[]) {
        if (!isRecord(item)) continue;
        timeExitRuleRows.push({
          ...createEmptyTimeExitRuleRow(),
          elapsedMinutes: toStringValue(item.elapsedMinutes),
          remainingPct: toStringValue(item.remainingPct),
        });
      }
    }
    fields.presetKind = toStringValue(fields.presetKind || cfg.presetKind);
    const existingMode = String(fields.sizeMode ?? '').trim().toLowerCase();
    if (existingMode !== 'usdc' && existingMode !== 'pct') {
      const hasPct =
        typeof cfg.sizePct === 'number' ||
        (typeof cfg.sizePct === 'string' && cfg.sizePct.trim().length > 0) ||
        typeof cfg.sizePercent === 'number' ||
        (typeof cfg.sizePercent === 'string' && cfg.sizePercent.trim().length > 0);
      fields.sizeMode = hasPct ? 'pct' : 'usdc';
    }
    const parsedRows = parseNumberArrayToStringRows(cfg.triggerSizes).slice(0, 20);
    const parsedMaxTriggers = Number(fields.maxTriggers ?? '');
    const rowTarget =
      Number.isFinite(parsedMaxTriggers) && parsedMaxTriggers > 1
        ? Math.min(20, Math.floor(parsedMaxTriggers))
        : 0;
    triggerSizeRows =
      rowTarget > 0
        ? Array.from({ length: rowTarget }, (_, index) => parsedRows[index] ?? '')
        : parsedRows;

    const isPresetPlaceOrder = isPresetPlaceOrderMarker(
      fields.presetKind,
      fields.refKey || cfg.refKey
    );
    const isPresetBuySell = isPresetBuySellPlaceOrderMarker(
      fields.presetKind,
      fields.refKey || cfg.refKey
    );
    if (isPresetPlaceOrder) {
      if (!(fields.presetKind ?? '').trim()) {
        const ref = toStringValue(fields.refKey || cfg.refKey).trim().toLowerCase();
        if (ref === 'preset_sell_current_position') {
          fields.presetKind = 'sell_current_position';
        } else if (ref === 'preset_buy_current_position') {
          fields.presetKind = 'buy_current_position';
        } else if (ref === 'preset_place_order') {
          fields.presetKind = 'place_order';
        }
      }
      fields.kind = 'immediate';
      fields.triggerCondition = '';
      fields.triggerPrice = '';
      fields.triggerPriceCent = '';
      if (isPresetBuySell) {
        fields.executionMode = 'market';
      }
    }
  }
  if (nodeType === 'action.resolve_market') {
    const legacy = normalizeResolveMarketScope(cfg.marketScope);
    if (!fields.asset.trim()) {
      fields.asset = toStringValue(cfg.asset).trim().toLowerCase() || legacy?.asset || 'btc';
    }
    if (!fields.timeframe.trim()) {
      fields.timeframe =
        toStringValue(cfg.timeframe).trim().toLowerCase() || legacy?.timeframe || '5m';
    }
    if (!fields.selection.trim()) fields.selection = 'latest_by_slug';
    if (!fields.outcomeLabel.trim()) {
      fields.outcomeLabel = toStringValue(cfg.outcomeLabel).trim().toLowerCase() || 'yes';
    }
    if (!fields.failOnMissingMarket.trim()) fields.failOnMissingMarket = 'true';
    if (!fields.requireYesNoTokens.trim()) fields.requireYesNoTokens = 'true';
    if (!fields.requireTokenId.trim()) fields.requireTokenId = 'true';
    if (!fields.varPrefix.trim()) fields.varPrefix = 'resolved_market';
  }
  if (nodeType === 'action.dual_dca') {
    const asset =
      toStringValue(cfg.asset).trim().toLowerCase() ||
      toStringValue(cfg.coin).trim().toLowerCase();
    const timeframeRaw =
      toStringValue(cfg.timeframe).trim().toLowerCase() ||
      toStringValue(cfg.marketPeriod).trim().toLowerCase();
    const timeframe =
      timeframeRaw === '5' || timeframeRaw === '5min' || timeframeRaw === '5 min'
        ? '5m'
        : timeframeRaw === '15' || timeframeRaw === '15min' || timeframeRaw === '15 min'
          ? '15m'
          : timeframeRaw;
    const sideModeRaw =
      toStringValue(cfg.sideMode).trim().toLowerCase() ||
      toStringValue(cfg.side).trim().toLowerCase();
    const sideMode =
      sideModeRaw === 'up' || sideModeRaw === 'down' || sideModeRaw === 'all'
        ? sideModeRaw
        : '';
    const baseSizingRaw =
      toStringValue(cfg.baseSizing).trim().toLowerCase() ||
      toStringValue(cfg.baseSizeMode).trim().toLowerCase();

    if (!fields.asset.trim() && asset) fields.asset = asset;
    if (!fields.timeframe.trim() && timeframe) fields.timeframe = timeframe;
    if (!fields.sideMode.trim() && sideMode) fields.sideMode = sideMode;
    if (
      !fields.baseSizing.trim() &&
      (baseSizingRaw === 'usdc' || baseSizingRaw === 'shares')
    ) {
      fields.baseSizing = baseSizingRaw;
    }
    if (!fields.tpProfitPct.trim()) {
      fields.tpProfitPct = toStringValue(cfg.tpProfitPct ?? cfg.tpProfit);
    }
    if (!fields.slLossPct.trim()) {
      fields.slLossPct = toStringValue(cfg.slLossPct ?? cfg.slLoss);
    }
    if (!fields.slSpreadPct.trim()) {
      fields.slSpreadPct = toStringValue(cfg.slSpreadPct ?? cfg.slSpread);
    }
  }

  if (nodeType === 'trigger.market_price') {
    const marketModeRaw = toStringValue(cfg.marketMode).trim().toLowerCase();
    const marketMode = marketModeRaw === 'auto_scope' ? 'auto_scope' : 'fixed';
    fields.marketMode = marketMode;
    const priceModeRaw = toStringValue(cfg.priceMode).trim().toLowerCase();
    const validPriceModes = ['composite', 'midpoint', 'raw', 'last_trade', 'site_display', 'best_bid', 'best_ask'];
    fields.priceMode = validPriceModes.includes(priceModeRaw) ? priceModeRaw : 'composite';

    const scopeRaw = toStringValue(cfg.marketScope).trim().toLowerCase();
    if (scopeRaw && RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[scopeRaw]) {
      fields.marketScope = scopeRaw;
    }

    const selectionRaw = toStringValue(cfg.marketSelection).trim().toLowerCase();
    fields.marketSelection = selectionRaw || 'latest_by_slug';
    const protectionModeRaw = toStringValue(cfg.protectionMode).trim().toLowerCase();
    fields.protectionMode =
      protectionModeRaw === 'underlying_confirm' ? 'underlying_confirm' : 'off';
    const protectionPresetRaw = toStringValue(cfg.protectionPreset).trim().toLowerCase();
    fields.protectionPreset =
      protectionPresetRaw === 'loose' ||
      protectionPresetRaw === 'balanced' ||
      protectionPresetRaw === 'strict'
        ? protectionPresetRaw
        : 'balanced';

    const repeatModeRaw = toStringValue(fields.repeatMode || cfg.repeatMode).trim().toLowerCase();
    fields.repeatMode = repeatModeRaw === 'once' ? 'once' : 'loop';
    fields.onceScope = resolveTriggerMarketOnceScope(cfg, marketMode, fields.repeatMode as 'once' | 'loop');
    const bindingModeRaw = toStringValue(cfg.bindingMode).trim().toLowerCase();
    fields.bindingMode = bindingModeRaw === 'pair_lock_only' ? 'pair_lock_only' : 'standard';

    const cycleWindowFields = readTriggerMarketPriceCycleWindowFields(cfg);
    fields.cycleWindowMode = cycleWindowFields.cycleWindowMode;
    fields.cycleWindowSecs = cycleWindowFields.cycleWindowSecs;
    fields.cycleWindowStartSec = cycleWindowFields.cycleWindowStartSec;
    fields.cycleWindowEndSec = cycleWindowFields.cycleWindowEndSec;
    if (cycleWindowFields.autoSellOnWindowEnd != null) {
      fields.autoSellOnWindowEnd = cycleWindowFields.autoSellOnWindowEnd;
    }
    if (
      (fields.priceToBeatTriggerEnabled ?? '').trim().toLowerCase() === 'true' &&
      !['manual', 'auto_last_3_avg_excursion', 'auto_vol_pct'].includes(
        (fields.priceToBeatMode ?? '').trim().toLowerCase()
      )
    ) {
      fields.priceToBeatMode = 'manual';
    }
    if (
      (fields.priceToBeatTriggerEnabled ?? '').trim().toLowerCase() === 'true' &&
      (fields.priceToBeatMode ?? '').trim().toLowerCase() === 'manual' &&
      !['usd', 'cent'].includes((fields.priceToBeatTriggerUnit ?? '').trim().toLowerCase())
    ) {
      fields.priceToBeatTriggerUnit = 'usd';
    }

  }

  if (nodeType === 'trigger.open_positions') {
    fields.maxPriceCent = toCentStringValue(fields.maxPriceCent || cfg.maxPriceCent, cfg.maxPrice);
  }

  const outcomeConditionRows: OutcomeConditionRow[] = [];
  let drawdownRuleRows: DrawdownRuleRow[] = [];
  if (nodeType === 'trigger.open_positions' || nodeType === 'trigger.market_price') {
    const pairLockOnlyTrigger =
      nodeType === 'trigger.market_price' &&
      toStringValue(cfg.bindingMode).trim().toLowerCase() === 'pair_lock_only';
    if (Array.isArray(cfg.outcomeConditions) && !pairLockOnlyTrigger) {
      for (const item of cfg.outcomeConditions as Record<string, unknown>[]) {
        if (!isRecord(item)) continue;
        outcomeConditionRows.push({
          id: createId('oc'),
          tokenId: toStringValue(item.tokenId),
          outcomeLabel: toStringValue(item.outcomeLabel),
          triggerCondition: toStringValue(item.triggerCondition),
          triggerPriceCent: toCentStringValue(item.triggerPriceCent, item.triggerPrice),
          maxPriceCent: toCentStringValue(item.maxPriceCent, item.maxPrice),
        });
      }
    } else if (
      !pairLockOnlyTrigger &&
      toStringValue(cfg.tokenId).trim() &&
      (
        toStringValue(cfg.triggerCondition).trim() ||
        (
          nodeType === 'trigger.market_price' &&
          cfg.priceToBeatTriggerEnabled === true
        )
      )
    ) {
      outcomeConditionRows.push({
        id: createId('oc'),
        tokenId: toStringValue(cfg.tokenId),
        outcomeLabel: toStringValue(cfg.outcomeLabel),
        triggerCondition: toStringValue(cfg.triggerCondition),
        triggerPriceCent: toCentStringValue(cfg.triggerPriceCent, cfg.triggerPrice),
        maxPriceCent: toCentStringValue(cfg.maxPriceCent, cfg.maxPrice),
      });
    }
  }
  if (nodeType === 'trigger.position_drawdown') {
    fields.tokenId = toStringValue(cfg.tokenId).trim();
    if (!fields.entryPriceCent?.trim()) {
      const legacyEntry = Number(toStringValue(cfg.entryPrice).trim());
      if (Number.isFinite(legacyEntry) && legacyEntry > 0) {
        fields.entryPriceCent = String(legacyEntry * 100);
      }
    }
    if (Array.isArray(cfg.lossRules)) {
      for (const item of cfg.lossRules as Record<string, unknown>[]) {
        if (!isRecord(item)) continue;
        const lossPctRaw = toStringValue(item.lossPct).trim();
        const directionRaw = toStringValue(item.direction).trim().toLowerCase();
        const direction: 'down' | 'up' = directionRaw === 'up' ? 'up' : 'down';
        const windowMsValue = Number(toStringValue(item.windowMs).trim());
        const durationValue =
          Number.isFinite(windowMsValue) && windowMsValue > 0 ? String(Math.floor(windowMsValue)) : '';
        drawdownRuleRows.push({
          id: createId('dr'),
          direction,
          lossPct: lossPctRaw,
          durationValue,
        });
      }
    }
    if (drawdownRuleRows.length === 0) {
      const fallbackLossPct = toStringValue(cfg.lossPct).trim();
      const fallbackWindowMs = Number(toStringValue(cfg.windowMs).trim());
      const durationValue =
        Number.isFinite(fallbackWindowMs) && fallbackWindowMs > 0 ? String(Math.floor(fallbackWindowMs)) : '';
      if (fallbackLossPct) {
        drawdownRuleRows.push({
          id: createId('dr'),
          direction: 'down',
          lossPct: fallbackLossPct,
          durationValue,
        });
      }
    }
    if (drawdownRuleRows.length === 0) {
      drawdownRuleRows = [createEmptyDrawdownRuleRow()];
    }
  }

  const expression = parseExpressionDraft(cfg.expression);
  const patchRows = objectToRows(cfg.statePatch ?? cfg.state);

  return {
    fields,
    triggerSizeRows,
    outcomeConditionRows,
    drawdownRuleRows,
    tpRuleRows,
    counterLegTpRuleRows,
    slRuleRows,
    ptbStopLossRuleRows,
    timeExitRuleRows,
    expressionRows: expression.rows,
    expressionJoin: expression.join,
    expressionSupported: expression.supported,
    nestedExprMode: false,
    nestedExprGroup: null,
    statePatchRows: patchRows.length > 0 ? patchRows : [createEmptyKeyValueDraft()],
    advancedJson: safeJsonStringify(cfg),
  };
}

export function buildNodeConfigFromForm(
  nodeType: string,
  form: NodeConfigFormState
): Record<string, unknown> {
  const config: Record<string, unknown> = {};

  for (const field of NODE_FIELD_SCHEMAS[nodeType] || []) {
    const raw = (form.fields[field.key] ?? '').trim();
    if (!raw) continue;

    if (field.input === 'datetime-local') {
      const parsed = new Date(raw);
      config[field.key] = Number.isNaN(parsed.getTime()) ? raw : parsed.toISOString();
      continue;
    }

    if (NUMERIC_KEYS.has(field.key)) {
      const parsed = Number(raw);
      if (Number.isFinite(parsed)) {
        config[field.key] = parsed;
      }
      continue;
    }
    if (BOOLEAN_KEYS.has(field.key)) {
      const normalized = raw.toLowerCase();
      if (['true', '1', 'yes', 'y', 'on'].includes(normalized)) {
        config[field.key] = true;
        continue;
      }
      if (['false', '0', 'no', 'n', 'off'].includes(normalized)) {
        config[field.key] = false;
        continue;
      }
    }
    config[field.key] = raw;
  }

  if (nodeType === 'action.place_order') {
    const presetKindRaw = (form.fields.presetKind ?? '').trim();
    if (presetKindRaw) {
      config.presetKind = presetKindRaw;
    }

    const executionModeRaw = (form.fields.executionMode ?? '').trim().toLowerCase();
    if (executionModeRaw === 'market' || executionModeRaw === 'limit') {
      config.executionMode = executionModeRaw;
    } else {
      delete config.executionMode;
    }

    const sizeModeRaw = (form.fields.sizeMode ?? '').trim().toLowerCase();
    const sizeMode = sizeModeRaw === 'pct' ? 'pct' : 'usdc';
    config.sizeMode = sizeMode;

    if (sizeMode === 'pct') {
      delete config.sizeUsdc;
      delete config.targetNotionalUsdc;
    } else {
      delete config.sizePct;
    }

    const parsedMaxTriggers = Number(form.fields.maxTriggers ?? '');
    const normalizedMaxTriggers =
      Number.isFinite(parsedMaxTriggers) && parsedMaxTriggers > 0
        ? Math.min(20, Math.floor(parsedMaxTriggers))
        : null;
    const triggerSizes = (form.triggerSizeRows || [])
      .map((value) => Number(value.trim()))
      .filter((value) => Number.isFinite(value) && value > 0);
    if ((normalizedMaxTriggers ?? 0) > 1 && triggerSizes.length > 0) {
      const normalizedTriggerSizes = triggerSizes.slice(0, normalizedMaxTriggers ?? triggerSizes.length);
      config.triggerSizes = normalizedTriggerSizes;
      const firstValue = normalizedTriggerSizes[0];
      if (sizeMode === 'pct' && config.sizePct == null) {
        config.sizePct = firstValue;
      }
      if (sizeMode !== 'pct' && config.sizeUsdc == null && config.targetNotionalUsdc == null) {
        config.sizeUsdc = firstValue;
      }
    } else {
      delete config.triggerSizes;
    }

    const isPresetPlaceOrder = isPresetPlaceOrderMarker(config.presetKind, config.refKey);
    if (isPresetPlaceOrder) {
      config.kind = 'immediate';
      delete config.triggerCondition;
      delete config.triggerPrice;
      delete config.triggerPriceCent;
      if (isPresetBuySellPlaceOrderMarker(config.presetKind, config.refKey)) {
        config.executionMode = 'market';
      }
    }

    const sideRaw = toStringValue(config.side).trim().toLowerCase();
    const isBuySide = sideRaw === 'buy';
    const pairLockMode = toStringValue(config.mode).trim().toLowerCase() === 'pair_lock';
    const tpRules = (form.tpRuleRows || [])
      .map((row) => {
        const priceCent = Number(row.priceCent.trim());
        const sizePct = Number(row.sizePct.trim());
        if (!Number.isFinite(priceCent) || priceCent <= 0 || priceCent > 100) return null;
        if (!Number.isFinite(sizePct) || sizePct <= 0 || sizePct > 100) return null;
        return { priceCent, sizePct };
      })
      .filter((item): item is { priceCent: number; sizePct: number } => item != null);
    const counterLegTpRules = (form.counterLegTpRuleRows || [])
      .map((row) => {
        const priceCent = Number(row.priceCent.trim());
        const sizePct = Number(row.sizePct.trim());
        if (!Number.isFinite(priceCent) || priceCent <= 0 || priceCent > 100) return null;
        if (!Number.isFinite(sizePct) || sizePct <= 0 || sizePct > 100) return null;
        return { priceCent, sizePct };
      })
      .filter((item): item is { priceCent: number; sizePct: number } => item != null);
    const slRules = (form.slRuleRows || [])
      .map((row) => {
        const priceCent = Number(row.priceCent.trim());
        const sizePct = Number(row.sizePct.trim());
        if (!Number.isFinite(priceCent) || priceCent <= 0 || priceCent > 100) return null;
        if (!Number.isFinite(sizePct) || sizePct <= 0 || sizePct > 100) return null;
        return { priceCent, sizePct };
      })
      .filter((item): item is { priceCent: number; sizePct: number } => item != null);
    const ptbStopLossRules = buildPtbStopLossRules(form.ptbStopLossRuleRows || []);
    const timeExitRules = (form.timeExitRuleRows || [])
      .map((row) => {
        const elapsedMinutes = Number(row.elapsedMinutes.trim());
        const remainingPct = Number(row.remainingPct.trim());
        if (!Number.isFinite(elapsedMinutes) || elapsedMinutes <= 0 || !Number.isInteger(elapsedMinutes)) return null;
        if (!Number.isFinite(remainingPct) || remainingPct <= 0 || remainingPct > 100) return null;
        return { elapsedMinutes, remainingPct };
      })
      .filter((item): item is { elapsedMinutes: number; remainingPct: number } => item != null);
    const tpEnabled = config.tpEnabled === true || tpRules.length > 0;
    const counterLegTpEnabled =
      config.counterLegTpEnabled === true || counterLegTpRules.length > 0;
    const slEnabled = config.slEnabled === true || slRules.length > 0;
    const ptbStopLossEnabled = config.ptbStopLossEnabled === true;
    const anyStopLossEnabled = slEnabled || ptbStopLossEnabled || ptbStopLossRules.length > 0;
    if (pairLockMode) {
      if (tpRules.length > 0) {
        config.tpRules = tpRules;
      } else {
        delete config.tpRules;
      }
      if (counterLegTpRules.length > 0) {
        config.counterLegTpRules = counterLegTpRules;
      } else {
        delete config.counterLegTpRules;
      }
      if (ptbStopLossRules.length > 0) {
        config.ptbStopLossRules = ptbStopLossRules;
      } else {
        delete config.ptbStopLossRules;
      }
      normalizePairLockBuildConfig(config);
      if (!counterLegTpEnabled) {
        delete config.counterLegTpEnabled;
        delete config.counterLegTpPriceCent;
        delete config.counterLegNotifyOnTpHit;
      }
      normalizePairLockTakeProfitBuildConfig(config);
    } else {
      delete config.mode;
      for (const key of PAIR_LOCK_CONFIG_KEYS) delete config[key];
    }
    if (!isBuySide) {
      delete config.tpEnabled;
      delete config.tpPriceCent;
      delete config.tpPrice;
      delete config.tpRules;
      delete config.slEnabled;
      delete config.slPriceCent;
      delete config.slPrice;
      delete config.ptbStopLossEnabled;
      delete config.ptbStopLossGapUsd;
      delete config.ptbStopLossGapUnit;
      delete config.ptbStopLossRules;
      delete config.slRules;
      delete config.slTriggerPriceMode;
      delete config.timeExitRules;
      delete config.notifyOnTriggerPriceBlocked;
      delete config.notifyOnExecutionFloorBlocked;
      delete config.notifyOnMaxPriceBlocked;
      delete config.retryOnMaxPriceBlock;
      delete config.retryOnTriggerPriceGuardBlock;
      delete config.retryOnExecutionFloorGuardBlock;
      delete config.executionFloorPriceCent;
      delete config.retryOnPriceToBeatGuardBlock;
      delete config.priceToBeatGuardEnabled;
      delete config.priceToBeatMaxDiff;
      delete config.priceToBeatMaxDiffUnit;
      delete config.priceToBeatStopLossBumpEnabled;
      delete config.priceToBeatStopLossBumpAmount;
      delete config.priceToBeatStopLossBumpUnit;
      delete config.priceToBeatStopLossBumpScope;
      delete config.priceToBeatStopLossBumpDecayWindows;
      delete config.notifyOnPriceToBeatGapBlocked;
      delete config.priceToBeatMaxPriceRelaxMissCount;
      delete config.priceToBeatMaxPriceRelaxHistoryCount;
      delete config.priceToBeatMaxPriceRelaxMinValue;
      delete config.priceToBeatMaxPriceRelaxMinUnit;
      delete config.priceToBeatMaxPriceRelaxMinDepthUsd;
      delete config.priceToBeatMaxPriceRelaxStepMode;
      delete config.priceToBeatMaxPriceRelaxStepValue;
      delete config.priceToBeatMaxPriceRelaxStepUnit;
      delete config.notifyOnTpHit;
      delete config.notifyOnSlHit;
      delete config.reenterOnSlHit;
      delete config.reentryMaxAttempts;
      delete config.reentryCooldownSec;
      delete config.reentrySkipCurrentWindow;
      delete config.reentryMinPriceCent;
      delete config.reentryMaxPriceCent;
      delete config.reentryThresholdDecay;
      delete config.reentryMaxPriceTightenBps;
      delete config.reentryPriceToBeatMaxDiff;
      delete config.reentryPriceToBeatMaxDiffUnit;
      delete config.ptbStopLossTimeDecayMode;
    } else if (pairLockMode) {
      normalizePairLockStopLossBuildConfig(config);
    } else {
      if (config.triggerPriceGuardEnabled !== true) {
        delete config.notifyOnTriggerPriceBlocked;
        delete config.retryOnTriggerPriceGuardBlock;
      }
      if (config.executionFloorGuardEnabled !== true) {
        delete config.notifyOnExecutionFloorBlocked;
        delete config.retryOnExecutionFloorGuardBlock;
        delete config.executionFloorPriceCent;
      }
      if (config.priceToBeatGuardEnabled !== true) {
        delete config.priceToBeatMode;
        delete config.priceToBeatMaxDiff;
        delete config.priceToBeatMaxDiffUnit;
        delete config.priceToBeatStopLossBumpEnabled;
        delete config.priceToBeatStopLossBumpAmount;
        delete config.priceToBeatStopLossBumpUnit;
        delete config.priceToBeatStopLossBumpScope;
        delete config.priceToBeatStopLossBumpDecayWindows;
        delete config.priceToBeatMaxPriceRelaxMissCount;
        delete config.priceToBeatMaxPriceRelaxHistoryCount;
        delete config.priceToBeatMaxPriceRelaxMinValue;
        delete config.priceToBeatMaxPriceRelaxMinUnit;
        delete config.priceToBeatMaxPriceRelaxMinDepthUsd;
        delete config.priceToBeatMaxPriceRelaxStepMode;
        delete config.priceToBeatMaxPriceRelaxStepValue;
        delete config.priceToBeatMaxPriceRelaxStepUnit;
        delete config.reentryPriceToBeatMaxDiff;
        delete config.reentryPriceToBeatMaxDiffUnit;
        delete config.reentryThresholdDecay;
        delete config.notifyOnPriceToBeatGapBlocked;
        delete config.retryOnPriceToBeatGuardBlock;
      } else {
        const priceToBeatModeRaw = toStringValue(config.priceToBeatMode).trim().toLowerCase();
        config.priceToBeatMode =
          priceToBeatModeRaw === 'auto_last_3_avg_excursion'
            ? 'auto_last_3_avg_excursion'
            : priceToBeatModeRaw === 'auto_vol_pct'
              ? 'auto_vol_pct'
            : 'manual';
        if (config.priceToBeatMode === 'manual') {
          const priceToBeatUnitRaw = toStringValue(config.priceToBeatMaxDiffUnit).trim().toLowerCase();
          config.priceToBeatMaxDiffUnit =
            priceToBeatUnitRaw === 'cent' ? 'cent' : 'usd';
        } else {
          delete config.priceToBeatMaxDiff;
          delete config.priceToBeatMaxDiffUnit;
        }
        const relaxMissCount = Number(
          toStringValue(config.priceToBeatMaxPriceRelaxMissCount).trim()
        );
        if (Number.isInteger(relaxMissCount) && relaxMissCount > 0) {
          config.priceToBeatMaxPriceRelaxMissCount = relaxMissCount;
        } else {
          delete config.priceToBeatMaxPriceRelaxMissCount;
        }
        const relaxHistoryCount = Number(
          toStringValue(config.priceToBeatMaxPriceRelaxHistoryCount).trim()
        );
        if (Number.isInteger(relaxHistoryCount) && relaxHistoryCount > 0) {
          config.priceToBeatMaxPriceRelaxHistoryCount = relaxHistoryCount;
        } else {
          delete config.priceToBeatMaxPriceRelaxHistoryCount;
        }
        const relaxMinValue = Number(
          toStringValue(config.priceToBeatMaxPriceRelaxMinValue).trim()
        );
        if (Number.isFinite(relaxMinValue) && relaxMinValue > 0) {
          config.priceToBeatMaxPriceRelaxMinValue = relaxMinValue;
          const relaxMinUnitRaw = toStringValue(config.priceToBeatMaxPriceRelaxMinUnit)
            .trim()
            .toLowerCase();
          config.priceToBeatMaxPriceRelaxMinUnit =
            relaxMinUnitRaw === 'usd' || relaxMinUnitRaw === 'cent'
              ? relaxMinUnitRaw
              : 'usd';
        } else {
          delete config.priceToBeatMaxPriceRelaxMinValue;
          delete config.priceToBeatMaxPriceRelaxMinUnit;
        }
        const relaxMinDepthUsd = Number(
          toStringValue(config.priceToBeatMaxPriceRelaxMinDepthUsd).trim()
        );
        if (Number.isFinite(relaxMinDepthUsd) && relaxMinDepthUsd > 0) {
          config.priceToBeatMaxPriceRelaxMinDepthUsd = relaxMinDepthUsd;
        } else {
          delete config.priceToBeatMaxPriceRelaxMinDepthUsd;
        }
        const relaxStepModeRaw = toStringValue(config.priceToBeatMaxPriceRelaxStepMode)
          .trim()
          .toLowerCase();
        config.priceToBeatMaxPriceRelaxStepMode =
          relaxStepModeRaw === 'absolute' ? 'absolute' : 'percent';
        const relaxStepValue = Number(
          toStringValue(config.priceToBeatMaxPriceRelaxStepValue).trim()
        );
        if (
          Number.isFinite(relaxStepValue) &&
          relaxStepValue > 0 &&
          (config.priceToBeatMaxPriceRelaxStepMode !== 'percent' || relaxStepValue <= 100)
        ) {
          config.priceToBeatMaxPriceRelaxStepValue = relaxStepValue;
        } else {
          config.priceToBeatMaxPriceRelaxStepMode = 'percent';
          config.priceToBeatMaxPriceRelaxStepValue = 25;
        }
        if (config.priceToBeatMaxPriceRelaxStepMode === 'absolute') {
          const relaxStepUnitRaw = toStringValue(config.priceToBeatMaxPriceRelaxStepUnit)
            .trim()
            .toLowerCase();
          config.priceToBeatMaxPriceRelaxStepUnit =
            relaxStepUnitRaw === 'cent' ? 'cent' : 'usd';
        } else {
          delete config.priceToBeatMaxPriceRelaxStepUnit;
        }
        if (config.priceToBeatStopLossBumpEnabled === true) {
          const bumpAmount = Number(
            toStringValue(config.priceToBeatStopLossBumpAmount).trim()
          );
          if (Number.isFinite(bumpAmount) && bumpAmount > 0) {
            config.priceToBeatStopLossBumpAmount = bumpAmount;
          } else {
            delete config.priceToBeatStopLossBumpAmount;
          }
          const bumpMaxValue = Number(
            toStringValue(config.priceToBeatStopLossBumpMaxValue).trim()
          );
          if (Number.isFinite(bumpMaxValue) && bumpMaxValue > 0) {
            config.priceToBeatStopLossBumpMaxValue = bumpMaxValue;
          } else {
            delete config.priceToBeatStopLossBumpMaxValue;
          }
          const bumpUnitRaw = toStringValue(config.priceToBeatStopLossBumpUnit)
            .trim()
            .toLowerCase();
          config.priceToBeatStopLossBumpUnit =
            bumpUnitRaw === 'usd' || bumpUnitRaw === 'cent'
              ? bumpUnitRaw
              : config.priceToBeatMode === 'manual' &&
                  (config.priceToBeatMaxDiffUnit === 'usd' ||
                    config.priceToBeatMaxDiffUnit === 'cent')
                ? config.priceToBeatMaxDiffUnit
                : 'usd';
          const bumpScopeRaw = toStringValue(config.priceToBeatStopLossBumpScope)
            .trim()
            .toLowerCase();
          config.priceToBeatStopLossBumpScope =
            bumpScopeRaw === 'global' ? 'global' : 'per_scope';
          const bumpDecayWindows = Number(
            toStringValue(config.priceToBeatStopLossBumpDecayWindows).trim()
          );
          if (Number.isInteger(bumpDecayWindows) && bumpDecayWindows > 0) {
            config.priceToBeatStopLossBumpDecayWindows = bumpDecayWindows;
          } else {
            delete config.priceToBeatStopLossBumpDecayWindows;
          }
        } else {
          delete config.priceToBeatStopLossBumpEnabled;
          delete config.priceToBeatStopLossBumpAmount;
          delete config.priceToBeatStopLossBumpMaxValue;
          delete config.priceToBeatStopLossBumpUnit;
          delete config.priceToBeatStopLossBumpScope;
          delete config.priceToBeatStopLossBumpDecayWindows;
        }
        if (config.priceToBeatMode !== 'manual') {
          delete config.priceToBeatStopLossBumpMaxValue;
        }
      }
      if (config.maxPriceCent == null) {
        delete config.notifyOnMaxPriceBlocked;
        delete config.retryOnMaxPriceBlock;
      }
      if (tpRules.length > 0) {
        config.tpRules = tpRules;
      } else {
        delete config.tpRules;
      }
      if (!tpEnabled) {
        delete config.tpEnabled;
        delete config.tpPriceCent;
        delete config.tpPrice;
        delete config.notifyOnTpHit;
      }
      if (slRules.length > 0) {
        config.slRules = slRules;
      } else {
        delete config.slRules;
      }
      if (ptbStopLossRules.length > 0) {
        config.ptbStopLossRules = ptbStopLossRules;
      } else {
        delete config.ptbStopLossRules;
      }
      if (config.ptbStopLossEnabled !== true) {
        delete config.ptbStopLossEnabled;
        delete config.ptbStopLossGapUsd;
        delete config.ptbStopLossGapUnit;
        delete config.ptbStopLossRules;
        delete config.ptbStopLossTimeDecayMode;
      } else {
        config.ptbStopLossGapUnit = normalizePtbStopLossGapUnit(config.ptbStopLossGapUnit);
        delete config.ptbStopLossGapUsd;
        const ptbStopLossTimeDecayModeRaw = toStringValue(config.ptbStopLossTimeDecayMode)
          .trim()
          .toLowerCase();
        config.ptbStopLossTimeDecayMode =
          ptbStopLossTimeDecayModeRaw === 'none' ||
          ptbStopLossTimeDecayModeRaw === 'relax'
            ? ptbStopLossTimeDecayModeRaw
            : 'tighten';
      }
      if (!slEnabled) {
        delete config.slEnabled;
        delete config.slPriceCent;
        delete config.slPrice;
        delete config.slRules;
        delete config.slTriggerPriceMode;
      }
      if (!anyStopLossEnabled) {
        delete config.notifyOnSlHit;
        delete config.reenterOnSlHit;
        delete config.reentryMaxAttempts;
        delete config.reentryCooldownSec;
        delete config.reentrySkipCurrentWindow;
        delete config.reentryMinPriceCent;
        delete config.reentryMaxPriceCent;
        delete config.reentryThresholdDecay;
        delete config.reentryMaxPriceTightenBps;
        delete config.reentryPriceToBeatMaxDiff;
        delete config.reentryPriceToBeatMaxDiffUnit;
      }
      if (timeExitRules.length > 0) {
        config.timeExitRules = timeExitRules;
      } else {
        delete config.timeExitRules;
      }
      if (slRules.length === 0 && ptbStopLossRules.length === 0) {
        delete config.stagedSlReentryOnlyAfterAllStages;
      }
      if (config.reenterOnSlHit !== true) {
        delete config.reentryMaxAttempts;
        delete config.reentryCooldownSec;
        delete config.reentrySkipCurrentWindow;
        delete config.reentryMinPriceCent;
        delete config.reentryMaxPriceCent;
        delete config.reentryThresholdDecay;
        delete config.reentryMaxPriceTightenBps;
        delete config.reentryPriceToBeatMaxDiff;
        delete config.reentryPriceToBeatMaxDiffUnit;
        delete config.stagedSlReentryOnlyAfterAllStages;
      } else if (config.priceToBeatGuardEnabled === true) {
        const reentryPriceToBeatMaxDiff = Number(
          toStringValue(config.reentryPriceToBeatMaxDiff).trim()
        );
        if (
          Number.isFinite(reentryPriceToBeatMaxDiff) &&
          reentryPriceToBeatMaxDiff > 0
        ) {
          config.reentryPriceToBeatMaxDiff = reentryPriceToBeatMaxDiff;
          const reentryPriceToBeatUnitRaw = toStringValue(
            config.reentryPriceToBeatMaxDiffUnit
          )
            .trim()
            .toLowerCase();
          if (reentryPriceToBeatUnitRaw === 'usd' || reentryPriceToBeatUnitRaw === 'cent') {
            config.reentryPriceToBeatMaxDiffUnit = reentryPriceToBeatUnitRaw;
          } else {
            delete config.reentryPriceToBeatMaxDiffUnit;
          }
        } else {
          delete config.reentryPriceToBeatMaxDiff;
          delete config.reentryPriceToBeatMaxDiffUnit;
        }
        const reentryThresholdDecay = Number(
          toStringValue(config.reentryThresholdDecay).trim()
        );
        if (
          Number.isFinite(reentryThresholdDecay) &&
          reentryThresholdDecay > 0 &&
          reentryThresholdDecay <= 1
        ) {
          config.reentryThresholdDecay = reentryThresholdDecay;
        } else {
          delete config.reentryThresholdDecay;
        }
      } else {
        delete config.reentryPriceToBeatMaxDiff;
        delete config.reentryPriceToBeatMaxDiffUnit;
        delete config.reentryThresholdDecay;
      }
      const reentryCooldownSec = Number(toStringValue(config.reentryCooldownSec).trim());
      if (Number.isInteger(reentryCooldownSec) && reentryCooldownSec >= 0) {
        config.reentryCooldownSec = reentryCooldownSec;
      } else {
        delete config.reentryCooldownSec;
      }
      if (config.reentrySkipCurrentWindow !== true) {
        delete config.reentrySkipCurrentWindow;
      }
      const reentryMaxPriceTightenBps = Number(
        toStringValue(config.reentryMaxPriceTightenBps).trim()
      );
      if (
        Number.isInteger(reentryMaxPriceTightenBps) &&
        reentryMaxPriceTightenBps >= 0 &&
        reentryMaxPriceTightenBps <= 10_000
      ) {
        config.reentryMaxPriceTightenBps = reentryMaxPriceTightenBps;
      } else {
        delete config.reentryMaxPriceTightenBps;
      }
    }
  }

  if (nodeType === 'action.resolve_market') {
    const derivedScope = toResolveMarketScope(config.asset, config.timeframe);
    if (derivedScope) {
      config.marketScope = derivedScope;
    } else {
      delete config.marketScope;
    }
    delete config.slugPrefix;
  }
  if (nodeType === 'action.dual_dca') {
    const assetRaw =
      toStringValue(config.asset).trim().toLowerCase() ||
      toStringValue(config.coin).trim().toLowerCase();
    if (assetRaw) {
      config.asset = assetRaw;
      config.coin = assetRaw.toUpperCase();
    } else {
      delete config.asset;
      delete config.coin;
    }

    const timeframeRaw =
      toStringValue(config.timeframe).trim().toLowerCase() ||
      toStringValue(config.marketPeriod).trim().toLowerCase();
    const timeframe =
      timeframeRaw === '5' || timeframeRaw === '5min' || timeframeRaw === '5 min'
        ? '5m'
        : timeframeRaw === '15' || timeframeRaw === '15min' || timeframeRaw === '15 min'
          ? '15m'
          : timeframeRaw;
    if (timeframe) {
      config.timeframe = timeframe;
      config.marketPeriod = timeframe;
    } else {
      delete config.timeframe;
      delete config.marketPeriod;
    }

    const sideModeRaw =
      toStringValue(config.sideMode).trim().toLowerCase() ||
      toStringValue(config.side).trim().toLowerCase();
    if (sideModeRaw) {
      const sideMode =
        sideModeRaw === 'up' || sideModeRaw === 'down' || sideModeRaw === 'all'
          ? sideModeRaw
          : sideModeRaw;
      config.sideMode = sideMode;
      config.side = sideMode;
    } else {
      delete config.sideMode;
      delete config.side;
    }

    const baseSizingRaw =
      toStringValue(config.baseSizing).trim().toLowerCase() ||
      toStringValue(config.baseSizeMode).trim().toLowerCase();
    if (baseSizingRaw) {
      const baseSizing =
        baseSizingRaw === 'usdc' || baseSizingRaw === 'shares'
          ? baseSizingRaw
          : baseSizingRaw;
      config.baseSizing = baseSizing;
      config.baseSizeMode = baseSizing;
      if (baseSizing === 'shares') {
        delete config.baseUsdc;
      } else if (baseSizing === 'usdc') {
        delete config.baseShares;
      }
    } else {
      delete config.baseSizing;
      delete config.baseSizeMode;
    }

    const derivedScope = toResolveMarketScope(config.asset, config.timeframe);
    if (derivedScope) {
      config.marketScope = derivedScope;
    } else {
      delete config.marketScope;
    }

    if (!toStringValue(config.refKey).trim()) {
      delete config.refKey;
    }
  }

  if (nodeType === 'trigger.market_price') {
    const marketModeRaw = toStringValue(form.fields.marketMode ?? config.marketMode).trim().toLowerCase();
    const marketMode = marketModeRaw === 'auto_scope' ? 'auto_scope' : 'fixed';
    config.marketMode = marketMode;
    const priceModeRaw = toStringValue(config.priceMode).trim().toLowerCase();
    const validPriceModes2 = ['composite', 'midpoint', 'raw', 'last_trade', 'site_display', 'best_bid', 'best_ask'];
    config.priceMode = validPriceModes2.includes(priceModeRaw) ? priceModeRaw : 'composite';

    const repeatModeRaw = toStringValue(config.repeatMode).trim().toLowerCase();
    config.repeatMode = repeatModeRaw === 'once' ? 'once' : 'loop';

    const onceScopeRaw = toStringValue(config.onceScope).trim().toLowerCase();
    if (config.repeatMode === 'once') {
      if (onceScopeRaw === 'market' || onceScopeRaw === 'run') {
        config.onceScope = onceScopeRaw;
      } else {
        config.onceScope = marketMode === 'auto_scope' ? 'market' : 'run';
      }
      config.onceScopeVersion = TRIGGER_MARKET_ONCE_SCOPE_VERSION;
    }

    const selectionRaw = toStringValue(config.marketSelection).trim().toLowerCase();
    config.marketSelection = selectionRaw || 'latest_by_slug';

    const confirmationMsRaw = toStringValue(form.fields.confirmationMs).trim();
    if (confirmationMsRaw) {
      const parsedConfirmationMs = Number(confirmationMsRaw);
      if (Number.isInteger(parsedConfirmationMs) && parsedConfirmationMs >= 0) {
        config.confirmationMs = parsedConfirmationMs;
      } else {
        delete config.confirmationMs;
      }
    }

    const scopeRaw = toStringValue(config.marketScope).trim().toLowerCase();
    if (marketMode === 'auto_scope') {
      if (scopeRaw && RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[scopeRaw]) {
        config.marketScope = scopeRaw;
      } else {
        delete config.marketScope;
      }
      const protectionModeRaw = toStringValue(config.protectionMode).trim().toLowerCase();
      if (protectionModeRaw === 'underlying_confirm') {
        config.protectionMode = 'underlying_confirm';
        const protectionPresetRaw = toStringValue(config.protectionPreset).trim().toLowerCase();
        config.protectionPreset =
          protectionPresetRaw === 'loose' ||
          protectionPresetRaw === 'balanced' ||
          protectionPresetRaw === 'strict'
            ? protectionPresetRaw
            : 'balanced';
      } else {
        delete config.protectionMode;
        delete config.protectionPreset;
      }
      // auto_scope resolves market slug at runtime.
      delete config.marketSlug;
      // Cycle window focus
      const normalizedCycleWindowConfig = normalizeTriggerMarketPriceCycleWindowConfig(config);
      for (const key of [
        'cycleWindowMode',
        'cycleWindowSecs',
        'cycleWindowStartSec',
        'cycleWindowEndSec',
        'autoSellOnWindowEnd',
      ] as const) {
        if (key in normalizedCycleWindowConfig) {
          config[key] = normalizedCycleWindowConfig[key];
        } else {
          delete config[key];
        }
      }
      if (config.priceToBeatTriggerEnabled === true) {
        const priceToBeatModeRaw = toStringValue(config.priceToBeatMode).trim().toLowerCase();
        config.priceToBeatMode =
          priceToBeatModeRaw === 'auto_last_3_avg_excursion'
            ? 'auto_last_3_avg_excursion'
            : priceToBeatModeRaw === 'auto_vol_pct'
              ? 'auto_vol_pct'
            : 'manual';
        if (config.priceToBeatMode === 'manual') {
          const ptbUnitRaw = toStringValue(config.priceToBeatTriggerUnit).trim().toLowerCase();
          config.priceToBeatTriggerUnit =
            ptbUnitRaw === 'cent' || ptbUnitRaw === 'usd' ? ptbUnitRaw : 'usd';
          const minGap = Number(toStringValue(config.priceToBeatTriggerMinGap).trim());
          if (Number.isFinite(minGap) && minGap > 0) {
            config.priceToBeatTriggerMinGap = minGap;
          } else {
            delete config.priceToBeatTriggerMinGap;
          }
          const maxGapRaw = toStringValue(config.priceToBeatTriggerMaxGap).trim();
          const maxGap = Number(maxGapRaw);
          if (
            maxGapRaw &&
            Number.isFinite(maxGap) &&
            maxGap > 0 &&
            Number.isFinite(minGap) &&
            maxGap >= minGap
          ) {
            config.priceToBeatTriggerMaxGap = maxGap;
          } else {
            delete config.priceToBeatTriggerMaxGap;
          }
        } else {
          delete config.priceToBeatTriggerUnit;
          delete config.priceToBeatTriggerMinGap;
          delete config.priceToBeatTriggerMaxGap;
        }
      } else {
        delete config.priceToBeatMode;
        delete config.priceToBeatTriggerUnit;
        delete config.priceToBeatTriggerMinGap;
        delete config.priceToBeatTriggerMaxGap;
      }
      if (toStringValue(config.bindingMode).trim().toLowerCase() === 'pair_lock_only') {
        delete config.priceToBeatTriggerEnabled;
        delete config.priceToBeatMode;
        delete config.priceToBeatTriggerUnit;
        delete config.priceToBeatTriggerMinGap;
        delete config.priceToBeatTriggerMaxGap;
        delete config.outcomeConditions;
        delete config.tokenId;
        delete config.outcomeLabel;
        delete config.triggerCondition;
        delete config.triggerPriceCent;
        delete config.triggerPrice;
        delete config.maxPriceCent;
        delete config.maxPrice;
      }
    } else {
      delete config.marketScope;
      delete config.marketSelection;
      delete config.protectionMode;
      delete config.protectionPreset;
      delete config.cycleWindowMode;
      delete config.cycleWindowSecs;
      delete config.cycleWindowStartSec;
      delete config.cycleWindowEndSec;
      delete config.autoSellOnWindowEnd;
      delete config.priceToBeatTriggerEnabled;
      delete config.priceToBeatMode;
      delete config.priceToBeatTriggerUnit;
      delete config.priceToBeatTriggerMinGap;
      delete config.priceToBeatTriggerMaxGap;
    }

    if (config.repeatMode !== 'once') {
      delete config.onceScope;
      delete config.onceScopeVersion;
    }
  }

  if (nodeType === 'trigger.position_drawdown') {
    const combineModeRaw = toStringValue(config.combineMode).trim().toLowerCase();
    if (combineModeRaw === 'and' || combineModeRaw === 'or') {
      config.combineMode = combineModeRaw;
    } else {
      delete config.combineMode;
    }
    const tokenIdRaw = toStringValue(form.fields.tokenId).trim();
    if (tokenIdRaw) {
      config.tokenId = tokenIdRaw;
    } else {
      delete config.tokenId;
    }

    const entryPriceCentRaw = Number(form.fields.entryPriceCent?.trim() ?? '');
    if (Number.isFinite(entryPriceCentRaw) && entryPriceCentRaw > 0 && entryPriceCentRaw <= 100) {
      config.entryPriceCent = entryPriceCentRaw;
    } else {
      delete config.entryPriceCent;
    }

    const rules = (form.drawdownRuleRows || [])
      .map((row) => {
        const lossPct = Number(row.lossPct.trim());
        if (!Number.isFinite(lossPct) || lossPct <= 0 || lossPct > 100) return null;
        const direction = row.direction === 'up' ? 'up' : 'down';

        const durationRaw = row.durationValue.trim();
        let windowMs: number | undefined;
        if (durationRaw) {
          const durationValue = Number(durationRaw);
          if (!Number.isFinite(durationValue) || durationValue <= 0) return null;
          windowMs = Math.floor(durationValue);
          if (!Number.isFinite(windowMs) || windowMs <= 0) return null;
        }

        const item: Record<string, unknown> = { lossPct, direction };
        if (windowMs != null) item.windowMs = windowMs;
        return item;
      })
      .filter((item): item is Record<string, unknown> => item != null);

    if (rules.length > 0) {
      config.lossRules = rules;
    } else {
      delete config.lossRules;
    }
    delete config.sourceTradeId;
    delete config.entryPrice;
    delete config.lossPct;
    delete config.windowSec;
    delete config.windowMs;
  }

  if (nodeType === 'trigger.open_positions') {
    const maxPriceCentRaw = Number(form.fields.maxPriceCent?.trim() ?? '');
    if (Number.isFinite(maxPriceCentRaw) && maxPriceCentRaw > 0 && maxPriceCentRaw <= 100) {
      config.maxPriceCent = maxPriceCentRaw;
    } else {
      delete config.maxPriceCent;
    }
    delete config.maxPrice;
  }

  if (
    (nodeType === 'trigger.open_positions' || nodeType === 'trigger.market_price') &&
    form.outcomeConditionRows.length > 0 &&
    !(
      nodeType === 'trigger.market_price' &&
      toStringValue(config.bindingMode).trim().toLowerCase() === 'pair_lock_only'
    )
  ) {
    const ptbTriggerEnabled = nodeType === 'trigger.market_price' && config.priceToBeatTriggerEnabled === true;
    const conditions = form.outcomeConditionRows
      .map((row) => ({
        row,
        validation: validateOutcomeConditionRow({
          nodeType,
          tokenId: row.tokenId,
          outcomeLabel: row.outcomeLabel,
          triggerCondition: row.triggerCondition,
          triggerPriceCent: row.triggerPriceCent,
          maxPriceCent: row.maxPriceCent,
          priceToBeatTriggerEnabled: ptbTriggerEnabled,
        }),
      }))
      .filter(({ validation }) => validation.isValid)
      .map(({ row, validation }) => {
        const triggerCondition = row.triggerCondition.trim();
        const priceCentRaw = row.triggerPriceCent.trim();
        const priceCent = Number(priceCentRaw);
        const maxPriceCentRaw = row.maxPriceCent.trim();
        const maxPriceCent = maxPriceCentRaw ? Number(maxPriceCentRaw) : null;
        const condition: Record<string, unknown> = {
          tokenId: row.tokenId.trim(),
          outcomeLabel: row.outcomeLabel.trim(),
        };
        if (!validation.isPtbOnly) {
          condition.triggerCondition = triggerCondition;
          condition.triggerPriceCent = Number.isFinite(priceCent) ? priceCent : 0;
        }
        if (
          !validation.isPtbOnly &&
          maxPriceCentRaw &&
          Number.isFinite(maxPriceCent) &&
          (maxPriceCent as number) > 0 &&
          (maxPriceCent as number) <= 100
        ) {
          condition.maxPriceCent = maxPriceCent;
        }
        return condition;
      });
    if (conditions.length > 0) {
      config.outcomeConditions = conditions;
      delete config.tokenId;
      delete config.triggerCondition;
      delete config.triggerPriceCent;
      delete config.maxPriceCent;
      delete config.maxPrice;
    }
  }

  if (nodeType === 'logic.if' || nodeType === 'logic.switch') {
    if (form.nestedExprMode && form.nestedExprGroup) {
      config.expression = nestedExprGroupToJsonLogic(form.nestedExprGroup);
    } else {
      config.expression = buildExpression(form.expressionRows, form.expressionJoin);
    }
  }

  if (nodeType === 'action.set_state') {
    config.statePatch = buildObjectFromKeyValueDrafts(form.statePatchRows);
  }

  if (nodeType === 'action.telegram_notify') {
    delete config.botToken;
  }

  return config;
}
