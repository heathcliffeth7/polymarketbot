import type { TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { isPtbCurrentPriceSource } from '@/lib/trade-flow-config-mappers/ptb-modes';
import { isRecord, toFiniteNumber } from './shared';
import { pushNodeError } from './validation-core';

export interface ParsedPtbStopLossRule {
  gapUsd: number;
  sizePct: number;
}

export function normalizePtbStopLossGapUnitValue(value: unknown): 'usd' | 'cent' | null {
  const normalized = String(value ?? '').trim().toLowerCase();
  if (!normalized || normalized === 'usd') return 'usd';
  if (normalized === 'cent') return 'cent';
  return null;
}

export function parsePtbStopLossRules(raw: unknown): {
  isArray: boolean;
  validRules: ParsedPtbStopLossRule[];
  invalidItem: boolean;
} {
  if (!Array.isArray(raw)) {
    return { isArray: false, validRules: [], invalidItem: false };
  }

  const validRules: ParsedPtbStopLossRule[] = [];
  let invalidItem = false;
  for (const item of raw) {
    if (!isRecord(item)) {
      invalidItem = true;
      continue;
    }
    const gapUsd = toFiniteNumber(item.gapUsd);
    const sizePct = toFiniteNumber(item.sizePct);
    if (gapUsd == null || sizePct == null || sizePct <= 0 || sizePct > 100) {
      invalidItem = true;
      continue;
    }
    validRules.push({ gapUsd, sizePct });
  }

  if (raw.length > 0 && validRules.length === 0) {
    invalidItem = true;
  }

  return { isArray: true, validRules, invalidItem };
}

interface ValidateActionPlaceOrderPtbStopLossConfigInput {
  side: string;
  graphMarketSlug: string;
  hasResolveMarketNode: boolean;
  hasUpstreamMarketPriceAutoScope: boolean;
  ptbStopLossEnabled: boolean | null;
  parsedPtbStopLossRules: {
    isArray: boolean;
    validRules: ParsedPtbStopLossRule[];
    invalidItem: boolean;
  };
}

export function validateActionPlaceOrderPtbStopLossConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>,
  {
    side,
    graphMarketSlug,
    hasResolveMarketNode,
    hasUpstreamMarketPriceAutoScope,
    ptbStopLossEnabled,
    parsedPtbStopLossRules,
  }: ValidateActionPlaceOrderPtbStopLossConfigInput
) {
  const ptbStopLossGapUnit = normalizePtbStopLossGapUnitValue(config.ptbStopLossGapUnit);
  if (config.ptbStopLossGapUnit != null && ptbStopLossGapUnit == null) {
    pushNodeError(
      issues,
      node,
      'invalid_ptb_stop_loss_gap_unit',
      'action.place_order ptbStopLossGapUnit must be usd or cent when provided.'
    );
  }

  const ptbStopLossGapConfigured = config.ptbStopLossGapUsd != null;
  const ptbStopLossRulesConfigured = config.ptbStopLossRules != null;
  const hasPtbStopLossRules = parsedPtbStopLossRules.validRules.length > 0;
  const ptbStopLossActive = ptbStopLossEnabled === true || hasPtbStopLossRules;
  const ptbStopLossCurrentSourceRaw = String(config.ptbStopLossCurrentPriceSource ?? '')
    .trim()
    .toLowerCase();

  if (ptbStopLossCurrentSourceRaw && !isPtbCurrentPriceSource(ptbStopLossCurrentSourceRaw)) {
    pushNodeError(
      issues,
      node,
      'invalid_ptb_stop_loss_current_price_source',
      'action.place_order ptbStopLossCurrentPriceSource must be chainlink, binance, or coinbase.'
    );
  }
  if (ptbStopLossCurrentSourceRaw && !ptbStopLossActive) {
    pushNodeError(
      issues,
      node,
      'ptb_stop_loss_current_price_source_requires_ptb_stop_loss',
      'action.place_order ptbStopLossCurrentPriceSource requires ptbStopLossEnabled=true or ptbStopLossRules.'
    );
  }

  if (ptbStopLossEnabled === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_ptb_stop_loss_side',
      'action.place_order ptbStopLossEnabled is only valid for side=buy.'
    );
  }
  if (parsedPtbStopLossRules.isArray && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_ptb_stop_loss_rules_side',
      'action.place_order ptbStopLossRules is only valid for side=buy.'
    );
  }
  if (
    ptbStopLossEnabled !== true &&
    side === 'buy' &&
    (ptbStopLossGapConfigured || ptbStopLossRulesConfigured)
  ) {
    pushNodeError(
      issues,
      node,
      'ptb_stop_loss_toggle_required',
      'action.place_order PTB stop-loss config requires ptbStopLossEnabled=true.'
    );
  }
  if (parsedPtbStopLossRules.isArray && parsedPtbStopLossRules.validRules.length > 5) {
    pushNodeError(
      issues,
      node,
      'invalid_ptb_stop_loss_rules_length',
      'action.place_order ptbStopLossRules cannot contain more than 5 entries.'
    );
  }
  if (parsedPtbStopLossRules.invalidItem) {
    pushNodeError(
      issues,
      node,
      'invalid_ptb_stop_loss_rules',
      'action.place_order ptbStopLossRules entries must provide finite gapUsd and sizePct in (0, 100].'
    );
  }

  if (hasPtbStopLossRules) {
    const ptbStopLossRulesSum = parsedPtbStopLossRules.validRules.reduce(
      (sum, item) => sum + item.sizePct,
      0
    );
    if (Math.abs(ptbStopLossRulesSum - 100) > 0.000001) {
      pushNodeError(
        issues,
        node,
        'invalid_ptb_stop_loss_rules_sum',
        'action.place_order ptbStopLossRules total sizePct must equal 100.'
      );
    }
    for (let index = 1; index < parsedPtbStopLossRules.validRules.length; index += 1) {
      if (
        parsedPtbStopLossRules.validRules[index - 1].gapUsd <=
        parsedPtbStopLossRules.validRules[index].gapUsd
      ) {
        pushNodeError(
          issues,
          node,
          'invalid_ptb_stop_loss_rules_order',
          'action.place_order ptbStopLossRules gapUsd values must be strictly decreasing.'
        );
        break;
      }
    }
  }

  const ptbStopLossGapUsd = toFiniteNumber(config.ptbStopLossGapUsd);
  if (ptbStopLossActive) {
    if (ptbStopLossGapUsd == null && !hasPtbStopLossRules) {
      pushNodeError(
        issues,
        node,
        'missing_ptb_stop_loss_config',
        'action.place_order ptbStopLossEnabled=true requires ptbStopLossGapUsd or ptbStopLossRules.'
      );
    } else if (ptbStopLossGapUsd != null && ptbStopLossGapUsd < 0) {
      // Negative gap is allowed; it means waiting for counter-direction overshoot past parity/PTB reference.
    }

    const effectiveMarketSlug = String(config.marketSlug ?? graphMarketSlug).trim().toLowerCase();
    const hasSupportedRuntimeMarket = hasResolveMarketNode || hasUpstreamMarketPriceAutoScope;
    const isSupportedExplicitMarket =
      effectiveMarketSlug.length > 0 &&
      /^(btc|eth|sol|xrp)-updown-(5m|15m)-/.test(effectiveMarketSlug);
    if (effectiveMarketSlug.length > 0 && !isSupportedExplicitMarket) {
      pushNodeError(
        issues,
        node,
        'invalid_ptb_stop_loss_market',
        'ptbStopLossEnabled and ptbStopLossRules only support 5m/15m updown market slugs.'
      );
    } else if (effectiveMarketSlug.length === 0 && !hasSupportedRuntimeMarket) {
      pushNodeError(
        issues,
        node,
        'missing_ptb_stop_loss_market',
        'PTB stop-loss requires a 5m/15m updown market slug or an upstream trigger.market_price/runtime market resolver.'
      );
    }
  } else if (config.ptbStopLossGapUsd != null && ptbStopLossGapUsd == null) {
    pushNodeError(
      issues,
      node,
      'invalid_ptb_stop_loss_gap_usd',
      'action.place_order ptbStopLossGapUsd must be a finite number.'
    );
  }
}
