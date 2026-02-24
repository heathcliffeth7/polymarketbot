import { NextRequest, NextResponse } from 'next/server';
import { pool } from '@/lib/db';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const { searchParams } = new URL(req.url);
    let tradeId = parseInt(searchParams.get('tradeId') || '0', 10);

    if (!Number.isFinite(tradeId) || tradeId <= 0) {
      const active = await pool.query(
        `SELECT id FROM trades
         WHERE state NOT IN ('Settled', 'Halted', 'Idle')
         ORDER BY opened_at DESC
         LIMIT 1`
      );
      tradeId = active.rows[0]?.id || 0;
    }

    if (!tradeId) {
      return NextResponse.json({ tradeId: null, rules: [] });
    }

    const { rows } = await pool.query(
      `SELECT leg_side, drop_sell_pct, enabled, updated_at
       FROM position_exit_rules
       WHERE trade_id = $1
       ORDER BY leg_side ASC`,
      [tradeId]
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
    const body = await req.json();
    const tradeId = Number(body?.tradeId);
    const rules = Array.isArray(body?.rules) ? body.rules : [];

    if (!Number.isFinite(tradeId) || tradeId <= 0) {
      return NextResponse.json({ error: 'tradeId is required' }, { status: 400 });
    }

    if (rules.length === 0) {
      return NextResponse.json({ error: 'rules are required' }, { status: 400 });
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
