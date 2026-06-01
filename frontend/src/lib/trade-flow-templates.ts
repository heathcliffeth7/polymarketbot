import type { TradeFlowGraph } from '@/lib/types';

export interface TradeFlowTemplateOutcome {
  token_id: string;
  label: string;
}

export function createStarterTradeFlowGraph(marketSlug: string | null, outcome: TradeFlowTemplateOutcome | null): TradeFlowGraph {
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

export function createDcaTradeFlowGraph(marketSlug: string | null, outcome: TradeFlowTemplateOutcome | null): TradeFlowGraph {
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
        config: {
          startAt: '',
          endAt: '',
          varKey: 'time_window_open',
          minIntervalMs: 60000,
        },
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
      {
        key: 'edge_1',
        source: 'trigger_time',
        target: 'delay_loop',
        type: 'default',
        condition: null,
      },
      {
        key: 'edge_2',
        source: 'delay_loop',
        target: 'action_buy_dca',
        type: 'default',
        condition: null,
      },
    ],
  };
}

export function createStopLossTakeProfitGraph(marketSlug: string | null, outcome: TradeFlowTemplateOutcome | null): TradeFlowGraph {
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
        config: {
          marketSlug: marketSlug || '',
          tokenId: outcome?.token_id || '',
          pollIntervalMs: 1000,
        },
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
          side: 'sell',
          marketSlug: marketSlug || '',
          tokenId: outcome?.token_id || '',
          executionMode: 'market',
          outcomeLabel: outcome?.label || '',
          sizeMode: 'pct',
          sizePct: 100,
          minPriceDistanceCent: 1,
          maxTriggers: 1,
          refKey: 'take_profit',
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
          side: 'sell',
          marketSlug: marketSlug || '',
          tokenId: outcome?.token_id || '',
          executionMode: 'market',
          outcomeLabel: outcome?.label || '',
          sizeMode: 'pct',
          sizePct: 100,
          minPriceDistanceCent: 1,
          maxTriggers: 1,
          refKey: 'stop_loss',
        },
      },
    ],
    edges: [
      {
        key: 'edge_1',
        source: 'trigger_price',
        target: 'logic_sl_tp',
        type: 'default',
        condition: null,
      },
      {
        key: 'edge_2',
        source: 'logic_sl_tp',
        target: 'action_tp_sell',
        type: 'on_true',
        condition: null,
      },
      {
        key: 'edge_3',
        source: 'logic_sl_tp',
        target: 'logic_sl_check',
        type: 'on_false',
        condition: null,
      },
      {
        key: 'edge_4',
        source: 'logic_sl_check',
        target: 'action_sl_sell',
        type: 'on_true',
        condition: null,
      },
    ],
  };
}

export function createPositionMonitorNotifyGraph(marketSlug: string | null, outcome: TradeFlowTemplateOutcome | null): TradeFlowGraph {
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
          marketSlug: marketSlug || '',
          tokenId: outcome?.token_id || '',
          outcomeLabel: outcome?.label || '',
          minIntervalMs: 5000,
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
      {
        key: 'edge_1',
        source: 'trigger_pos',
        target: 'logic_check',
        type: 'default',
        condition: null,
      },
      {
        key: 'edge_2',
        source: 'logic_check',
        target: 'action_notify',
        type: 'on_true',
        condition: null,
      },
    ],
  };
}

export function createMultiLegHedgeGraph(marketSlug: string | null, outcome: TradeFlowTemplateOutcome | null): TradeFlowGraph {
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
        config: {
          marketSlug: marketSlug || '',
          tokenId: outcome?.token_id || '',
          pollIntervalMs: 1000,
        },
      },
      {
        key: 'action_sell_leg',
        type: 'action.place_order',
        positionX: 400,
        positionY: 100,
        config: {
          side: 'sell',
          marketSlug: marketSlug || '',
          tokenId: outcome?.token_id || '',
          executionMode: 'market',
          outcomeLabel: outcome?.label || '',
          sizeMode: 'pct',
          sizePct: 50,
          minPriceDistanceCent: 1,
          maxTriggers: 1,
          refKey: 'hedge_sell',
        },
      },
      {
        key: 'action_buy_leg',
        type: 'action.place_order',
        positionX: 400,
        positionY: 260,
        config: {
          side: 'buy',
          marketSlug: marketSlug || '',
          tokenId: outcome?.token_id || '',
          executionMode: 'market',
          outcomeLabel: outcome?.label || '',
          sizeUsdc: 15,
          minPriceDistanceCent: 1,
          maxTriggers: 1,
          refKey: 'hedge_buy',
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
      {
        key: 'edge_1',
        source: 'trigger_market',
        target: 'action_sell_leg',
        type: 'default',
        condition: null,
      },
      {
        key: 'edge_2',
        source: 'trigger_market',
        target: 'action_buy_leg',
        type: 'default',
        condition: null,
      },
      {
        key: 'edge_3',
        source: 'action_sell_leg',
        target: 'action_done',
        type: 'on_success',
        condition: null,
      },
      {
        key: 'edge_4',
        source: 'action_buy_leg',
        target: 'action_done',
        type: 'on_success',
        condition: null,
      },
    ],
  };
}

export function createRevengeFlip10_80Graph(marketSlug: string | null, outcome: TradeFlowTemplateOutcome | null): TradeFlowGraph {
  return {
    context: {
      sourceTradeId: 0,
      marketSlug: marketSlug || '',
      tokenId: outcome?.token_id || '',
      outcomeLabel: outcome?.label || '',
    },
    nodes: [
      {
        key: 'trigger_revenge_flip',
        type: 'trigger.market_price',
        positionX: 80,
        positionY: 180,
        config: {
          marketMode: marketSlug ? 'fixed' : 'auto_scope',
          marketScope: marketSlug ? '' : 'btc_5m_updown',
          marketSlug: marketSlug || '',
          bindingMode: 'revenge_flip_only',
          repeatMode: 'loop',
          priceMode: 'composite',
          pollIntervalMs: 1000,
          priceToBeatTriggerEnabled: false,
          outcomeConditions: [],
        },
      },
      {
        key: 'action_revenge_flip',
        type: 'action.place_order',
        positionX: 420,
        positionY: 180,
        config: {
          mode: 'revenge_flip_v1',
          side: 'buy',
          executionMode: 'market',
          kind: 'immediate',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          orderType: 'FAK',
          postOnly: false,
          buyFillLockEnabled: false,
          tpEnabled: false,
          slEnabled: false,
          autoSellOnWindowEnd: false,
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'manual',
          priceToBeatMinDiff: 10,
          priceToBeatMinDiffUnit: 'usd',
          priceToBeatCurrentPriceSource: 'chainlink',
          triggerPrice: {
            enabled: false,
            minCent: 0,
            maxCent: 100,
          },
          revengeFlip: {
            initialOrderUsdc: 5,
            profitTargetUsdc: 0.25,
            stopLossPct: 0.2,
            stopLossRules: [],
            reentrySideMode: 'rule_match',
            minReentryShares: 5,
            entryPtbRules: [
              {
                minFlip: 0,
                maxFlip: 0,
                sideMode: 'any',
                priceToBeatMinDiff: 10,
                priceToBeatMinDiffUnit: 'usd',
                maxPriceCent: 80,
              },
              {
                minFlip: 1,
                sideMode: 'any',
                priceToBeatMinDiff: 4,
                priceToBeatMinDiffUnit: 'cent',
              },
            ],
            maxFlip: 0,
            lotLimitPct: 0.98,
            closeOnlySec: 10,
            timeRules: [],
            ptbStopLossBumpEnabled: false,
            ptbStopLossBumpAmount: 0,
            ptbStopLossBumpUnit: 'cent',
          },
        },
      },
    ],
    edges: [
      {
        key: 'edge_revenge_flip',
        source: 'trigger_revenge_flip',
        target: 'action_revenge_flip',
        type: 'default',
        condition: null,
      },
    ],
  };
}

export function createPairLockHyperliquid70To80Graph(marketSlug: string | null, outcome: TradeFlowTemplateOutcome | null): TradeFlowGraph {
  return {
    context: {
      sourceTradeId: 0,
      marketSlug: marketSlug || '',
      tokenId: outcome?.token_id || '',
      outcomeLabel: outcome?.label || '',
    },
    nodes: [
      {
        key: 'trigger_pairlock',
        type: 'trigger.market_price',
        positionX: 80,
        positionY: 180,
        config: {
          marketMode: marketSlug ? 'fixed' : 'auto_scope',
          marketScope: marketSlug ? '' : 'btc_5m_updown',
          marketSlug: marketSlug || '',
          bindingMode: 'pair_lock_only',
          repeatMode: 'once',
          priceMode: 'composite',
          pollIntervalMs: 1000,
        },
      },
      {
        key: 'action_pairlock_buy',
        type: 'action.place_order',
        positionX: 420,
        positionY: 180,
        config: {
          mode: 'pair_lock',
          pairLockStrategy: 'legacy',
          side: 'buy',
          executionMode: 'market',
          kind: 'immediate',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 97,
          maxPriceCent: 80,
          executionFloorGuardEnabled: true,
          executionFloorPriceCent: 70,
          retryOnExecutionFloorGuardBlock: true,
          retryOnMaxPriceBlock: true,
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'manual',
          priceToBeatCurrentPriceSource: 'hyperliquid',
          priceToBeatMaxDiff: 20,
          priceToBeatMaxDiffUnit: 'usd',
          counterLegEnabled: true,
          counterLegSizeUsdc: 5,
          counterLegOutcomeLabel: 'opposite',
          counterLegMaxPriceCent: 80,
          counterLegExecutionFloorPriceCent: 70,
          counterLegPriceToBeatGuardEnabled: true,
          counterLegPriceToBeatMode: 'manual',
          counterLegPriceToBeatCurrentPriceSource: 'hyperliquid',
          counterLegPriceToBeatMaxDiff: 20,
          counterLegPriceToBeatMaxDiffUnit: 'usd',
          pairProtectiveUnwindEnabled: true,
          notifyOnPairLocked: true,
          notifyOnPairUnwind: true,
          notifyOnPriceToBeatGapBlocked: true,
          notifyOnMaxPriceBlocked: true,
        },
      },
    ],
    edges: [
      {
        key: 'edge_pairlock_buy',
        source: 'trigger_pairlock',
        target: 'action_pairlock_buy',
        type: 'default',
        condition: null,
      },
    ],
  };
}

export function createPositiveQuantityFlipGrid1UsdcGraph(marketSlug: string | null, outcome: TradeFlowTemplateOutcome | null): TradeFlowGraph {
  return {
    context: {
      sourceTradeId: 0,
      marketSlug: marketSlug || '',
      tokenId: outcome?.token_id || '',
      outcomeLabel: outcome?.label || '',
    },
    nodes: [
      {
        key: 'trigger_positive_grid',
        type: 'trigger.market_price',
        positionX: 80,
        positionY: 180,
        config: {
          marketMode: marketSlug ? 'fixed' : 'auto_scope',
          marketScope: marketSlug ? '' : 'btc_5m_updown',
          marketSlug: marketSlug || '',
          bindingMode: 'positive_quantity_flip_grid_only',
          repeatMode: 'loop',
          priceMode: 'composite',
          pollIntervalMs: 1000,
          priceToBeatTriggerEnabled: false,
          outcomeConditions: [],
        },
      },
      {
        key: 'action_positive_grid_buy',
        type: 'action.place_order',
        positionX: 420,
        positionY: 180,
        config: {
          mode: 'positive_quantity_flip_grid_v1',
          side: 'buy',
          executionMode: 'market',
          kind: 'immediate',
          sizeMode: 'usdc',
          sizeUsdc: 1,
          orderType: 'FAK',
          postOnly: false,
          buyFillLockEnabled: false,
          tpEnabled: false,
          slEnabled: false,
          autoSellOnWindowEnd: false,
          priceToBeatGuardEnabled: false,
          positiveQuantityFlipGrid: {
            baseBuyUsdc: 1.05,
            minMarketableBuyUsdc: 1.05,
            entryBandMinCent: 50,
            entryBandMaxCent: 60,
            preferredTriggerCent: 53,
            triggerToleranceCent: 3,
            exitPriceForSizingCent: 98,
            sizingPriceBufferCent: 3,
            quantitySizingMode: 'profit_target',
            inventoryBalanceLeadQty: 0,
            minPositiveProfitUsdc: 1,
            minSellNetProfitUsdc: 1,
            maxSingleBuyUsdc: 2.2,
            maxTotalSpentPerMarketUsdc: 9.5,
            maxActiveMarkets: 1,
            maxOpenGridBuysPerMarket: 8,
            hardMaxPriceCent: 60,
            worstPriceCent: 60,
            rescueBuyEnabled: false,
            rescueBuyMinPriceCent: 60,
            rescueBuyMaxPriceCent: 70,
            blockConsecutiveSameSideBuys: true,
            noBuyRanges: [],
            sellBidMinCent: 98,
            cycleWindowMode: 'custom_range',
            cycleWindowStartSec: 0,
            cycleWindowEndSec: 300,
            newGridBuyStartRemainingSec: 300,
            newGridBuyEndRemainingSec: 90,
            positiveCompletionBuyEndRemainingSec: 30,
            noNewBuyUnderSec: 30,
            orderType: 'FAK',
            executionFloorGuardEnabled: true,
            triggerPriceGuardEnabled: false,
            ptbGuardEnabled: false,
            ptbMinDiff: 2,
            ptbDiffUnit: 'usd',
            ptbCurrentPriceSource: 'hyperliquid',
            depthGuardEnabled: true,
          },
        },
      },
    ],
    edges: [
      {
        key: 'edge_positive_grid_buy',
        source: 'trigger_positive_grid',
        target: 'action_positive_grid_buy',
        type: 'default',
        condition: null,
      },
    ],
  };
}

export function createPositiveQuantityFlipGridInventoryBalanceGraph(marketSlug: string | null, outcome: TradeFlowTemplateOutcome | null): TradeFlowGraph {
  const graph = createPositiveQuantityFlipGrid1UsdcGraph(marketSlug, outcome);
  const action = graph.nodes.find((node) => node.key === 'action_positive_grid_buy');
  const grid = action?.config && typeof action.config === 'object' ? (action.config.positiveQuantityFlipGrid as Record<string, unknown> | undefined) : undefined;
  if (grid) {
    grid.quantitySizingMode = 'inventory_balance';
    grid.inventoryBalanceLeadQty = 0;
    grid.minPositiveProfitUsdc = 0.02;
    grid.minSellNetProfitUsdc = 0.02;
    grid.maxSingleBuyUsdc = 5;
    grid.maxTotalSpentPerMarketUsdc = 12;
    grid.maxOpenGridBuysPerMarket = 5;
    grid.rescueBuyEnabled = false;
  }
  return graph;
}

export function createPositiveFlipPairlockCompressionGraph(marketSlug: string | null, outcome: TradeFlowTemplateOutcome | null): TradeFlowGraph {
  const graph = createPositiveQuantityFlipGrid1UsdcGraph(marketSlug, outcome);
  const action = graph.nodes.find((node) => node.key === 'action_positive_grid_buy');
  const config = action?.config && typeof action.config === 'object' ? (action.config as Record<string, unknown>) : undefined;
  const grid = config?.positiveQuantityFlipGrid as Record<string, unknown> | undefined;
  if (config) {
    config.mode = 'positive_flip_pairlock_compression_v1';
    config.sizeUsdc = 2;
    config.orderType = 'FAK';
  }
  if (grid) {
    grid.baseBuyUsdc = 2;
    grid.minMarketableBuyUsdc = 1.05;
    grid.entryBandMinCent = 52;
    grid.entryBandMaxCent = 58;
    grid.preferredTriggerCent = 55;
    grid.triggerToleranceCent = 3;
    grid.exitPriceForSizingCent = 98;
    grid.sizingPriceBufferCent = 1;
    grid.quantitySizingMode = 'fixed_usdc';
    grid.minPositiveProfitUsdc = 0.05;
    grid.minSellNetProfitUsdc = 0.05;
    delete grid.maxSingleBuyUsdc;
    delete grid.maxTotalSpentPerMarketUsdc;
    grid.maxActiveMarkets = 1;
    grid.maxOpenGridBuysPerMarket = 10;
    grid.hardMaxPriceCent = 58;
    grid.worstPriceCent = 58;
    grid.rescueBuyEnabled = true;
    grid.rescueBuyMinPriceCent = 58;
    grid.rescueBuyMaxPriceCent = 75;
    grid.blockConsecutiveSameSideBuys = true;
    grid.noBuyRanges = [];
    grid.sellBidMinCent = 98;
    grid.cycleWindowMode = 'last';
    grid.cycleWindowSecs = 120;
    delete grid.cycleWindowStartSec;
    delete grid.cycleWindowEndSec;
    grid.positiveCompletionBuyEndRemainingSec = 0;
    grid.noNewBuyUnderSec = 0;
    grid.pairlockCompressionEnabled = true;
    grid.stopBuysAfterPairlockMerge = true;
    grid.targetPairlockProfitCent = 5;
    grid.feeBufferCent = 1;
    grid.maxPairCostCent = 94;
    grid.pairlockOrderType = 'FOK';
    grid.maxPositiveFlipBuysPerMarket = 2;
    grid.maxUnmergedExposureUsdc = 2;
    grid.basketExitEnabled = false;
    grid.directExitEnabled = false;
    grid.minBasketProfitUsdc = 0.1;
    grid.minDirectProfitUsdc = 0.05;
    grid.maxSpreadCent = 4;
    grid.requireFreshBookSeconds = 2;
    grid.requireDepthMultiplier = 1;
    grid.noSameSideAveraging = true;
    grid.noBlindHedge = true;
  }
  return graph;
}
