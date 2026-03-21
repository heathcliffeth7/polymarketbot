import { BOOLEAN_KEYS, NUMERIC_KEYS, RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME } from './constants';
import { buildObjectFromKeyValueDrafts, buildExpression, nestedExprGroupToJsonLogic, objectToRows, parseExpressionDraft, parseNumberArrayToStringRows } from './expressions';
import { createEmptyDrawdownRuleRow, createEmptyKeyValueDraft } from './drafts';
import { NODE_FIELD_SCHEMAS } from './schemas';
import {
  isPresetBuySellPlaceOrderMarker,
  isPresetPlaceOrderMarker,
  normalizeResolveMarketScope,
  resolveTriggerMarketOnceScope,
  toResolveMarketScope,
} from './presets';
import { TRIGGER_MARKET_ONCE_SCOPE_VERSION } from './constants';
import type { DrawdownRuleRow, NodeConfigFormState, OutcomeConditionRow } from './types';
import { createId, isRecord, safeJsonStringify, toCentStringValue, toDateTimeLocalString, toStringValue } from './utils';

export function parseNodeConfigToForm(nodeType: string, config: unknown): NodeConfigFormState {
  const cfg = isRecord(config) ? config : {};
  const fields: Record<string, string> = {};
  let triggerSizeRows: string[] = [];
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
      !(fields.notifyOnPriceToBeatGapBlocked ?? '').trim()
    ) {
      fields.notifyOnPriceToBeatGapBlocked = 'true';
    }
    if (
      (fields.priceToBeatGuardEnabled ?? '').trim().toLowerCase() === 'true' &&
      !['usd', 'cent'].includes((fields.priceToBeatMaxDiffUnit ?? '').trim().toLowerCase())
    ) {
      fields.priceToBeatMaxDiffUnit = 'usd';
    }
    if (!fields.sizePct.trim()) {
      fields.sizePct = toStringValue(cfg.sizePercent);
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

    const cycleWindowModeRaw = toStringValue(cfg.cycleWindowMode).trim().toLowerCase();
    if (cycleWindowModeRaw === 'first' || cycleWindowModeRaw === 'last' || cycleWindowModeRaw === 'custom_range') {
      fields.cycleWindowMode = cycleWindowModeRaw;
    } else {
      fields.cycleWindowMode = 'off';
    }
    fields.cycleWindowSecs = toStringValue(cfg.cycleWindowSecs);
    fields.cycleWindowStartSec = toStringValue(cfg.cycleWindowStartSec);
    fields.cycleWindowEndSec = toStringValue(cfg.cycleWindowEndSec);
    if (cycleWindowModeRaw === 'custom_range') {
      fields.autoSellOnWindowEnd = cfg.autoSellOnWindowEnd === true ? 'true' : 'false';
    }
    if (
      (fields.priceToBeatTriggerEnabled ?? '').trim().toLowerCase() === 'true' &&
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
    if (Array.isArray(cfg.outcomeConditions)) {
      for (const item of cfg.outcomeConditions as Record<string, unknown>[]) {
        if (!isRecord(item)) continue;
        outcomeConditionRows.push({
          id: createId('oc'),
          tokenId: toStringValue(item.tokenId),
          outcomeLabel: toStringValue(item.outcomeLabel),
          triggerCondition: toStringValue(item.triggerCondition),
          triggerPriceCent: toStringValue(item.triggerPriceCent),
          maxPriceCent: toCentStringValue(item.maxPriceCent, item.maxPrice),
        });
      }
    } else if (
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
        triggerPriceCent: toStringValue(cfg.triggerPriceCent),
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
    const tpEnabled = config.tpEnabled === true;
    const slEnabled = config.slEnabled === true;
    if (!isBuySide) {
      delete config.tpEnabled;
      delete config.tpPriceCent;
      delete config.tpPrice;
      delete config.slEnabled;
      delete config.slPriceCent;
      delete config.slPrice;
      delete config.slTriggerPriceMode;
      delete config.notifyOnTriggerPriceBlocked;
      delete config.notifyOnExecutionFloorBlocked;
      delete config.notifyOnMaxPriceBlocked;
      delete config.retryOnMaxPriceBlock;
      delete config.retryOnTriggerPriceGuardBlock;
      delete config.retryOnExecutionFloorGuardBlock;
      delete config.retryOnPriceToBeatGuardBlock;
      delete config.priceToBeatGuardEnabled;
      delete config.priceToBeatMaxDiff;
      delete config.priceToBeatMaxDiffUnit;
      delete config.notifyOnPriceToBeatGapBlocked;
      delete config.notifyOnTpHit;
      delete config.notifyOnSlHit;
      delete config.reenterOnSlHit;
      delete config.reentryMaxAttempts;
    } else {
      if (config.triggerPriceGuardEnabled !== true) {
        delete config.notifyOnTriggerPriceBlocked;
        delete config.retryOnTriggerPriceGuardBlock;
      }
      if (config.executionFloorGuardEnabled !== true) {
        delete config.notifyOnExecutionFloorBlocked;
        delete config.retryOnExecutionFloorGuardBlock;
      }
      if (config.priceToBeatGuardEnabled !== true) {
        delete config.priceToBeatMaxDiff;
        delete config.priceToBeatMaxDiffUnit;
        delete config.notifyOnPriceToBeatGapBlocked;
        delete config.retryOnPriceToBeatGuardBlock;
      } else {
        const priceToBeatUnitRaw = toStringValue(config.priceToBeatMaxDiffUnit).trim().toLowerCase();
        config.priceToBeatMaxDiffUnit =
          priceToBeatUnitRaw === 'cent' ? 'cent' : 'usd';
      }
      if (config.maxPriceCent == null) {
        delete config.notifyOnMaxPriceBlocked;
        delete config.retryOnMaxPriceBlock;
      }
      if (!tpEnabled) {
        delete config.tpEnabled;
        delete config.tpPriceCent;
        delete config.tpPrice;
        delete config.notifyOnTpHit;
      }
      if (!slEnabled) {
        delete config.slEnabled;
        delete config.slPriceCent;
        delete config.slPrice;
        delete config.slTriggerPriceMode;
        delete config.notifyOnSlHit;
        delete config.reenterOnSlHit;
        delete config.reentryMaxAttempts;
      }
      if (config.reenterOnSlHit !== true) {
        delete config.reentryMaxAttempts;
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
      const cycleWindowModeRaw2 = toStringValue(config.cycleWindowMode).trim().toLowerCase();
      if (cycleWindowModeRaw2 === 'first' || cycleWindowModeRaw2 === 'last') {
        config.cycleWindowMode = cycleWindowModeRaw2;
        const cwSecsRaw = Number(toStringValue(config.cycleWindowSecs).trim());
        if (Number.isInteger(cwSecsRaw) && cwSecsRaw > 0) {
          config.cycleWindowSecs = cwSecsRaw;
        } else {
          delete config.cycleWindowMode;
          delete config.cycleWindowSecs;
        }
        delete config.cycleWindowStartSec;
        delete config.cycleWindowEndSec;
        delete config.autoSellOnWindowEnd;
      } else if (cycleWindowModeRaw2 === 'custom_range') {
        config.cycleWindowMode = 'custom_range';
        const startSec = Number(toStringValue(config.cycleWindowStartSec).trim());
        const endSec = Number(toStringValue(config.cycleWindowEndSec).trim());
        if (Number.isInteger(startSec) && startSec >= 0 &&
            Number.isInteger(endSec) && endSec > startSec) {
          config.cycleWindowStartSec = startSec;
          config.cycleWindowEndSec = endSec;
        } else {
          delete config.cycleWindowMode;
          delete config.cycleWindowStartSec;
          delete config.cycleWindowEndSec;
        }
        delete config.cycleWindowSecs;
        if (config.autoSellOnWindowEnd !== true) {
          delete config.autoSellOnWindowEnd;
        }
      } else {
        delete config.cycleWindowMode;
        delete config.cycleWindowSecs;
        delete config.cycleWindowStartSec;
        delete config.cycleWindowEndSec;
        delete config.autoSellOnWindowEnd;
      }
      if (config.priceToBeatTriggerEnabled === true) {
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

  if ((nodeType === 'trigger.open_positions' || nodeType === 'trigger.market_price') && form.outcomeConditionRows.length > 0) {
    const ptbTriggerEnabled = nodeType === 'trigger.market_price' && config.priceToBeatTriggerEnabled === true;
    const conditions = form.outcomeConditionRows
      .filter((row) => {
        const tokenId = row.tokenId.trim();
        const outcomeLabel = row.outcomeLabel.trim();
        const triggerCondition = row.triggerCondition.trim();
        const triggerPriceCentRaw = row.triggerPriceCent.trim();
        const triggerPriceCent = Number(triggerPriceCentRaw);
        const maxPriceCentRaw = row.maxPriceCent.trim();
        const maxPriceCent = maxPriceCentRaw ? Number(maxPriceCentRaw) : null;
        const hasValidMaxPriceCent =
          !maxPriceCentRaw ||
          (Number.isFinite(maxPriceCent) && (maxPriceCent as number) > 0 && (maxPriceCent as number) <= 100);
        if (!tokenId || !outcomeLabel) return false;
        if (nodeType === 'trigger.market_price') {
          const isPtbOnly = ptbTriggerEnabled && !triggerCondition && !triggerPriceCentRaw;
          if (isPtbOnly) return true;
          const isSupportedTriggerCondition =
            ['cross_above', 'cross_below', 'level_above', 'level_below'].includes(triggerCondition);
          if (!isSupportedTriggerCondition) return false;
          return Number.isFinite(triggerPriceCent) && triggerPriceCent > 0 && triggerPriceCent <= 100 && hasValidMaxPriceCent;
        }
        const isSupportedTriggerCondition =
          triggerCondition === 'cross_above' || triggerCondition === 'cross_below';
        if (!isSupportedTriggerCondition) return false;
        return Number.isFinite(triggerPriceCent) && triggerPriceCent > 0 && triggerPriceCent <= 100 && hasValidMaxPriceCent;
      })
      .map((row) => {
        const triggerCondition = row.triggerCondition.trim();
        const priceCentRaw = row.triggerPriceCent.trim();
        const priceCent = Number(priceCentRaw);
        const maxPriceCentRaw = row.maxPriceCent.trim();
        const maxPriceCent = maxPriceCentRaw ? Number(maxPriceCentRaw) : null;
        const condition: Record<string, unknown> = {
          tokenId: row.tokenId.trim(),
          outcomeLabel: row.outcomeLabel.trim(),
        };
        const isPtbOnly = nodeType === 'trigger.market_price' && ptbTriggerEnabled && !triggerCondition && !priceCentRaw;
        if (!isPtbOnly) {
          condition.triggerCondition = triggerCondition;
          condition.triggerPriceCent = Number.isFinite(priceCent) ? priceCent : 0;
        }
        if (
          !isPtbOnly &&
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
