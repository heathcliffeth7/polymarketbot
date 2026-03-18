'use client';

import { formatClientRequestError } from '@/lib/http-client';
import {
  buildContextFromForm,
  type ContextFormState,
} from '@/lib/trade-flow-config-mappers';
import {
  createDcaTradeFlowGraph,
  createMultiLegHedgeGraph,
  createPositionMonitorNotifyGraph,
  createStarterTradeFlowGraph,
  createStopLossTakeProfitGraph,
} from '@/lib/trade-flow-templates';
import type { TradeFlowGraph } from '@/lib/types';
import type { TemplateKind } from '@/components/trade-builder/flow-engine-types';
import { createSellBuyIfElseTemplate, isRecord } from '@/components/trade-builder/flow-engine-utils';

export function buildFlowDraftPersistPayload(
  graphJson: TradeFlowGraph,
  draftName: string,
  draftDescription: string
) {
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

export function createTemplateGraph(
  kind: TemplateKind,
  defaultMarketSlug: string | null,
  defaultOutcome: { token_id: string; label: string } | null
): TradeFlowGraph {
  const templateMap: Record<TemplateKind, () => TradeFlowGraph> = {
    starter: () => createStarterTradeFlowGraph(defaultMarketSlug, defaultOutcome),
    sell_buy_if: () => createSellBuyIfElseTemplate(defaultMarketSlug, defaultOutcome),
    dca: () => createDcaTradeFlowGraph(defaultMarketSlug, defaultOutcome),
    sl_tp: () => createStopLossTakeProfitGraph(defaultMarketSlug, defaultOutcome),
    position_monitor: () =>
      createPositionMonitorNotifyGraph(defaultMarketSlug, defaultOutcome),
    multi_leg_hedge: () => createMultiLegHedgeGraph(defaultMarketSlug, defaultOutcome),
  };

  return templateMap[kind]();
}

export function formatFlowOperationError(error: unknown, fallback: string): string {
  return formatClientRequestError(error, fallback);
}

export function getTemplateCreatedMessage(kind: TemplateKind): string {
  const templateLabels: Record<TemplateKind, string> = {
    starter: 'Starter flow olusturuldu.',
    sell_buy_if: 'Satis + If/Else + Alis sablonu olusturuldu.',
    dca: 'DCA sablonu olusturuldu.',
    sl_tp: 'Stop Loss + Take Profit sablonu olusturuldu.',
    position_monitor: 'Pozisyon Izleme + Bildirim sablonu olusturuldu.',
    multi_leg_hedge: 'Multi-Leg Hedge sablonu olusturuldu.',
  };

  return templateLabels[kind];
}

export function resolveFlowContextInput(
  contextTab: 'basic' | 'advanced',
  contextForm: ContextFormState
) {
  if (contextTab === 'advanced') {
    try {
      const parsed = JSON.parse(contextForm.advancedJson) as unknown;
      if (!isRecord(parsed)) throw new Error('Context JSON nesne olmali.');
      return { context: parsed as Record<string, unknown>, errorMessage: null };
    } catch (error) {
      return {
        context: null,
        errorMessage:
          error instanceof Error ? `Context JSON hatali: ${error.message}` : 'Context JSON hatali.',
      };
    }
  }

  return {
    context: buildContextFromForm(contextForm),
    errorMessage: null,
  };
}
