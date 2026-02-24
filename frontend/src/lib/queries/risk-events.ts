import { pool } from '@/lib/db';
import type { RiskEvent, PaginatedResponse } from '@/lib/types';

interface RiskEventFilters {
  page?: number;
  limit?: number;
  eventType?: string;
  decision?: string;
}

export async function getRiskEvents(filters: RiskEventFilters): Promise<PaginatedResponse<RiskEvent>> {
  const page = filters.page || 1;
  const limit = Math.min(filters.limit || 30, 100);
  const offset = (page - 1) * limit;

  const conditions: string[] = [];
  const params: unknown[] = [];
  let paramIdx = 1;

  if (filters.eventType) {
    conditions.push(`event_type = $${paramIdx++}`);
    params.push(filters.eventType);
  }
  if (filters.decision) {
    conditions.push(`decision = $${paramIdx++}`);
    params.push(filters.decision);
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';

  const countQuery = `SELECT COUNT(*) as total FROM risk_events ${where}`;
  const dataQuery = `
    SELECT id, trade_id, event_type, decision, details, created_at
    FROM risk_events ${where}
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
