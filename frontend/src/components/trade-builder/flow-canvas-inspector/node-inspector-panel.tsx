import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
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
} from '@/lib/trade-flow-config-mappers';
import {
  NODE_FIELD_HELP_CONTENT,
  NODE_TYPE_OPTIONS,
} from '../flow-canvas-constants';
import { normalizeDateTimeInput } from '../flow-canvas-utils';
import { Settings2, Trash2, Plus, Zap } from 'lucide-react';
import { EMPTY_SELECT_SENTINEL } from './shared';
import {
  DrawdownRulesSection,
  ExpressionSection,
  OpenPositionsSection,
  OutcomeConditionsSection,
  StatePatchSection,
} from './sections';
import type { NodeInspectorPanelProps } from './types';

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
  userTelegramBotTokenMasked,
  userTelegramDefaultChatId,
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
  const telegramLegacyBotToken = (form.fields.botToken ?? '').trim();
  const telegramUserBotToken = (userTelegramBotTokenMasked ?? '').trim();
  const telegramNodeChatId = (form.fields.chatId ?? '').trim();
  const telegramUserDefaultChatId = (userTelegramDefaultChatId ?? '').trim();
  const telegramBotTokenMasked = telegramUserBotToken;
  const telegramBotTokenSource = telegramUserBotToken
    ? 'user'
    : telegramLegacyBotToken
      ? 'legacy_ignored'
      : 'missing';
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
                  {telegramBotTokenSource === 'user'
                    ? 'Bu token mevcut kullanicinin Telegram ayarindan gelir ve workflow icinde tekrar saklanmaz.'
                    : telegramBotTokenSource === 'legacy_ignored'
                      ? 'Bu workflow eski inline token ile acildi, fakat artik kullanilmaz. Settings -> Telegram ekranindan mevcut kullanici tokenini kaydet.'
                      : 'Telegram bot token henuz tanimli degil. Settings -> Telegram ekranindan ekle.'}
                </p>
              </div>
            )}

            {nodeTypeDraft === 'action.telegram_notify' && (
              <div className="space-y-1">
                <Label className="text-[11px] font-medium text-slate-600">
                  Default Chat ID (Fallback)
                </Label>
                <Input
                  value={telegramUserDefaultChatId}
                  disabled
                  placeholder="Settings -> Telegram"
                  className="h-8 border-slate-200 bg-slate-50 text-xs text-slate-500"
                />
                <p className="text-[10px] leading-relaxed text-slate-400 italic">
                  {telegramNodeChatId
                    ? 'Node Chat ID doluysa runtime onu kullanir. Varsayilan Chat ID sadece node bos oldugunda fallback olur.'
                    : telegramUserDefaultChatId
                      ? 'Node Chat ID bos. Runtime bu kullanicinin varsayilan Chat ID degerini kullanir.'
                      : 'Varsayilan Chat ID opsiyoneldir. Burasi da bossa node icinde Chat ID doldurman gerekir.'}
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
              <OpenPositionsSection
                openPositions={openPositions}
                openPositionsMeta={openPositionsMeta}
                openPositionsLoading={openPositionsLoading}
                openPositionApplyingKey={openPositionApplyingKey}
                canApplyOpenPosition={canApplyOpenPosition}
                actions={actions}
              />
            )}

            {nodeTypeDraft === 'trigger.position_drawdown' && (
              <DrawdownRulesSection rows={form.drawdownRuleRows || []} actions={actions} />
            )}

            {(nodeTypeDraft === 'trigger.open_positions' ||
              nodeTypeDraft === 'trigger.market_price') && (
              <OutcomeConditionsSection
                rows={form.outcomeConditionRows}
                marketOutcomes={marketOutcomes}
                marketOutcomesLoading={marketOutcomesLoading}
                actions={actions}
              />
            )}

            {(nodeTypeDraft === 'logic.if' || nodeTypeDraft === 'logic.switch') && (
              <ExpressionSection form={form} actions={actions} />
            )}

            {nodeTypeDraft === 'action.set_state' && (
              <StatePatchSection rows={form.statePatchRows} actions={actions} />
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
