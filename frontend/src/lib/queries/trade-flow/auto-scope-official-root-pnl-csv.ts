import type { AutoScopeTradeAnalysisRow } from '@/lib/types';

export const AUTO_SCOPE_OFFICIAL_ROOT_PNL_CSV_HEADERS = [
  'official_root_pnl_usdc',
  'official_pnl_source',
  'official_buy_usdc',
  'official_sell_usdc',
  'official_redeem_usdc',
  'official_delta_usdc',
  'polymarket_position_pnl_usdc',
  'polymarket_position_source',
  'polymarket_total_bet_usdc',
  'polymarket_amount_returned_usdc',
  'polymarket_realized_pnl_usdc',
  'polymarket_cash_pnl_usdc',
];

export function autoScopeOfficialRootPnlCsvValues(
  row: AutoScopeTradeAnalysisRow
): Array<number | string | null> {
  return [
    row.officialRootPnlUsdc,
    row.officialPnlSource,
    row.officialBuyUsdc,
    row.officialSellUsdc,
    row.officialRedeemUsdc,
    row.officialDeltaUsdc,
    row.polymarketPositionPnlUsdc ?? null,
    row.polymarketPositionSource ?? null,
    row.polymarketTotalBetUsdc ?? null,
    row.polymarketAmountReturnedUsdc ?? null,
    row.polymarketRealizedPnlUsdc ?? null,
    row.polymarketCashPnlUsdc ?? null,
  ];
}
