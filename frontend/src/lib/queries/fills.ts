import { pool } from '@/lib/db';
import type { Fill, PaginatedResponse } from '@/lib/types';

interface FillFilters {
  page?: number;
  limit?: number;
  orderId?: number;
}

export async function getFills(filters: FillFilters): Promise<PaginatedResponse<Fill>> {
  const page = filters.page || 1;
  const limit = Math.min(filters.limit || 20, 100);
  const offset = (page - 1) * limit;

  const conditions: string[] = [];
  const params: unknown[] = [];
  let paramIdx = 1;

  if (filters.orderId) {
    conditions.push(`order_id = $${paramIdx++}`);
    params.push(filters.orderId);
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';

  const countQuery = `SELECT COUNT(*) as total FROM fills ${where}`;
  const dataQuery = `
    SELECT id, order_id, fill_id, price, size, fee, filled_at
    FROM fills ${where}
    ORDER BY filled_at DESC
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
