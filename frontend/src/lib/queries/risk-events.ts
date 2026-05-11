import { pool } from '@/lib/db';
import type { RiskEvent, PaginatedResponse } from '@/lib/types';

interface RiskEventFilters {
  userId: number;
  page?: number;
  limit?: number;
  eventType?: string;
  decision?: string;
}

export async function getRiskEvents(filters: RiskEventFilters): Promise<PaginatedResponse<RiskEvent>> {
  const page = filters.page || 1;
  const limit = Math.min(filters.limit || 30, 100);
  const offset = (page - 1) * limit;

  const conditions: string[] = [`t.user_id = $1`];
  const params: unknown[] = [filters.userId];
  let paramIdx = 2;

  if (filters.eventType) {
    conditions.push(`r.event_type = $${paramIdx++}`);
    params.push(filters.eventType);
  }
  if (filters.decision) {
    conditions.push(`r.decision = $${paramIdx++}`);
    params.push(filters.decision);
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';

  const countQuery = `
    SELECT COUNT(*) as total
    FROM risk_events r
    JOIN trades t ON t.id = r.trade_id
    ${where}
  `;
  const dataQuery = `
    SELECT r.id, r.trade_id, r.event_type, r.decision, r.details, r.created_at
    FROM risk_events r
    JOIN trades t ON t.id = r.trade_id
    ${where}
    ORDER BY r.created_at DESC
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
