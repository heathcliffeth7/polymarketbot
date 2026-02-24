import { pool } from '@/lib/db';
import type { BotRun, Trade, DashboardData } from '@/lib/types';
import { readConfig } from '@/lib/config';

export async function getDashboardData(): Promise<DashboardData> {
  const [lastRun, activeTrade, dailyPnl, recentTrades, riskSummary] = await Promise.all([
    getLastRun(),
    getActiveTrade(),
    getDailyPnl(),
    getRecentTrades(),
    getRiskSummary(),
  ]);

  const [activePosition, pressure, positionExitRules] = activeTrade
    ? await Promise.all([
        getActivePosition(activeTrade.id),
        getPressureSnapshot(activeTrade.id),
        getPositionExitRules(activeTrade.id),
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

async function getActiveTrade(): Promise<Trade | null> {
  const { rows } = await pool.query(
    `SELECT t.*, m.market_slug FROM trades t
     JOIN markets m ON m.id = t.market_id
     WHERE t.state NOT IN ('Settled', 'Halted', 'Idle')
     ORDER BY t.opened_at DESC LIMIT 1`
  );
  return rows[0] || null;
}

async function getDailyPnl() {
  const { rows } = await pool.query(
    `SELECT
       COALESCE(SUM(realized_pnl), 0) as total_pnl,
       COUNT(*) as trade_count,
       COUNT(*) FILTER (WHERE realized_pnl > 0) as win_count,
       COUNT(*) FILTER (WHERE realized_pnl < 0) as loss_count
     FROM trades
     WHERE closed_at::date = CURRENT_DATE`
  );
  const r = rows[0];
  return {
    totalPnl: parseFloat(r.total_pnl) || 0,
    tradeCount: parseInt(r.trade_count) || 0,
    winCount: parseInt(r.win_count) || 0,
    lossCount: parseInt(r.loss_count) || 0,
  };
}

async function getRecentTrades(): Promise<Trade[]> {
  const { rows } = await pool.query(
    `SELECT t.*, m.market_slug FROM trades t
     JOIN markets m ON m.id = t.market_id
     ORDER BY t.opened_at DESC NULLS LAST LIMIT 10`
  );
  return rows;
}

async function getRiskSummary() {
  const [openOrders, consecutiveLosses, haltCount, riskConfig] = await Promise.all([
    pool.query("SELECT COUNT(*) as cnt FROM orders WHERE status IN ('open', 'partially_filled')"),
    pool.query(
      'SELECT realized_pnl FROM trades WHERE closed_at IS NOT NULL ORDER BY closed_at DESC LIMIT 10'
    ),
    pool.query(
      "SELECT COUNT(*) as cnt FROM risk_events WHERE decision = 'halt' AND created_at::date = CURRENT_DATE"
    ),
    readConfig('risk').catch(() => ({ manual_kill_switch_active: false })),
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

async function getActivePosition(tradeId: number): Promise<DashboardData['activePosition']> {
  try {
    const [tradeRes, legsRes] = await Promise.all([
      pool.query('SELECT t.id, m.market_slug FROM trades t JOIN markets m ON m.id = t.market_id WHERE t.id = $1', [tradeId]),
      pool.query(
        `SELECT leg_side, token_id, qty, avg_entry, levels_filled, last_fill_price, updated_at
         FROM leg_positions WHERE trade_id = $1 ORDER BY leg_side ASC`,
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

async function getPressureSnapshot(tradeId: number): Promise<DashboardData['pressure']> {
  try {
    const { rows } = await pool.query(
      `SELECT trade_id, pressure_score, bid_ask_imbalance, sell_ratio, yes_price, no_price, trigger_reason, triggered, updated_at
       FROM pressure_snapshots
       WHERE trade_id = $1`,
      [tradeId]
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
  tradeId: number
): Promise<NonNullable<DashboardData['positionExitRules']>> {
  try {
    const { rows } = await pool.query(
      `SELECT leg_side, drop_sell_pct, enabled, updated_at
       FROM position_exit_rules
       WHERE trade_id = $1
       ORDER BY leg_side ASC`,
      [tradeId]
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
