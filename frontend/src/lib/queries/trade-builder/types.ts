export interface ActiveUpdownMarketsCacheEntry {
  expiresAt: number
  markets: Array<Record<string, unknown>>
}

export interface TradeBuilderFilters {
  userId: number
  page?: number
  limit?: number
  status?: string
}

export interface TradeBuilderOrderEventFilters {
  userId: number
  orderId: number
  page?: number
  limit?: number
  eventType?: string
}

export interface TradeBuilderWorkflowFilters {
  userId: number
  page?: number
  limit?: number
  status?: string
}

export interface TradeBuilderWorkflowEventFilters {
  userId: number
  workflowId: number
  page?: number
  limit?: number
  eventType?: string
}

export interface CreateTradeBuilderOrderInput {
  userId: number
  kind: 'immediate' | 'conditional'
  marketSlug: string
  tokenId: string
  outcomeLabel: string
  side: 'buy' | 'sell'
  executionMode?: 'limit' | 'market'
  sizeUsdc: number
  minPriceDistanceCent: number
  triggerCondition?: 'cross_above' | 'cross_below'
  triggerPriceCent?: number
  expiresAt?: string
  maxTriggers?: number
}

export interface CreateTradeBuilderWorkflowInput {
  userId: number
  name?: string
  sourceTradeId: number
  sellTargetPct: number
  buyStartAfterSellProgressPct: number
  buyTriggerMode: 'sell_progress_only' | 'price_only' | 'sell_progress_and_price'
  buyAllocationPct: number
  expiresAt?: string | null
  sellLeg: {
    marketSlug: string
    tokenId: string
    outcomeLabel: string
    side: 'buy' | 'sell'
    triggerCondition?: 'cross_above' | 'cross_below'
    triggerPriceCent?: number
    minPriceDistanceCent: number
  }
  buyLeg: {
    marketSlug: string
    tokenId: string
    outcomeLabel: string
    side: 'buy' | 'sell'
    triggerCondition?: 'cross_above' | 'cross_below'
    triggerPriceCent?: number
    minPriceDistanceCent: number
  }
}
