import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { isPtbStopLossCurrentPriceSource } from '@/lib/trade-flow-config-mappers/ptb-modes';
import { findUniqueUpstreamMarketPriceTrigger } from './graph';
import { isRecord, toBooleanish, toFiniteNumber, toTrimmedString } from './shared';
import { pushNodeError } from './validation-core';

const REVENGE_FLIP_BINDING_MODE = 'revenge_flip_only';
const REVENGE_FLIP_SIDE_MODES = new Set(['any', 'same', 'opposite', 'up', 'down']);

function revengeConfig(config: Record<string, unknown>): Record<string, unknown> {
  return isRecord(config.revengeFlip) ? config.revengeFlip : {};
}

function nestedNumber(
  config: Record<string, unknown>,
  nested: Record<string, unknown>,
  key: string,
): number | null {
  return toFiniteNumber(nested[key] ?? config[key]);
}

function validatePositiveNumber(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  code: string,
  label: string,
  value: number | null,
) {
  if (value == null || value <= 0) {
    pushNodeError(issues, node, code, `action.place_order revenge_flip_v1 ${label} must be > 0.`);
  }
}

function validateTimeRules(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  raw: unknown,
) {
  if (raw == null) return;
  if (!Array.isArray(raw)) {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_time_rules',
      'action.place_order revenge_flip_v1 timeRules must be an array.'
    );
    return;
  }
  for (const item of raw) {
    if (!isRecord(item)) {
      pushNodeError(
        issues,
        node,
        'invalid_revenge_flip_time_rule_item',
        'action.place_order revenge_flip_v1 timeRules entries must be objects.'
      );
      return;
    }
    const min = toFiniteNumber(item.minRemainingSec ?? item.remainingSecMin) ?? 0;
    const max = toFiniteNumber(item.maxRemainingSec ?? item.remainingSecMax) ?? min;
    if (!Number.isInteger(min) || !Number.isInteger(max) || min < 0 || max < min) {
      pushNodeError(
        issues,
        node,
        'invalid_revenge_flip_time_rule_range',
        'action.place_order revenge_flip_v1 timeRules remaining seconds must be integer ranges.'
      );
      return;
    }
    const diff = toFiniteNumber(item.priceToBeatMinDiff ?? item.ptbMinDiff ?? item.priceToBeatMaxDiff ?? item.ptbMaxDiff);
    if (diff != null && diff < 0) {
      pushNodeError(
        issues,
        node,
        'invalid_revenge_flip_time_rule_ptb',
        'action.place_order revenge_flip_v1 timeRules priceToBeatMinDiff must be >= 0.'
      );
      return;
    }
    const unit = toTrimmedString(item.priceToBeatMinDiffUnit ?? item.priceToBeatMaxDiffUnit ?? item.ptbDiffUnit).toLowerCase();
    if (unit && unit !== 'usd' && unit !== 'cent') {
      pushNodeError(
        issues,
        node,
        'invalid_revenge_flip_time_rule_ptb_unit',
        'action.place_order revenge_flip_v1 timeRules unit must be usd or cent.'
      );
      return;
    }
  }
}

function validateStopLossRules(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  raw: unknown,
) {
  if (raw == null) return;
  if (!Array.isArray(raw)) {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_stop_loss_rules',
      'action.place_order revenge_flip_v1 stopLossRules must be an array.'
    );
    return;
  }
  for (const item of raw) {
    if (!isRecord(item)) {
      pushNodeError(
        issues,
        node,
        'invalid_revenge_flip_stop_loss_rule_item',
        'action.place_order revenge_flip_v1 stopLossRules entries must be objects.'
      );
      return;
    }
    const minFlip = toFiniteNumber(item.minFlip) ?? 0;
    const maxFlip = toFiniteNumber(item.maxFlip);
    const stopLossPct = toFiniteNumber(item.stopLossPct);
    if (
      !Number.isInteger(minFlip) ||
      minFlip < 0 ||
      (maxFlip != null && (!Number.isInteger(maxFlip) || maxFlip < minFlip))
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_revenge_flip_stop_loss_rule_range',
        'action.place_order revenge_flip_v1 stopLossRules minFlip/maxFlip must be integer ranges.'
      );
      return;
    }
    if (stopLossPct == null || stopLossPct <= 0 || stopLossPct >= 1) {
      pushNodeError(
        issues,
        node,
        'invalid_revenge_flip_stop_loss_rule_pct',
        'action.place_order revenge_flip_v1 stopLossRules stopLossPct must be > 0 and < 1.'
      );
      return;
    }
  }
}

function validateEntryPtbRules(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  raw: unknown,
) {
  if (raw == null) return;
  if (!Array.isArray(raw)) {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_entry_ptb_rules',
      'action.place_order revenge_flip_v1 entryPtbRules must be an array.'
    );
    return;
  }
  for (const item of raw) {
    if (!isRecord(item)) {
      pushNodeError(
        issues,
        node,
        'invalid_revenge_flip_entry_ptb_rule_item',
        'action.place_order revenge_flip_v1 entryPtbRules entries must be objects.'
      );
      return;
    }
    const minFlip = toFiniteNumber(item.minFlip) ?? 0;
    const maxFlip = toFiniteNumber(item.maxFlip);
    const sideMode = toTrimmedString(item.sideMode ?? item.entrySideMode).toLowerCase();
    if (
      !Number.isInteger(minFlip) ||
      minFlip < 0 ||
      (maxFlip != null && (!Number.isInteger(maxFlip) || maxFlip < minFlip))
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_revenge_flip_entry_ptb_rule_flip_range',
        'action.place_order revenge_flip_v1 entryPtbRules minFlip/maxFlip must be integer ranges.'
      );
      return;
    }
    if (sideMode && !REVENGE_FLIP_SIDE_MODES.has(sideMode)) {
      pushNodeError(
        issues,
        node,
        'invalid_revenge_flip_entry_ptb_rule_side_mode',
        'action.place_order revenge_flip_v1 entryPtbRules sideMode must be any, same, opposite, up, or down.'
      );
      return;
    }
    const minRemainingSec = toFiniteNumber(item.minRemainingSec ?? item.remainingSecMin);
    const maxRemainingSec = toFiniteNumber(item.maxRemainingSec ?? item.remainingSecMax);
    if (
      (minRemainingSec != null && (!Number.isInteger(minRemainingSec) || minRemainingSec < 0)) ||
      (maxRemainingSec != null &&
        (!Number.isInteger(maxRemainingSec) ||
          maxRemainingSec < (minRemainingSec ?? 0)))
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_revenge_flip_entry_ptb_rule_time_range',
        'action.place_order revenge_flip_v1 entryPtbRules remaining seconds must be integer ranges.'
      );
      return;
    }
    const diff = toFiniteNumber(item.priceToBeatMinDiff ?? item.ptbMinDiff ?? item.priceToBeatMaxDiff ?? item.ptbMaxDiff);
    if (diff == null || diff < 0) {
      pushNodeError(
        issues,
        node,
        'invalid_revenge_flip_entry_ptb_rule_diff',
        'action.place_order revenge_flip_v1 entryPtbRules priceToBeatMinDiff must be >= 0.'
      );
      return;
    }
    const unit = toTrimmedString(item.priceToBeatMinDiffUnit ?? item.priceToBeatMaxDiffUnit ?? item.ptbDiffUnit).toLowerCase();
    if (unit && unit !== 'usd' && unit !== 'cent') {
      pushNodeError(
        issues,
        node,
        'invalid_revenge_flip_entry_ptb_rule_unit',
        'action.place_order revenge_flip_v1 entryPtbRules unit must be usd or cent.'
      );
      return;
    }
    const maxPriceCent = toFiniteNumber(item.maxPriceCent ?? item.entryMaxPriceCent);
    if (maxPriceCent != null && (maxPriceCent <= 0 || maxPriceCent > 100)) {
      pushNodeError(
        issues,
        node,
        'invalid_revenge_flip_entry_ptb_rule_max_price',
        'action.place_order revenge_flip_v1 entryPtbRules maxPriceCent must be > 0 and <= 100.'
      );
      return;
    }
  }
}

function validatePtbStopLoss(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>,
  revenge: Record<string, unknown>,
) {
  const enabled = toBooleanish(revenge.ptbStopLossEnabled ?? config.ptbStopLossEnabled);
  const gap = toFiniteNumber(revenge.ptbStopLossGapUsd ?? config.ptbStopLossGapUsd);
  const unit = toTrimmedString(revenge.ptbStopLossGapUnit ?? config.ptbStopLossGapUnit).toLowerCase();
  const currentSource = toTrimmedString(
    revenge.ptbStopLossCurrentPriceSource ?? config.ptbStopLossCurrentPriceSource
  ).toLowerCase();
  const timeMode = toTrimmedString(
    revenge.ptbStopLossTimeDecayMode ?? config.ptbStopLossTimeDecayMode
  ).toLowerCase();

  if (unit && unit !== 'usd' && unit !== 'cent') {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_ptb_stop_loss_gap_unit',
      'action.place_order revenge_flip_v1 ptbStopLossGapUnit must be usd or cent.'
    );
  }
  if (currentSource && !isPtbStopLossCurrentPriceSource(currentSource)) {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_ptb_stop_loss_current_source',
      'action.place_order revenge_flip_v1 ptbStopLossCurrentPriceSource must be chainlink, binance, coinbase, hyperliquid, binance_hyperliquid, cex_consensus, chainlink_cex_consensus, chainlink_cex_consensus_confirmed, or cex_median_fast.'
    );
  }
  if (timeMode && timeMode !== 'tighten' && timeMode !== 'relax' && timeMode !== 'none') {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_ptb_stop_loss_time_mode',
      'action.place_order revenge_flip_v1 ptbStopLossTimeDecayMode must be tighten, relax, or none.'
    );
  }
  if (enabled === true && gap == null) {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_ptb_stop_loss_gap',
      'action.place_order revenge_flip_v1 ptbStopLossGapUsd must be finite when PTB stop-loss is enabled.'
    );
  }
}

export function validateActionPlaceOrderRevengeFlipConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  graph: TradeFlowGraph,
  config: Record<string, unknown>
) {
  const revenge = revengeConfig(config);
  const upstreamKey = findUniqueUpstreamMarketPriceTrigger(node.key, graph);
  const upstreamNode = upstreamKey ? graph.nodes.find((candidate) => candidate.key === upstreamKey) : null;
  const upstreamConfig = upstreamNode && isRecord(upstreamNode.config) ? upstreamNode.config : {};
  const bindingMode = toTrimmedString(upstreamConfig.bindingMode).toLowerCase() || 'standard';
  if (bindingMode !== REVENGE_FLIP_BINDING_MODE) {
    pushNodeError(
      issues,
      node,
      'revenge_flip_requires_revenge_flip_only_trigger',
      'action.place_order revenge_flip_v1 requires exactly one upstream trigger.market_price bindingMode=revenge_flip_only.'
    );
  }

  validatePositiveNumber(
    issues,
    node,
    'invalid_revenge_flip_initial_order_usdc',
    'initialOrderUsdc',
    nestedNumber(config, revenge, 'initialOrderUsdc')
  );
  const profitTargetUsdc = nestedNumber(config, revenge, 'profitTargetUsdc');
  if (profitTargetUsdc == null) {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_profit_target_usdc',
      'action.place_order revenge_flip_v1 profitTargetUsdc must be finite.'
    );
  }
  const classicStopLossEnabled =
    toBooleanish(revenge.classicStopLossEnabled ?? config.classicStopLossEnabled) !== false;
  const ptbStopLossEnabled =
    toBooleanish(revenge.ptbStopLossEnabled ?? config.ptbStopLossEnabled) === true;
  const stopLossPct = nestedNumber(config, revenge, 'stopLossPct');
  if (classicStopLossEnabled && (stopLossPct == null || stopLossPct <= 0 || stopLossPct >= 1)) {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_stop_loss_pct',
      'action.place_order revenge_flip_v1 stopLossPct must be > 0 and < 1.'
    );
  }
  if (!classicStopLossEnabled && !ptbStopLossEnabled) {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_classic_stop_loss_disabled',
      'action.place_order revenge_flip_v1 classicStopLossEnabled=false requires ptbStopLossEnabled=true.'
    );
  }
  if (classicStopLossEnabled) {
    validateStopLossRules(issues, node, revenge.stopLossRules ?? config.stopLossRules);
  }
  validateEntryPtbRules(issues, node, revenge.entryPtbRules ?? config.entryPtbRules);
  const reentrySideMode = toTrimmedString(revenge.reentrySideMode ?? config.reentrySideMode)
    .toLowerCase();
  if (reentrySideMode && reentrySideMode !== 'opposite' && reentrySideMode !== 'rule_match') {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_reentry_side_mode',
      'action.place_order revenge_flip_v1 reentrySideMode must be opposite or rule_match.'
    );
  }
  const postStopLossIvMismatch =
    revenge.postStopLossIvMismatchEnabled ?? config.postStopLossIvMismatchEnabled;
  if (postStopLossIvMismatch != null && toBooleanish(postStopLossIvMismatch) == null) {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_post_stop_loss_iv_mismatch',
      'action.place_order revenge_flip_v1 postStopLossIvMismatchEnabled must be boolean.'
    );
  }
  validatePtbStopLoss(issues, node, config, revenge);
  const lotLimitPct = nestedNumber(config, revenge, 'lotLimitPct');
  if (lotLimitPct == null || lotLimitPct <= 0 || lotLimitPct > 1) {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_lot_limit_pct',
      'action.place_order revenge_flip_v1 lotLimitPct must be > 0 and <= 1.'
    );
  }
  const rawMinReentryShares = revenge.minReentryShares ?? config.minReentryShares;
  const minReentryShares = toFiniteNumber(rawMinReentryShares);
  if (rawMinReentryShares != null && (minReentryShares == null || minReentryShares < 0)) {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_min_reentry_shares',
      'action.place_order revenge_flip_v1 minReentryShares must be >= 0.'
    );
  }
  const closeOnlySec = nestedNumber(config, revenge, 'closeOnlySec');
  if (closeOnlySec == null || !Number.isInteger(closeOnlySec) || closeOnlySec < 0) {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_close_only_sec',
      'action.place_order revenge_flip_v1 closeOnlySec must be an integer >= 0.'
    );
  }
  const maxFlip = nestedNumber(config, revenge, 'maxFlip');
  if (maxFlip != null && (!Number.isInteger(maxFlip) || maxFlip < 0)) {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_max_flip',
      'action.place_order revenge_flip_v1 maxFlip must be an integer >= 0.'
    );
  }

  const triggerPrice = isRecord(config.triggerPrice) ? config.triggerPrice : {};
  const triggerEnabled = toBooleanish(triggerPrice.enabled) === true;
  if (triggerEnabled) {
    const minCent = toFiniteNumber(triggerPrice.minCent);
    const maxCent = toFiniteNumber(triggerPrice.maxCent);
    if (
      minCent == null ||
      maxCent == null ||
      minCent < 0 ||
      maxCent > 100 ||
      minCent > maxCent
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_revenge_flip_trigger_price_range',
        'action.place_order revenge_flip_v1 triggerPrice min/max must be within 0..100 cent.'
      );
    }
  }

  const ptbUnit = toTrimmedString(config.priceToBeatMinDiffUnit ?? config.priceToBeatMaxDiffUnit).toLowerCase();
  if (ptbUnit && ptbUnit !== 'usd' && ptbUnit !== 'cent') {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_ptb_unit',
      'action.place_order revenge_flip_v1 priceToBeatMinDiffUnit must be usd or cent.'
    );
  }
  const ptbDiff = toFiniteNumber(config.priceToBeatMinDiff ?? config.priceToBeatMaxDiff);
  if (ptbDiff != null && ptbDiff < 0) {
    pushNodeError(
      issues,
      node,
      'invalid_revenge_flip_ptb_diff',
      'action.place_order revenge_flip_v1 priceToBeatMinDiff must be >= 0.'
    );
  }

  validateTimeRules(issues, node, revenge.timeRules ?? config.timeRules);
}
