import type { TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { toBooleanish, toFiniteNumber, toTrimmedString } from './shared';
import { pushNodeError } from './validation-core';

const BASIS_VALUES = new Set(['own_open_gap', 'current_price']);
const OWN_OPEN_GAP_MODES = new Set([
  'binance_coinbase',
  'open_gap_okx_plus_one',
  'open_gap_okx_plus_one_or_clean_pair',
]);
const CURRENT_PRICE_MODES = new Set([
  'binance_coinbase',
  'asset_auto_plus_one_or_clean_pair',
  'bybit_plus_one',
  'bybit_plus_one_or_clean_pair',
  'okx_plus_one',
  'okx_plus_one_or_clean_pair',
  'gate_plus_one',
  'gate_plus_one_or_clean_pair',
]);

export function validateActionPlaceOrderCexEntryConsensusConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>
): void {
  const source = toTrimmedString(config.priceToBeatCurrentPriceSource).toLowerCase();
  if (source !== 'chainlink_cex_consensus') return;

  const basis = toTrimmedString(config.cexEntryConsensusBasis).toLowerCase();
  if (config.cexEntryConsensusBasis != null && !BASIS_VALUES.has(basis)) {
    pushNodeError(
      issues,
      node,
      'invalid_cex_entry_consensus_basis',
      'action.place_order cexEntryConsensusBasis must be own_open_gap or current_price.'
    );
  }

  const effectiveBasis = basis === 'current_price' ? 'current_price' : 'own_open_gap';
  const mode = toTrimmedString(config.cexEntryConsensusMode).toLowerCase();
  if (config.cexEntryConsensusMode != null) {
    const validModes =
      effectiveBasis === 'current_price' ? CURRENT_PRICE_MODES : OWN_OPEN_GAP_MODES;
    if (!validModes.has(mode)) {
      pushNodeError(
        issues,
        node,
        'invalid_cex_entry_consensus_mode',
        effectiveBasis === 'current_price'
          ? 'action.place_order current_price CEX entry mode must be asset_auto_plus_one_or_clean_pair, binance_coinbase, or a rollback current-price mode.'
          : 'action.place_order own_open_gap CEX entry mode must be binance_coinbase, open_gap_okx_plus_one, or open_gap_okx_plus_one_or_clean_pair.'
      );
    }
  }

  validateNumber(
    issues,
    node,
    config,
    'cexEntryOpenGapThresholdUsd',
    'invalid_cex_entry_open_gap_threshold_usd',
    (value) => value > 0
  );
  validateNumber(
    issues,
    node,
    config,
    'cexEntryOpenGapMinVenues',
    'invalid_cex_entry_open_gap_min_venues',
    (value) => Number.isInteger(value) && value >= 2 && value <= 3
  );
  validateNumber(
    issues,
    node,
    config,
    'cexEntryOpenGapRatioMin',
    'invalid_cex_entry_open_gap_ratio_min',
    (value) => value > 0 && value <= 1
  );
  validateNumber(
    issues,
    node,
    config,
    'cexEntryOpenGapSpreadFloorUsd',
    'invalid_cex_entry_open_gap_spread_floor_usd',
    (value) => value >= 0
  );
  validateNumber(
    issues,
    node,
    config,
    'cexEntryOpenGapSpreadExpectedMoveMult',
    'invalid_cex_entry_open_gap_spread_expected_move_mult',
    (value) => value > 0
  );
  validateBoolean(
    issues,
    node,
    config,
    'cexEntryOpenGapAllowCleanPairWithoutAnchor',
    'invalid_cex_entry_open_gap_allow_clean_pair_without_anchor'
  );
  validateBoolean(
    issues,
    node,
    config,
    'cexEntryChainlinkSanityCheck',
    'invalid_cex_entry_chainlink_sanity_check'
  );
}

function validateNumber(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>,
  key: string,
  code: string,
  valid: (value: number) => boolean
): void {
  if (config[key] == null) return;
  const value = toFiniteNumber(config[key]);
  if (value == null || !valid(value)) {
    pushNodeError(issues, node, code, `action.place_order ${key} is outside the supported range.`);
  }
}

function validateBoolean(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>,
  key: string,
  code: string
): void {
  if (config[key] != null && toBooleanish(config[key]) == null) {
    pushNodeError(issues, node, code, `action.place_order ${key} must be boolean (true/false).`);
  }
}
