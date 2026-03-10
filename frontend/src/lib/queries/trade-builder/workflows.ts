import { pool } from '@/lib/db'
import type {
  PaginatedResponse,
  TradeBuilderWorkflow,
  TradeBuilderWorkflowDetail,
  TradeBuilderWorkflowEvent,
} from '@/lib/types'
import type {
  CreateTradeBuilderWorkflowInput,
  TradeBuilderWorkflowEventFilters,
  TradeBuilderWorkflowFilters,
} from './types'

export async function getTradeBuilderWorkflows(
  filters: TradeBuilderWorkflowFilters
): Promise<PaginatedResponse<TradeBuilderWorkflowDetail>> {
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
    pool.query(`SELECT COUNT(*)::int AS total FROM trade_builder_workflows ${where}`, params),
    pool.query(
      `SELECT * FROM trade_builder_workflows ${where} ORDER BY created_at DESC LIMIT $${idx++} OFFSET $${idx++}`,
      [...params, limit, offset]
    ),
  ])

  const workflows = dataRes.rows as TradeBuilderWorkflow[]
  const workflowIds = workflows.map((x) => x.id)
  const legsByWorkflowId = new Map<number, unknown[]>()
  if (workflowIds.length > 0) {
    const legsRes = await pool.query(
      `SELECT * FROM trade_builder_workflow_legs
       WHERE workflow_id = ANY($1::bigint[])
       ORDER BY workflow_id, leg_type, id`,
      [workflowIds]
    )
    for (const row of legsRes.rows) {
      const workflowId = Number(row.workflow_id)
      if (!legsByWorkflowId.has(workflowId)) legsByWorkflowId.set(workflowId, [])
      legsByWorkflowId.get(workflowId)?.push(row)
    }
  }

  const total = Number(countRes.rows[0]?.total || 0)
  return {
    data: workflows.map((workflow) => ({
      workflow,
      legs: (legsByWorkflowId.get(workflow.id) || []) as TradeBuilderWorkflowDetail['legs'],
    })),
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
  }
}

export async function getTradeBuilderWorkflowById(
  userId: number,
  workflowId: number
): Promise<TradeBuilderWorkflowDetail | null> {
  const workflowRes = await pool.query(
    `SELECT * FROM trade_builder_workflows WHERE id = $1 AND user_id = $2 LIMIT 1`,
    [workflowId, userId]
  )
  if (workflowRes.rowCount === 0) return null

  const legsRes = await pool.query(
    `SELECT * FROM trade_builder_workflow_legs
     WHERE workflow_id = $1
     ORDER BY leg_type, id`,
    [workflowId]
  )

  return {
    workflow: workflowRes.rows[0] as TradeBuilderWorkflow,
    legs: legsRes.rows as TradeBuilderWorkflowDetail['legs'],
  }
}

export async function createTradeBuilderWorkflow(
  input: CreateTradeBuilderWorkflowInput
): Promise<TradeBuilderWorkflowDetail> {
  const client = await pool.connect()
  try {
    await client.query('BEGIN')

    const sellTriggerPrice =
      Number.isFinite(input.sellLeg.triggerPriceCent as number)
        ? Number(input.sellLeg.triggerPriceCent) / 100
        : null
    const buyTriggerPrice =
      Number.isFinite(input.buyLeg.triggerPriceCent as number)
        ? Number(input.buyLeg.triggerPriceCent) / 100
        : null

    const tokenNotionalRes = await client.query(
      `SELECT COALESCE(SUM(qty * COALESCE(last_fill_price, avg_entry)), 0)::double precision AS notional
       FROM leg_positions lp
       JOIN trades t ON t.id = lp.trade_id
       WHERE lp.trade_id = $1 AND lp.token_id = $2 AND t.user_id = $3`,
      [input.sourceTradeId, input.sellLeg.tokenId, input.userId]
    )
    let sourceNotional = Number(tokenNotionalRes.rows[0]?.notional || 0)
    if (sourceNotional <= 0) {
      const fallbackNotionalRes = await client.query(
        `SELECT COALESCE(SUM(qty * COALESCE(last_fill_price, avg_entry)), 0)::double precision AS notional
         FROM leg_positions lp
         JOIN trades t ON t.id = lp.trade_id
         WHERE lp.trade_id = $1 AND t.user_id = $2`,
        [input.sourceTradeId, input.userId]
      )
      sourceNotional = Number(fallbackNotionalRes.rows[0]?.notional || 0)
    }
    if (sourceNotional <= 0) {
      throw new Error('Source trade position notional is zero')
    }

    const sellTargetNotional = sourceNotional * (input.sellTargetPct / 100)
    const buyTargetNotional = sellTargetNotional * (input.buyAllocationPct / 100)
    if (sellTargetNotional <= 0 || buyTargetNotional <= 0) {
      throw new Error('Computed workflow notionals must be > 0')
    }

    const workflowRes = await client.query(
      `INSERT INTO trade_builder_workflows
         (user_id, name, status, source_trade_id, sell_target_pct, buy_start_after_sell_progress_pct, buy_trigger_mode, buy_allocation_pct, expires_at, created_at, updated_at)
       VALUES
         ($1, $2, 'armed', $3, $4, $5, $6, $7, $8, NOW(), NOW())
       RETURNING *`,
      [
        input.userId,
        input.name?.trim() || 'workflow',
        input.sourceTradeId,
        input.sellTargetPct,
        input.buyStartAfterSellProgressPct,
        input.buyTriggerMode,
        input.buyAllocationPct,
        input.expiresAt ? new Date(input.expiresAt) : null,
      ]
    )
    const workflow = workflowRes.rows[0] as TradeBuilderWorkflow

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
    )

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
    )

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
    )

    await client.query('COMMIT')
    return {
      workflow,
      legs: [sellLegRes.rows[0], buyLegRes.rows[0]] as TradeBuilderWorkflowDetail['legs'],
    }
  } catch (err) {
    await client.query('ROLLBACK')
    throw err
  } finally {
    client.release()
  }
}

export async function updateTradeBuilderWorkflow(
  userId: number,
  workflowId: number,
  updates: {
    buyStartAfterSellProgressPct?: number
    buyTriggerMode?: 'sell_progress_only' | 'price_only' | 'sell_progress_and_price'
    buyAllocationPct?: number
    expiresAt?: string | null
  }
): Promise<void> {
  const fields: string[] = []
  const params: unknown[] = [workflowId, userId]
  let idx = 3

  if (updates.buyStartAfterSellProgressPct !== undefined) {
    fields.push(`buy_start_after_sell_progress_pct = $${idx++}`)
    params.push(updates.buyStartAfterSellProgressPct)
  }
  if (updates.buyTriggerMode !== undefined) {
    fields.push(`buy_trigger_mode = $${idx++}`)
    params.push(updates.buyTriggerMode)
  }
  if (updates.buyAllocationPct !== undefined) {
    fields.push(`buy_allocation_pct = $${idx++}`)
    params.push(updates.buyAllocationPct)
  }
  if (updates.expiresAt !== undefined) {
    fields.push(`expires_at = $${idx++}`)
    params.push(updates.expiresAt ? new Date(updates.expiresAt) : null)
  }

  if (fields.length === 0) return
  fields.push('updated_at = NOW()')

  const result = await pool.query(
    `UPDATE trade_builder_workflows
     SET ${fields.join(', ')}
     WHERE id = $1 AND user_id = $2`,
    params
  )
  if ((result.rowCount ?? 0) === 0) {
    throw new Error('Trade builder workflow not found')
  }
}

export async function requestCancelTradeBuilderWorkflow(
  userId: number,
  workflowId: number
): Promise<void> {
  const client = await pool.connect()
  try {
    await client.query('BEGIN')

    const workflowRes = await client.query(
      `SELECT id FROM trade_builder_workflows WHERE id = $1 AND user_id = $2 LIMIT 1`,
      [workflowId, userId]
    )
    if ((workflowRes.rowCount ?? 0) === 0) {
      throw new Error('Trade builder workflow not found')
    }

    const legRes = await client.query(
      `SELECT id, builder_order_id FROM trade_builder_workflow_legs WHERE workflow_id = $1`,
      [workflowId]
    )

    for (const leg of legRes.rows) {
      if (leg.builder_order_id) {
        await client.query(
          `UPDATE trade_builder_orders
           SET status = CASE WHEN active_exchange_order_id IS NULL THEN 'canceled' ELSE 'canceled_requested' END,
               updated_at = NOW()
           WHERE id = $1 AND user_id = $2`,
          [leg.builder_order_id, userId]
        )
      }
    }

    await client.query(
      `UPDATE trade_builder_workflow_legs
       SET status = 'canceled', updated_at = NOW()
       WHERE workflow_id = $1`,
      [workflowId]
    )
    await client.query(
      `UPDATE trade_builder_workflows
       SET status = 'canceled', updated_at = NOW()
       WHERE id = $1 AND user_id = $2`,
      [workflowId, userId]
    )
    await client.query(
      `INSERT INTO trade_builder_workflow_events (workflow_id, leg_id, event_type, payload_json, created_at)
       VALUES ($1, NULL, 'canceled_by_user', $2, NOW())`,
      [workflowId, JSON.stringify({ reason: 'user_request' })]
    )

    await client.query('COMMIT')
  } catch (err) {
    await client.query('ROLLBACK')
    throw err
  } finally {
    client.release()
  }
}

export async function hardDeleteTradeBuilderWorkflow(
  userId: number,
  workflowId: number
): Promise<void> {
  const client = await pool.connect()
  try {
    await client.query('BEGIN')
    const workflowRes = await client.query(
      'SELECT id FROM trade_builder_workflows WHERE id = $1 AND user_id = $2 LIMIT 1',
      [workflowId, userId]
    )
    if ((workflowRes.rowCount ?? 0) === 0) {
      throw new Error('Trade builder workflow not found')
    }
    const legRes = await client.query(
      'SELECT builder_order_id FROM trade_builder_workflow_legs WHERE workflow_id = $1',
      [workflowId]
    )
    for (const leg of legRes.rows) {
      if (leg.builder_order_id) {
        await client.query('DELETE FROM trade_builder_order_events WHERE builder_order_id = $1', [leg.builder_order_id])
        await client.query('DELETE FROM trade_builder_orders WHERE id = $1', [leg.builder_order_id])
      }
    }
    await client.query('DELETE FROM trade_builder_workflow_events WHERE workflow_id = $1', [workflowId])
    await client.query('DELETE FROM trade_builder_workflow_legs WHERE workflow_id = $1', [workflowId])
    await client.query('DELETE FROM trade_builder_workflows WHERE id = $1 AND user_id = $2', [workflowId, userId])
    await client.query('COMMIT')
  } catch (err) {
    await client.query('ROLLBACK')
    throw err
  } finally {
    client.release()
  }
}

export async function getTradeBuilderWorkflowEvents(
  filters: TradeBuilderWorkflowEventFilters
): Promise<PaginatedResponse<TradeBuilderWorkflowEvent>> {
  const page = filters.page || 1
  const limit = Math.min(filters.limit || 25, 100)
  const offset = (page - 1) * limit

  const whereParts: string[] = ['w.user_id = $1', 'e.workflow_id = $2']
  const params: unknown[] = [filters.userId, filters.workflowId]
  let idx = 3

  if (filters.eventType) {
    whereParts.push(`e.event_type = $${idx++}`)
    params.push(filters.eventType)
  }

  const where = `WHERE ${whereParts.join(' AND ')}`
  const [countRes, dataRes] = await Promise.all([
    pool.query(
      `SELECT COUNT(*)::int AS total
       FROM trade_builder_workflow_events e
       JOIN trade_builder_workflows w ON w.id = e.workflow_id
       ${where}`,
      params
    ),
    pool.query(
      `SELECT e.id, e.workflow_id, e.leg_id, e.event_type, e.payload_json, e.created_at
       FROM trade_builder_workflow_events e
       JOIN trade_builder_workflows w ON w.id = e.workflow_id
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
