import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { pool } from '@/lib/db';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const { searchParams } = new URL(req.url);
    let tradeId = parseInt(searchParams.get('tradeId') || '0', 10);

    if (!Number.isFinite(tradeId) || tradeId <= 0) {
      const active = await pool.query(
        `SELECT id FROM trades
         WHERE user_id = $1
           AND state NOT IN ('Settled', 'Halted', 'Idle')
         ORDER BY opened_at DESC
         LIMIT 1`,
        [user.userId]
      );
      tradeId = active.rows[0]?.id || 0;
    }

    if (!tradeId) {
      return NextResponse.json({ tradeId: null, rules: [] });
    }

    const tradeRes = await pool.query(
      'SELECT id FROM trades WHERE id = $1 AND user_id = $2 LIMIT 1',
      [tradeId, user.userId]
    );
    if ((tradeRes.rowCount ?? 0) === 0) {
      return NextResponse.json({ error: 'Trade not found' }, { status: 404 });
    }

    const { rows } = await pool.query(
      `SELECT per.leg_side, per.drop_sell_pct, per.enabled, per.updated_at
       FROM position_exit_rules per
       JOIN trades t ON t.id = per.trade_id
       WHERE per.trade_id = $1
         AND t.user_id = $2
       ORDER BY per.leg_side ASC`,
      [tradeId, user.userId]
    );

    return NextResponse.json({
      tradeId,
      rules: rows.map((row) => ({
        legSide: row.leg_side === 'no' ? 'no' : 'yes',
        dropSellPct: parseFloat(row.drop_sell_pct) || 0,
        enabled: !!row.enabled,
        updatedAt: row.updated_at ?? null,
      })),
    });
  } catch (err) {
    console.error('Position rules GET error:', err);
    return NextResponse.json({ error: 'Failed to load position rules' }, { status: 500 });
  }
}

export async function PUT(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const body = await req.json();
    const tradeId = Number(body?.tradeId);
    const rules = Array.isArray(body?.rules) ? body.rules : [];

    if (!Number.isFinite(tradeId) || tradeId <= 0) {
      return NextResponse.json({ error: 'tradeId is required' }, { status: 400 });
    }

    if (rules.length === 0) {
      return NextResponse.json({ error: 'rules are required' }, { status: 400 });
    }

    const tradeRes = await pool.query(
      'SELECT id FROM trades WHERE id = $1 AND user_id = $2 LIMIT 1',
      [tradeId, user.userId]
    );
    if ((tradeRes.rowCount ?? 0) === 0) {
      return NextResponse.json({ error: 'Trade not found' }, { status: 404 });
    }

    for (const item of rules) {
      const legSide = String(item?.legSide || '').toLowerCase();
      const dropSellPct = Number(item?.dropSellPct);
      const enabled = !!item?.enabled;

      if (!['yes', 'no'].includes(legSide)) {
        return NextResponse.json({ error: 'legSide must be yes or no' }, { status: 400 });
      }
      if (!Number.isFinite(dropSellPct) || dropSellPct <= 0 || dropSellPct > 100) {
        return NextResponse.json(
          { error: 'dropSellPct must be > 0 and <= 100' },
          { status: 400 }
        );
      }

      await pool.query(
        `INSERT INTO position_exit_rules (trade_id, leg_side, drop_sell_pct, enabled, updated_at)
         VALUES ($1, $2, $3, $4, NOW())
         ON CONFLICT (trade_id, leg_side) DO UPDATE SET
           drop_sell_pct = EXCLUDED.drop_sell_pct,
           enabled = EXCLUDED.enabled,
           updated_at = NOW()`,
        [tradeId, legSide, dropSellPct, enabled]
      );
    }

    return NextResponse.json({ success: true });
  } catch (err) {
    console.error('Position rules PUT error:', err);
    return NextResponse.json({ error: 'Failed to save position rules' }, { status: 500 });
  }
}
