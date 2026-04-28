import type {
  AutoScopeTradeAnalysisRow,
  AutoScopeTradePositionSnapshot,
  AutoScopeTradeRiskFlags,
} from '@/lib/types';
import {
  AUTO_SCOPE_CASH_PNL_CSV_HEADERS,
  autoScopeCashPnlCsvValues,
} from './auto-scope-analysis-cash-metrics';
import {
  AUTO_SCOPE_OFFICIAL_ROOT_PNL_CSV_HEADERS,
  autoScopeOfficialRootPnlCsvValues,
} from './auto-scope-official-root-pnl-csv';

function csvField(value: unknown): string {
  if (value == null) return '';
  const text = String(value);
  if (!/[",\r\n]/.test(text)) return text;
  return `"${text.replaceAll('"', '""')}"`;
}

function csvRiskFlags(flags: AutoScopeTradeRiskFlags | undefined): string {
  if (!flags) return '';
  return flags.reasons.length > 0 ? flags.reasons.join('|') : 'none';
}

function csvPositionSnapshot(snapshot: AutoScopeTradePositionSnapshot | undefined): string {
  if (!snapshot) return '';
  const formatLeg = (leg: AutoScopeTradePositionSnapshot['before']) =>
    `U=${leg.upQty};D=${leg.downQty};cost=${leg.costUsdc};floor=${leg.floorQty};floor_pnl=${leg.floorPnlUsdc}`;
  return `before:${formatLeg(snapshot.before)} after:${formatLeg(snapshot.after)}`;
}

function csvJson(value: unknown): string {
  if (value == null) return '';
  return JSON.stringify(value);
}

function forensicPayload(
  row: AutoScopeTradeAnalysisRow,
  eventType: string
): Record<string, unknown> | null {
  return (
    row.forensic?.rawEvents.find((event) => event.eventType === eventType)?.payload ??
    null
  );
}

function payloadValue(payload: Record<string, unknown> | null | undefined, path: string[]): unknown {
  let current: unknown = payload;
  for (const key of path) {
    if (!current || typeof current !== 'object' || Array.isArray(current)) return null;
    current = (current as Record<string, unknown>)[key];
  }
  return current ?? null;
}

function recordValue(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function nodeSnapshotPayload(
  row: AutoScopeTradeAnalysisRow,
  entry: Record<string, unknown> | null
): Record<string, unknown> | null {
  return recordValue(payloadValue(entry, ['node_snapshot'])) ?? row.forensic?.nodeSnapshot ?? null;
}

export function buildAutoScopeTradeAnalysisCsv(
  rows: AutoScopeTradeAnalysisRow[]
): string {
  const headers = [
    'workflow',
    'definition_id',
    'run_id',
    'root_order_id',
    'exit_order_id',
    'row_type',
    'position_state',
    'valuation_kind',
    'market_slug',
    'token_id',
    'outcome_label',
    'exit_reason',
    'entry_node_key',
    'entry_node_config_hash',
    'entry_ptb_trend',
    'entry_volume_regime',
    'entry_shadow_guard_would_block',
    'market_open_at',
    'market_end_at',
    'triggered_at',
    'buy_filled_at',
    'sell_filled_at',
    'mark_price_captured_at',
    'open_to_trigger_ms',
    'trigger_to_buy_fill_ms',
    'buy_avg_price',
    'sell_or_live_price',
    'diagnosis_code',
    'diagnosis_label',
    'entry_quality_score',
    'exit_quality_score',
    'row_qty',
    'remaining_qty_after_exit',
    'buy_notional_usdc',
    'buy_fee_usdc',
    'cost_basis_usdc',
    'sell_notional_usdc',
    'sell_fee_usdc',
    'mark_value_usdc',
    'net_value_usdc',
    'row_pnl_usdc',
    'diagnostic_pnl_usdc',
    ...AUTO_SCOPE_CASH_PNL_CSV_HEADERS,
    ...AUTO_SCOPE_OFFICIAL_ROOT_PNL_CSV_HEADERS,
    'cash_buy_usdc',
    'cash_sell_usdc',
    'cash_redeem_usdc',
    'economic_pnl_usdc',
    'pending_inventory_qty',
    'pending_inventory_value_usdc',
    'pending_redeemable_value_usdc',
    'cash_status',
    'pnl_pct',
    'required_q',
    'q_margin',
    'risk_flags',
    'submitted_best_ask',
    'submitted_estimated_avg_fill',
    'submitted_vwap_slippage',
    'submitted_target_qty',
    'submitted_estimated_notional',
    'submitted_q_final',
    'submitted_model_book_gap',
    'submitted_model_book_zone',
    'submitted_participation_credit',
    'fill_actual_price',
    'fill_actual_qty',
    'fill_actual_notional',
    'fill_slippage_vs_vwap',
    'fill_slippage_vs_best_ask',
    'fill_source',
    'if_up_pnl',
    'if_down_pnl',
    'ev_pnl',
    'worst_pnl',
    'position_before_after',
    'tp_status',
    'realized_pnl',
    'mark_pnl',
    'worst_case_pnl',
  ];
  const lines = [headers.map(csvField).join(',')];

  for (const row of rows) {
    const entry = forensicPayload(row, 'ENTRY_EVALUATED');
    lines.push(
      [
        row.definitionName ?? '',
        row.definitionId,
        row.runId,
        row.rootOrderId,
        row.exitOrderId,
        row.rowType,
        row.positionState,
        row.valuationKind,
        row.marketSlug,
        row.tokenId,
        row.outcomeLabel,
        row.exitReason,
        row.forensic?.entryNodeKey ?? null,
        row.forensic?.entryNodeConfigHash ?? null,
        payloadValue(entry, ['ptb', 'trend']),
        payloadValue(entry, ['volume', 'polymarket', 'regime']),
        payloadValue(entry, ['guard_breakdown', 'shadow_volume_guard', 'would_block']),
        row.marketOpenAt,
        row.marketEndAt,
        row.triggeredAt,
        row.buyFilledAt,
        row.sellFilledAt,
        row.markPriceCapturedAt,
        row.openToTriggerMs,
        row.triggerToBuyFillMs,
        row.buyAvgPrice,
        row.sellOrLivePrice,
        row.primaryDiagnosisCode,
        row.diagnosisLabel,
        row.entryQualityScore,
        row.exitQualityScore,
        row.rowQty,
        row.remainingQtyAfterExit,
        row.buyNotionalUsdc,
        row.buyFeeUsdc,
        row.costBasisUsdc,
        row.sellNotionalUsdc,
        row.sellFeeUsdc,
        row.markValueUsdc,
        row.netValueUsdc,
        row.rowPnlUsdc,
        row.diagnosticPnlUsdc,
        ...autoScopeCashPnlCsvValues(row),
        ...autoScopeOfficialRootPnlCsvValues(row),
        row.cashBuyUsdc,
        row.cashSellUsdc,
        row.cashRedeemUsdc,
        row.economicPnlUsdc,
        row.pendingInventoryQty,
        row.pendingInventoryValueUsdc,
        row.pendingRedeemableValueUsdc,
        row.cashStatus,
        row.pnlPct,
        row.signalQuality?.requiredQ ?? null,
        row.signalQuality?.qMargin ?? null,
        csvRiskFlags(row.riskFlags),
        row.executionTelemetry?.submittedBestAsk ?? null,
        row.executionTelemetry?.submittedEstimatedAvgFill ?? null,
        row.executionTelemetry?.submittedVwapSlippage ?? null,
        row.executionTelemetry?.submittedTargetQty ?? null,
        row.executionTelemetry?.submittedEstimatedNotional ?? null,
        row.executionTelemetry?.submittedQFinal ?? null,
        row.executionTelemetry?.submittedModelBookGap ?? null,
        row.executionTelemetry?.submittedModelBookZone ?? null,
        row.executionTelemetry?.submittedParticipationCredit ?? null,
        row.executionTelemetry?.fillActualPrice ?? null,
        row.executionTelemetry?.fillActualQty ?? null,
        row.executionTelemetry?.fillActualNotional ?? null,
        row.executionTelemetry?.fillSlippageVsVwap ?? null,
        row.executionTelemetry?.fillSlippageVsBestAsk ?? null,
        row.executionTelemetry?.fillSource ?? null,
        row.scenarioPnl?.ifUpUsdc ?? null,
        row.scenarioPnl?.ifDownUsdc ?? null,
        row.scenarioPnl?.evUsdc ?? null,
        row.scenarioPnl?.worstUsdc ?? null,
        csvPositionSnapshot(row.positionSnapshot),
        row.tpStatus?.status ?? null,
        row.scenarioPnl?.realizedPnlUsdc ?? null,
        row.scenarioPnl?.markPnlUsdc ?? null,
        row.scenarioPnl?.worstUsdc ?? null,
      ]
        .map(csvField)
        .join(',')
    );
  }

  return `${lines.join('\n')}\n`;
}

export function buildAutoScopeTradeAnalysisForensicCsv(
  rows: AutoScopeTradeAnalysisRow[]
): string {
  const headers = [
    'workflow',
    'definition_id',
    'run_id',
    'root_order_id',
    'market_slug',
    'outcome_label',
    'decision_id',
    'sl_event_id',
    'entry_node_key',
    'entry_node_type',
    'entry_node_config_hash',
    'entry_action_node_json',
    'entry_upstream_nodes_json',
    'entry_resolved_order_input_json',
    'entry_node_snapshot_json',
    'entry_decision',
    'entry_reason',
    'entry_ptb_gap_now',
    'entry_ptb_gap_slope_5s',
    'entry_ptb_trend',
    'entry_ptb_peak_last_30s',
    'entry_ptb_drawdown_from_peak',
    'entry_volume_regime',
    'entry_volume_ratio',
    'entry_shadow_would_block',
    'entry_shadow_reason',
    'entry_risk_tags',
    'entry_risk_tag_values_json',
    'entry_guard_breakdown_json',
    'entry_stop_loss_plan_json',
    'entry_config_json',
    'entry_data_freshness_json',
    'order_submitted_count',
    'order_filled_count',
    'order_expired_count',
    'order_error_count',
    'last_submit_payload_json',
    'last_fill_payload_json',
    'last_expire_payload_json',
    'last_error_payload_json',
    'sl_armed_payload_json',
    'ptb_sl_trigger_payload_json',
    'post_sl_check_10s_json',
    'post_sl_check_30s_json',
    'post_sl_market_end_json',
    'post_sl_resolution_final_json',
    'sl_classification',
    'actual_sl_pnl',
    'hold_to_resolution_pnl',
    'entry_payload_json',
    'sl_payload_json',
    'raw_events_json',
    'row_pnl_usdc',
    'diagnostic_pnl_usdc',
    ...AUTO_SCOPE_CASH_PNL_CSV_HEADERS,
    ...AUTO_SCOPE_OFFICIAL_ROOT_PNL_CSV_HEADERS,
    'cash_buy_usdc',
    'cash_sell_usdc',
    'cash_redeem_usdc',
    'economic_pnl_usdc',
    'pending_inventory_qty',
    'pending_inventory_value_usdc',
    'pending_redeemable_value_usdc',
    'cash_status',
    'exit_reason',
  ];
  const lines = [headers.map(csvField).join(',')];

  for (const row of rows) {
    const events = row.forensic?.rawEvents ?? [];
    const entry = forensicPayload(row, 'ENTRY_EVALUATED');
    const submitted = forensicPayload(row, 'ORDER_SUBMITTED');
    const filled = forensicPayload(row, 'ORDER_FILLED');
    const expired = forensicPayload(row, 'ORDER_EXPIRED');
    const errorPayload = forensicPayload(row, 'ORDER_ERROR');
    const slArmed = forensicPayload(row, 'STOP_LOSS_ARMED');
    const ptbSl = forensicPayload(row, 'PTB_STOP_LOSS_TRIGGERED');
    const postChecks = events.filter((event) => event.eventType === 'POST_SL_CHECK');
    const post10 = postChecks.find((event) => payloadValue(event.payload, ['check_after_s']) === 10)?.payload ?? null;
    const post30 = postChecks.find((event) => payloadValue(event.payload, ['check_after_s']) === 30)?.payload ?? null;
    const postEnd = forensicPayload(row, 'POST_SL_MARKET_END');
    const postFinal = forensicPayload(row, 'POST_SL_RESOLUTION_FINAL');
    const nodeSnapshot = nodeSnapshotPayload(row, entry);

    lines.push(
      [
        row.definitionName ?? '',
        row.definitionId,
        row.runId,
        row.rootOrderId,
        row.marketSlug,
        row.outcomeLabel,
        row.forensic?.rawEvents[0]?.decisionId ?? null,
        row.forensic?.rawEvents.find((event) => event.slEventId)?.slEventId ?? null,
        payloadValue(nodeSnapshot, ['node_key']) ?? payloadValue(nodeSnapshot, ['action_node', 'key']),
        payloadValue(nodeSnapshot, ['node_type']) ?? payloadValue(nodeSnapshot, ['action_node', 'type']),
        payloadValue(nodeSnapshot, ['node_config_hash']),
        csvJson(payloadValue(nodeSnapshot, ['action_node'])),
        csvJson(payloadValue(nodeSnapshot, ['upstream_nodes'])),
        csvJson(payloadValue(nodeSnapshot, ['resolved_order_input'])),
        csvJson(nodeSnapshot),
        payloadValue(entry, ['decision']),
        payloadValue(entry, ['decision_reason']),
        payloadValue(entry, ['ptb', 'gap_now']),
        payloadValue(entry, ['ptb', 'slope_5s']),
        payloadValue(entry, ['ptb', 'trend']),
        payloadValue(entry, ['ptb', 'peak_last_30s']),
        payloadValue(entry, ['ptb', 'drawdown_from_peak']),
        payloadValue(entry, ['volume', 'polymarket', 'regime']),
        payloadValue(entry, ['volume', 'polymarket', 'ratio']),
        payloadValue(entry, ['guard_breakdown', 'shadow_volume_guard', 'would_block']),
        payloadValue(entry, ['guard_breakdown', 'shadow_volume_guard', 'reason']),
        csvJson(payloadValue(entry, ['risk_tags'])),
        csvJson(payloadValue(entry, ['risk_tag_values'])),
        csvJson(payloadValue(entry, ['guard_breakdown'])),
        csvJson(payloadValue(entry, ['stop_loss_config_at_entry'])),
        csvJson(payloadValue(entry, ['config'])),
        csvJson(payloadValue(entry, ['data_freshness'])),
        events.filter((event) => event.eventType === 'ORDER_SUBMITTED').length,
        events.filter((event) => event.eventType === 'ORDER_FILLED').length,
        events.filter((event) => event.eventType === 'ORDER_EXPIRED').length,
        events.filter((event) => event.eventType === 'ORDER_ERROR').length,
        csvJson(submitted),
        csvJson(filled),
        csvJson(expired),
        csvJson(errorPayload),
        csvJson(slArmed),
        csvJson(ptbSl),
        csvJson(post10),
        csvJson(post30),
        csvJson(postEnd),
        csvJson(postFinal),
        payloadValue(postFinal, ['sl_classification']),
        payloadValue(postFinal, ['pnl_comparison', 'actual_sl_pnl']),
        payloadValue(postFinal, ['pnl_comparison', 'hold_to_resolution_pnl']),
        csvJson(entry),
        csvJson(ptbSl ?? slArmed),
        csvJson(events),
        row.rowPnlUsdc,
        row.diagnosticPnlUsdc,
        ...autoScopeCashPnlCsvValues(row),
        ...autoScopeOfficialRootPnlCsvValues(row),
        row.cashBuyUsdc,
        row.cashSellUsdc,
        row.cashRedeemUsdc,
        row.economicPnlUsdc,
        row.pendingInventoryQty,
        row.pendingInventoryValueUsdc,
        row.pendingRedeemableValueUsdc,
        row.cashStatus,
        row.exitReason,
      ]
        .map((value) => csvField(value == null ? null : String(value)))
        .join(',')
    );
  }

  return `${lines.join('\n')}\n`;
}
