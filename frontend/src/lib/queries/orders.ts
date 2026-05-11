import { pool } from '@/lib/db';
import type { Order, PaginatedResponse } from '@/lib/types';

interface OrderFilters {
  userId: number;
  page?: number;
  limit?: number;
  tradeId?: number;
  status?: string;
  intent?: string;
}

export async function getOrders(filters: OrderFilters): Promise<PaginatedResponse<Order>> {
  const page = filters.page || 1;
  const limit = Math.min(filters.limit || 20, 100);
  const offset = (page - 1) * limit;

  const conditions: string[] = [`t.user_id = $1`];
  const params: unknown[] = [filters.userId];
  let paramIdx = 2;

  if (filters.tradeId) {
    conditions.push(`o.trade_id = $${paramIdx++}`);
    params.push(filters.tradeId);
  }
  if (filters.status) {
    conditions.push(`o.status = $${paramIdx++}`);
    params.push(filters.status);
  }
  if (filters.intent) {
    conditions.push(`o.intent = $${paramIdx++}`);
    params.push(filters.intent);
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';

  const countQuery = `
    SELECT COUNT(*) as total
    FROM orders o
    JOIN trades t ON t.id = o.trade_id
    ${where}
  `;
  const dataQuery = `
    SELECT o.id, o.trade_id, o.exchange_order_id, o.client_order_id, o.intent, o.side,
           price, size, status, last_exchange_status, reject_reason,
           created_at, updated_at
    FROM orders o
    JOIN trades t ON t.id = o.trade_id
    ${where}
    ORDER BY o.created_at DESC
    LIMIT $${paramIdx++} OFFSET $${paramIdx++}
  `;

  const [countResult, dataResult] = await Promise.all([
    pool.query(countQuery, params),
    pool.query(dataQuery, [...params, limit, offset]),
  ]);

  const total = parseInt(countResult.rows[0].total);

  return {
    data: dataResult.rows,
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
  };
}
