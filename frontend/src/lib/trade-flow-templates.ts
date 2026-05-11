import type { TradeFlowGraph } from '@/lib/types';

export interface TradeFlowTemplateOutcome {
  token_id: string;
  label: string;
}

export function createStarterTradeFlowGraph(
  marketSlug: string | null,
  outcome: TradeFlowTemplateOutcome | null
): TradeFlowGraph {
  return {
    context: {
      sourceTradeId: 0,
      marketSlug: marketSlug || '',
      tokenId: outcome?.token_id || '',
      outcomeLabel: outcome?.label || '',
    },
    nodes: [
      {
        key: 'trigger_market',
        type: 'trigger.market_price',
        positionX: 100,
        positionY: 120,
        config: {
          marketSlug: marketSlug || '',
          tokenId: outcome?.token_id || '',
          pollIntervalMs: 1500,
        },
      },
      {
        key: 'action_notify',
        type: 'action.notify',
        positionX: 420,
        positionY: 120,
        config: {
          channel: 'ui',
          message: 'Starter flow tetiklendi.',
        },
      },
    ],
    edges: [
      {
        key: 'edge_trigger_notify',
        source: 'trigger_market',
        target: 'action_notify',
        type: 'default',
        condition: null,
      },
    ],
  };
}

export function createDcaTradeFlowGraph(
  marketSlug: string | null,
  outcome: TradeFlowTemplateOutcome | null
): TradeFlowGraph {
  return {
    context: {
      sourceTradeId: 0,
      marketSlug: marketSlug || '',
      tokenId: outcome?.token_id || '',
      outcomeLabel: outcome?.label || '',
    },
    nodes: [
      {
        key: 'trigger_time',
        type: 'trigger.time_window',
        positionX: 80,
        positionY: 140,
        config: { startAt: '', endAt: '', varKey: 'time_window_open', minIntervalMs: 60000 },
      },
      {
        key: 'delay_loop',
        type: 'logic.delay',
        positionX: 380,
        positionY: 140,
        config: { delayMs: 300000 },
      },
      {
        key: 'action_buy_dca',
        type: 'action.place_order',
        positionX: 680,
        positionY: 140,
        config: {
          side: 'buy',
          executionMode: 'market',
          marketSlug: marketSlug || '',
          tokenId: outcome?.token_id || '',
          outcomeLabel: outcome?.label || '',
          sizeUsdc: 10,
          minPriceDistanceCent: 1,
          maxTriggers: 5,
        },
      },
    ],
    edges: [
      { key: 'edge_1', source: 'trigger_time', target: 'delay_loop', type: 'default', condition: null },
      { key: 'edge_2', source: 'delay_loop', target: 'action_buy_dca', type: 'default', condition: null },
    ],
  };
}

export function createStopLossTakeProfitGraph(
  marketSlug: string | null,
  outcome: TradeFlowTemplateOutcome | null
): TradeFlowGraph {
  return {
    context: {
      sourceTradeId: 0,
      marketSlug: marketSlug || '',
      tokenId: outcome?.token_id || '',
      outcomeLabel: outcome?.label || '',
    },
    nodes: [
      {
        key: 'trigger_price',
        type: 'trigger.market_price',
        positionX: 80,
        positionY: 180,
        config: { marketSlug: marketSlug || '', tokenId: outcome?.token_id || '', pollIntervalMs: 1000 },
      },
      {
        key: 'logic_sl_tp',
        type: 'logic.if',
        positionX: 400,
        positionY: 180,
        config: {
          expression: { '>=': [{ var: 'market_price' }, 80] },
          comment: 'market_price >= 80 ise take profit, degilse stop loss kontrolu.',
        },
      },
      {
        key: 'action_tp_sell',
        type: 'action.place_order',
        positionX: 740,
        positionY: 100,
        config: {
          side: 'sell', marketSlug: marketSlug || '', tokenId: outcome?.token_id || '',
          executionMode: 'market',
          outcomeLabel: outcome?.label || '', sizeMode: 'pct', sizePct: 100,
          minPriceDistanceCent: 1, maxTriggers: 1, refKey: 'take_profit',
        },
      },
      {
        key: 'logic_sl_check',
        type: 'logic.if',
        positionX: 740,
        positionY: 260,
        config: {
          expression: { '<=': [{ var: 'market_price' }, 30] },
          comment: 'market_price <= 30 ise stop loss tetikle.',
        },
      },
      {
        key: 'action_sl_sell',
        type: 'action.place_order',
        positionX: 1060,
        positionY: 260,
        config: {
          side: 'sell', marketSlug: marketSlug || '', tokenId: outcome?.token_id || '',
          executionMode: 'market',
          outcomeLabel: outcome?.label || '', sizeMode: 'pct', sizePct: 100,
          minPriceDistanceCent: 1, maxTriggers: 1, refKey: 'stop_loss',
        },
      },
    ],
    edges: [
      { key: 'edge_1', source: 'trigger_price', target: 'logic_sl_tp', type: 'default', condition: null },
      { key: 'edge_2', source: 'logic_sl_tp', target: 'action_tp_sell', type: 'on_true', condition: null },
      { key: 'edge_3', source: 'logic_sl_tp', target: 'logic_sl_check', type: 'on_false', condition: null },
      { key: 'edge_4', source: 'logic_sl_check', target: 'action_sl_sell', type: 'on_true', condition: null },
    ],
  };
}

export function createPositionMonitorNotifyGraph(
  marketSlug: string | null,
  outcome: TradeFlowTemplateOutcome | null
): TradeFlowGraph {
  return {
    context: {
      sourceTradeId: 0,
      marketSlug: marketSlug || '',
      tokenId: outcome?.token_id || '',
      outcomeLabel: outcome?.label || '',
    },
    nodes: [
      {
        key: 'trigger_pos',
        type: 'trigger.open_positions',
        positionX: 80,
        positionY: 140,
        config: {
          marketSlug: marketSlug || '', tokenId: outcome?.token_id || '',
          outcomeLabel: outcome?.label || '', minIntervalMs: 5000,
        },
      },
      {
        key: 'logic_check',
        type: 'logic.if',
        positionX: 400,
        positionY: 140,
        config: {
          expression: { '>=': [{ var: 'position_current_value' }, 50] },
          comment: 'Pozisyon degeri 50 USD ustune ciktiysa bildir.',
        },
      },
      {
        key: 'action_notify',
        type: 'action.notify',
        positionX: 720,
        positionY: 140,
        config: { channel: 'ui', message: 'Pozisyon hedef degere ulasti.' },
      },
    ],
    edges: [
      { key: 'edge_1', source: 'trigger_pos', target: 'logic_check', type: 'default', condition: null },
      { key: 'edge_2', source: 'logic_check', target: 'action_notify', type: 'on_true', condition: null },
    ],
  };
}

export function createMultiLegHedgeGraph(
  marketSlug: string | null,
  outcome: TradeFlowTemplateOutcome | null
): TradeFlowGraph {
  return {
    context: {
      sourceTradeId: 0,
      marketSlug: marketSlug || '',
      tokenId: outcome?.token_id || '',
      outcomeLabel: outcome?.label || '',
    },
    nodes: [
      {
        key: 'trigger_market',
        type: 'trigger.market_price',
        positionX: 80,
        positionY: 180,
        config: { marketSlug: marketSlug || '', tokenId: outcome?.token_id || '', pollIntervalMs: 1000 },
      },
      {
        key: 'action_sell_leg',
        type: 'action.place_order',
        positionX: 400,
        positionY: 100,
        config: {
          side: 'sell', marketSlug: marketSlug || '', tokenId: outcome?.token_id || '',
          executionMode: 'market',
          outcomeLabel: outcome?.label || '', sizeMode: 'pct', sizePct: 50,
          minPriceDistanceCent: 1, maxTriggers: 1, refKey: 'hedge_sell',
        },
      },
      {
        key: 'action_buy_leg',
        type: 'action.place_order',
        positionX: 400,
        positionY: 260,
        config: {
          side: 'buy', marketSlug: marketSlug || '', tokenId: outcome?.token_id || '',
          executionMode: 'market',
          outcomeLabel: outcome?.label || '', sizeUsdc: 15,
          minPriceDistanceCent: 1, maxTriggers: 1, refKey: 'hedge_buy',
        },
      },
      {
        key: 'action_done',
        type: 'action.notify',
        positionX: 720,
        positionY: 180,
        config: { channel: 'ui', message: 'Multi-leg hedge tamamlandi.' },
      },
    ],
    edges: [
      { key: 'edge_1', source: 'trigger_market', target: 'action_sell_leg', type: 'default', condition: null },
      { key: 'edge_2', source: 'trigger_market', target: 'action_buy_leg', type: 'default', condition: null },
      { key: 'edge_3', source: 'action_sell_leg', target: 'action_done', type: 'on_success', condition: null },
      { key: 'edge_4', source: 'action_buy_leg', target: 'action_done', type: 'on_success', condition: null },
    ],
  };
}
