import { pool } from '@/lib/db';
import type { Market, PaginatedResponse } from '@/lib/types';

interface MarketFilters {
  page?: number;
  limit?: number;
  status?: string;
}

export async function getMarkets(filters: MarketFilters): Promise<PaginatedResponse<Market>> {
  const page = filters.page || 1;
  const limit = Math.min(filters.limit || 20, 100);
  const offset = (page - 1) * limit;

  const conditions: string[] = [];
  const params: unknown[] = [];
  let paramIdx = 1;

  if (filters.status) {
    conditions.push(`status = $${paramIdx++}`);
    params.push(filters.status);
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';

  const countQuery = `SELECT COUNT(*) as total FROM markets ${where}`;
  const dataQuery = `
    SELECT id, market_slug, starts_at, ends_at, status
    FROM markets ${where}
    ORDER BY starts_at DESC
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
