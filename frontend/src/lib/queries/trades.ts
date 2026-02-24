import { pool } from '@/lib/db';
import type { Trade, PaginatedResponse } from '@/lib/types';

interface TradeFilters {
  page?: number;
  limit?: number;
  state?: string;
  from?: string;
  to?: string;
}

export async function getTrades(filters: TradeFilters): Promise<PaginatedResponse<Trade>> {
  const page = filters.page || 1;
  const limit = Math.min(filters.limit || 20, 100);
  const offset = (page - 1) * limit;

  const conditions: string[] = [];
  const params: unknown[] = [];
  let paramIdx = 1;

  if (filters.state) {
    conditions.push(`t.state = $${paramIdx++}`);
    params.push(filters.state);
  }
  if (filters.from) {
    conditions.push(`t.opened_at >= $${paramIdx++}`);
    params.push(filters.from);
  }
  if (filters.to) {
    conditions.push(`t.opened_at <= $${paramIdx++}`);
    params.push(filters.to);
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';

  const countQuery = `SELECT COUNT(*) as total FROM trades t ${where}`;
  const dataQuery = `
    SELECT t.*, m.market_slug FROM trades t
    JOIN markets m ON m.id = t.market_id
    ${where}
    ORDER BY t.opened_at DESC NULLS LAST
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
