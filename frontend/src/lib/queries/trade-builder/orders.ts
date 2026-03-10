import { pool } from '@/lib/db'
import type {
  PaginatedResponse,
  TradeBuilderOrder,
  TradeBuilderOrderEvent,
} from '@/lib/types'
import type {
  CreateTradeBuilderOrderInput,
  TradeBuilderFilters,
  TradeBuilderOrderEventFilters,
} from './types'

export async function getTradeBuilderOrders(
  filters: TradeBuilderFilters
): Promise<PaginatedResponse<TradeBuilderOrder>> {
  const page = filters.page || 1
  const limit = Math.min(filters.limit || 20, 100)
  const offset = (page - 1) * limit

  const whereParts: string[] = ['user_id = $1']
  const params: unknown[] = [filters.userId]
  let idx = 2

  if (filters.status) {
    whereParts.push(`status = $${idx++}`)
    params.push(filters.status)
  }

  const where = whereParts.length ? `WHERE ${whereParts.join(' AND ')}` : ''

  const [countRes, dataRes] = await Promise.all([
    pool.query(`SELECT COUNT(*)::int AS total FROM trade_builder_orders ${where}`, params),
    pool.query(
      `SELECT * FROM trade_builder_orders ${where} ORDER BY created_at DESC LIMIT $${idx++} OFFSET $${idx++}`,
      [...params, limit, offset]
    ),
  ])

  const total = Number(countRes.rows[0]?.total || 0)
  return {
    data: dataRes.rows,
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
  }
}

export async function getTradeBuilderOrderEvents(
  filters: TradeBuilderOrderEventFilters
): Promise<PaginatedResponse<TradeBuilderOrderEvent>> {
  const page = filters.page || 1
  const limit = Math.min(filters.limit || 25, 100)
  const offset = (page - 1) * limit

  const whereParts: string[] = ['o.user_id = $1', 'e.builder_order_id = $2']
  const params: unknown[] = [filters.userId, filters.orderId]
  let idx = 3

  if (filters.eventType) {
    whereParts.push(`e.event_type = $${idx++}`)
    params.push(filters.eventType)
  }

  const where = `WHERE ${whereParts.join(' AND ')}`

  const [countRes, dataRes] = await Promise.all([
    pool.query(
      `SELECT COUNT(*)::int AS total
       FROM trade_builder_order_events e
       JOIN trade_builder_orders o ON o.id = e.builder_order_id
       ${where}`,
      params
    ),
    pool.query(
      `SELECT e.id, e.builder_order_id, e.event_type, e.payload_json, e.created_at
       FROM trade_builder_order_events e
       JOIN trade_builder_orders o ON o.id = e.builder_order_id
       ${where}
       ORDER BY e.created_at DESC
       LIMIT $${idx++} OFFSET $${idx++}`,
      [...params, limit, offset]
    ),
  ])

  const total = Number(countRes.rows[0]?.total || 0)
  return {
    data: dataRes.rows,
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
  }
}

export async function createTradeBuilderOrder(
  input: CreateTradeBuilderOrderInput
): Promise<TradeBuilderOrder> {
  const executionMode = input.executionMode === 'market' ? 'market' : 'limit'
  const now = new Date()
  const startsAt = now
  const endsAt = input.expiresAt
    ? new Date(input.expiresAt)
    : new Date(now.getTime() + 7 * 24 * 3600 * 1000)
  const triggerPrice =
    input.kind === 'conditional' && Number.isFinite(input.triggerPriceCent)
      ? Number(input.triggerPriceCent) / 100
      : null

  const client = await pool.connect()
  try {
    await client.query('BEGIN')

    const marketRes = await client.query(
      `INSERT INTO markets (market_slug, starts_at, ends_at, status)
       VALUES ($1, $2, $3, 'open')
       ON CONFLICT (market_slug) DO UPDATE SET
         starts_at = LEAST(markets.starts_at, EXCLUDED.starts_at),
         ends_at = GREATEST(markets.ends_at, EXCLUDED.ends_at),
         status = CASE WHEN markets.status = 'settled' THEN markets.status ELSE 'open' END
       RETURNING id`,
      [input.marketSlug, startsAt, endsAt]
    )

    const marketId = marketRes.rows[0].id
    const referencePrice = triggerPrice ? Number(triggerPrice) : 0.5

    const tradeRes = await client.query(
      `INSERT INTO trades (market_id, user_id, state, entry_price, notional_usdc, strategy_mode, opened_at)
       VALUES ($1, $2, 'Idle', $3, $4, 'manual_trade_builder', NOW())
       RETURNING id`,
      [marketId, input.userId, referencePrice, input.sizeUsdc]
    )

    const tradeId = tradeRes.rows[0].id

    const orderRes = await client.query(
      `INSERT INTO trade_builder_orders
         (trade_id, user_id, kind, status, market_slug, token_id, outcome_label, side, execution_mode, trigger_condition, trigger_price,
          size_usdc, min_price_distance_cent, expires_at, max_triggers, triggers_fired, created_at, updated_at)
       VALUES
         ($1, $2, $3, 'pending', $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, 0, NOW(), NOW())
       RETURNING *`,
      [
        tradeId,
        input.userId,
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
    )

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
    )

    await client.query('COMMIT')
    return orderRes.rows[0]
  } catch (err) {
    await client.query('ROLLBACK')
    throw err
  } finally {
    client.release()
  }
}

export async function updateTradeBuilderOrder(
  userId: number,
  id: number,
  updates: { minPriceDistanceCent?: number; maxTriggers?: number; expiresAt?: string | null }
): Promise<void> {
  const fields: string[] = []
  const params: unknown[] = [id, userId]
  let idx = 3

  if (updates.minPriceDistanceCent !== undefined) {
    fields.push(`min_price_distance_cent = $${idx++}`)
    params.push(updates.minPriceDistanceCent)
  }
  if (updates.maxTriggers !== undefined) {
    fields.push(`max_triggers = $${idx++}`)
    params.push(updates.maxTriggers)
  }
  if (updates.expiresAt !== undefined) {
    fields.push(`expires_at = $${idx++}`)
    params.push(updates.expiresAt ? new Date(updates.expiresAt) : null)
  }

  if (fields.length === 0) return

  fields.push('updated_at = NOW()')
  const result = await pool.query(
    `UPDATE trade_builder_orders
     SET ${fields.join(', ')}
     WHERE id = $1 AND user_id = $2`,
    params
  )
  if ((result.rowCount ?? 0) === 0) {
    throw new Error('Trade builder order not found')
  }
}

export async function requestCancelTradeBuilderOrder(userId: number, id: number): Promise<void> {
  const result = await pool.query(
    `UPDATE trade_builder_orders
     SET status = CASE WHEN active_exchange_order_id IS NULL THEN 'canceled' ELSE 'canceled_requested' END,
         updated_at = NOW()
     WHERE id = $1 AND user_id = $2`,
    [id, userId]
  )
  if ((result.rowCount ?? 0) === 0) {
    throw new Error('Trade builder order not found')
  }
}

export async function hardDeleteAllTradeBuilderOrders(userId: number): Promise<number> {
  const client = await pool.connect()
  try {
    await client.query('BEGIN')
    await client.query(
      `DELETE FROM trade_builder_order_events
       WHERE builder_order_id IN (
         SELECT id FROM trade_builder_orders WHERE user_id = $1
       )`,
      [userId]
    )
    const res = await client.query('DELETE FROM trade_builder_orders WHERE user_id = $1', [userId])
    await client.query('COMMIT')
    return res.rowCount ?? 0
  } catch (err) {
    await client.query('ROLLBACK')
    throw err
  } finally {
    client.release()
  }
}

export async function hardDeleteTradeBuilderOrder(userId: number, id: number): Promise<void> {
  const client = await pool.connect()
  try {
    await client.query('BEGIN')
    const orderRes = await client.query(
      `SELECT id FROM trade_builder_orders WHERE id = $1 AND user_id = $2 LIMIT 1`,
      [id, userId]
    )
    if ((orderRes.rowCount ?? 0) === 0) {
      throw new Error('Trade builder order not found')
    }
    await client.query('DELETE FROM trade_builder_order_events WHERE builder_order_id = $1', [id])
    await client.query('DELETE FROM trade_builder_orders WHERE id = $1 AND user_id = $2', [id, userId])
    await client.query('COMMIT')
  } catch (err) {
    await client.query('ROLLBACK')
    throw err
  } finally {
    client.release()
  }
}
