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
import { validateTriggerMarketPriceNodeConfig } from './validation-trigger-market-price';

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
    validateTriggerMarketPriceNodeConfig(issues, node, graph);
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
}
