import { useMemo, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import { Separator } from '@/components/ui/separator';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import {
  NODE_FIELD_SCHEMAS,
  isPresetBuySellPlaceOrderMarker,
  isPresetPlaceOrderMarker,
  jsonLogicToNestedExprGroup,
  type ConditionDraft,
  type DrawdownRuleRow,
  type EdgeConditionFormState,
  type NodeConfigFormState,
  type OutcomeConditionRow,
  type PrimitiveValueType,
} from '@/lib/trade-flow-config-mappers';
import type { ExpressionGroup, TradeBuilderOutcome, TradeFlowOpenPositionOption, TradeFlowOpenPositionsMeta } from '@/lib/types';
import {
  EDGE_TYPE_OPTIONS,
  NODE_FIELD_HELP_CONTENT,
  NODE_TYPE_OPTIONS,
  type FlowEdge,
} from './flow-canvas-constants';
import { normalizeDateTimeInput } from './flow-canvas-utils';
import { ExpressionBuilder } from './flow-expression-builder';
import {
  Settings2,
  Trash2,
  Plus,
  Wallet,
  GitBranch,
  Zap,
  Database,
} from 'lucide-react';

const EMPTY_SELECT_SENTINEL = '__none__';

export interface NodeInspectorActions {
  onNodeKeyChange: (key: string) => void;
  onNodeTypeChange: (type: string) => void;
  onTabChange: (tab: 'basic' | 'advanced') => void;
  onFormChange: React.Dispatch<React.SetStateAction<NodeConfigFormState | null>>;
  onUpdateField: (key: string, value: string) => void;
  onUpdateTriggerSizeRow: (index: number, value: string) => void;
  onCreateNode: () => void;
  onUpdateNode: () => void;
  onDeleteNode: () => void;
  onCreateFromAdvanced: () => void;
  onUpdateFromAdvanced: () => void;
  onApplyOpenPosition: (position: TradeFlowOpenPositionOption) => void;
  onUpdateExpressionRow: (rowId: string, patch: Partial<ConditionDraft>) => void;
  onAddExpressionRow: () => void;
  onRemoveExpressionRow: (rowId: string) => void;
  onUpdateStatePatchRow: (
    rowId: string,
    patch: Partial<{ key: string; value: string; valueType: PrimitiveValueType }>
  ) => void;
  onAddStatePatchRow: () => void;
  onRemoveStatePatchRow: (rowId: string) => void;
  onAddOutcomeCondition: (tokenId: string, outcomeLabel: string) => void;
  onRemoveOutcomeCondition: (rowId: string) => void;
  onUpdateOutcomeCondition: (rowId: string, patch: Partial<OutcomeConditionRow>) => void;
  onAddDrawdownRule: () => void;
  onRemoveDrawdownRule: (rowId: string) => void;
  onUpdateDrawdownRule: (rowId: string, patch: Partial<DrawdownRuleRow>) => void;
}

export interface EdgeInspectorActions {
  onEdgeTypeChange: (type: string) => void;
  onTabChange: (tab: 'basic' | 'advanced') => void;
  onFormChange: React.Dispatch<React.SetStateAction<EdgeConditionFormState | null>>;
  onUpdateConditionRow: (patch: Partial<ConditionDraft>) => void;
  onApplyBasic: () => void;
  onApplyAdvanced: () => void;
  onDeleteEdge: () => void;
}

interface NodeInspectorPanelProps {
  form: NodeConfigFormState;
  nodeKeyDraft: string;
  nodeTypeDraft: string;
  tab: 'basic' | 'advanced';
  openPositions: TradeFlowOpenPositionOption[];
  openPositionsMeta: TradeFlowOpenPositionsMeta | null;
  openPositionsLoading: boolean;
  openPositionApplyingKey: string | null;
  canApplyOpenPosition: (p: TradeFlowOpenPositionOption) => boolean;
  marketOutcomes: TradeBuilderOutcome[];
  marketOutcomesLoading: boolean;
  upstreamAutoScope: boolean;
  globalTelegramBotTokenMasked: string | null;
  globalTelegramChatId: string | null;
  actions: NodeInspectorActions;
}

function shortText(value: string, max = 36) {
  const trimmed = value.trim();
  if (!trimmed) return '-';
  if (trimmed.length <= max) return trimmed;
  return `${trimmed.slice(0, max)}...`;
}

export function NodeInspectorPanel({
  form,
  nodeKeyDraft,
  nodeTypeDraft,
  tab,
  openPositions,
  openPositionsMeta,
  openPositionsLoading,
  openPositionApplyingKey,
  canApplyOpenPosition,
  marketOutcomes,
  marketOutcomesLoading,
  upstreamAutoScope,
  globalTelegramBotTokenMasked,
  globalTelegramChatId,
  actions,
}: NodeInspectorPanelProps) {
  const [openFieldHelpState, setOpenFieldHelpState] = useState<{ nodeType: string; key: string } | null>(null);
  const nodeSchema = NODE_FIELD_SCHEMAS[nodeTypeDraft] || [];
  const nodeFieldHelp = NODE_FIELD_HELP_CONTENT[nodeTypeDraft] || {};
  const placeOrderSizeMode = (form.fields.sizeMode ?? '').trim().toLowerCase();
  const dualDcaBaseSizing = (form.fields.baseSizing ?? '').trim().toLowerCase();
  const triggerMarketMode = (form.fields.marketMode ?? '').trim().toLowerCase();
  const triggerRepeatMode = (form.fields.repeatMode ?? '').trim().toLowerCase();
  const triggerCycleWindowMode = (form.fields.cycleWindowMode ?? '').trim().toLowerCase();
  const placeOrderMaxTriggersRaw = Number(form.fields.maxTriggers ?? '');
  const placeOrderMaxTriggers =
    Number.isFinite(placeOrderMaxTriggersRaw) && placeOrderMaxTriggersRaw > 0
      ? Math.min(20, Math.floor(placeOrderMaxTriggersRaw))
      : 1;
  const placeOrderTriggerRows = form.triggerSizeRows || [];
  const placeOrderTriggerNumericRows = placeOrderTriggerRows.map((raw) => {
    const trimmed = raw.trim();
    if (!trimmed) return null;
    const parsed = Number(trimmed);
    return Number.isFinite(parsed) ? parsed : null;
  });
  const placeOrderTriggerSum = placeOrderTriggerNumericRows.reduce<number>(
    (sum, value) => (typeof value === 'number' ? sum + value : sum),
    0
  );
  const placeOrderTriggerRowInvalid = placeOrderTriggerNumericRows.some((value, index) => {
    const raw = placeOrderTriggerRows[index]?.trim() ?? '';
    if (!raw) return false;
    return value == null || value <= 0;
  });
  const placeOrderTriggerSumInvalid =
    nodeTypeDraft === 'action.place_order' &&
    placeOrderSizeMode === 'pct' &&
    placeOrderTriggerRows.some((row) => row.trim().length > 0) &&
    placeOrderTriggerSum > 100.000001;
  const isPresetPlaceOrder =
    nodeTypeDraft === 'action.place_order' &&
    isPresetPlaceOrderMarker(
      form.fields.presetKind,
      form.fields.refKey
    );
  const isPresetBuySellPlaceOrder =
    nodeTypeDraft === 'action.place_order' &&
    isPresetBuySellPlaceOrderMarker(
      form.fields.presetKind,
      form.fields.refKey
    );
  const placeOrderSide =
    nodeTypeDraft === 'action.place_order'
      ? (form.fields.side ?? '').toString().trim().toLowerCase()
      : '';
  const hideAutoScopePlaceOrderOutcomeFields =
    isPresetPlaceOrder && upstreamAutoScope && placeOrderSide === 'buy';
  const supportsOpenPositionPicker =
    nodeTypeDraft === 'trigger.open_positions' || nodeTypeDraft === 'action.place_order';
  const marketOutcomeByTokenId = useMemo(
    () => new Map(marketOutcomes.map((outcome) => [outcome.token_id, outcome])),
    [marketOutcomes]
  );
  const telegramLegacyBotToken = (form.fields.botToken ?? '').trim();
  const telegramGlobalBotToken = (globalTelegramBotTokenMasked ?? '').trim();
  const telegramNodeChatId = (form.fields.chatId ?? '').trim();
  const telegramGlobalChatId = (globalTelegramChatId ?? '').trim();
  const telegramBotTokenMasked =
    telegramGlobalBotToken || (telegramLegacyBotToken ? '********' : '');
  const telegramBotTokenSource =
    telegramGlobalBotToken ? 'global' : telegramLegacyBotToken ? 'legacy' : 'missing';
  const visibleNodeSchema = nodeSchema.filter((field) => {
    if (nodeTypeDraft === 'action.place_order') {
      if (field.key === 'sizePct') return placeOrderSizeMode === 'pct';
      if (field.key === 'sizeUsdc') {
        return placeOrderSizeMode !== 'pct';
      }
      if (
        isPresetPlaceOrder &&
        (field.key === 'kind' ||
          field.key === 'triggerCondition' ||
          field.key === 'triggerPrice' ||
          field.key === 'triggerPriceCent')
      ) {
        return false;
      }
      if (
        hideAutoScopePlaceOrderOutcomeFields &&
        (field.key === 'marketSlug' || field.key === 'tokenId' || field.key === 'outcomeLabel')
      ) {
        return false;
      }
      if (field.key === 'tpEnabled') {
        return placeOrderSide === 'buy';
      }
      if (field.key === 'tpPriceCent') {
        const tpEnabled = (form.fields.tpEnabled ?? '').toString().trim().toLowerCase();
        return placeOrderSide === 'buy' && tpEnabled === 'true';
      }
      if (field.key === 'slEnabled') {
        return placeOrderSide === 'buy';
      }
      if (field.key === 'slPriceCent') {
        const slEnabled = (form.fields.slEnabled ?? '').toString().trim().toLowerCase();
        return placeOrderSide === 'buy' && slEnabled === 'true';
      }
    }
    if (nodeTypeDraft === 'action.dual_dca') {
      if (field.key === 'baseShares') return dualDcaBaseSizing !== 'usdc';
      if (field.key === 'baseUsdc') return dualDcaBaseSizing === 'usdc';
    }
    if (nodeTypeDraft === 'trigger.market_price') {
      if (field.key === 'marketScope' || field.key === 'marketSelection') {
        return triggerMarketMode === 'auto_scope';
      }
      if (field.key === 'protectionMode') {
        return triggerMarketMode === 'auto_scope';
      }
      if (field.key === 'protectionPreset') {
        const protectionMode = (form.fields.protectionMode ?? '').trim().toLowerCase();
        return triggerMarketMode === 'auto_scope' && protectionMode === 'underlying_confirm';
      }
      if (field.key === 'marketSlug') {
        return triggerMarketMode !== 'auto_scope';
      }
      if (field.key === 'onceScope') {
        return triggerRepeatMode === 'once';
      }
      if (field.key === 'cycleWindowMode') {
        return triggerMarketMode === 'auto_scope';
      }
      if (field.key === 'cycleWindowSecs') {
        return triggerMarketMode === 'auto_scope' &&
          (triggerCycleWindowMode === 'first' || triggerCycleWindowMode === 'last');
      }
    }
    return true;
  });
  const openFieldHelpKey =
    openFieldHelpState?.nodeType === nodeTypeDraft &&
    visibleNodeSchema.some((field) => field.key === openFieldHelpState.key)
      ? openFieldHelpState.key
      : null;

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex items-center gap-2 pb-1">
        <Settings2 className="h-4 w-4 text-sky-500" />
        <h3 className="text-sm font-semibold text-slate-800">Node Ayarlari</h3>
      </div>
      <Separator className="mb-2" />

      <Tabs
        value={tab}
        onValueChange={(v) => actions.onTabChange(v as 'basic' | 'advanced')}
        className="flex min-h-0 flex-1 flex-col"
      >
        <TabsList className="bg-slate-100">
          <TabsTrigger value="basic">Form</TabsTrigger>
          <TabsTrigger value="advanced">Advanced</TabsTrigger>
        </TabsList>

        <div className="min-h-0 flex-1 overflow-y-auto">
          <TabsContent value="basic" className="space-y-3 pt-2">
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">Node Key</Label>
              <Input
                value={nodeKeyDraft}
                onChange={(e) => actions.onNodeKeyChange(e.target.value)}
                className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
              />
            </div>

            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">Node Type</Label>
              <Select value={nodeTypeDraft} onValueChange={(v) => actions.onNodeTypeChange(v)}>
                <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {NODE_TYPE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            {nodeTypeDraft === 'action.telegram_notify' && (
              <div className="space-y-1">
                <Label className="text-[11px] font-medium text-slate-600">Bot Token</Label>
                <Input
                  value={telegramBotTokenMasked}
                  disabled
                  placeholder="Settings -> Telegram"
                  className="h-8 border-slate-200 bg-slate-50 text-xs text-slate-500"
                />
                <p className="text-[10px] leading-relaxed text-slate-400 italic">
                  {telegramBotTokenSource === 'global'
                    ? 'Bu token merkezi Telegram ayarindan gelir ve workflow icinde tekrar saklanmaz.'
                    : telegramBotTokenSource === 'legacy'
                      ? 'Bu workflow eski inline token ile acildi. Node kaydedilince global token modeline normalize olur.'
                      : 'Global Telegram bot token henuz tanimli degil. Settings -> Telegram ekranindan ekle.'}
                </p>
              </div>
            )}

            {nodeTypeDraft === 'action.telegram_notify' && (
              <div className="space-y-1">
                <Label className="text-[11px] font-medium text-slate-600">
                  Global Chat ID (Fallback)
                </Label>
                <Input
                  value={telegramGlobalChatId}
                  disabled
                  placeholder="Settings -> Telegram"
                  className="h-8 border-slate-200 bg-slate-50 text-xs text-slate-500"
                />
                <p className="text-[10px] leading-relaxed text-slate-400 italic">
                  {telegramNodeChatId
                    ? 'Node Chat ID doluysa runtime onu kullanir. Global Chat ID sadece node bos oldugunda fallback olur.'
                    : telegramGlobalChatId
                      ? 'Node Chat ID bos. Runtime bu global Chat ID fallback degerini kullanir.'
                      : 'Global Chat ID opsiyoneldir. Burasi da bossa node icinde Chat ID doldurman gerekir.'}
                </p>
              </div>
            )}

            {visibleNodeSchema.map((field) => {
              const selectOptions = field.input === 'select'
                ? (
                  isPresetBuySellPlaceOrder && field.key === 'executionMode'
                      ? [{ label: 'market (IOC)', value: 'market' }]
                      : (field.options || [])
                )
                : [];
              const selectValue =
                isPresetBuySellPlaceOrder && field.key === 'executionMode'
                    ? 'market'
                    : (form.fields[field.key] ?? '');
              return (
                <div key={field.key} className="space-y-1">
                <div className="flex items-center gap-1">
                  <Label className="text-[11px] font-medium text-slate-600">{field.label}</Label>
                  {nodeTypeDraft === 'action.dual_dca' && nodeFieldHelp[field.key] && (
                    <button
                      type="button"
                      className="inline-flex h-4 w-4 items-center justify-center rounded-full border border-sky-300 text-sky-700 transition hover:bg-sky-100"
                      aria-label={`${field.label} alan bilgisi`}
                      aria-expanded={openFieldHelpKey === field.key}
                      aria-controls={`dual-dca-field-help-${field.key}`}
                      onClick={() =>
                        setOpenFieldHelpState((prev) =>
                          prev?.nodeType === nodeTypeDraft && prev.key === field.key
                            ? null
                            : { nodeType: nodeTypeDraft, key: field.key }
                        )
                      }
                    >
                      <span className="h-1.5 w-1.5 rounded-full bg-sky-600" />
                    </button>
                  )}
                </div>
                {field.key === 'outcomeLabel' &&
                  (nodeTypeDraft === 'trigger.open_positions' ||
                    nodeTypeDraft === 'trigger.market_price' ||
                    nodeTypeDraft === 'trigger.position_drawdown') ? (
                  <Select
                    value={(form.fields[field.key] ?? '') || EMPTY_SELECT_SENTINEL}
                    onValueChange={(v) => {
                      const label = v === EMPTY_SELECT_SENTINEL ? '' : v;
                      actions.onUpdateField(field.key, label);
                      const matched = marketOutcomes.find((o) => o.label === label);
                      if (matched) {
                        actions.onUpdateField('tokenId', matched.token_id);
                      }
                    }}
                  >
                    <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value={EMPTY_SELECT_SENTINEL}>Sec...</SelectItem>
                      {marketOutcomes.map((o) => (
                        <SelectItem key={o.token_id} value={o.label}>
                          {o.label}{o.price != null ? ` ($${o.price.toFixed(2)})` : ''}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                ) : field.input === 'select' ? (
                  <Select
                    value={selectValue || EMPTY_SELECT_SENTINEL}
                    onValueChange={(v) =>
                      actions.onUpdateField(field.key, v === EMPTY_SELECT_SENTINEL ? '' : v)
                    }
                  >
                    <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {selectOptions.map((option) => (
                        <SelectItem
                          key={option.value || EMPTY_SELECT_SENTINEL}
                          value={option.value || EMPTY_SELECT_SENTINEL}
                        >
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                ) : field.input === 'textarea' ? (
                  <textarea
                    value={form.fields[field.key] ?? ''}
                    onChange={(e) => actions.onUpdateField(field.key, e.target.value)}
                    className="min-h-20 w-full rounded-md border border-slate-200 bg-white p-2 text-[11px] text-slate-900 focus-visible:ring-sky-300"
                  />
                ) : (
                  <Input
                    type={field.input}
                    value={
                      field.input === 'datetime-local'
                        ? normalizeDateTimeInput(form.fields[field.key] ?? '')
                        : form.fields[field.key] ?? ''
                    }
                    onChange={(e) => actions.onUpdateField(field.key, e.target.value)}
                    placeholder={field.placeholder}
                    className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                  />
                )}
                {nodeTypeDraft === 'action.dual_dca' &&
                  openFieldHelpKey === field.key &&
                  nodeFieldHelp[field.key] && (
                    <div
                      id={`dual-dca-field-help-${field.key}`}
                      className="rounded-lg border border-sky-200/60 border-l-2 border-l-sky-400 bg-gradient-to-br from-sky-50 to-indigo-50/50 p-2.5 shadow-sm"
                    >
                      {/* Baslik */}
                      <p className="text-[11px] font-semibold text-slate-800">
                        {nodeFieldHelp[field.key].title}
                      </p>
                      <p className="mt-0.5 text-[10px] leading-relaxed text-slate-600">
                        {nodeFieldHelp[field.key].description}
                      </p>

                      {/* Etki */}
                      {nodeFieldHelp[field.key].effect && (
                        <div className="mt-1.5 flex items-start gap-1.5">
                          <span className="mt-px inline-block rounded bg-sky-100 px-1 py-px text-[9px] font-semibold text-sky-700 whitespace-nowrap">Etki</span>
                          <p className="text-[10px] leading-relaxed text-slate-700">{nodeFieldHelp[field.key].effect}</p>
                        </div>
                      )}

                      {/* Ornek */}
                      {nodeFieldHelp[field.key].example && (
                        <div className="mt-1.5 flex items-start gap-1.5">
                          <span className="mt-px inline-block rounded bg-emerald-100 px-1 py-px text-[9px] font-semibold text-emerald-700 whitespace-nowrap">Ornek</span>
                          <p className="text-[10px] font-mono leading-relaxed text-slate-600 bg-white/60 rounded px-1">{nodeFieldHelp[field.key].example}</p>
                        </div>
                      )}

                      {/* Dusuk/Yuksek Etki */}
                      {(nodeFieldHelp[field.key].whatHappensIfLowHigh || []).length > 0 && (
                        <div className="mt-1.5">
                          <span className="inline-block rounded bg-amber-100 px-1 py-px text-[9px] font-semibold text-amber-700">Deger Etkisi</span>
                          <div className="mt-1 grid grid-cols-1 gap-0.5">
                            {(nodeFieldHelp[field.key].whatHappensIfLowHigh || []).map((item) => (
                              <p key={item} className="text-[10px] leading-relaxed text-slate-600 pl-1 border-l border-amber-200">
                                {item}
                              </p>
                            ))}
                          </div>
                        </div>
                      )}

                      {/* Basit Ornekler */}
                      {(nodeFieldHelp[field.key].simpleExamples || []).length > 0 && (
                        <div className="mt-1.5">
                          <span className="inline-block rounded bg-violet-100 px-1 py-px text-[9px] font-semibold text-violet-700">Ornekler</span>
                          <div className="mt-1 space-y-0.5">
                            {(nodeFieldHelp[field.key].simpleExamples || []).map((simple) => (
                              <p key={simple} className="text-[10px] leading-relaxed text-slate-600 pl-1 border-l border-violet-200">
                                {simple}
                              </p>
                            ))}
                          </div>
                        </div>
                      )}

                      {/* Ipuclari */}
                      {(nodeFieldHelp[field.key].tips || []).length > 0 && (
                        <div className="mt-1.5 rounded bg-slate-100/80 px-1.5 py-1">
                          {(nodeFieldHelp[field.key].tips || []).map((tip) => (
                            <p key={tip} className="text-[10px] leading-relaxed text-slate-500">
                              ⚡ {tip}
                            </p>
                          ))}
                        </div>
                      )}
                    </div>
                  )}
                {field.help && (
                  <p className="text-[10px] leading-relaxed text-slate-400 italic">{field.help}</p>
                )}
                </div>
              );
            })}
            {isPresetPlaceOrder && (
              <p className="text-[10px] leading-relaxed text-slate-400 italic">
                Bu preset node tetik gelince calisir; node ici tetik kosulu kullanmaz. Al/Sat preset
                node&apos;lar market (IOC) modunda sabittir.
              </p>
            )}
            {isPresetPlaceOrder && upstreamAutoScope && (
              <p className="text-[10px] leading-relaxed text-slate-400 italic">
                {placeOrderSide === 'buy'
                  ? 'Buy: market/token auto-scope tetikten runtime’da cozulur; sourceTradeId yoksa backend usdc sizing ile local source trade uretebilir.'
                  : placeOrderSide === 'sell'
                    ? 'Sell: mevcut sourceTradeId veya pozisyon baglami gerekir; auto-scope tek basina yeterli degildir.'
                    : 'Auto-scope zincirinde buy runtime binding kullanabilir; sell tarafi mevcut sourceTradeId/pozisyon ister.'}
              </p>
            )}

            {nodeTypeDraft === 'action.place_order' && placeOrderMaxTriggers > 1 && (
              <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
                <div className="flex items-center gap-1.5">
                  <Zap className="h-3.5 w-3.5 text-sky-500" />
                  <p className="text-[11px] font-semibold text-slate-700">Tetik Bazli Tutar Plani</p>
                </div>
                <p className="text-[10px] leading-relaxed text-slate-400 italic">
                  Her tetik icin ayri {placeOrderSizeMode === 'pct' ? '%' : 'USDC'} degeri girebilirsin.
                </p>
                <p className="text-[10px] leading-relaxed text-slate-400 italic">
                  maxTriggers: {placeOrderMaxTriggers} (satir biterse order tamamlanir).
                </p>
                <div className="space-y-2">
                  {placeOrderTriggerRows.map((value, index) => (
                    <div key={`trigger-size-row-${index}`} className="space-y-1">
                      <Label className="text-[10px] font-medium text-slate-600">
                        Tetik #{index + 1} {placeOrderSizeMode === 'pct' ? '(%)' : '(USDC)'}
                      </Label>
                      <Input
                        type="number"
                        value={value}
                        onChange={(event) => actions.onUpdateTriggerSizeRow(index, event.target.value)}
                        placeholder={placeOrderSizeMode === 'pct' ? '25' : '10'}
                        className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                      />
                    </div>
                  ))}
                </div>
                {placeOrderSizeMode === 'pct' && (
                  <p
                    className={`text-[10px] ${
                      placeOrderTriggerSumInvalid ? 'text-red-500' : 'text-slate-500'
                    }`}
                  >
                    Toplam: {placeOrderTriggerSum.toFixed(2)}%
                  </p>
                )}
                {placeOrderTriggerRowInvalid && (
                  <p className="text-[10px] text-red-500">
                    Satir degerleri 0&apos;dan buyuk sayi olmali.
                  </p>
                )}
                {placeOrderTriggerSumInvalid && (
                  <p className="text-[10px] text-red-500">Yuzde toplami 100&apos;u gecemez.</p>
                )}
              </div>
            )}

            {!supportsOpenPositionPicker && (
              <p className="text-[10px] leading-relaxed text-slate-400 italic">
                Acik pozisyon listesi yalnizca{' '}
                <span className="text-slate-700">Tetik: Mevcut Pozisyonlar</span> veya{' '}
                <span className="text-slate-700">Aksiyon: Place Order</span> node&apos;lari
                secildiginde gorunur.
              </p>
            )}

            {supportsOpenPositionPicker && (
              <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
                <div className="flex items-center gap-1.5">
                  <Wallet className="h-3.5 w-3.5 text-sky-500" />
                  <p className="text-[11px] font-semibold text-slate-700">
                    Polymarket Acik Pozisyonlar
                  </p>
                </div>
                <p className="text-[10px] leading-relaxed text-slate-400 italic">
                  Bir pozisyon secince sourceTradeId ve context alanlari otomatik dolar. Eslesen trade
                  yoksa sistem otomatik local source trade olusturur.
                </p>
                {openPositionsMeta && (
                  <div className="space-y-1 rounded-lg border border-slate-200 bg-white/90 p-2.5 text-[10px] text-slate-500">
                    <p>Cuzdan: {openPositionsMeta.walletAddressUsed}</p>
                    <p>Toplam pozisyon: {openPositionsMeta.count}</p>
                    <p>Filtre: currentValue &gt;= ${openPositionsMeta.minCurrentValueUsd}</p>
                    <p>
                      Son guncelleme: {new Date(openPositionsMeta.fetchedAt).toLocaleString()}
                    </p>
                  </div>
                )}
                {openPositionsLoading ? (
                  <p className="text-[11px] text-slate-500">Pozisyonlar yukleniyor...</p>
                ) : openPositions.length === 0 ? (
                  <p className="text-[11px] text-slate-500">
                    Bu cuzdanda {openPositionsMeta?.minCurrentValueUsd ?? 5} USD ve uzeri acik
                    pozisyon gorunmuyor.
                  </p>
                ) : (
                  <div className="max-h-48 space-y-2 overflow-auto">
                    {openPositions.map((position) => (
                      <div
                        key={position.positionKey}
                        className="space-y-1.5 rounded-lg border border-slate-200 bg-white p-2.5 shadow-sm transition hover:border-sky-200 hover:shadow"
                      >
                        <p className="text-[11px] font-medium leading-snug text-slate-900">
                          {position.marketTitle}
                        </p>
                        <div className="flex flex-wrap items-center gap-1">
                          <Badge variant="secondary" className="text-[9px]">
                            {position.outcomeLabel}
                          </Badge>
                          <Badge variant="outline" className="text-[9px]">
                            qty {Number.isFinite(Number(position.size))
                              ? Number(position.size).toFixed(4)
                              : '0.0000'}
                          </Badge>
                        </div>
                        <p className="truncate text-[10px] text-slate-400">
                          {shortText(position.marketSlug, 52)}
                        </p>
                        <p className="text-[10px] text-slate-500">
                          Eslesen trade:{' '}
                          {position.matchedTradeId == null
                            ? 'yok'
                            : `#${position.matchedTradeId}`}
                          <Badge variant="outline" className="ml-1 text-[9px]">
                            {position.matchConfidence}
                          </Badge>
                        </p>
                        <Button
                          size="sm"
                          variant="outline"
                          className="mt-1 w-full border-sky-200 text-sky-700 hover:bg-sky-50"
                          disabled={
                            openPositionApplyingKey != null || !canApplyOpenPosition(position)
                          }
                          onClick={() => actions.onApplyOpenPosition(position)}
                        >
                          {openPositionApplyingKey === position.positionKey
                            ? 'Uygulaniyor...'
                            : 'Bu Pozisyonu Kullan'}
                        </Button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}

            {nodeTypeDraft === 'trigger.position_drawdown' && (
              <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
                <div className="flex items-center gap-1.5">
                  <Zap className="h-3.5 w-3.5 text-rose-500" />
                  <p className="text-[11px] font-semibold text-slate-700">Drawdown Kurallari</p>
                </div>
                <p className="text-[10px] leading-relaxed text-slate-400 italic">
                  Slug gir, outcome sec, entry fiyatini yaz. Yon sec; loss % o yone gore tetikler.
                  Sure opsiyonel (ms).
                </p>
                <div className="space-y-2">
                  {(form.drawdownRuleRows || []).map((row, index) => (
                    <div key={row.id} className="space-y-1.5 rounded-md border border-slate-200 bg-white p-2.5">
                      <div className="flex items-center justify-between">
                        <Badge variant="secondary" className="text-[10px]">
                          Kural #{index + 1}
                        </Badge>
                        <Button
                          size="sm"
                          variant="ghost"
                          className="h-6 w-6 p-0 text-red-400 hover:text-red-600"
                          disabled={(form.drawdownRuleRows || []).length <= 1}
                          onClick={() => actions.onRemoveDrawdownRule(row.id)}
                        >
                          <Trash2 className="h-3 w-3" />
                        </Button>
                      </div>
                      <div className="grid grid-cols-3 gap-1.5">
                        <div className="space-y-0.5">
                          <Label className="text-[10px] font-medium text-slate-600">Yon</Label>
                          <Select
                            value={row.direction === 'up' ? 'up' : 'down'}
                            onValueChange={(v) =>
                              actions.onUpdateDrawdownRule(row.id, {
                                direction: v === 'up' ? 'up' : 'down',
                              })
                            }
                          >
                            <SelectTrigger className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900" size="sm">
                              <SelectValue />
                            </SelectTrigger>
                            <SelectContent>
                              <SelectItem value="down">Asagi (down)</SelectItem>
                              <SelectItem value="up">Yukari (up)</SelectItem>
                            </SelectContent>
                          </Select>
                        </div>
                        <div className="space-y-0.5">
                          <Label className="text-[10px] font-medium text-slate-600">Loss (%)</Label>
                          <Input
                            type="number"
                            value={row.lossPct}
                            onChange={(e) =>
                              actions.onUpdateDrawdownRule(row.id, { lossPct: e.target.value })
                            }
                            placeholder="ör: 10"
                            className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900"
                          />
                        </div>
                        <div className="space-y-0.5">
                          <Label className="text-[10px] font-medium text-slate-600">
                            Sure (ms, ops.)
                          </Label>
                          <Input
                            type="number"
                            value={row.durationValue}
                            onChange={(e) =>
                              actions.onUpdateDrawdownRule(row.id, {
                                durationValue: e.target.value,
                              })
                            }
                            placeholder="ör: 1500"
                            className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900"
                          />
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
                <Button
                  size="sm"
                  variant="outline"
                  className="h-7 border-slate-300 px-2 text-[11px] text-slate-700"
                  onClick={actions.onAddDrawdownRule}
                >
                  <Plus className="mr-1 h-3 w-3" />
                  Kural Ekle
                </Button>
              </div>
            )}

            {(nodeTypeDraft === 'trigger.open_positions' || nodeTypeDraft === 'trigger.market_price') && (
              <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
                <div className="flex items-center gap-1.5">
                  <Zap className="h-3.5 w-3.5 text-amber-500" />
                  <p className="text-[11px] font-semibold text-slate-700">
                    Market Outcome Kosullari
                  </p>
                </div>
                <p className="text-[10px] leading-relaxed text-slate-400 italic">
                  Outcome secimi zorunlu ve sadece marketten gelen listeden secilir.
                  Sonrasinda kosulu (yukari/asagi) ve tetik fiyatini belirle.
                </p>

                {marketOutcomesLoading ? (
                  <p className="text-[10px] text-slate-500">Outcome&apos;lar yukleniyor...</p>
                ) : marketOutcomes.length === 0 ? (
                  <p className="text-[10px] text-slate-500">
                    Market slug girilince outcome&apos;lar otomatik yuklenecek.
                  </p>
                ) : (
                  <div className="flex flex-wrap gap-1.5">
                    {marketOutcomes.map((outcome) => {
                      const alreadyAdded = form.outcomeConditionRows.some(
                        (r) => r.tokenId === outcome.token_id
                      );
                      return (
                        <button
                          key={outcome.token_id}
                          type="button"
                          disabled={alreadyAdded}
                          className={`rounded-full border px-2.5 py-1 text-[10px] font-medium transition ${
                            alreadyAdded
                              ? 'border-sky-300 bg-sky-50 text-sky-600 cursor-default'
                              : 'border-slate-300 bg-white text-slate-700 hover:border-sky-300 hover:bg-sky-50'
                          }`}
                          onClick={() => actions.onAddOutcomeCondition(outcome.token_id, outcome.label)}
                        >
                          {outcome.label}
                          {outcome.price != null && (
                            <span className="ml-1 text-slate-400">${outcome.price.toFixed(2)}</span>
                          )}
                        </button>
                      );
                    })}
                  </div>
                )}
                {form.outcomeConditionRows.length > 0 && (
                  <div className="space-y-2">
                    {form.outcomeConditionRows.map((row) => (
                      <div
                        key={row.id}
                        className="space-y-1.5 rounded-md border border-slate-200 bg-white p-2.5"
                      >
                        <div className="flex items-center justify-between">
                          <Badge variant="secondary" className="text-[10px]">
                            {marketOutcomeByTokenId.get(row.tokenId)?.label || row.outcomeLabel || row.tokenId.slice(0, 12) || 'Kosul'}
                          </Badge>
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-6 w-6 p-0 text-red-400 hover:text-red-600"
                            onClick={() => actions.onRemoveOutcomeCondition(row.id)}
                          >
                            <Trash2 className="h-3 w-3" />
                          </Button>
                        </div>
                        <div className="grid grid-cols-1 gap-1.5 sm:grid-cols-3">
                          <div className="space-y-0.5">
                            <Label className="text-[10px] font-medium text-slate-600">Tetik Kosulu</Label>
                            <Select
                              value={row.triggerCondition || '__none__'}
                              onValueChange={(v) =>
                                actions.onUpdateOutcomeCondition(row.id, {
                                  triggerCondition: v === '__none__' ? '' : v,
                                })
                              }
                            >
                              <SelectTrigger className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900" size="sm">
                                <SelectValue />
                              </SelectTrigger>
                              <SelectContent>
                                <SelectItem value="__none__">Seciniz...</SelectItem>
                                <SelectItem value="cross_above">Yukari Gecerse ↑</SelectItem>
                                <SelectItem value="cross_below">Asagi Gecerse ↓</SelectItem>
                              </SelectContent>
                            </Select>
                          </div>
                          <div className="space-y-0.5">
                            <Label className="text-[10px] font-medium text-slate-600">Tetik Fiyati (cent)</Label>
                            <Input
                              type="number"
                              value={row.triggerPriceCent}
                              onChange={(e) =>
                                actions.onUpdateOutcomeCondition(row.id, {
                                  triggerPriceCent: e.target.value,
                                })
                              }
                              placeholder="ör: 30"
                              className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900"
                            />
                          </div>
                          <div className="space-y-0.5">
                            <Label className="text-[10px] font-medium text-slate-600">Tavan Fiyati (cent)</Label>
                            <Input
                              type="number"
                              value={row.maxPriceCent}
                              onChange={(e) =>
                                actions.onUpdateOutcomeCondition(row.id, {
                                  maxPriceCent: e.target.value,
                                })
                              }
                              placeholder="opsiyonel: 90"
                              className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900"
                            />
                            <p className="text-[10px] leading-relaxed text-slate-400">
                              Bos birakirsan ust limit uygulanmaz.
                            </p>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}

            {(nodeTypeDraft === 'logic.if' || nodeTypeDraft === 'logic.switch') && (
              <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-1.5">
                    <GitBranch className="h-3.5 w-3.5 text-sky-500" />
                    <p className="text-[11px] font-semibold text-slate-700">Kosul Builder</p>
                  </div>
                  <button
                    type="button"
                    className="rounded-md border border-slate-300 bg-white px-2 py-0.5 text-[10px] text-slate-600 hover:bg-slate-100"
                    onClick={() => {
                      actions.onFormChange((prev) => {
                        if (!prev) return prev;
                        if (prev.nestedExprMode) {
                          return { ...prev, nestedExprMode: false };
                        }
                        const existingConfig = JSON.parse(prev.advancedJson || '{}') as Record<
                          string,
                          unknown
                        >;
                        const parsed = jsonLogicToNestedExprGroup(existingConfig.expression);
                        const fallback: ExpressionGroup = {
                          type: 'group',
                          operator: 'and',
                          children: [
                            {
                              type: 'leaf',
                              leftVar: 'market_price',
                              operator: '<=',
                              rightValue: 50,
                              rightType: 'number',
                            },
                          ],
                        };
                        return {
                          ...prev,
                          nestedExprMode: true,
                          nestedExprGroup: parsed ?? fallback,
                        };
                      });
                    }}
                  >
                    {form.nestedExprMode ? 'Basit Mod' : 'Gelismis Ifade'}
                  </button>
                </div>

                {form.nestedExprMode ? (
                  form.nestedExprGroup && (
                    <ExpressionBuilder
                      value={form.nestedExprGroup}
                      onChange={(next) =>
                        actions.onFormChange((prev) =>
                          prev ? { ...prev, nestedExprGroup: next } : prev
                        )
                      }
                    />
                  )
                ) : (
                  <>
                    {!form.expressionSupported && (
                      <p className="text-[10px] text-amber-400">
                        Mevcut expression gelismis formatta. Form yeniden yazdiginda simple formatta
                        kaydedilir.
                      </p>
                    )}
                    <div className="space-y-1">
                      <Label className="text-[11px] font-medium text-slate-600">Baglac</Label>
                      <Select
                        value={form.expressionJoin}
                        onValueChange={(v) =>
                          actions.onFormChange((prev) =>
                            prev ? { ...prev, expressionJoin: v as 'and' | 'or' } : prev
                          )
                        }
                      >
                        <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="and">AND</SelectItem>
                          <SelectItem value="or">OR</SelectItem>
                        </SelectContent>
                      </Select>
                    </div>

                    {form.expressionRows.map((row) => (
                      <div
                        key={row.id}
                        className="grid grid-cols-2 gap-2 rounded-md border border-slate-200 p-2"
                      >
                        <Input
                          value={row.leftVar}
                          onChange={(e) =>
                            actions.onUpdateExpressionRow(row.id, { leftVar: e.target.value })
                          }
                          placeholder="market_price"
                          className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                        />
                        <Select
                          value={row.operator}
                          onValueChange={(v) =>
                            actions.onUpdateExpressionRow(row.id, {
                              operator: v as ConditionDraft['operator'],
                            })
                          }
                        >
                          <SelectTrigger className="h-8 border-slate-200 bg-white text-xs text-slate-900" size="sm">
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            <SelectItem value=">">&gt;</SelectItem>
                            <SelectItem value=">=">&gt;=</SelectItem>
                            <SelectItem value="<">&lt;</SelectItem>
                            <SelectItem value="<=">&lt;=</SelectItem>
                            <SelectItem value="==">==</SelectItem>
                            <SelectItem value="!=">!=</SelectItem>
                          </SelectContent>
                        </Select>
                        <Select
                          value={row.rightType}
                          onValueChange={(v) =>
                            actions.onUpdateExpressionRow(row.id, {
                              rightType: v as PrimitiveValueType,
                            })
                          }
                        >
                          <SelectTrigger className="h-8 border-slate-200 bg-white text-xs text-slate-900" size="sm">
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            <SelectItem value="number">number</SelectItem>
                            <SelectItem value="string">string</SelectItem>
                            <SelectItem value="boolean">boolean</SelectItem>
                          </SelectContent>
                        </Select>
                        <Input
                          value={row.rightValue}
                          onChange={(e) =>
                            actions.onUpdateExpressionRow(row.id, { rightValue: e.target.value })
                          }
                          placeholder="50"
                          className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                        />
                        <div className="col-span-2 flex justify-end">
                          <Button
                            size="sm"
                            variant="outline"
                            className="border-red-200 text-red-600 hover:bg-red-50 hover:text-red-700"
                            onClick={() => actions.onRemoveExpressionRow(row.id)}
                          >
                            <Trash2 className="mr-1 h-3.5 w-3.5" /> Sil
                          </Button>
                        </div>
                      </div>
                    ))}
                    <Button
                      size="sm"
                      variant="outline"
                      className="w-full border-slate-300 text-slate-700"
                      onClick={actions.onAddExpressionRow}
                    >
                      + Kosul Ekle
                    </Button>
                  </>
                )}
              </div>
            )}

            {nodeTypeDraft === 'action.set_state' && (
              <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
                <div className="flex items-center gap-1.5">
                  <Database className="h-3.5 w-3.5 text-sky-500" />
                  <p className="text-[11px] font-semibold text-slate-700">State Patch Alanlari</p>
                </div>
                {form.statePatchRows.map((row) => (
                  <div
                    key={row.id}
                    className="grid grid-cols-3 gap-2 rounded-md border border-slate-200 p-2"
                  >
                    <Input
                      value={row.key}
                      onChange={(e) =>
                        actions.onUpdateStatePatchRow(row.id, { key: e.target.value })
                      }
                      placeholder="key"
                      className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                    />
                    <Select
                      value={row.valueType}
                      onValueChange={(v) =>
                        actions.onUpdateStatePatchRow(row.id, {
                          valueType: v as PrimitiveValueType,
                        })
                      }
                    >
                      <SelectTrigger className="h-8 border-slate-200 bg-white text-xs text-slate-900" size="sm">
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="string">string</SelectItem>
                        <SelectItem value="number">number</SelectItem>
                        <SelectItem value="boolean">boolean</SelectItem>
                      </SelectContent>
                    </Select>
                    <Input
                      value={row.value}
                      onChange={(e) =>
                        actions.onUpdateStatePatchRow(row.id, { value: e.target.value })
                      }
                      placeholder="value"
                      className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                    />
                    <div className="col-span-3 flex justify-end">
                      <Button
                        size="sm"
                        variant="outline"
                        className="border-red-200 text-red-600 hover:bg-red-50 hover:text-red-700"
                        onClick={() => actions.onRemoveStatePatchRow(row.id)}
                      >
                        <Trash2 className="mr-1 h-3.5 w-3.5" /> Sil
                      </Button>
                    </div>
                  </div>
                ))}
                <Button
                  size="sm"
                  variant="outline"
                  className="w-full border-slate-300 text-slate-700"
                  onClick={actions.onAddStatePatchRow}
                >
                  + State Alani Ekle
                </Button>
              </div>
            )}

            <p className="text-[10px] leading-relaxed text-slate-400 italic">
              Yeni node icin <span className="text-slate-700">Node Ekle</span>, secili node icin{' '}
              <span className="text-slate-700">Node Guncelle</span> kullan.
            </p>
          </TabsContent>

          <TabsContent value="advanced" className="space-y-2 pt-2">
            <p className="text-[11px] text-amber-400">
              Gelismis mod JSON icindir. Yanlis JSON flow dogrulamasini bozabilir.
            </p>
            <textarea
              value={form.advancedJson}
              onChange={(e) =>
                actions.onFormChange((prev) =>
                  prev ? { ...prev, advancedJson: e.target.value } : prev
                )
              }
              className="min-h-60 w-full rounded-md border border-slate-200 bg-white p-2 text-[11px] text-slate-900 focus-visible:ring-sky-300"
            />
            <p className="text-[10px] leading-relaxed text-slate-400 italic">
              JSON ile yeni node ekleyebilir veya secili node&apos;u guncelleyebilirsin.
            </p>
          </TabsContent>
        </div>
      </Tabs>

      <div className="shrink-0 border-t bg-white py-2 flex gap-2">
        {tab === 'basic' ? (
          <>
            <Button size="sm" className="flex-1" onClick={actions.onCreateNode}>
              <Plus className="mr-1 h-3.5 w-3.5" /> Node Ekle
            </Button>
            <Button
              size="sm"
              variant="secondary"
              className="flex-1"
              onClick={actions.onUpdateNode}
            >
              Node Guncelle
            </Button>
          </>
        ) : (
          <>
            <Button size="sm" className="flex-1" onClick={actions.onCreateFromAdvanced}>
              <Plus className="mr-1 h-3.5 w-3.5" /> JSON ile Ekle
            </Button>
            <Button
              size="sm"
              variant="secondary"
              className="flex-1"
              onClick={actions.onUpdateFromAdvanced}
            >
              JSON ile Guncelle
            </Button>
          </>
        )}
        <Button
          size="sm"
          variant="outline"
          className="border-red-200 text-red-600 hover:bg-red-50 hover:text-red-700"
          onClick={actions.onDeleteNode}
        >
          <Trash2 className="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>
  );
}

interface EdgeInspectorPanelProps {
  edge: FlowEdge;
  form: EdgeConditionFormState;
  edgeTypeDraft: string;
  tab: 'basic' | 'advanced';
  actions: EdgeInspectorActions;
}

export function EdgeInspectorPanel({
  edge,
  form,
  edgeTypeDraft,
  tab,
  actions,
}: EdgeInspectorPanelProps) {
  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-2 pb-1">
        <GitBranch className="h-4 w-4 text-sky-500" />
        <h3 className="text-sm font-semibold text-slate-800">Edge Ayarlari</h3>
      </div>
      <p className="text-[11px] text-slate-500">
        {edge.source} &rarr; {edge.target}
      </p>
      <Separator className="my-2" />

      <Tabs
        value={tab}
        onValueChange={(v) => actions.onTabChange(v as 'basic' | 'advanced')}
        className="flex min-h-0 flex-1 flex-col"
      >
        <TabsList className="bg-slate-100">
          <TabsTrigger value="basic">Form</TabsTrigger>
          <TabsTrigger value="advanced">Advanced</TabsTrigger>
        </TabsList>

        <div className="min-h-0 flex-1 overflow-y-auto">
          <TabsContent value="basic" className="space-y-3 pt-2">
            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">Edge Type</Label>
              <Select value={edgeTypeDraft} onValueChange={(v) => actions.onEdgeTypeChange(v)}>
                <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {EDGE_TYPE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-1">
              <Label className="text-[11px] font-medium text-slate-600">Kosul Kullan</Label>
              <Select
                value={form.enabled ? 'yes' : 'no'}
                onValueChange={(v) =>
                  actions.onFormChange((prev) =>
                    prev ? { ...prev, enabled: v === 'yes' } : prev
                  )
                }
              >
                <SelectTrigger className="h-8 w-full border-slate-200 bg-white text-xs text-slate-900" size="sm">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="no">Hayir</SelectItem>
                  <SelectItem value="yes">Evet</SelectItem>
                </SelectContent>
              </Select>
            </div>

            {form.enabled && (
              <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
                {!form.conditionSupported && (
                  <p className="text-[10px] text-amber-400">
                    Mevcut condition gelismis formatta. Form ile kaydedince simple condition
                    formatina doner.
                  </p>
                )}
                <Input
                  value={form.conditionRow.leftVar}
                  onChange={(e) => actions.onUpdateConditionRow({ leftVar: e.target.value })}
                  placeholder="market_price"
                  className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                />
                <div className="grid grid-cols-3 gap-2">
                  <Select
                    value={form.conditionRow.operator}
                    onValueChange={(v) =>
                      actions.onUpdateConditionRow({
                        operator: v as ConditionDraft['operator'],
                      })
                    }
                  >
                    <SelectTrigger className="h-8 border-slate-200 bg-white text-xs text-slate-900" size="sm">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value=">">&gt;</SelectItem>
                      <SelectItem value=">=">&gt;=</SelectItem>
                      <SelectItem value="<">&lt;</SelectItem>
                      <SelectItem value="<=">&lt;=</SelectItem>
                      <SelectItem value="==">==</SelectItem>
                      <SelectItem value="!=">!=</SelectItem>
                    </SelectContent>
                  </Select>
                  <Select
                    value={form.conditionRow.rightType}
                    onValueChange={(v) =>
                      actions.onUpdateConditionRow({
                        rightType: v as PrimitiveValueType,
                      })
                    }
                  >
                    <SelectTrigger className="h-8 border-slate-200 bg-white text-xs text-slate-900" size="sm">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="number">number</SelectItem>
                      <SelectItem value="string">string</SelectItem>
                      <SelectItem value="boolean">boolean</SelectItem>
                    </SelectContent>
                  </Select>
                  <Input
                    value={form.conditionRow.rightValue}
                    onChange={(e) => actions.onUpdateConditionRow({ rightValue: e.target.value })}
                    className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
                  />
                </div>
              </div>
            )}
          </TabsContent>

          <TabsContent value="advanced" className="space-y-2 pt-2">
            <p className="text-[11px] text-amber-400">Gelismis mod condition JSON icindir.</p>
            <textarea
              value={form.advancedJson}
              onChange={(e) =>
                actions.onFormChange((prev) =>
                  prev ? { ...prev, advancedJson: e.target.value } : prev
                )
              }
              className="min-h-48 w-full rounded-md border border-slate-200 bg-white p-2 text-[11px] text-slate-900 focus-visible:ring-sky-300"
            />
          </TabsContent>
        </div>
      </Tabs>

      <Separator className="mt-2" />
      <div className="flex gap-2 pt-2">
        {tab === 'basic' ? (
          <Button size="sm" className="flex-1" onClick={actions.onApplyBasic}>
            Edge Uygula
          </Button>
        ) : (
          <Button size="sm" className="flex-1" onClick={actions.onApplyAdvanced}>
            JSON Uygula
          </Button>
        )}
        <Button
          size="sm"
          variant="outline"
          className="border-red-200 text-red-600 hover:bg-red-50 hover:text-red-700"
          onClick={actions.onDeleteEdge}
        >
          <Trash2 className="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>
  );
}
