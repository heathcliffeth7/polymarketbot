import { pool } from '@/lib/db';
import type { Fill, PaginatedResponse } from '@/lib/types';

interface FillFilters {
  userId: number;
  page?: number;
  limit?: number;
  orderId?: number;
}

export async function getFills(filters: FillFilters): Promise<PaginatedResponse<Fill>> {
  const page = filters.page || 1;
  const limit = Math.min(filters.limit || 20, 100);
  const offset = (page - 1) * limit;

  const conditions: string[] = [`t.user_id = $1`];
  const params: unknown[] = [filters.userId];
  let paramIdx = 2;

  if (filters.orderId) {
    conditions.push(`f.order_id = $${paramIdx++}`);
    params.push(filters.orderId);
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';

  const countQuery = `
    SELECT COUNT(*) as total
    FROM fills f
    JOIN orders o ON o.id = f.order_id
    JOIN trades t ON t.id = o.trade_id
    ${where}
  `;
  const dataQuery = `
    SELECT f.id, f.order_id, f.fill_id, f.price, f.size, f.fee, f.filled_at
    FROM fills f
    JOIN orders o ON o.id = f.order_id
    JOIN trades t ON t.id = o.trade_id
    ${where}
    ORDER BY f.filled_at DESC
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
