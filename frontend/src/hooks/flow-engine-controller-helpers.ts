'use client';

import { formatClientRequestError, hasClientRequestErrorCode } from '@/lib/http-client';
import { FLOW_DEFINITION_BUSY_CODE, FLOW_DEFINITION_BUSY_MESSAGE } from '@/lib/queries/trade-flow/mutation-errors';
import { buildContextFromForm, type ContextFormState } from '@/lib/trade-flow-config-mappers';
import {
  createAvgReboundPairlockRescueGraph,
  createAvgReboundPairlockRescueMicro20Graph,
  createConfidenceLadderHedgeLockGraph,
  createDcaTradeFlowGraph,
  createMultiLegHedgeGraph,
  createPairLockHyperliquid70To80Graph,
  createPositionMonitorNotifyGraph,
  createPositiveFlipPairlockCompressionGraph,
  createPositiveQuantityFlipGrid1UsdcGraph,
  createPositiveQuantityFlipGridInventoryBalanceGraph,
  createRevengeFlip10_80Graph,
  createStarterTradeFlowGraph,
  createStopLossTakeProfitGraph,
} from '@/lib/trade-flow-templates';
import type { TradeFlowGraph } from '@/lib/types';
import type { TemplateKind } from '@/components/trade-builder/flow-engine-types';
import { createSellBuyIfElseTemplate, isRecord } from '@/components/trade-builder/flow-engine-utils';

export function buildFlowDraftPersistPayload(graphJson: TradeFlowGraph, draftName: string, draftDescription: string) {
  const name = draftName.trim();
  if (!name) {
    throw new Error('Flow adi bos olamaz.');
  }
  return {
    name,
    description: draftDescription.trim() || null,
    graphJson,
  };
}

export function createTemplateGraph(kind: TemplateKind, defaultMarketSlug: string | null, defaultOutcome: { token_id: string; label: string } | null): TradeFlowGraph {
  const templateMap: Record<TemplateKind, () => TradeFlowGraph> = {
    starter: () => createStarterTradeFlowGraph(defaultMarketSlug, defaultOutcome),
    sell_buy_if: () => createSellBuyIfElseTemplate(defaultMarketSlug, defaultOutcome),
    dca: () => createDcaTradeFlowGraph(defaultMarketSlug, defaultOutcome),
    sl_tp: () => createStopLossTakeProfitGraph(defaultMarketSlug, defaultOutcome),
    position_monitor: () => createPositionMonitorNotifyGraph(defaultMarketSlug, defaultOutcome),
    multi_leg_hedge: () => createMultiLegHedgeGraph(defaultMarketSlug, defaultOutcome),
    revenge_flip_10_80: () => createRevengeFlip10_80Graph(defaultMarketSlug, defaultOutcome),
    confidence_ladder_hedge_lock: () => createConfidenceLadderHedgeLockGraph(defaultMarketSlug, defaultOutcome),
    avg_rebound_pairlock_rescue_50usdc: () => createAvgReboundPairlockRescueGraph(defaultMarketSlug, defaultOutcome),
    avg_rebound_pairlock_rescue_micro_20usdc: () => createAvgReboundPairlockRescueMicro20Graph(defaultMarketSlug, defaultOutcome),
    pairlock_hyperliquid_70_80: () => createPairLockHyperliquid70To80Graph(defaultMarketSlug, defaultOutcome),
    positive_quantity_flip_grid_1usdc: () => createPositiveQuantityFlipGrid1UsdcGraph(defaultMarketSlug, defaultOutcome),
    positive_quantity_flip_grid_inventory_balance: () => createPositiveQuantityFlipGridInventoryBalanceGraph(defaultMarketSlug, defaultOutcome),
    positive_flip_pairlock_compression: () => createPositiveFlipPairlockCompressionGraph(defaultMarketSlug, defaultOutcome),
  };

  return templateMap[kind]();
}

export function formatFlowOperationError(error: unknown, fallback: string): string {
  if (hasClientRequestErrorCode(error, FLOW_DEFINITION_BUSY_CODE)) {
    return FLOW_DEFINITION_BUSY_MESSAGE;
  }
  return formatClientRequestError(error, fallback);
}

export function isFlowDefinitionBusyMessage(message: string | null | undefined): boolean {
  return (message ?? '').trim().startsWith(FLOW_DEFINITION_BUSY_MESSAGE);
}

export function getTemplateCreatedMessage(kind: TemplateKind): string {
  const templateLabels: Record<TemplateKind, string> = {
    starter: 'Starter flow olusturuldu.',
    sell_buy_if: 'Satis + If/Else + Alis sablonu olusturuldu.',
    dca: 'DCA sablonu olusturuldu.',
    sl_tp: 'Stop Loss + Take Profit sablonu olusturuldu.',
    position_monitor: 'Pozisyon Izleme + Bildirim sablonu olusturuldu.',
    multi_leg_hedge: 'Multi-Leg Hedge sablonu olusturuldu.',
    revenge_flip_10_80: 'RevengeFlip 10/80 sablonu olusturuldu.',
    confidence_ladder_hedge_lock: 'BTC 5m Confidence Ladder + Hedge Lock sablonu olusturuldu.',
    avg_rebound_pairlock_rescue_50usdc: 'Avg-Rebound Pairlock Rescue 50 USDC sablonu olusturuldu.',
    avg_rebound_pairlock_rescue_micro_20usdc: 'Avg-Rebound Micro 23 USDC sablonu olusturuldu.',
    pairlock_hyperliquid_70_80: 'PairLock 70-80 Hyperliquid sablonu olusturuldu.',
    positive_quantity_flip_grid_1usdc: 'Positive Quantity Flip Grid 1 USDC sablonu olusturuldu.',
    positive_quantity_flip_grid_inventory_balance: 'Positive Grid Inventory Balance sablonu olusturuldu.',
    positive_flip_pairlock_compression: 'Positive Flip Pairlock Compression sablonu olusturuldu.',
  };

  return templateLabels[kind];
}

export function resolveFlowContextInput(contextTab: 'basic' | 'advanced', contextForm: ContextFormState) {
  if (contextTab === 'advanced') {
    try {
      const parsed = JSON.parse(contextForm.advancedJson) as unknown;
      if (!isRecord(parsed)) throw new Error('Context JSON nesne olmali.');
      return { context: parsed as Record<string, unknown>, errorMessage: null };
    } catch (error) {
      return {
        context: null,
        errorMessage: error instanceof Error ? `Context JSON hatali: ${error.message}` : 'Context JSON hatali.',
      };
    }
  }

  return {
    context: buildContextFromForm(contextForm),
    errorMessage: null,
  };
}
