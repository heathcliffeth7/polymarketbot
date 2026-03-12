import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { validateActionPlaceOrderConfig } from './validation-action-place-order';
import {
  countValidOutcomeConditions,
  hasProvidedValue,
  isRecord,
  isSupportedTriggerCondition,
  RESOLVE_MARKET_ALLOWED_ASSETS,
  RESOLVE_MARKET_ALLOWED_TIMEFRAMES,
  RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME,
  toBooleanish,
  toFiniteNumber,
  toTrimmedString,
} from './shared';
import { pushNodeError, pushNodeWarning, validateAuxiliaryNodeConfig } from './validation-core';

export function validateNodeConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  graph: TradeFlowGraph
) {
  const config = isRecord(node.config) ? node.config : {};
  const graphSourceTradeId = toFiniteNumber(graph.context.sourceTradeId);
  const graphMarketSlug = String(graph.context.marketSlug ?? '').trim();
  const graphTokenId = String(graph.context.tokenId ?? '').trim();
  const graphOutcomeLabel = String(graph.context.outcomeLabel ?? '').trim();
  validateAuxiliaryNodeConfig(issues, node, config);
  if (node.type === 'trigger.market_price') {
    const marketMode = toTrimmedString(config.marketMode).toLowerCase();
    const autoScope = marketMode === 'auto_scope';
    const protectionMode = toTrimmedString(config.protectionMode).toLowerCase();
    const protectionPreset = toTrimmedString(config.protectionPreset).toLowerCase();
    if (autoScope) {
      const marketScope = toTrimmedString(config.marketScope).toLowerCase();
      if (!marketScope) {
        pushNodeError(
          issues,
          node,
          'missing_market_scope',
          'trigger.market_price auto_scope requires marketScope.'
        );
      } else if (!RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[marketScope]) {
        pushNodeError(
          issues,
          node,
          'invalid_market_scope',
          'trigger.market_price marketScope is unsupported.'
        );
      }
      const marketSelection = toTrimmedString(config.marketSelection).toLowerCase();
      if (marketSelection && marketSelection !== 'latest_by_slug') {
        pushNodeError(
          issues,
          node,
          'invalid_market_selection',
          'trigger.market_price marketSelection must be latest_by_slug.'
        );
      }
      if (protectionMode && protectionMode !== 'off' && protectionMode !== 'underlying_confirm') {
        pushNodeError(
          issues,
          node,
          'invalid_protection_mode',
          'trigger.market_price protectionMode must be off or underlying_confirm.'
        );
      }
      if (protectionMode === 'underlying_confirm') {
        if (!marketScope || !RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[marketScope]) {
          pushNodeError(
            issues,
            node,
            'invalid_protection_scope',
            'trigger.market_price underlying_confirm requires a supported auto_scope marketScope.'
          );
        }
        if (
          protectionPreset &&
          protectionPreset !== 'loose' &&
          protectionPreset !== 'balanced' &&
          protectionPreset !== 'strict'
        ) {
          pushNodeError(
            issues,
            node,
            'invalid_protection_preset',
            'trigger.market_price protectionPreset must be loose, balanced, or strict.'
          );
        }
      }
    } else if (!String(config.marketSlug ?? graphMarketSlug).trim()) {
      pushNodeError(
        issues,
        node,
        'missing_market_slug',
        'trigger.market_price requires marketSlug in node config or graph context.'
      );
    }
    if (!autoScope && protectionMode === 'underlying_confirm') {
      pushNodeError(
        issues,
        node,
        'invalid_protection_mode_scope',
        'trigger.market_price underlying_confirm is only valid when marketMode is auto_scope.'
      );
    }

    if (config.confirmationMs != null && toTrimmedString(config.confirmationMs).length > 0) {
      const confirmationMs = toFiniteNumber(config.confirmationMs);
      if (
        confirmationMs == null ||
        !Number.isInteger(confirmationMs) ||
        confirmationMs < 0
      ) {
        pushNodeError(
          issues,
          node,
          'invalid_confirmation_ms',
          'trigger.market_price confirmationMs must be an integer >= 0.'
        );
      }
    }

    const priceMode = toTrimmedString(config.priceMode).toLowerCase();
    const validPriceModes = ['composite', 'midpoint', 'raw', 'last_trade', 'site_display', 'best_bid', 'best_ask'];
    if (priceMode && !validPriceModes.includes(priceMode)) {
      pushNodeError(issues, node, 'invalid_price_mode', 'trigger.market_price priceMode must be composite, midpoint, raw, last_trade, site_display, best_bid, or best_ask.');
    }

    if (countValidOutcomeConditions(config) <= 0) {
      pushNodeError(
        issues,
        node,
        'missing_outcome_conditions',
        'trigger.market_price requires at least one valid outcome condition.'
      );
    }
  }

  if (node.type === 'trigger.sell_progress') {
    const sourceTradeId = toFiniteNumber(config.sourceTradeId);
    if ((sourceTradeId ?? graphSourceTradeId ?? 0) <= 0) {
      pushNodeError(
        issues,
        node,
        'missing_source_trade_id',
        'trigger.sell_progress requires sourceTradeId in node config or graph context.'
      );
    }
  }

  if (node.type === 'trigger.open_positions') {
    const sourceTradeId = toFiniteNumber(config.sourceTradeId);
    if ((sourceTradeId ?? graphSourceTradeId ?? 0) <= 0) {
      pushNodeError(
        issues,
        node,
        'missing_source_trade_id',
        'trigger.open_positions requires sourceTradeId in node config or graph context.'
      );
    }
    if (countValidOutcomeConditions(config) <= 0) {
      pushNodeError(
        issues,
        node,
        'missing_outcome_conditions',
        'trigger.open_positions requires at least one valid outcome condition.'
      );
    }

    const minPositionQty = toFiniteNumber(config.minPositionQty);
    if (minPositionQty != null && minPositionQty < 0) {
      pushNodeError(
        issues,
        node,
        'invalid_min_position_qty',
        'trigger.open_positions minPositionQty must be >= 0.'
      );
    }

    const triggerConditionRaw = config.triggerCondition;
    const hasTriggerCondition = String(triggerConditionRaw ?? '').trim().length > 0;
    if (hasTriggerCondition && !isSupportedTriggerCondition(triggerConditionRaw)) {
      pushNodeError(
        issues,
        node,
        'invalid_trigger_condition',
        'trigger.open_positions triggerCondition must be cross_above or cross_below.'
      );
    }
    if (isSupportedTriggerCondition(triggerConditionRaw)) {
      const triggerPriceCent = toFiniteNumber(config.triggerPriceCent);
      const triggerPrice = toFiniteNumber(config.triggerPrice);
      const maxPriceCentProvided = hasProvidedValue(config.maxPriceCent);
      const maxPriceProvided = !maxPriceCentProvided && hasProvidedValue(config.maxPrice);
      if (triggerPriceCent == null && triggerPrice == null) {
        pushNodeError(
          issues,
          node,
          'missing_trigger_price',
          'trigger.open_positions triggerCondition requires triggerPriceCent (or legacy triggerPrice).'
        );
      }
      if (triggerPriceCent != null && (triggerPriceCent <= 0 || triggerPriceCent > 100)) {
        pushNodeError(
          issues,
          node,
          'invalid_trigger_price_cent',
          'trigger.open_positions triggerPriceCent must be in (0, 100].'
        );
      }
      if (triggerPrice != null && (triggerPrice <= 0 || triggerPrice > 1)) {
        pushNodeError(
          issues,
          node,
          'invalid_trigger_price',
          'trigger.open_positions triggerPrice must be in (0, 1].'
        );
      }
      if (maxPriceCentProvided) {
        const maxPriceCent = toFiniteNumber(config.maxPriceCent);
        if (maxPriceCent == null || maxPriceCent <= 0 || maxPriceCent > 100) {
          pushNodeError(
            issues,
            node,
            'invalid_max_price_cent',
            'trigger.open_positions maxPriceCent must be in (0, 100].'
          );
        }
      } else if (maxPriceProvided) {
        const maxPrice = toFiniteNumber(config.maxPrice);
        if (maxPrice == null || maxPrice <= 0 || maxPrice > 1) {
          pushNodeError(
            issues,
            node,
            'invalid_max_price',
            'trigger.open_positions maxPrice must be in (0, 1].'
          );
        }
      }
      const marketSlug = String(config.marketSlug ?? graph.context.marketSlug ?? '').trim();
      if (!marketSlug) {
        pushNodeError(
          issues,
          node,
          'missing_market_slug',
          'trigger.open_positions with triggerCondition requires marketSlug.'
        );
      }
      const tokenId = String(config.tokenId ?? graph.context.tokenId ?? '').trim();
      if (!tokenId) {
        pushNodeError(
          issues,
          node,
          'missing_token_id',
          'trigger.open_positions with triggerCondition requires tokenId.'
        );
      }
    }

    const minIntervalMs = toFiniteNumber(config.minIntervalMs);
    if (minIntervalMs != null && minIntervalMs < 250) {
      pushNodeError(
        issues,
        node,
        'invalid_min_interval',
        'trigger.open_positions minIntervalMs must be >= 250.'
      );
    }
  }

  if (node.type === 'trigger.position_drawdown') {
    const marketSlug = String(config.marketSlug ?? graphMarketSlug).trim();
    if (!marketSlug) {
      pushNodeError(
        issues,
        node,
        'missing_market_slug',
        'trigger.position_drawdown requires marketSlug in node config or graph context.'
      );
    }

    const tokenId = String(config.tokenId ?? graphTokenId).trim();
    if (!tokenId) {
      pushNodeError(
        issues,
        node,
        'missing_token_id',
        'trigger.position_drawdown requires tokenId in node config or graph context.'
      );
    }
    const outcomeLabel = String(config.outcomeLabel ?? graphOutcomeLabel).trim();
    if (!outcomeLabel) {
      pushNodeError(
        issues,
        node,
        'missing_outcome_label',
        'trigger.position_drawdown requires outcomeLabel in node config or graph context.'
      );
    }

    const entryPriceCent = toFiniteNumber(config.entryPriceCent);
    const entryPrice = toFiniteNumber(config.entryPrice);
    if (entryPriceCent == null && entryPrice == null) {
      pushNodeError(
        issues,
        node,
        'missing_entry_price',
        'trigger.position_drawdown requires entryPriceCent (or legacy entryPrice).'
      );
    }
    if (entryPriceCent != null && (entryPriceCent <= 0 || entryPriceCent > 100)) {
      pushNodeError(
        issues,
        node,
        'invalid_entry_price_cent',
        'trigger.position_drawdown entryPriceCent must be in (0, 100].'
      );
    }
    if (entryPrice != null && (entryPrice <= 0 || entryPrice > 1)) {
      pushNodeError(
        issues,
        node,
        'invalid_entry_price',
        'trigger.position_drawdown entryPrice must be in (0, 1].'
      );
    }

    const minIntervalMs = toFiniteNumber(config.minIntervalMs);
    if (minIntervalMs != null && minIntervalMs < 250) {
      pushNodeError(
        issues,
        node,
        'invalid_min_interval',
        'trigger.position_drawdown minIntervalMs must be >= 250.'
      );
    }

    const combineMode = toTrimmedString(config.combineMode).toLowerCase();
    if (combineMode && combineMode !== 'and' && combineMode !== 'or') {
      pushNodeError(
        issues,
        node,
        'invalid_combine_mode',
        'trigger.position_drawdown combineMode must be and, or, or empty.'
      );
    }

    let validRuleCount = 0;
    let invalidDirectionFound = false;
    const hasDeprecatedWindowSec =
      Object.prototype.hasOwnProperty.call(config, 'windowSec') ||
      (Array.isArray(config.lossRules) &&
        config.lossRules.some(
          (item) => isRecord(item) && Object.prototype.hasOwnProperty.call(item, 'windowSec')
        ));
    if (Array.isArray(config.lossRules)) {
      for (const item of config.lossRules) {
        if (!isRecord(item)) continue;
        const direction = toTrimmedString(item.direction).toLowerCase();
        if (direction && direction !== 'down' && direction !== 'up') {
          invalidDirectionFound = true;
          continue;
        }
        const lossPct = toFiniteNumber(item.lossPct);
        if (lossPct == null || lossPct <= 0 || lossPct > 100) {
          continue;
        }
        const windowMs = toFiniteNumber(item.windowMs);
        if (windowMs != null && windowMs <= 0) {
          continue;
        }
        validRuleCount += 1;
      }
    } else {
      const legacyLossPct = toFiniteNumber(config.lossPct);
      const legacyWindowMs = toFiniteNumber(config.windowMs);
      if (
        legacyLossPct != null &&
        legacyLossPct > 0 &&
        legacyLossPct <= 100 &&
        (legacyWindowMs == null || legacyWindowMs > 0)
      ) {
        validRuleCount += 1;
      }
    }

    if (invalidDirectionFound) {
      pushNodeError(
        issues,
        node,
        'invalid_rule_direction',
        'trigger.position_drawdown lossRules[].direction must be down, up, or empty.'
      );
    }
    if (hasDeprecatedWindowSec) {
      pushNodeError(
        issues,
        node,
        'invalid_deprecated_window_sec',
        'trigger.position_drawdown windowSec is deprecated; use windowMs.'
      );
    }

    if (validRuleCount <= 0) {
      pushNodeError(
        issues,
        node,
        'missing_loss_rules',
        'trigger.position_drawdown requires at least one valid loss rule (lossPct in (0,100], optional windowMs > 0).'
      );
    }
  }

  if (node.type === 'action.resolve_market') {
    const marketScope = String(config.marketScope ?? '').trim().toLowerCase();
    const marketScopeResolved = marketScope
      ? RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[marketScope] || null
      : null;
    const asset = String(config.asset ?? marketScopeResolved?.asset ?? '').trim().toLowerCase();
    const timeframe = String(config.timeframe ?? marketScopeResolved?.timeframe ?? '').trim().toLowerCase();

    if (marketScope && !marketScopeResolved) {
      pushNodeWarning(
        issues,
        node,
        'legacy_scope_unknown',
        'action.resolve_market marketScope is unknown; asset/timeframe should be used.'
      );
    }
    if (asset && !RESOLVE_MARKET_ALLOWED_ASSETS.has(asset)) {
      pushNodeError(
        issues,
        node,
        'invalid_asset',
        'action.resolve_market asset must be one of: btc, eth, sol, xrp.'
      );
    }
    if (timeframe && !RESOLVE_MARKET_ALLOWED_TIMEFRAMES.has(timeframe)) {
      pushNodeError(
        issues,
        node,
        'invalid_timeframe',
        'action.resolve_market timeframe must be one of: 5m, 15m.'
      );
    }
    if ((!asset || !timeframe) && !marketScopeResolved) {
      pushNodeWarning(
        issues,
        node,
        'missing_asset_timeframe',
        'action.resolve_market missing asset/timeframe; runtime falls back to bot market_scope.'
      );
    }

    const selection = String(config.selection ?? '').trim();
    if (selection && selection !== 'latest_by_slug') {
      pushNodeError(
        issues,
        node,
        'invalid_selection',
        'action.resolve_market selection must be latest_by_slug.'
      );
    }

    const outcomeLabel = String(config.outcomeLabel ?? graphOutcomeLabel).trim().toLowerCase();
    if (outcomeLabel && outcomeLabel !== 'yes' && outcomeLabel !== 'no') {
      pushNodeError(
        issues,
        node,
        'invalid_outcome_label',
        'action.resolve_market outcomeLabel must be yes or no.'
      );
    }

    for (const boolKey of ['failOnMissingMarket', 'requireYesNoTokens', 'requireTokenId']) {
      if (config[boolKey] != null && toBooleanish(config[boolKey]) == null) {
        pushNodeError(
          issues,
          node,
          `invalid_${boolKey.toLowerCase()}`,
          `action.resolve_market ${boolKey} must be boolean (true/false).`
        );
      }
    }
  }

  if (node.type === 'action.place_order') {
    validateActionPlaceOrderConfig(issues, node, graph);
  }

  if (node.type === 'action.dual_dca') {
    const sourceTradeId = toFiniteNumber(config.sourceTradeId);
    if ((sourceTradeId ?? graphSourceTradeId ?? 0) <= 0) {
      pushNodeError(
        issues,
        node,
        'missing_source_trade_id',
        'action.dual_dca requires sourceTradeId in node config or graph context. Publish sirasinda otomatik olusturulmasi icin asset/timeframe alanlari dolu olmali.'
      );
    }

    const asset = String(config.asset ?? config.coin ?? '').trim().toLowerCase();
    if (!asset) {
      pushNodeError(
        issues,
        node,
        'missing_asset',
        'action.dual_dca requires asset (btc, eth, sol, xrp).'
      );
    } else if (!RESOLVE_MARKET_ALLOWED_ASSETS.has(asset)) {
      pushNodeError(
        issues,
        node,
        'invalid_asset',
        'action.dual_dca asset must be one of: btc, eth, sol, xrp.'
      );
    }

    const timeframeRaw = String(config.timeframe ?? config.marketPeriod ?? '').trim().toLowerCase();
    const timeframe =
      timeframeRaw === '5' || timeframeRaw === '5min' || timeframeRaw === '5 min'
        ? '5m'
        : timeframeRaw === '15' || timeframeRaw === '15min' || timeframeRaw === '15 min'
          ? '15m'
          : timeframeRaw;
    if (!timeframe) {
      pushNodeError(
        issues,
        node,
        'missing_timeframe',
        'action.dual_dca requires timeframe (5m or 15m).'
      );
    } else if (!RESOLVE_MARKET_ALLOWED_TIMEFRAMES.has(timeframe)) {
      pushNodeError(
        issues,
        node,
        'invalid_timeframe',
        'action.dual_dca timeframe must be one of: 5m, 15m.'
      );
    }

    const sideMode = String(config.sideMode ?? config.side ?? '').trim().toLowerCase();
    if (!sideMode) {
      pushNodeError(
        issues,
        node,
        'missing_side_mode',
        'action.dual_dca requires sideMode (up/down/all).'
      );
    } else if (sideMode !== 'up' && sideMode !== 'down' && sideMode !== 'all') {
      pushNodeError(
        issues,
        node,
        'invalid_side_mode',
        'action.dual_dca sideMode must be up, down or all.'
      );
    }

    const baseSizing = String(config.baseSizing ?? config.baseSizeMode ?? '').trim().toLowerCase();
    if (!baseSizing) {
      pushNodeError(
        issues,
        node,
        'missing_base_sizing',
        'action.dual_dca requires baseSizing (shares/usdc).'
      );
    } else if (baseSizing !== 'shares' && baseSizing !== 'usdc') {
      pushNodeError(
        issues,
        node,
        'invalid_base_sizing',
        'action.dual_dca baseSizing must be shares or usdc.'
      );
    }
    const baseShares = toFiniteNumber(config.baseShares);
    const baseUsdc = toFiniteNumber(config.baseUsdc);
    if (baseSizing === 'shares') {
      if (baseShares == null || baseShares <= 0) {
        pushNodeError(
          issues,
          node,
          'invalid_base_shares',
          'action.dual_dca baseShares must be > 0 when baseSizing is shares.'
        );
      }
    } else if (baseSizing === 'usdc' && (baseUsdc == null || baseUsdc <= 0)) {
      pushNodeError(
        issues,
        node,
        'invalid_base_usdc',
        'action.dual_dca baseUsdc must be > 0 when baseSizing is usdc.'
      );
    }
    const basePrice = toFiniteNumber(config.basePriceUsdc ?? config.basePrice);
    if (basePrice != null && (basePrice <= 0 || basePrice > 1)) {
      pushNodeError(
        issues,
        node,
        'invalid_base_price',
        'action.dual_dca basePriceUsdc must be in (0, 1].'
      );
    }

    const dcaLevels = toFiniteNumber(config.dcaLevels);
    if (dcaLevels == null) {
      pushNodeError(
        issues,
        node,
        'missing_dca_levels',
        'action.dual_dca requires dcaLevels.'
      );
    } else if (dcaLevels < 1 || dcaLevels > 20) {
      pushNodeError(
        issues,
        node,
        'invalid_dca_levels',
        'action.dual_dca dcaLevels must be in [1, 20].'
      );
    }

    const nearStep = toFiniteNumber(config.nearStep);
    if (nearStep == null) {
      pushNodeError(
        issues,
        node,
        'missing_near_step',
        'action.dual_dca requires nearStep.'
      );
    } else if (nearStep <= 0 || nearStep >= 1) {
      pushNodeError(
        issues,
        node,
        'invalid_near_step',
        'action.dual_dca nearStep must be in (0, 1).'
      );
    }

    const stepMult = toFiniteNumber(config.stepMult);
    if (stepMult == null) {
      pushNodeError(
        issues,
        node,
        'missing_step_mult',
        'action.dual_dca requires stepMult.'
      );
    } else if (stepMult < 1) {
      pushNodeError(
        issues,
        node,
        'invalid_step_mult',
        'action.dual_dca stepMult must be >= 1.'
      );
    }

    const sizeMult = toFiniteNumber(config.sizeMult);
    if (sizeMult == null) {
      pushNodeError(
        issues,
        node,
        'missing_size_mult',
        'action.dual_dca requires sizeMult.'
      );
    } else if (sizeMult <= 0) {
      pushNodeError(
        issues,
        node,
        'invalid_size_mult',
        'action.dual_dca sizeMult must be > 0.'
      );
    }

    const minDistance = toFiniteNumber(config.minPriceDistanceCent);
    if (minDistance == null) {
      pushNodeError(
        issues,
        node,
        'missing_min_price_distance',
        'action.dual_dca requires minPriceDistanceCent.'
      );
    } else if (minDistance <= 0) {
      pushNodeError(
        issues,
        node,
        'invalid_min_price_distance',
        'action.dual_dca minPriceDistanceCent must be > 0.'
      );
    }

    const cutoffMin = toFiniteNumber(config.cutoffMin);
    if (cutoffMin == null) {
      pushNodeError(
        issues,
        node,
        'missing_cutoff_min',
        'action.dual_dca requires cutoffMin.'
      );
    } else if (cutoffMin < 0) {
      pushNodeError(
        issues,
        node,
        'invalid_cutoff_min',
        'action.dual_dca cutoffMin must be >= 0.'
      );
    }

    for (const riskKey of ['tpProfitPct', 'slLossPct', 'slSpreadPct']) {
      const value = toFiniteNumber(config[riskKey]);
      if (value == null) {
        pushNodeError(
          issues,
          node,
          `missing_${riskKey.toLowerCase()}`,
          `action.dual_dca requires ${riskKey}.`
        );
      } else if (value < 0) {
        pushNodeError(
          issues,
          node,
          `invalid_${riskKey.toLowerCase()}`,
          `action.dual_dca ${riskKey} must be >= 0.`
        );
      }
    }
  }
}
