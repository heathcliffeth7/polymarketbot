import { pool } from '@/lib/db';
import type { Order, PaginatedResponse } from '@/lib/types';

interface OrderFilters {
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

  const conditions: string[] = [];
  const params: unknown[] = [];
  let paramIdx = 1;

  if (filters.tradeId) {
    conditions.push(`trade_id = $${paramIdx++}`);
    params.push(filters.tradeId);
  }
  if (filters.status) {
    conditions.push(`status = $${paramIdx++}`);
    params.push(filters.status);
  }
  if (filters.intent) {
    conditions.push(`intent = $${paramIdx++}`);
    params.push(filters.intent);
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';

  const countQuery = `SELECT COUNT(*) as total FROM orders ${where}`;
  const dataQuery = `
    SELECT id, trade_id, exchange_order_id, client_order_id, intent, side,
           price, size, status, last_exchange_status, reject_reason,
           created_at, updated_at
    FROM orders ${where}
    ORDER BY created_at DESC
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
