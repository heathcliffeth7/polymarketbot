import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import {
  countValidMarketPriceOutcomeConditions,
  hasProvidedValue,
  isRecord,
  isSupportedMarketPriceTriggerCondition,
  RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME,
  toBooleanish,
  toFiniteNumber,
  toTrimmedString,
} from './shared';
import { pushNodeError } from './validation-core';

export function validateTriggerMarketPriceNodeConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  graph: TradeFlowGraph
) {
  const config = isRecord(node.config) ? node.config : {};
  const graphMarketSlug = String(graph.context.marketSlug ?? '').trim();
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
    if (confirmationMs == null || !Number.isInteger(confirmationMs) || confirmationMs < 0) {
      pushNodeError(
        issues,
        node,
        'invalid_confirmation_ms',
        'trigger.market_price confirmationMs must be an integer >= 0.'
      );
    }
  }

  const priceMode = toTrimmedString(config.priceMode).toLowerCase();
  const validPriceModes = [
    'composite',
    'midpoint',
    'raw',
    'last_trade',
    'site_display',
    'best_bid',
    'best_ask',
  ];
  if (priceMode && !validPriceModes.includes(priceMode)) {
    pushNodeError(
      issues,
      node,
      'invalid_price_mode',
      'trigger.market_price priceMode must be composite, midpoint, raw, last_trade, site_display, best_bid, or best_ask.'
    );
  }

  const repeatMode = toTrimmedString(config.repeatMode).toLowerCase();
  const priceToBeatTriggerEnabled = toBooleanish(config.priceToBeatTriggerEnabled);
  if (config.priceToBeatTriggerEnabled != null && priceToBeatTriggerEnabled == null) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_trigger_enabled',
      'trigger.market_price priceToBeatTriggerEnabled must be boolean (true/false).'
    );
  }
  if (priceToBeatTriggerEnabled === true && !autoScope) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_trigger_scope',
      'trigger.market_price priceToBeatTriggerEnabled is only valid when marketMode is auto_scope.'
    );
  }
  if (priceToBeatTriggerEnabled === true) {
    const minGap = toFiniteNumber(config.priceToBeatTriggerMinGap);
    if (minGap == null || minGap <= 0) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_trigger_min_gap',
        'trigger.market_price priceToBeatTriggerMinGap must be > 0 when gate is enabled.'
      );
    }
    if (hasProvidedValue(config.priceToBeatTriggerMaxGap)) {
      const maxGap = toFiniteNumber(config.priceToBeatTriggerMaxGap);
      if (maxGap == null || maxGap <= 0 || (minGap != null && maxGap < minGap)) {
        pushNodeError(
          issues,
          node,
          'invalid_price_to_beat_trigger_max_gap',
          'trigger.market_price priceToBeatTriggerMaxGap must be >= min gap when provided.'
        );
      }
    }
    const unit = toTrimmedString(config.priceToBeatTriggerUnit).toLowerCase();
    if (unit && unit !== 'usd' && unit !== 'cent') {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_trigger_unit',
        'trigger.market_price priceToBeatTriggerUnit must be usd or cent.'
      );
    }
  }

  if (Array.isArray(config.outcomeConditions)) {
    for (const item of config.outcomeConditions) {
      if (!isRecord(item)) continue;
      const triggerCondition = toTrimmedString(item.triggerCondition).toLowerCase();
      const triggerPriceProvided =
        hasProvidedValue(item.triggerPriceCent) || hasProvidedValue(item.triggerPrice);
      const ptbOnly =
        priceToBeatTriggerEnabled === true && !triggerCondition && !triggerPriceProvided;

      if (ptbOnly) {
        continue;
      }
      if (triggerCondition && !isSupportedMarketPriceTriggerCondition(triggerCondition)) {
        pushNodeError(
          issues,
          node,
          'invalid_market_price_outcome_trigger_condition',
          'trigger.market_price outcomeConditions triggerCondition must be cross_above, cross_below, level_above, or level_below.'
        );
        break;
      }
      if (!!triggerCondition !== triggerPriceProvided) {
        pushNodeError(
          issues,
          node,
          'incomplete_market_price_outcome_trigger',
          'trigger.market_price outcomeConditions rows must provide both triggerCondition and triggerPrice unless priceToBeatTriggerEnabled is using PTB-only mode.'
        );
        break;
      }
      if (
        (triggerCondition === 'level_above' || triggerCondition === 'level_below') &&
        repeatMode !== 'once'
      ) {
        pushNodeError(
          issues,
          node,
          'invalid_level_trigger_repeat_mode',
          'trigger.market_price level_above/level_below only support repeatMode=once.'
        );
        break;
      }
    }
  }

  if (countValidMarketPriceOutcomeConditions(config) <= 0) {
    pushNodeError(
      issues,
      node,
      'missing_outcome_conditions',
      'trigger.market_price requires at least one valid outcome condition or PTB-only outcome row.'
    );
  }

  if (autoScope) {
    const cycleWindowMode = toTrimmedString(config.cycleWindowMode).toLowerCase();
    if (cycleWindowMode === 'custom_range') {
      const startSec = toFiniteNumber(config.cycleWindowStartSec);
      const endSec = toFiniteNumber(config.cycleWindowEndSec);
      if (startSec == null || !Number.isInteger(startSec) || startSec < 0) {
        pushNodeError(
          issues,
          node,
          'invalid_cycle_window_start_sec',
          'custom_range cycleWindowStartSec must be an integer >= 0.'
        );
      }
      if (endSec == null || !Number.isInteger(endSec) || endSec <= 0) {
        pushNodeError(
          issues,
          node,
          'invalid_cycle_window_end_sec',
          'custom_range cycleWindowEndSec must be an integer > 0.'
        );
      }
      if (startSec != null && endSec != null && startSec >= endSec) {
        pushNodeError(
          issues,
          node,
          'invalid_cycle_window_range',
          'custom_range requires cycleWindowStartSec < cycleWindowEndSec.'
        );
      }
      const cwMarketScope = toTrimmedString(config.marketScope).toLowerCase();
      const scopeInfo = RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[cwMarketScope];
      if (scopeInfo && endSec != null) {
        const cycleDuration = scopeInfo.timeframe === '15m' ? 900 : 300;
        if (endSec > cycleDuration) {
          pushNodeError(
            issues,
            node,
            'cycle_window_exceeds_duration',
            `custom_range cycleWindowEndSec (${endSec}) exceeds cycle duration (${cycleDuration}s).`
          );
        }
      }
    }
  }
}
