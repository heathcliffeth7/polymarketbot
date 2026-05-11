import { pool } from '@/lib/db'
import type {
  PaginatedResponse,
  TradeBuilderOrder,
  TradeBuilderOrderEvent,
  TradeBuilderOrderDiagnosticSummary,
  TradeBuilderOrderEventsResponse,
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
): Promise<TradeBuilderOrderEventsResponse> {
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
  const [countRes, dataRes, diagnosticRes] = await Promise.all([
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
    pool.query(
      `SELECT e.id, e.builder_order_id, e.event_type, e.payload_json, e.created_at
       FROM trade_builder_order_events e
       JOIN trade_builder_orders o ON o.id = e.builder_order_id
       WHERE o.user_id = $1 AND e.builder_order_id = $2
       ORDER BY e.created_at DESC, e.id DESC
       LIMIT 250`,
      [filters.userId, filters.orderId]
    ),
  ])

  const total = Number(countRes.rows[0]?.total || 0)
  const diagnosticSummary = buildTradeBuilderOrderDiagnosticSummary(
    diagnosticRes.rows as TradeBuilderOrderEvent[]
  )
  return {
    data: dataRes.rows,
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
    diagnostic_summary: diagnosticSummary,
  }
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null
}

function asString(value: unknown): string | null {
  return typeof value === 'string' && value.trim() ? value : null
}

function asBoolean(value: unknown): boolean | null {
  return typeof value === 'boolean' ? value : null
}

function deriveLegacyGuardSummary(event: TradeBuilderOrderEvent): {
  scope: string | null
  decision: string | null
  reasonCode: string | null
} {
  const payload = asRecord(event.payload_json)
  const reasonCode = asString(payload?.reason_code)
  switch (event.event_type) {
    case 'trigger_price_blocked':
      return { scope: 'trigger_price', decision: 'blocked', reasonCode }
    case 'trigger_price_waiting':
      return { scope: 'trigger_price', decision: 'waiting', reasonCode: reasonCode || 'below_trigger_price_guard' }
    case 'execution_floor_blocked':
      return { scope: 'execution_floor', decision: 'blocked', reasonCode }
    case 'execution_floor_waiting':
      return { scope: 'execution_floor', decision: 'waiting', reasonCode }
    case 'max_price_blocked':
      return { scope: 'max_price', decision: 'blocked', reasonCode }
    case 'max_price_waiting':
      return { scope: 'max_price', decision: 'waiting', reasonCode: reasonCode || 'above_max_price' }
    default:
      return { scope: null, decision: null, reasonCode: null }
  }
}

function buildTradeBuilderOrderDiagnosticSummary(
  events: TradeBuilderOrderEvent[]
): TradeBuilderOrderDiagnosticSummary {
  const latestFlowEvent = events.find(
    (event) => event.event_type === 'flow_created' || event.event_type === 'flow_rearmed'
  )
  const latestFlowPayload = asRecord(latestFlowEvent?.payload_json)
  const priceToBeatGuard = asRecord(latestFlowPayload?.price_to_beat_guard)
  const priceToBeatPassed = asBoolean(priceToBeatGuard?.passed)
  const priceToBeatReasonCode =
    asString(priceToBeatGuard?.reason_code) ??
    (priceToBeatPassed === true ? 'passed' : null)

  const latestGuardEvent = events.find((event) => event.event_type === 'guard_evaluated')
  const latestGuardPayload = asRecord(latestGuardEvent?.payload_json)
  const latestGuardScope = asString(latestGuardPayload?.effective_guard_scope)
  const latestGuardDecision = asString(latestGuardPayload?.effective_decision)
  const latestGuardReasonCode = asString(latestGuardPayload?.effective_reason_code)

  const latestLegacyGuard = events
    .map(deriveLegacyGuardSummary)
    .find((summary) => summary.scope !== null)

  const firstMeaningfulEvent = events.find((event) => {
    return !['flow_created', 'flow_rearmed', 'notification_sent'].includes(event.event_type)
  })

  const submitAttempted = events.some((event) =>
    ['submitted', 'fatal_exchange_rejection', 'terminal_exchange_status'].includes(event.event_type)
  )

  let effectiveOutcome = 'submit_not_attempted_yet'
  let effectiveReasonCode: string | null = null

  for (const event of events) {
    const payload = asRecord(event.payload_json)
    if (event.event_type === 'submitted') {
      effectiveOutcome = 'submitted'
      effectiveReasonCode = null
      break
    }
    if (event.event_type === 'blocked_by_risk') {
      effectiveOutcome = 'blocked'
      effectiveReasonCode = asString(payload?.reason_code) || 'risk_blocked'
      break
    }
    if (event.event_type === 'expired') {
      effectiveOutcome = 'expired'
      effectiveReasonCode = asString(payload?.reason_code) || 'expired'
      break
    }
    if (event.event_type === 'price_unavailable_retry') {
      effectiveOutcome = 'submit_skipped'
      effectiveReasonCode = asString(payload?.reason_code) || 'runtime_price_unavailable'
      break
    }
    if (event.event_type === 'guard_evaluated') {
      const decision = asString(payload?.effective_decision)
      effectiveOutcome = decision === 'passed' ? 'guards_passed' : decision || 'unknown'
      effectiveReasonCode = asString(payload?.effective_reason_code)
      break
    }
    const legacy = deriveLegacyGuardSummary(event)
    if (legacy.scope) {
      effectiveOutcome = legacy.decision || 'unknown'
      effectiveReasonCode = legacy.reasonCode
      break
    }
  }

  return {
    buy_created: Boolean(latestFlowEvent),
    processing_started: Boolean(firstMeaningfulEvent),
    guards_ran: Boolean(priceToBeatGuard || latestGuardEvent || latestLegacyGuard?.scope),
    builder_guards_ran: Boolean(latestGuardEvent || latestLegacyGuard?.scope),
    price_to_beat_ran: Boolean(priceToBeatGuard),
    price_to_beat_decision:
      priceToBeatPassed === true ? 'passed' : priceToBeatReasonCode ? 'blocked' : null,
    price_to_beat_reason_code: priceToBeatReasonCode,
    last_guard_scope: latestGuardScope ?? latestLegacyGuard?.scope ?? null,
    last_guard_decision: latestGuardDecision ?? latestLegacyGuard?.decision ?? null,
    last_guard_reason_code: latestGuardReasonCode ?? latestLegacyGuard?.reasonCode ?? null,
    submit_attempted: submitAttempted,
    effective_outcome: effectiveOutcome,
    effective_reason_code: effectiveReasonCode,
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
