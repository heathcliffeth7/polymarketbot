'use client';

import { PageShell } from '@/components/layout/page-shell';
import { ConfigEditor } from '@/components/settings/config-editor';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';

export default function SettingsPage() {
  return (
    <PageShell title="Settings">
      <Tabs defaultValue="strategy" className="space-y-4">
        <TabsList className="bg-zinc-800">
          <TabsTrigger value="strategy" className="data-[state=active]:bg-zinc-700">Strategy</TabsTrigger>
          <TabsTrigger value="risk" className="data-[state=active]:bg-zinc-700">Risk</TabsTrigger>
          <TabsTrigger value="execution" className="data-[state=active]:bg-zinc-700">Execution</TabsTrigger>
          <TabsTrigger value="bot" className="data-[state=active]:bg-zinc-700">Bot</TabsTrigger>
          <TabsTrigger value="exchange" className="data-[state=active]:bg-zinc-700">Exchange</TabsTrigger>
          <TabsTrigger value="claim" className="data-[state=active]:bg-zinc-700">Claim</TabsTrigger>
          <TabsTrigger value="telegram" className="data-[state=active]:bg-zinc-700">Telegram</TabsTrigger>
        </TabsList>

        <TabsContent value="strategy">
          <ConfigEditor
            file="strategy"
            title="Strategy Config"
            fields={[
              { key: 'dual_side_enabled', label: 'Dual Side Enabled', type: 'boolean' },
              { key: 'max_price_relax_enabled', label: 'PTB Max Price Relax', type: 'boolean' },
              { key: 'total_notional_usdc', label: 'Total Notional (USDC)', type: 'number', min: 0.01, step: 0.01 },
              { key: 'per_leg_initial_notional_usdc', label: 'Per-Leg Initial Notional (USDC)', type: 'number', min: 0.01, step: 0.01 },
              { key: 'dca_interval_sec', label: 'DCA Interval (sec)', type: 'number', min: 1 },
              { key: 'dca_step_pct', label: 'DCA Step %', type: 'number', min: 0.0001, max: 1, step: 0.0001 },
              { key: 'max_dca_levels_per_leg', label: 'Max DCA Levels / Leg', type: 'number', min: 1 },
              { key: 'leg_tp_pct', label: 'Leg TP %', type: 'number', min: 0.0001, max: 1, step: 0.0001 },
              { key: 'basket_tp_usdc', label: 'Basket TP (USDC)', type: 'number', min: 0.01, step: 0.01 },
              { key: 'basket_sl_usdc', label: 'Basket SL (USDC, negative)', type: 'number', max: -0.0001, step: 0.01 },
              { key: 'force_flatten_sec_before_close', label: 'Force Flatten Before Close (sec)', type: 'number', min: 1 },
              { key: 'entry_price', label: 'Entry Price', type: 'number', min: 0, max: 1, step: 0.01 },
              { key: 'tp_pct', label: 'Take Profit %', type: 'number', min: 0.01, step: 0.01 },
              { key: 'base_sl_pct', label: 'Base SL %', type: 'number', min: 0.01, step: 0.01 },
              { key: 'aggressive_sl_pct', label: 'Aggressive SL %', type: 'number', min: 0.01, step: 0.01 },
              { key: 'entry_window_sec', label: 'Entry Window (sec)', type: 'number', min: 1 },
              { key: 'max_hold_sec', label: 'Max Hold (sec)', type: 'number', min: 1 },
              { key: 'sl_renew_interval_ms', label: 'SL Renew Interval (ms)', type: 'number', min: 100 },
            ]}
          />
        </TabsContent>

        <TabsContent value="risk">
          <ConfigEditor
            file="risk"
            title="Risk Config"
            fields={[
              { key: 'max_daily_loss_usdc', label: 'Max Daily Loss (USDC)', type: 'number', min: 0 },
              { key: 'max_consecutive_losses', label: 'Max Consecutive Losses', type: 'number', min: 1 },
              { key: 'max_notional_per_market_usdc', label: 'Max Notional/Market (USDC)', type: 'number', min: 0.01 },
              { key: 'max_open_orders', label: 'Max Open Orders', type: 'number', min: 1 },
              { key: 'max_stale_data_ms', label: 'Max Stale Data (ms)', type: 'number', min: 100 },
              { key: 'min_balance_usdc', label: 'Min Balance (USDC)', type: 'number', min: 0, step: 0.5 },
              { key: 'kill_switch_mode', label: 'Kill Switch Mode', type: 'select', options: ['disabled', 'manual_only', 'manual_or_policy'] },
              { key: 'manual_kill_switch_active', label: 'Manual Kill Switch', type: 'boolean' },
            ]}
          />
        </TabsContent>

        <TabsContent value="execution">
          <ConfigEditor
            file="execution"
            title="Execution Config"
            fields={[
              { key: 'order_type', label: 'Order Type', type: 'select', options: ['limit', 'market'] },
              { key: 'time_in_force', label: 'Time in Force', type: 'select', options: ['GTC', 'IOC', 'FOK'] },
              { key: 'retry_count', label: 'Retry Count', type: 'number', min: 0 },
              { key: 'retry_backoff_ms', label: 'Retry Backoff (ms)', type: 'number', min: 0 },
              { key: 'reconcile_interval_ms', label: 'Reconcile Interval (ms)', type: 'number', min: 100 },
            ]}
          />
        </TabsContent>

        <TabsContent value="bot">
          <ConfigEditor
            file="bot"
            title="Bot Config"
            fields={[
              { key: 'mode', label: 'Execution Mode', type: 'select', options: ['paper', 'live'] },
              {
                key: 'market_scopes',
                label: 'Market Scopes',
                type: 'multiselect',
                options: [
                  'btc_5m_updown',
                  'btc_15m_updown',
                  'eth_5m_updown',
                  'eth_15m_updown',
                  'sol_5m_updown',
                  'sol_15m_updown',
                  'xrp_5m_updown',
                  'xrp_15m_updown',
                ],
              },
              { key: 'market_slug_override', label: 'Market Slug Override (slug or URL)', type: 'text' },
              { key: 'market_selection', label: 'Market Selection', type: 'select', options: ['latest_by_slug'] },
              { key: 'market_discovery_retry_interval_ms', label: 'Market Discovery Retry (ms)', type: 'number', min: 500, step: 100 },
              { key: 'market_discovery_timeout_sec', label: 'Market Discovery Timeout (sec, 0 = unlimited)', type: 'number', min: 0, step: 1 },
              { key: 'loop_interval_ms', label: 'Loop Interval (ms)', type: 'number', min: 100 },
            ]}
          />
        </TabsContent>

        <TabsContent value="exchange">
          <ConfigEditor
            file="exchange"
            title="Exchange Config (Encrypted Credentials)"
            fields={[
              { key: 'gamma_base_url', label: 'Gamma URL', type: 'text' },
              { key: 'clob_base_url', label: 'CLOB URL', type: 'text' },
              { key: 'clob_ws_url', label: 'CLOB WS URL', type: 'text' },
              { key: 'chain_id', label: 'Chain ID', type: 'number' },
              { key: 'ctf_exchange_address', label: 'CLOB Exchange Address', type: 'text' },
              { key: 'neg_risk_ctf_exchange_address', label: 'Neg-Risk CLOB Exchange Address', type: 'text' },
              { key: 'api_address', label: 'Wallet Address', type: 'text' },
              { key: 'api_key', label: 'POLY API Key', type: 'text' },
              { key: 'api_secret', label: 'POLY API Secret', type: 'text' },
              { key: 'api_passphrase', label: 'POLY API Passphrase', type: 'text' },
              { key: 'builder_code', label: 'Builder Code (bytes32)', type: 'text' },
              { key: 'builder_api_key', label: 'Builder API Key', type: 'text' },
              { key: 'builder_api_secret', label: 'Builder API Secret', type: 'text' },
              { key: 'builder_api_passphrase', label: 'Builder API Passphrase', type: 'text' },
              { key: 'signer_private_key', label: 'Signer Private Key', type: 'text' },
              { key: 'gnosis_safe_address', label: 'Gnosis Safe Address (Optional)', type: 'text' },
            ]}
          />
        </TabsContent>

        <TabsContent value="claim">
          <ConfigEditor
            file="claim"
            title="Claim Config"
            fields={[
              { key: 'enabled', label: 'Auto Claim Enabled', type: 'boolean' },
              { key: 'execution_mode', label: 'Execution Mode', type: 'select', options: ['direct', 'builder_relayer'] },
              { key: 'rpc_url', label: 'RPC URL', type: 'text' },
              { key: 'data_api_base_url', label: 'Data API Base URL', type: 'text' },
              { key: 'user_address', label: 'Claim Wallet Address', type: 'text' },
              { key: 'private_key', label: 'Claim Private Key', type: 'text' },
              { key: 'chain_id', label: 'Chain ID', type: 'number', min: 1 },
              { key: 'ctf_contract_address', label: 'CTF Contract Address', type: 'text' },
              { key: 'collateral_token_address', label: 'Collateral Token Address', type: 'text' },
              { key: 'discovery_interval_sec', label: 'Discovery Interval (sec)', type: 'number', min: 5 },
              { key: 'positions_page_size', label: 'Positions Page Size', type: 'number', min: 1 },
              { key: 'positions_max_pages', label: 'Positions Max Pages', type: 'number', min: 1 },
              { key: 'process_batch_size', label: 'Process Batch Size', type: 'number', min: 1 },
              { key: 'max_attempts', label: 'Max Attempts', type: 'number', min: 1 },
              { key: 'retry_backoff_ms', label: 'Retry Backoff (ms)', type: 'number', min: 1000, step: 1000 },
            ]}
          />
        </TabsContent>

        <TabsContent value="telegram">
          <ConfigEditor
            file="telegram"
            title="Telegram Config (Auto-Applies After Save)"
            fields={[
              { key: 'bot_token', label: 'Telegram Bot Token', type: 'text' },
              { key: 'chat_id', label: 'Default Telegram Chat ID (Optional)', type: 'text' },
            ]}
          />
        </TabsContent>
      </Tabs>
    </PageShell>
  );
}
