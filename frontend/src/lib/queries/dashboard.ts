import { pool } from '@/lib/db';
import type { BotRun, Trade, DashboardData } from '@/lib/types';
import { readConfig } from '@/lib/config';

export async function getDashboardData(
  context: { userId: number; username: string }
): Promise<DashboardData> {
  const [lastRun, activeTrade, dailyPnl, recentTrades, riskSummary] = await Promise.all([
    getLastRun(),
    getActiveTrade(context.userId),
    getDailyPnl(context.userId),
    getRecentTrades(context.userId),
    getRiskSummary(context),
  ]);

  const [activePosition, pressure, positionExitRules] = activeTrade
    ? await Promise.all([
        getActivePosition(context.userId, activeTrade.id),
        getPressureSnapshot(context.userId, activeTrade.id),
        getPositionExitRules(context.userId, activeTrade.id),
      ])
    : [null, null, [] as NonNullable<DashboardData['positionExitRules']>];

  return {
    botStatus: {
      serviceActive: false,
      lastRun,
      controlAvailable: false,
      controlReason: null,
      controlReasonCode: null,
      marketDiscoveryState: 'ready',
      selectedMarketSlug: null,
      marketDiscoveryMessage: null,
    },
    activeTrade,
    dailyPnl,
    recentTrades,
    riskSummary,
    activePosition,
    pressure,
    positionExitRules,
  };
}

async function getLastRun(): Promise<BotRun | null> {
  const { rows } = await pool.query(
    'SELECT id, mode, version, started_at, stopped_at, reason FROM bot_runs ORDER BY started_at DESC LIMIT 1'
  );
  return rows[0] || null;
}

async function getActiveTrade(userId: number): Promise<Trade | null> {
  const { rows } = await pool.query(
    `SELECT t.*, m.market_slug FROM trades t
     JOIN markets m ON m.id = t.market_id
     WHERE t.state NOT IN ('Settled', 'Halted', 'Idle')
       AND t.user_id = $1
     ORDER BY t.opened_at DESC LIMIT 1`,
    [userId]
  );
  return rows[0] || null;
}

async function getDailyPnl(userId: number) {
  const { rows } = await pool.query(
    `SELECT
       COALESCE(SUM(realized_pnl), 0) as total_pnl,
       COUNT(*) as trade_count,
       COUNT(*) FILTER (WHERE realized_pnl > 0) as win_count,
       COUNT(*) FILTER (WHERE realized_pnl < 0) as loss_count
     FROM trades
     WHERE closed_at::date = CURRENT_DATE
       AND user_id = $1`,
    [userId]
  );
  const r = rows[0];
  return {
    totalPnl: parseFloat(r.total_pnl) || 0,
    tradeCount: parseInt(r.trade_count) || 0,
    winCount: parseInt(r.win_count) || 0,
    lossCount: parseInt(r.loss_count) || 0,
  };
}

async function getRecentTrades(userId: number): Promise<Trade[]> {
  const { rows } = await pool.query(
    `SELECT t.*, m.market_slug FROM trades t
     JOIN markets m ON m.id = t.market_id
     WHERE t.user_id = $1
     ORDER BY t.opened_at DESC NULLS LAST LIMIT 10`,
    [userId]
  );
  return rows;
}

async function getRiskSummary(context: { userId: number; username: string }) {
  const [openOrders, consecutiveLosses, haltCount, riskConfig] = await Promise.all([
    pool.query(
      `SELECT COUNT(*) as cnt
       FROM orders o
       JOIN trades t ON t.id = o.trade_id
       WHERE o.status IN ('open', 'partially_filled')
         AND t.user_id = $1`,
      [context.userId]
    ),
    pool.query(
      `SELECT realized_pnl
       FROM trades
       WHERE closed_at IS NOT NULL
         AND user_id = $1
       ORDER BY closed_at DESC
       LIMIT 10`,
      [context.userId]
    ),
    pool.query(
      `SELECT COUNT(*) as cnt
       FROM risk_events r
       JOIN trades t ON t.id = r.trade_id
       WHERE r.decision = 'halt'
         AND r.created_at::date = CURRENT_DATE
         AND t.user_id = $1`,
      [context.userId]
    ),
    readConfig('risk', context).catch(() => ({ manual_kill_switch_active: false })),
  ]);

  let losses = 0;
  for (const row of consecutiveLosses.rows) {
    if (parseFloat(row.realized_pnl) < 0) losses++;
    else break;
  }

  return {
    openOrders: parseInt(openOrders.rows[0].cnt) || 0,
    consecutiveLosses: losses,
    haltCount: parseInt(haltCount.rows[0].cnt) || 0,
    killSwitchActive: !!(riskConfig as Record<string, unknown>).manual_kill_switch_active,
  };
}

async function getActivePosition(
  userId: number,
  tradeId: number
): Promise<DashboardData['activePosition']> {
  try {
    const [tradeRes, legsRes] = await Promise.all([
      pool.query(
        `SELECT t.id, m.market_slug
         FROM trades t
         JOIN markets m ON m.id = t.market_id
         WHERE t.id = $1
           AND t.user_id = $2`,
        [tradeId, userId]
      ),
      pool.query(
        `SELECT leg_side, token_id, qty, avg_entry, levels_filled, last_fill_price, updated_at
         FROM leg_positions
         WHERE trade_id = $1
         ORDER BY leg_side ASC`,
        [tradeId]
      ),
    ]);

    if (tradeRes.rows.length === 0) return null;

    const trade = tradeRes.rows[0];
    return {
      tradeId: trade.id,
      marketSlug: trade.market_slug,
      legs: legsRes.rows.map((row) => ({
        legSide: row.leg_side === 'no' ? 'no' : 'yes',
        tokenId: String(row.token_id),
        qty: parseFloat(row.qty) || 0,
        avgEntry: parseFloat(row.avg_entry) || 0,
        levelsFilled: parseInt(row.levels_filled) || 0,
        lastFillPrice: row.last_fill_price == null ? null : parseFloat(row.last_fill_price),
        updatedAt: row.updated_at ?? null,
      })),
    };
  } catch {
    return null;
  }
}

async function getPressureSnapshot(
  userId: number,
  tradeId: number
): Promise<DashboardData['pressure']> {
  try {
    const { rows } = await pool.query(
      `SELECT ps.trade_id, ps.pressure_score, ps.bid_ask_imbalance, ps.sell_ratio, ps.yes_price, ps.no_price, ps.trigger_reason, ps.triggered, ps.updated_at
       FROM pressure_snapshots ps
       JOIN trades t ON t.id = ps.trade_id
       WHERE ps.trade_id = $1
         AND t.user_id = $2`,
      [tradeId, userId]
    );
    if (rows.length === 0) return null;
    const row = rows[0];
    return {
      tradeId: row.trade_id,
      pressureScore: parseFloat(row.pressure_score) || 0,
      bidAskImbalance: row.bid_ask_imbalance == null ? null : parseFloat(row.bid_ask_imbalance),
      sellRatio: row.sell_ratio == null ? null : parseFloat(row.sell_ratio),
      yesPrice: row.yes_price == null ? null : parseFloat(row.yes_price),
      noPrice: row.no_price == null ? null : parseFloat(row.no_price),
      triggerReason: row.trigger_reason ?? null,
      triggered: !!row.triggered,
      updatedAt: row.updated_at ?? null,
    };
  } catch {
    return null;
  }
}

async function getPositionExitRules(
  userId: number,
  tradeId: number
): Promise<NonNullable<DashboardData['positionExitRules']>> {
  try {
    const { rows } = await pool.query(
      `SELECT per.leg_side, per.drop_sell_pct, per.enabled, per.updated_at
       FROM position_exit_rules per
       JOIN trades t ON t.id = per.trade_id
       WHERE per.trade_id = $1
         AND t.user_id = $2
       ORDER BY per.leg_side ASC`,
      [tradeId, userId]
    );

    return rows.map((row) => ({
      legSide: row.leg_side === 'no' ? 'no' : 'yes',
      dropSellPct: parseFloat(row.drop_sell_pct) || 0,
      enabled: !!row.enabled,
      updatedAt: row.updated_at ?? null,
    }));
  } catch {
    return [];
  }
}
