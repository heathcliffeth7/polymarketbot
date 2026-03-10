import { getTradeBuilderWorkflowById } from '../trade-builder';
import { pool } from '@/lib/db';
import type { TradeFlowGraph } from '@/lib/types';
import { DEFAULT_GRAPH } from './shared';
import { validateTradeFlowGraph } from './validation';
import { createTradeFlowDefinition } from './definitions';

function buildLegacyFlowGraph(workflow: NonNullable<Awaited<ReturnType<typeof getTradeBuilderWorkflowById>>>): TradeFlowGraph {
  const sellLeg = workflow.legs.find((leg) => leg.leg_type === 'sell');
  const buyLeg = workflow.legs.find((leg) => leg.leg_type === 'buy');
  if (!sellLeg || !buyLeg) {
    return DEFAULT_GRAPH;
  }

  const sellProgressExpr = {
    '>=': [{ var: 'sell_progress_pct' }, workflow.workflow.buy_start_after_sell_progress_pct],
  };

  const buyPriceExpr =
    buyLeg.trigger_condition === 'cross_above'
      ? { '>=': [{ var: 'market_price' }, (buyLeg.trigger_price || 0) * 100] }
      : buyLeg.trigger_condition === 'cross_below'
        ? { '<=': [{ var: 'market_price' }, (buyLeg.trigger_price || 0) * 100] }
        : null;

  let gateExpression: Record<string, unknown>;
  if (workflow.workflow.buy_trigger_mode === 'sell_progress_only') {
    gateExpression = sellProgressExpr;
  } else if (workflow.workflow.buy_trigger_mode === 'price_only') {
    gateExpression = buyPriceExpr || { '==': [1, 1] };
  } else {
    gateExpression = buyPriceExpr
      ? { and: [sellProgressExpr, buyPriceExpr] }
      : sellProgressExpr;
  }

  return {
    context: {
      sourceTradeId: workflow.workflow.source_trade_id,
      marketSlug: sellLeg.market_slug,
      tokenId: sellLeg.token_id,
      outcomeLabel: sellLeg.outcome_label,
    },
    nodes: [
      {
        key: 'trigger_market_tick',
        type: 'trigger.market_price',
        positionX: 100,
        positionY: 150,
        config: {
          marketSlug: sellLeg.market_slug,
          tokenId: sellLeg.token_id,
        },
      },
      {
        key: 'action_sell',
        type: 'action.place_order',
        positionX: 420,
        positionY: 80,
        config: {
          side: sellLeg.side,
          executionMode: 'market',
          marketSlug: sellLeg.market_slug,
          tokenId: sellLeg.token_id,
          outcomeLabel: sellLeg.outcome_label,
          minPriceDistanceCent: sellLeg.min_price_distance_cent,
          triggerCondition: sellLeg.trigger_condition,
          triggerPriceCent:
            sellLeg.trigger_price == null ? null : Math.round(sellLeg.trigger_price * 100),
          targetNotionalUsdc: sellLeg.target_notional_usdc,
        },
      },
      {
        key: 'if_buy_gate',
        type: 'logic.if',
        positionX: 720,
        positionY: 150,
        config: {
          expression: gateExpression,
          mode: workflow.workflow.buy_trigger_mode,
        },
      },
      {
        key: 'action_buy',
        type: 'action.place_order',
        positionX: 1020,
        positionY: 80,
        config: {
          side: buyLeg.side,
          executionMode: 'market',
          marketSlug: buyLeg.market_slug,
          tokenId: buyLeg.token_id,
          outcomeLabel: buyLeg.outcome_label,
          minPriceDistanceCent: buyLeg.min_price_distance_cent,
          triggerCondition: buyLeg.trigger_condition,
          triggerPriceCent: buyLeg.trigger_price == null ? null : Math.round(buyLeg.trigger_price * 100),
          targetNotionalUsdc: buyLeg.target_notional_usdc,
        },
      },
      {
        key: 'action_wait',
        type: 'action.set_state',
        positionX: 1020,
        positionY: 250,
        config: {
          statePatch: {
            state: 'waiting_sell_progress',
            reason: 'buy_gate_not_satisfied',
          },
        },
      },
    ],
    edges: [
      {
        key: 'e1',
        source: 'trigger_market_tick',
        target: 'action_sell',
        type: 'default',
        condition: null,
      },
      {
        key: 'e2',
        source: 'action_sell',
        target: 'if_buy_gate',
        type: 'on_success',
        condition: null,
      },
      {
        key: 'e3',
        source: 'if_buy_gate',
        target: 'action_buy',
        type: 'on_true',
        condition: null,
      },
      {
        key: 'e4',
        source: 'if_buy_gate',
        target: 'action_wait',
        type: 'on_false',
        condition: null,
      },
    ],
  };
}

export async function migrateLegacyWorkflowsToFlows(userId: number, limit = 50): Promise<number> {
  const pendingRes = await pool.query(
    `SELECT w.id
     FROM trade_builder_workflows w
     LEFT JOIN trade_flow_legacy_mappings m ON m.legacy_workflow_id = w.id
     WHERE m.legacy_workflow_id IS NULL
       AND w.user_id = $1
     ORDER BY w.id ASC
     LIMIT $2`,
    [userId, Math.max(1, limit)]
  );

  let migrated = 0;
  for (const row of pendingRes.rows) {
    const workflowId = Number(row.id);
    if (!Number.isFinite(workflowId) || workflowId <= 0) continue;

    try {
      const created = await createFlowFromLegacyWorkflow(userId, workflowId);
      if (created) migrated += 1;
    } catch (err) {
      console.error('Legacy workflow migration error:', workflowId, err);
    }
  }

  return migrated;
}

export async function createFlowFromLegacyWorkflow(userId: number, workflowId: number): Promise<boolean> {
  const existingMapRes = await pool.query(
    'SELECT definition_id FROM trade_flow_legacy_mappings WHERE legacy_workflow_id = $1 LIMIT 1',
    [workflowId]
  );
  if ((existingMapRes.rowCount ?? 0) > 0) {
    return false;
  }

  const legacy = await getTradeBuilderWorkflowById(userId, workflowId);
  if (!legacy) {
    throw new Error(`Legacy workflow not found: ${workflowId}`);
  }

  const graph = buildLegacyFlowGraph(legacy);
  const validation = validateTradeFlowGraph(graph);
  if (!validation.valid) {
    throw new Error(
      `Cannot migrate legacy workflow ${workflowId}: ${validation.issues
        .filter((issue) => issue.severity === 'error')
        .map((issue) => issue.message)
        .join(' | ')}`
    );
  }

  await createTradeFlowDefinition({
    userId,
    name: `Legacy ${legacy.workflow.name} (#${legacy.workflow.id})`,
    description: 'Migrated from trade_builder_workflows',
    graphJson: graph,
    legacyWorkflowId: legacy.workflow.id,
  });

  return true;
}
