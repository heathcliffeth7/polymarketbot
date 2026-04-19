import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { pushNodeError } from './validation-core';
import { toBooleanish, toFiniteNumber } from './shared';

export function validateActionPlaceOrderPtbV2Config(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  _graph: TradeFlowGraph,
  side: string,
  config: Record<string, unknown>,
  priceToBeatGuardEnabled: boolean,
  reenterOnSlHit: boolean,
  ptbStopLossEnabled: boolean
) {
  const relaxMinDepthUsd = toFiniteNumber(config.priceToBeatMaxPriceRelaxMinDepthUsd);
  if (
    config.priceToBeatMaxPriceRelaxMinDepthUsd != null &&
    (relaxMinDepthUsd == null || relaxMinDepthUsd <= 0)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_max_price_relax_min_depth_usd',
      'action.place_order priceToBeatMaxPriceRelaxMinDepthUsd must be > 0.'
    );
  }

  const bumpDecayWindows = toFiniteNumber(config.priceToBeatStopLossBumpDecayWindows);
  if (
    config.priceToBeatStopLossBumpDecayWindows != null &&
    (!Number.isInteger(bumpDecayWindows) || bumpDecayWindows == null || bumpDecayWindows <= 0)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_stop_loss_bump_decay_windows',
      'action.place_order priceToBeatStopLossBumpDecayWindows must be a positive integer.'
    );
  }

  const bumpScope = String(config.priceToBeatStopLossBumpScope ?? '').trim().toLowerCase();
  if (
    config.priceToBeatStopLossBumpScope != null &&
    bumpScope !== 'global' &&
    bumpScope !== 'per_scope'
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_stop_loss_bump_scope',
      'action.place_order priceToBeatStopLossBumpScope must be global or per_scope.'
    );
  }

  const reentryCooldownSec = toFiniteNumber(config.reentryCooldownSec);
  if (
    config.reentryCooldownSec != null &&
    (!Number.isInteger(reentryCooldownSec) || reentryCooldownSec == null || reentryCooldownSec < 0)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_reentry_cooldown_sec',
      'action.place_order reentryCooldownSec must be an integer >= 0.'
    );
  }

  const reentrySkipCurrentWindow = toBooleanish(config.reentrySkipCurrentWindow);
  if (config.reentrySkipCurrentWindow != null && reentrySkipCurrentWindow == null) {
    pushNodeError(
      issues,
      node,
      'invalid_reentry_skip_current_window',
      'action.place_order reentrySkipCurrentWindow must be boolean (true/false).'
    );
  }
  if (reentrySkipCurrentWindow === true && !reenterOnSlHit) {
    pushNodeError(
      issues,
      node,
      'reentry_skip_current_window_requires_reentry',
      'action.place_order reentrySkipCurrentWindow requires reenterOnSlHit=true.'
    );
  }

  const reentryThresholdDecay = toFiniteNumber(config.reentryThresholdDecay);
  if (
    config.reentryThresholdDecay != null &&
    (reentryThresholdDecay == null || reentryThresholdDecay <= 0 || reentryThresholdDecay > 1)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_reentry_threshold_decay',
      'action.place_order reentryThresholdDecay must be in (0, 1].'
    );
  }
  if (config.reentryThresholdDecay != null && (!reenterOnSlHit || !priceToBeatGuardEnabled)) {
    pushNodeError(
      issues,
      node,
      'reentry_threshold_decay_requires_ptb_reentry',
      'action.place_order reentryThresholdDecay requires reenterOnSlHit=true and priceToBeatGuardEnabled=true.'
    );
  }

  const reentryMaxPriceTightenBps = toFiniteNumber(config.reentryMaxPriceTightenBps);
  if (
    config.reentryMaxPriceTightenBps != null &&
    (!Number.isInteger(reentryMaxPriceTightenBps) ||
      reentryMaxPriceTightenBps == null ||
      reentryMaxPriceTightenBps < 0 ||
      reentryMaxPriceTightenBps > 10_000)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_reentry_max_price_tighten_bps',
      'action.place_order reentryMaxPriceTightenBps must be an integer in [0, 10000].'
    );
  }
  if (
    config.reentryMaxPriceTightenBps != null &&
    reentryMaxPriceTightenBps !== 0 &&
    !reenterOnSlHit
  ) {
    pushNodeError(
      issues,
      node,
      'reentry_max_price_tighten_bps_requires_reentry',
      'action.place_order reentryMaxPriceTightenBps requires reenterOnSlHit=true.'
    );
  }

  const ptbStopLossTimeDecayMode = String(config.ptbStopLossTimeDecayMode ?? '')
    .trim()
    .toLowerCase();
  if (
    config.ptbStopLossTimeDecayMode != null &&
    ptbStopLossTimeDecayMode !== 'none' &&
    ptbStopLossTimeDecayMode !== 'tighten' &&
    ptbStopLossTimeDecayMode !== 'relax'
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_ptb_stop_loss_time_decay_mode',
      'action.place_order ptbStopLossTimeDecayMode must be none, tighten, or relax.'
    );
  }
  if (config.ptbStopLossTimeDecayMode != null && !ptbStopLossEnabled && side === 'buy') {
    pushNodeError(
      issues,
      node,
      'ptb_stop_loss_time_decay_mode_requires_ptb_stop_loss',
      'action.place_order ptbStopLossTimeDecayMode requires ptbStopLossEnabled=true.'
    );
  }
}
