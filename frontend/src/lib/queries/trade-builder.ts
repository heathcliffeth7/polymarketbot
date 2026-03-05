import { pool } from '@/lib/db';
import type {
  PaginatedResponse,
  TradeBuilderMarketSearchItem,
  TradeBuilderOrder,
  TradeBuilderOrderEvent,
  TradeBuilderOutcome,
  TradeBuilderWorkflow,
  TradeBuilderWorkflowDetail,
  TradeBuilderWorkflowEvent,
} from '@/lib/types';

const GAMMA_BASE_URL = process.env.GAMMA_BASE_URL || 'https://gamma-api.polymarket.com';
const SCOPE_TO_UPDOWN_SLUG_PREFIX: Record<string, string> = {
  btc_5m_updown: 'btc-updown-5m-',
  btc_15m_updown: 'btc-updown-15m-',
  eth_5m_updown: 'eth-updown-5m-',
  eth_15m_updown: 'eth-updown-15m-',
  sol_5m_updown: 'sol-updown-5m-',
  sol_15m_updown: 'sol-updown-15m-',
  xrp_5m_updown: 'xrp-updown-5m-',
  xrp_15m_updown: 'xrp-updown-15m-',
};
const ACTIVE_UPDOWN_MARKETS_CACHE_TTL_MS = 30_000;

interface ActiveUpdownMarketsCacheEntry {
  expiresAt: number;
  markets: Array<Record<string, unknown>>;
}

let activeUpdownMarketsCache: ActiveUpdownMarketsCacheEntry | null = null;

interface TradeBuilderFilters {
  page?: number;
  limit?: number;
  status?: string;
}

interface TradeBuilderOrderEventFilters {
  orderId: number;
  page?: number;
  limit?: number;
  eventType?: string;
}

interface TradeBuilderWorkflowFilters {
  page?: number;
  limit?: number;
  status?: string;
}

interface TradeBuilderWorkflowEventFilters {
  workflowId: number;
  page?: number;
  limit?: number;
  eventType?: string;
}

interface CreateTradeBuilderOrderInput {
  kind: 'immediate' | 'conditional';
  marketSlug: string;
  tokenId: string;
  outcomeLabel: string;
  side: 'buy' | 'sell';
  executionMode?: 'limit' | 'market';
  sizeUsdc: number;
  minPriceDistanceCent: number;
  triggerCondition?: 'cross_above' | 'cross_below';
  triggerPriceCent?: number;
  expiresAt?: string;
  maxTriggers?: number;
}

interface CreateTradeBuilderWorkflowInput {
  name?: string;
  sourceTradeId: number;
  sellTargetPct: number;
  buyStartAfterSellProgressPct: number;
  buyTriggerMode: 'sell_progress_only' | 'price_only' | 'sell_progress_and_price';
  buyAllocationPct: number;
  expiresAt?: string | null;
  sellLeg: {
    marketSlug: string;
    tokenId: string;
    outcomeLabel: string;
    side: 'buy' | 'sell';
    triggerCondition?: 'cross_above' | 'cross_below';
    triggerPriceCent?: number;
    minPriceDistanceCent: number;
  };
  buyLeg: {
    marketSlug: string;
    tokenId: string;
    outcomeLabel: string;
    side: 'buy' | 'sell';
    triggerCondition?: 'cross_above' | 'cross_below';
    triggerPriceCent?: number;
    minPriceDistanceCent: number;
  };
}

export async function getTradeBuilderOrders(
  filters: TradeBuilderFilters
): Promise<PaginatedResponse<TradeBuilderOrder>> {
  const page = filters.page || 1;
  const limit = Math.min(filters.limit || 20, 100);
  const offset = (page - 1) * limit;

  const whereParts: string[] = [];
  const params: unknown[] = [];
  let idx = 1;

  if (filters.status) {
    whereParts.push(`status = $${idx++}`);
    params.push(filters.status);
  }

  const where = whereParts.length ? `WHERE ${whereParts.join(' AND ')}` : '';

  const [countRes, dataRes] = await Promise.all([
    pool.query(`SELECT COUNT(*)::int AS total FROM trade_builder_orders ${where}`, params),
    pool.query(
      `SELECT * FROM trade_builder_orders ${where} ORDER BY created_at DESC LIMIT $${idx++} OFFSET $${idx++}`,
      [...params, limit, offset]
    ),
  ]);

  const total = Number(countRes.rows[0]?.total || 0);
  return {
    data: dataRes.rows,
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
  };
}

export async function getTradeBuilderOrderEvents(
  filters: TradeBuilderOrderEventFilters
): Promise<PaginatedResponse<TradeBuilderOrderEvent>> {
  const page = filters.page || 1;
  const limit = Math.min(filters.limit || 25, 100);
  const offset = (page - 1) * limit;

  const whereParts: string[] = ['builder_order_id = $1'];
  const params: unknown[] = [filters.orderId];
  let idx = 2;

  if (filters.eventType) {
    whereParts.push(`event_type = $${idx++}`);
    params.push(filters.eventType);
  }

  const where = `WHERE ${whereParts.join(' AND ')}`;

  const [countRes, dataRes] = await Promise.all([
    pool.query(`SELECT COUNT(*)::int AS total FROM trade_builder_order_events ${where}`, params),
    pool.query(
      `SELECT id, builder_order_id, event_type, payload_json, created_at
       FROM trade_builder_order_events
       ${where}
       ORDER BY created_at DESC
       LIMIT $${idx++} OFFSET $${idx++}`,
      [...params, limit, offset]
    ),
  ]);

  const total = Number(countRes.rows[0]?.total || 0);
  return {
    data: dataRes.rows,
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
  };
}

export async function createTradeBuilderOrder(
  input: CreateTradeBuilderOrderInput
): Promise<TradeBuilderOrder> {
  const executionMode = input.executionMode === 'market' ? 'market' : 'limit';
  const now = new Date();
  const startsAt = now;
  const endsAt = input.expiresAt ? new Date(input.expiresAt) : new Date(now.getTime() + 7 * 24 * 3600 * 1000);
  const triggerPrice =
    input.kind === 'conditional' && Number.isFinite(input.triggerPriceCent)
      ? Number(input.triggerPriceCent) / 100
      : null;

  const client = await pool.connect();
  try {
    await client.query('BEGIN');

    const marketRes = await client.query(
      `INSERT INTO markets (market_slug, starts_at, ends_at, status)
       VALUES ($1, $2, $3, 'open')
       ON CONFLICT (market_slug) DO UPDATE SET
         starts_at = LEAST(markets.starts_at, EXCLUDED.starts_at),
         ends_at = GREATEST(markets.ends_at, EXCLUDED.ends_at),
         status = CASE WHEN markets.status = 'settled' THEN markets.status ELSE 'open' END
       RETURNING id`,
      [input.marketSlug, startsAt, endsAt]
    );

    const marketId = marketRes.rows[0].id;
    const referencePrice = triggerPrice ? Number(triggerPrice) : 0.5;

    const tradeRes = await client.query(
      `INSERT INTO trades (market_id, state, entry_price, notional_usdc, strategy_mode, opened_at)
       VALUES ($1, 'Idle', $2, $3, 'manual_trade_builder', NOW())
       RETURNING id`,
      [marketId, referencePrice, input.sizeUsdc]
    );

    const tradeId = tradeRes.rows[0].id;

    const orderRes = await client.query(
      `INSERT INTO trade_builder_orders
         (trade_id, kind, status, market_slug, token_id, outcome_label, side, execution_mode, trigger_condition, trigger_price,
          size_usdc, min_price_distance_cent, expires_at, max_triggers, triggers_fired, created_at, updated_at)
       VALUES
         ($1, $2, 'pending', $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 0, NOW(), NOW())
       RETURNING *`,
      [
        tradeId,
        input.kind,
        input.marketSlug,
        input.tokenId,
        input.outcomeLabel,
        input.side,
        executionMode,
        input.kind === 'conditional' ? input.triggerCondition || 'cross_above' : null,
        triggerPrice,
        input.sizeUsdc,
        input.minPriceDistanceCent,
        input.kind === 'conditional' ? input.expiresAt || null : null,
        input.maxTriggers || 3,
      ]
    );

    await client.query(
      `INSERT INTO trade_builder_order_events (builder_order_id, event_type, payload_json, created_at)
       VALUES ($1, 'created', $2, NOW())`,
      [
        orderRes.rows[0].id,
        JSON.stringify({
          kind: input.kind,
          marketSlug: input.marketSlug,
          tokenId: input.tokenId,
          side: input.side,
          executionMode,
          triggerCondition: input.triggerCondition || null,
          triggerPriceCent: input.triggerPriceCent || null,
          sizeUsdc: input.sizeUsdc,
          minPriceDistanceCent: input.minPriceDistanceCent,
        }),
      ]
    );

    await client.query('COMMIT');
    return orderRes.rows[0];
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function updateTradeBuilderOrder(
  id: number,
  updates: { minPriceDistanceCent?: number; maxTriggers?: number; expiresAt?: string | null }
): Promise<void> {
  const fields: string[] = [];
  const params: unknown[] = [id];
  let idx = 2;

  if (updates.minPriceDistanceCent !== undefined) {
    fields.push(`min_price_distance_cent = $${idx++}`);
    params.push(updates.minPriceDistanceCent);
  }
  if (updates.maxTriggers !== undefined) {
    fields.push(`max_triggers = $${idx++}`);
    params.push(updates.maxTriggers);
  }
  if (updates.expiresAt !== undefined) {
    fields.push(`expires_at = $${idx++}`);
    params.push(updates.expiresAt ? new Date(updates.expiresAt) : null);
  }

  if (fields.length === 0) return;

  fields.push('updated_at = NOW()');
  await pool.query(`UPDATE trade_builder_orders SET ${fields.join(', ')} WHERE id = $1`, params);
}

export async function requestCancelTradeBuilderOrder(id: number): Promise<void> {
  await pool.query(
    `UPDATE trade_builder_orders
     SET status = CASE WHEN active_exchange_order_id IS NULL THEN 'canceled' ELSE 'canceled_requested' END,
         updated_at = NOW()
     WHERE id = $1`,
    [id]
  );
}

export async function hardDeleteAllTradeBuilderOrders(): Promise<number> {
  const client = await pool.connect();
  try {
    await client.query('BEGIN');
    await client.query('DELETE FROM trade_builder_order_events');
    const res = await client.query('DELETE FROM trade_builder_orders');
    await client.query('COMMIT');
    return res.rowCount ?? 0;
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function hardDeleteTradeBuilderOrder(id: number): Promise<void> {
  const client = await pool.connect();
  try {
    await client.query('BEGIN');
    await client.query('DELETE FROM trade_builder_order_events WHERE builder_order_id = $1', [id]);
    await client.query('DELETE FROM trade_builder_orders WHERE id = $1', [id]);
    await client.query('COMMIT');
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function getTradeBuilderWorkflows(
  filters: TradeBuilderWorkflowFilters
): Promise<PaginatedResponse<TradeBuilderWorkflowDetail>> {
  const page = filters.page || 1;
  const limit = Math.min(filters.limit || 20, 100);
  const offset = (page - 1) * limit;

  const whereParts: string[] = [];
  const params: unknown[] = [];
  let idx = 1;

  if (filters.status) {
    whereParts.push(`status = $${idx++}`);
    params.push(filters.status);
  }

  const where = whereParts.length ? `WHERE ${whereParts.join(' AND ')}` : '';
  const [countRes, dataRes] = await Promise.all([
    pool.query(`SELECT COUNT(*)::int AS total FROM trade_builder_workflows ${where}`, params),
    pool.query(
      `SELECT * FROM trade_builder_workflows ${where} ORDER BY created_at DESC LIMIT $${idx++} OFFSET $${idx++}`,
      [...params, limit, offset]
    ),
  ]);

  const workflows = dataRes.rows as TradeBuilderWorkflow[];
  const workflowIds = workflows.map((x) => x.id);
  const legsByWorkflowId = new Map<number, unknown[]>();
  if (workflowIds.length > 0) {
    const legsRes = await pool.query(
      `SELECT * FROM trade_builder_workflow_legs
       WHERE workflow_id = ANY($1::bigint[])
       ORDER BY workflow_id, leg_type, id`,
      [workflowIds]
    );
    for (const row of legsRes.rows) {
      const workflowId = Number(row.workflow_id);
      if (!legsByWorkflowId.has(workflowId)) legsByWorkflowId.set(workflowId, []);
      legsByWorkflowId.get(workflowId)?.push(row);
    }
  }

  const total = Number(countRes.rows[0]?.total || 0);
  return {
    data: workflows.map((workflow) => ({
      workflow,
      legs: (legsByWorkflowId.get(workflow.id) || []) as TradeBuilderWorkflowDetail['legs'],
    })),
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
  };
}

export async function getTradeBuilderWorkflowById(
  workflowId: number
): Promise<TradeBuilderWorkflowDetail | null> {
  const workflowRes = await pool.query(
    `SELECT * FROM trade_builder_workflows WHERE id = $1 LIMIT 1`,
    [workflowId]
  );
  if (workflowRes.rowCount === 0) return null;

  const legsRes = await pool.query(
    `SELECT * FROM trade_builder_workflow_legs
     WHERE workflow_id = $1
     ORDER BY leg_type, id`,
    [workflowId]
  );

  return {
    workflow: workflowRes.rows[0] as TradeBuilderWorkflow,
    legs: legsRes.rows as TradeBuilderWorkflowDetail['legs'],
  };
}

export async function createTradeBuilderWorkflow(
  input: CreateTradeBuilderWorkflowInput
): Promise<TradeBuilderWorkflowDetail> {
  const client = await pool.connect();
  try {
    await client.query('BEGIN');

    const sellTriggerPrice =
      Number.isFinite(input.sellLeg.triggerPriceCent as number)
        ? Number(input.sellLeg.triggerPriceCent) / 100
        : null;
    const buyTriggerPrice =
      Number.isFinite(input.buyLeg.triggerPriceCent as number)
        ? Number(input.buyLeg.triggerPriceCent) / 100
        : null;

    const tokenNotionalRes = await client.query(
      `SELECT COALESCE(SUM(qty * COALESCE(last_fill_price, avg_entry)), 0)::double precision AS notional
       FROM leg_positions
       WHERE trade_id = $1 AND token_id = $2`,
      [input.sourceTradeId, input.sellLeg.tokenId]
    );
    let sourceNotional = Number(tokenNotionalRes.rows[0]?.notional || 0);
    if (sourceNotional <= 0) {
      const fallbackNotionalRes = await client.query(
        `SELECT COALESCE(SUM(qty * COALESCE(last_fill_price, avg_entry)), 0)::double precision AS notional
         FROM leg_positions
         WHERE trade_id = $1`,
        [input.sourceTradeId]
      );
      sourceNotional = Number(fallbackNotionalRes.rows[0]?.notional || 0);
    }
    if (sourceNotional <= 0) {
      throw new Error('Source trade position notional is zero');
    }

    const sellTargetNotional = sourceNotional * (input.sellTargetPct / 100);
    const buyTargetNotional = sellTargetNotional * (input.buyAllocationPct / 100);
    if (sellTargetNotional <= 0 || buyTargetNotional <= 0) {
      throw new Error('Computed workflow notionals must be > 0');
    }

    const workflowRes = await client.query(
      `INSERT INTO trade_builder_workflows
         (name, status, source_trade_id, sell_target_pct, buy_start_after_sell_progress_pct, buy_trigger_mode, buy_allocation_pct, expires_at, created_at, updated_at)
       VALUES
         ($1, 'armed', $2, $3, $4, $5, $6, $7, NOW(), NOW())
       RETURNING *`,
      [
        input.name?.trim() || 'workflow',
        input.sourceTradeId,
        input.sellTargetPct,
        input.buyStartAfterSellProgressPct,
        input.buyTriggerMode,
        input.buyAllocationPct,
        input.expiresAt ? new Date(input.expiresAt) : null,
      ]
    );
    const workflow = workflowRes.rows[0] as TradeBuilderWorkflow;

    const sellLegRes = await client.query(
      `INSERT INTO trade_builder_workflow_legs
         (workflow_id, leg_type, market_slug, token_id, outcome_label, side, trigger_condition, trigger_price, min_price_distance_cent, status, target_notional_usdc, allocated_notional_usdc, created_at, updated_at)
       VALUES
         ($1, 'sell', $2, $3, $4, $5, $6, $7, $8, 'pending', $9, 0, NOW(), NOW())
       RETURNING *`,
      [
        workflow.id,
        input.sellLeg.marketSlug,
        input.sellLeg.tokenId,
        input.sellLeg.outcomeLabel,
        input.sellLeg.side,
        input.sellLeg.triggerCondition || null,
        sellTriggerPrice,
        input.sellLeg.minPriceDistanceCent,
        sellTargetNotional,
      ]
    );

    const buyLegRes = await client.query(
      `INSERT INTO trade_builder_workflow_legs
         (workflow_id, leg_type, market_slug, token_id, outcome_label, side, trigger_condition, trigger_price, min_price_distance_cent, status, target_notional_usdc, allocated_notional_usdc, created_at, updated_at)
       VALUES
         ($1, 'buy', $2, $3, $4, $5, $6, $7, $8, 'waiting_sell_progress', $9, 0, NOW(), NOW())
       RETURNING *`,
      [
        workflow.id,
        input.buyLeg.marketSlug,
        input.buyLeg.tokenId,
        input.buyLeg.outcomeLabel,
        input.buyLeg.side,
        input.buyLeg.triggerCondition || null,
        buyTriggerPrice,
        input.buyLeg.minPriceDistanceCent,
        buyTargetNotional,
      ]
    );

    await client.query(
      `INSERT INTO trade_builder_workflow_events (workflow_id, leg_id, event_type, payload_json, created_at)
       VALUES
         ($1, NULL, 'created', $2, NOW()),
         ($1, $3, 'leg_created', $4, NOW()),
         ($1, $5, 'leg_created', $6, NOW())`,
      [
        workflow.id,
        JSON.stringify({
          name: workflow.name,
          sellTargetPct: workflow.sell_target_pct,
          buyStartAfterSellProgressPct: workflow.buy_start_after_sell_progress_pct,
          buyTriggerMode: workflow.buy_trigger_mode,
          buyAllocationPct: workflow.buy_allocation_pct,
          sourceNotional,
          sellTargetNotional,
          buyTargetNotional,
        }),
        sellLegRes.rows[0].id,
        JSON.stringify({
          legType: 'sell',
          marketSlug: input.sellLeg.marketSlug,
          tokenId: input.sellLeg.tokenId,
          side: input.sellLeg.side,
          triggerCondition: input.sellLeg.triggerCondition || null,
          triggerPriceCent: input.sellLeg.triggerPriceCent || null,
          targetNotionalUsdc: sellTargetNotional,
        }),
        buyLegRes.rows[0].id,
        JSON.stringify({
          legType: 'buy',
          marketSlug: input.buyLeg.marketSlug,
          tokenId: input.buyLeg.tokenId,
          side: input.buyLeg.side,
          triggerCondition: input.buyLeg.triggerCondition || null,
          triggerPriceCent: input.buyLeg.triggerPriceCent || null,
          targetNotionalUsdc: buyTargetNotional,
        }),
      ]
    );

    await client.query('COMMIT');
    return {
      workflow,
      legs: [sellLegRes.rows[0], buyLegRes.rows[0]] as TradeBuilderWorkflowDetail['legs'],
    };
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function updateTradeBuilderWorkflow(
  workflowId: number,
  updates: {
    buyStartAfterSellProgressPct?: number;
    buyTriggerMode?: 'sell_progress_only' | 'price_only' | 'sell_progress_and_price';
    buyAllocationPct?: number;
    expiresAt?: string | null;
  }
): Promise<void> {
  const fields: string[] = [];
  const params: unknown[] = [workflowId];
  let idx = 2;

  if (updates.buyStartAfterSellProgressPct !== undefined) {
    fields.push(`buy_start_after_sell_progress_pct = $${idx++}`);
    params.push(updates.buyStartAfterSellProgressPct);
  }
  if (updates.buyTriggerMode !== undefined) {
    fields.push(`buy_trigger_mode = $${idx++}`);
    params.push(updates.buyTriggerMode);
  }
  if (updates.buyAllocationPct !== undefined) {
    fields.push(`buy_allocation_pct = $${idx++}`);
    params.push(updates.buyAllocationPct);
  }
  if (updates.expiresAt !== undefined) {
    fields.push(`expires_at = $${idx++}`);
    params.push(updates.expiresAt ? new Date(updates.expiresAt) : null);
  }

  if (fields.length === 0) return;
  fields.push('updated_at = NOW()');

  await pool.query(
    `UPDATE trade_builder_workflows SET ${fields.join(', ')} WHERE id = $1`,
    params
  );
}

export async function requestCancelTradeBuilderWorkflow(workflowId: number): Promise<void> {
  const client = await pool.connect();
  try {
    await client.query('BEGIN');

    const legRes = await client.query(
      `SELECT id, builder_order_id FROM trade_builder_workflow_legs WHERE workflow_id = $1`,
      [workflowId]
    );

    for (const leg of legRes.rows) {
      if (leg.builder_order_id) {
        await client.query(
          `UPDATE trade_builder_orders
           SET status = CASE WHEN active_exchange_order_id IS NULL THEN 'canceled' ELSE 'canceled_requested' END,
               updated_at = NOW()
           WHERE id = $1`,
          [leg.builder_order_id]
        );
      }
    }

    await client.query(
      `UPDATE trade_builder_workflow_legs
       SET status = 'canceled', updated_at = NOW()
       WHERE workflow_id = $1`,
      [workflowId]
    );
    await client.query(
      `UPDATE trade_builder_workflows
       SET status = 'canceled', updated_at = NOW()
       WHERE id = $1`,
      [workflowId]
    );
    await client.query(
      `INSERT INTO trade_builder_workflow_events (workflow_id, leg_id, event_type, payload_json, created_at)
       VALUES ($1, NULL, 'canceled_by_user', $2, NOW())`,
      [workflowId, JSON.stringify({ reason: 'user_request' })]
    );

    await client.query('COMMIT');
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function hardDeleteTradeBuilderWorkflow(workflowId: number): Promise<void> {
  const client = await pool.connect();
  try {
    await client.query('BEGIN');
    const legRes = await client.query(
      'SELECT builder_order_id FROM trade_builder_workflow_legs WHERE workflow_id = $1',
      [workflowId]
    );
    for (const leg of legRes.rows) {
      if (leg.builder_order_id) {
        await client.query('DELETE FROM trade_builder_order_events WHERE builder_order_id = $1', [leg.builder_order_id]);
        await client.query('DELETE FROM trade_builder_orders WHERE id = $1', [leg.builder_order_id]);
      }
    }
    await client.query('DELETE FROM trade_builder_workflow_events WHERE workflow_id = $1', [workflowId]);
    await client.query('DELETE FROM trade_builder_workflow_legs WHERE workflow_id = $1', [workflowId]);
    await client.query('DELETE FROM trade_builder_workflows WHERE id = $1', [workflowId]);
    await client.query('COMMIT');
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function getTradeBuilderWorkflowEvents(
  filters: TradeBuilderWorkflowEventFilters
): Promise<PaginatedResponse<TradeBuilderWorkflowEvent>> {
  const page = filters.page || 1;
  const limit = Math.min(filters.limit || 25, 100);
  const offset = (page - 1) * limit;

  const whereParts: string[] = ['workflow_id = $1'];
  const params: unknown[] = [filters.workflowId];
  let idx = 2;

  if (filters.eventType) {
    whereParts.push(`event_type = $${idx++}`);
    params.push(filters.eventType);
  }

  const where = `WHERE ${whereParts.join(' AND ')}`;
  const [countRes, dataRes] = await Promise.all([
    pool.query(`SELECT COUNT(*)::int AS total FROM trade_builder_workflow_events ${where}`, params),
    pool.query(
      `SELECT id, workflow_id, leg_id, event_type, payload_json, created_at
       FROM trade_builder_workflow_events
       ${where}
       ORDER BY created_at DESC
       LIMIT $${idx++} OFFSET $${idx++}`,
      [...params, limit, offset]
    ),
  ]);

  const total = Number(countRes.rows[0]?.total || 0);
  return {
    data: dataRes.rows,
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
  };
}

export async function searchGammaMarkets(query: string): Promise<TradeBuilderMarketSearchItem[]> {
  const url = `${GAMMA_BASE_URL.replace(/\/$/, '')}/markets?active=true&closed=false&limit=200`;
  const res = await fetch(url, { cache: 'no-store' });
  if (!res.ok) return [];

  const rows = (await res.json()) as Array<Record<string, unknown>>;
  const needle = query.trim().toLowerCase();

  return rows
    .map((row) => {
      const slug = String(row.slug || '');
      const question = String(row.question || row.title || slug);
      const endDate = row.endDate ? String(row.endDate) : null;
      const active = row.active !== false;
      return {
        slug,
        title: question,
        endDate,
        active,
      };
    })
    .filter((item) => item.slug.length > 0)
    .filter((item) => {
      if (!needle) return true;
      return (
        item.slug.toLowerCase().includes(needle) ||
        item.title.toLowerCase().includes(needle)
      );
    })
    .slice(0, 40);
}

export async function getMarketOutcomesBySlug(slug: string): Promise<TradeBuilderOutcome[]> {
  const trimmed = slug.trim();
  if (!trimmed) return [];
  const normalized = trimmed.toLowerCase();

  // 0. If input is a market scope (auto_scope), resolve to latest live market first.
  const scopeMarket = await resolveMarketFromScope(normalized);
  if (scopeMarket) {
    const eventOutcomes = await tryExtractEventOutcomes(scopeMarket);
    if (eventOutcomes.length > 1) return eventOutcomes;
    return extractOutcomes(scopeMarket);
  }

  // 1. Try as market slug first
  const market = await fetchMarketBySlug(trimmed);
  if (market) {
    // Check if this market belongs to a multi-outcome event
    const eventOutcomes = await tryExtractEventOutcomes(market);
    if (eventOutcomes.length > 1) return eventOutcomes;
    // Single-market event or no event: return market-level outcomes
    return extractOutcomes(market);
  }

  // 2. Try as event slug
  const eventData = await fetchEventData(trimmed);
  if (eventData) {
    const markets = Array.isArray(eventData.markets)
      ? (eventData.markets as Array<Record<string, unknown>>)
      : [];
    if (markets.length > 1) return extractEventMarketOutcomes(markets);
    if (markets.length === 1) return extractOutcomes(markets[0] as Record<string, unknown>);
  }

  return [];
}

async function fetchMarketBySlug(slug: string): Promise<Record<string, unknown> | null> {
  const url = `${GAMMA_BASE_URL.replace(/\/$/, '')}/markets/slug/${encodeURIComponent(slug)}`;
  const res = await fetch(url, { cache: 'no-store' });
  if (!res.ok) return null;
  const data = (await res.json()) as unknown;
  if (Array.isArray(data)) return (data[0] as Record<string, unknown>) || null;
  if (data && typeof data === 'object') return data as Record<string, unknown>;
  return null;
}

async function fetchEventData(slug: string): Promise<Record<string, unknown> | null> {
  const url = `${GAMMA_BASE_URL.replace(/\/$/, '')}/events/slug/${encodeURIComponent(slug)}`;
  const res = await fetch(url, { cache: 'no-store' });
  if (!res.ok) return null;
  const data = (await res.json()) as unknown;
  if (data && typeof data === 'object' && !Array.isArray(data)) return data as Record<string, unknown>;
  return null;
}

function isTruthyValue(value: unknown): boolean {
  if (typeof value === 'boolean') return value;
  if (typeof value === 'number') return value !== 0;
  if (typeof value === 'string') {
    const normalized = value.trim().toLowerCase();
    return normalized === '1' || normalized === 'true' || normalized === 'yes' || normalized === 'on';
  }
  return false;
}

function isMarketActive(row: Record<string, unknown>): boolean {
  const activeRaw = row.active;
  const closedRaw = row.closed;
  const active = activeRaw == null ? true : isTruthyValue(activeRaw);
  const closed = closedRaw == null ? false : isTruthyValue(closedRaw);
  return active && !closed;
}

function parseDateMs(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) {
    if (value > 10_000_000_000) return Math.floor(value);
    if (value > 100_000_000) return Math.floor(value * 1000);
    return null;
  }
  if (typeof value !== 'string') return null;
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsedNumber = Number(trimmed);
  if (Number.isFinite(parsedNumber)) {
    if (parsedNumber > 10_000_000_000) return Math.floor(parsedNumber);
    if (parsedNumber > 100_000_000) return Math.floor(parsedNumber * 1000);
  }
  const parsedDate = Date.parse(trimmed);
  return Number.isFinite(parsedDate) ? parsedDate : null;
}

function scopeWindowSeconds(scope: string): number {
  return scope.includes('_15m_') ? 900 : 300;
}

function inferWindowFromScopeMarket(
  market: Record<string, unknown>,
  slugPrefix: string,
  windowSec: number
): { startsAtMs: number | null; endsAtMs: number | null } {
  const slug = String(market.slug || '').trim().toLowerCase();
  let startsAtMs: number | null = null;
  if (slug.startsWith(slugPrefix)) {
    const suffix = slug.slice(slugPrefix.length);
    const match = suffix.match(/^(\d{9,13})$/);
    if (match) {
      const rawTs = Number(match[1]);
      if (Number.isFinite(rawTs) && rawTs > 0) {
        startsAtMs = rawTs > 10_000_000_000 ? Math.floor(rawTs) : Math.floor(rawTs * 1000);
      }
    }
  }

  const endsAtMs =
    parseDateMs(market.endDate) ??
    parseDateMs(market.end_date) ??
    parseDateMs(market.endDateIso) ??
    parseDateMs(market.end_date_iso);

  if (startsAtMs != null && endsAtMs == null) {
    return { startsAtMs, endsAtMs: startsAtMs + windowSec * 1000 };
  }
  if (startsAtMs == null && endsAtMs != null) {
    return { startsAtMs: endsAtMs - windowSec * 1000, endsAtMs };
  }
  return { startsAtMs, endsAtMs };
}

function selectPreferredScopeMarket(
  markets: Array<Record<string, unknown>>,
  slugPrefix: string,
  windowSec: number
): Record<string, unknown> | null {
  if (markets.length === 0) return null;
  const nowMs = Date.now();
  const withWindow = markets.map((market) => {
    const slug = String(market.slug || '').trim().toLowerCase();
    const window = inferWindowFromScopeMarket(market, slugPrefix, windowSec);
    return { market, slug, ...window };
  });

  const inWindow = withWindow
    .filter((row) => row.startsAtMs != null && row.endsAtMs != null && row.startsAtMs <= nowMs && nowMs < row.endsAtMs)
    .sort((a, b) => {
      const startA = a.startsAtMs ?? 0;
      const startB = b.startsAtMs ?? 0;
      if (startA !== startB) return startB - startA;
      return b.slug.localeCompare(a.slug);
    });
  if (inWindow.length > 0) return inWindow[0].market;

  const nearestFuture = withWindow
    .filter((row) => row.startsAtMs != null && row.startsAtMs >= nowMs)
    .sort((a, b) => {
      const startA = a.startsAtMs ?? Number.MAX_SAFE_INTEGER;
      const startB = b.startsAtMs ?? Number.MAX_SAFE_INTEGER;
      if (startA !== startB) return startA - startB;
      return a.slug.localeCompare(b.slug);
    });
  if (nearestFuture.length > 0) return nearestFuture[0].market;

  return withWindow
    .sort((a, b) => b.slug.localeCompare(a.slug))
    .at(0)?.market || null;
}

function buildScopeCandidateSlugs(slugPrefix: string, windowSec: number, nowMs: number): string[] {
  const nowSec = Math.floor(nowMs / 1000);
  const base = nowSec - (nowSec % windowSec);
  return [base - windowSec, base, base + windowSec, base + 2 * windowSec]
    .filter((value) => value > 0)
    .map((value) => `${slugPrefix}${value}`);
}

async function fetchActiveUpdownMarkets(): Promise<Array<Record<string, unknown>>> {
  const now = Date.now();
  if (activeUpdownMarketsCache && activeUpdownMarketsCache.expiresAt > now) {
    return activeUpdownMarketsCache.markets;
  }

  const url = `${GAMMA_BASE_URL.replace(/\/$/, '')}/markets?active=true&closed=false&limit=1000`;
  const res = await fetch(url, { cache: 'no-store' });
  if (!res.ok) return [];
  const payload = (await res.json()) as unknown;
  const rows = Array.isArray(payload)
    ? payload.filter((item): item is Record<string, unknown> => !!item && typeof item === 'object')
    : [];
  const prefixes = new Set(Object.values(SCOPE_TO_UPDOWN_SLUG_PREFIX));
  const markets = rows.filter((row) => {
    if (!isMarketActive(row)) return false;
    const marketSlug = String(row.slug || '').trim().toLowerCase();
    if (!marketSlug) return false;
    for (const prefix of prefixes) {
      if (marketSlug.startsWith(prefix)) return true;
    }
    return false;
  });
  activeUpdownMarketsCache = {
    expiresAt: now + ACTIVE_UPDOWN_MARKETS_CACHE_TTL_MS,
    markets,
  };
  return markets;
}

async function resolveMarketFromScope(scope: string): Promise<Record<string, unknown> | null> {
  const normalizedScope = scope.trim().toLowerCase();
  const slugPrefix = SCOPE_TO_UPDOWN_SLUG_PREFIX[normalizedScope];
  if (!slugPrefix) return null;

  const windowSec = scopeWindowSeconds(normalizedScope);
  const activeMarkets = await fetchActiveUpdownMarkets();
  const scopedMarkets = activeMarkets.filter((market) =>
    String(market.slug || '').trim().toLowerCase().startsWith(slugPrefix)
  );
  const selected = selectPreferredScopeMarket(scopedMarkets, slugPrefix, windowSec);
  if (selected) return selected;

  // Gamma may briefly lag active listing during market boundary; probe nearby candidate slugs.
  const candidates = buildScopeCandidateSlugs(slugPrefix, windowSec, Date.now());
  const fetched = await Promise.all(candidates.map((candidateSlug) => fetchMarketBySlug(candidateSlug)));
  const fallbackScoped = fetched
    .filter((market): market is Record<string, unknown> => !!market)
    .filter((market) => isMarketActive(market))
    .filter((market) => String(market.slug || '').trim().toLowerCase().startsWith(slugPrefix));
  return selectPreferredScopeMarket(fallbackScoped, slugPrefix, windowSec);
}

async function tryExtractEventOutcomes(market: Record<string, unknown>): Promise<TradeBuilderOutcome[]> {
  // Market object may contain events array with parent event slug
  const events = Array.isArray(market.events) ? (market.events as Array<Record<string, unknown>>) : [];
  const eventSlug = events.length > 0 ? String(events[0].slug || '').trim() : '';
  if (!eventSlug) return [];
  const eventData = await fetchEventData(eventSlug);
  if (!eventData) return [];
  const markets = Array.isArray(eventData.markets) ? (eventData.markets as Array<Record<string, unknown>>) : [];
  if (markets.length <= 1) return [];
  return extractEventMarketOutcomes(markets);
}

function extractEventMarketOutcomes(markets: Array<Record<string, unknown>>): TradeBuilderOutcome[] {
  const out: TradeBuilderOutcome[] = [];
  for (const m of markets) {
    const rawLabel = String(m.groupItemTitle || m.title || '').trim();
    const label = rawLabel.includes('(') ? rawLabel.slice(0, rawLabel.indexOf('(')).trim() : rawLabel;
    if (!label) continue;
    // Get the Yes token ID (first element of clobTokenIds)
    const clobIds = parseStringArray(m.clobTokenIds || m.clob_token_ids);
    const tokens = Array.isArray(m.tokens) ? (m.tokens as Array<Record<string, unknown>>) : [];
    const yesToken = tokens.find((t) => String(t.outcome || '') === 'Yes');
    const tokenId = (
      (yesToken ? String(yesToken.token_id || yesToken.tokenId || yesToken.clobTokenId || '') : '') ||
      (clobIds.length > 0 ? clobIds[0] : '')
    ).trim();
    if (!tokenId) continue;
    // Try to get price from outcomePrices
    const outcomePrices = parseStringArray(m.outcomePrices);
    const priceStr = outcomePrices.length > 0 ? outcomePrices[0] : null;
    const price = priceStr ? parseFloat(priceStr) : null;
    out.push({ token_id: tokenId, label, price: Number.isFinite(price as number) ? (price as number) : null });
  }
  return out;
}

function extractOutcomes(market: Record<string, unknown>): TradeBuilderOutcome[] {
  const fromTokens = extractOutcomesFromTokens(market);
  if (fromTokens.length > 0) return fromTokens;
  return extractOutcomesFromArrays(market);
}

function extractOutcomesFromTokens(market: Record<string, unknown>): TradeBuilderOutcome[] {
  const tokens = Array.isArray(market.tokens) ? (market.tokens as Array<Record<string, unknown>>) : [];
  return tokens
    .map((token) => {
      const tokenId = String(token.token_id || token.tokenId || token.clobTokenId || token.id || '').trim();
      const label = String(token.outcome || token.name || token.title || '').trim();
      const priceValue = token.price ?? token.lastPrice ?? null;
      const price = typeof priceValue === 'number' ? priceValue : typeof priceValue === 'string' ? parseFloat(priceValue) : null;
      if (!tokenId || !label) return null;
      return { token_id: tokenId, label, price: Number.isFinite(price as number) ? (price as number) : null };
    })
    .filter((item): item is TradeBuilderOutcome => !!item);
}

function extractOutcomesFromArrays(market: Record<string, unknown>): TradeBuilderOutcome[] {
  const outcomesRaw = market.outcomes;
  const tokenIdsRaw = market.clobTokenIds || market.clob_token_ids;

  const outcomes = parseStringArray(outcomesRaw);
  const tokenIds = parseStringArray(tokenIdsRaw);

  if (outcomes.length === 0 || tokenIds.length === 0) return [];

  const len = Math.min(outcomes.length, tokenIds.length);
  const out: TradeBuilderOutcome[] = [];
  for (let i = 0; i < len; i += 1) {
    const tokenId = tokenIds[i]?.trim();
    const label = outcomes[i]?.trim();
    if (!tokenId || !label) continue;
    out.push({ token_id: tokenId, label, price: null });
  }
  return out;
}

function parseStringArray(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.map((x) => String(x));
  }
  if (typeof value === 'string') {
    try {
      const parsed = JSON.parse(value);
      if (Array.isArray(parsed)) return parsed.map((x) => String(x));
    } catch {
      return [];
    }
  }
  return [];
}
