import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { jsonLogicToNestedExprGroup, type ConditionDraft, type DrawdownRuleRow, type NodeConfigFormState, type OutcomeConditionRow, type PrimitiveValueType } from '@/lib/trade-flow-config-mappers';
import type { ExpressionGroup, TradeBuilderOutcome, TradeFlowOpenPositionOption, TradeFlowOpenPositionsMeta } from '@/lib/types';
import { Database, GitBranch, Plus, Trash2, Wallet, Zap } from 'lucide-react';
import { ExpressionBuilder } from '../flow-expression-builder';
import { shortText } from './shared';
import type { NodeInspectorActions } from './types';

interface OpenPositionsSectionProps {
  openPositions: TradeFlowOpenPositionOption[];
  openPositionsMeta: TradeFlowOpenPositionsMeta | null;
  openPositionsLoading: boolean;
  openPositionApplyingKey: string | null;
  canApplyOpenPosition: (p: TradeFlowOpenPositionOption) => boolean;
  actions: NodeInspectorActions;
}

export function OpenPositionsSection({
  openPositions,
  openPositionsMeta,
  openPositionsLoading,
  openPositionApplyingKey,
  canApplyOpenPosition,
  actions,
}: OpenPositionsSectionProps) {
  return (
    <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
      <div className="flex items-center gap-1.5">
        <Wallet className="h-3.5 w-3.5 text-sky-500" />
        <p className="text-[11px] font-semibold text-slate-700">Polymarket Acik Pozisyonlar</p>
      </div>
      <p className="text-[10px] leading-relaxed text-slate-400 italic">
        Bir pozisyon secince sourceTradeId ve context alanlari otomatik dolar. Eslesen trade yoksa sistem otomatik local source trade olusturur.
      </p>
      {openPositionsMeta && (
        <div className="space-y-1 rounded-lg border border-slate-200 bg-white/90 p-2.5 text-[10px] text-slate-500">
          <p>Cuzdan: {openPositionsMeta.walletAddressUsed}</p>
          <p>Toplam pozisyon: {openPositionsMeta.count}</p>
          <p>Filtre: currentValue &gt;= ${openPositionsMeta.minCurrentValueUsd}</p>
          <p>Son guncelleme: {new Date(openPositionsMeta.fetchedAt).toLocaleString()}</p>
        </div>
      )}
      {openPositionsLoading ? (
        <p className="text-[11px] text-slate-500">Pozisyonlar yukleniyor...</p>
      ) : openPositions.length === 0 ? (
        <p className="text-[11px] text-slate-500">
          Bu cuzdanda {openPositionsMeta?.minCurrentValueUsd ?? 5} USD ve uzeri acik pozisyon gorunmuyor.
        </p>
      ) : (
        <div className="max-h-48 space-y-2 overflow-auto">
          {openPositions.map((position) => (
            <div
              key={position.positionKey}
              className="space-y-1.5 rounded-lg border border-slate-200 bg-white p-2.5 shadow-sm transition hover:border-sky-200 hover:shadow"
            >
              <p className="text-[11px] font-medium leading-snug text-slate-900">{position.marketTitle}</p>
              <div className="flex flex-wrap items-center gap-1">
                <Badge variant="secondary" className="text-[9px]">{position.outcomeLabel}</Badge>
                <Badge variant="outline" className="text-[9px]">
                  qty {Number.isFinite(Number(position.size)) ? Number(position.size).toFixed(4) : '0.0000'}
                </Badge>
              </div>
              <p className="truncate text-[10px] text-slate-400">{shortText(position.marketSlug, 52)}</p>
              <p className="text-[10px] text-slate-500">
                Eslesen trade: {position.matchedTradeId == null ? 'yok' : `#${position.matchedTradeId}`}
                <Badge variant="outline" className="ml-1 text-[9px]">{position.matchConfidence}</Badge>
              </p>
              <Button
                size="sm"
                variant="outline"
                className="mt-1 w-full border-sky-200 text-sky-700 hover:bg-sky-50"
                disabled={openPositionApplyingKey != null || !canApplyOpenPosition(position)}
                onClick={() => actions.onApplyOpenPosition(position)}
              >
                {openPositionApplyingKey === position.positionKey ? 'Uygulaniyor...' : 'Bu Pozisyonu Kullan'}
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

interface StatePatchSectionProps {
  rows: Array<{ id: string; key: string; value: string; valueType: PrimitiveValueType }>;
  actions: NodeInspectorActions;
}

export function StatePatchSection({ rows, actions }: StatePatchSectionProps) {
  return (
    <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
      <div className="flex items-center gap-1.5">
        <Database className="h-3.5 w-3.5 text-sky-500" />
        <p className="text-[11px] font-semibold text-slate-700">State Patch Alanlari</p>
      </div>
      {rows.map((row) => (
        <div key={row.id} className="grid grid-cols-3 gap-2 rounded-md border border-slate-200 p-2">
          <Input
            value={row.key}
            onChange={(e) => actions.onUpdateStatePatchRow(row.id, { key: e.target.value })}
            placeholder="key"
            className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
          />
          <Select
            value={row.valueType}
            onValueChange={(v) => actions.onUpdateStatePatchRow(row.id, { valueType: v as PrimitiveValueType })}
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
            onChange={(e) => actions.onUpdateStatePatchRow(row.id, { value: e.target.value })}
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
      <Button size="sm" variant="outline" className="w-full border-slate-300 text-slate-700" onClick={actions.onAddStatePatchRow}>
        + State Alani Ekle
      </Button>
    </div>
  );
}

interface DrawdownRulesSectionProps {
  rows: DrawdownRuleRow[];
  actions: NodeInspectorActions;
}

export function DrawdownRulesSection({ rows, actions }: DrawdownRulesSectionProps) {
  return (
    <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
      <div className="flex items-center gap-1.5">
        <Zap className="h-3.5 w-3.5 text-rose-500" />
        <p className="text-[11px] font-semibold text-slate-700">Drawdown Kurallari</p>
      </div>
      <p className="text-[10px] leading-relaxed text-slate-400 italic">
        Slug gir, outcome sec, entry fiyatini yaz. Yon sec; loss % o yone gore tetikler. Sure opsiyonel (ms).
      </p>
      <div className="space-y-2">
        {rows.map((row, index) => (
          <div key={row.id} className="space-y-1.5 rounded-md border border-slate-200 bg-white p-2.5">
            <div className="flex items-center justify-between">
              <Badge variant="secondary" className="text-[10px]">Kural #{index + 1}</Badge>
              <Button
                size="sm"
                variant="ghost"
                className="h-6 w-6 p-0 text-red-400 hover:text-red-600"
                disabled={rows.length <= 1}
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
                  onValueChange={(v) => actions.onUpdateDrawdownRule(row.id, { direction: v === 'up' ? 'up' : 'down' })}
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
                  onChange={(e) => actions.onUpdateDrawdownRule(row.id, { lossPct: e.target.value })}
                  placeholder="ör: 10"
                  className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900"
                />
              </div>
              <div className="space-y-0.5">
                <Label className="text-[10px] font-medium text-slate-600">Sure (ms, ops.)</Label>
                <Input
                  type="number"
                  value={row.durationValue}
                  onChange={(e) => actions.onUpdateDrawdownRule(row.id, { durationValue: e.target.value })}
                  placeholder="ör: 1500"
                  className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900"
                />
              </div>
            </div>
          </div>
        ))}
      </div>
      <Button size="sm" variant="outline" className="h-7 border-slate-300 px-2 text-[11px] text-slate-700" onClick={actions.onAddDrawdownRule}>
        <Plus className="mr-1 h-3 w-3" />
        Kural Ekle
      </Button>
    </div>
  );
}

interface OutcomeConditionsSectionProps {
  rows: OutcomeConditionRow[];
  marketOutcomes: TradeBuilderOutcome[];
  marketOutcomesLoading: boolean;
  actions: NodeInspectorActions;
  nodeType: string;
}

export function OutcomeConditionsSection({ rows, marketOutcomes, marketOutcomesLoading, actions, nodeType }: OutcomeConditionsSectionProps) {
  const marketOutcomeByTokenId = new Map(marketOutcomes.map((outcome) => [outcome.token_id, outcome]));

  return (
    <div className="space-y-2.5 rounded-lg border border-slate-200/80 bg-gradient-to-b from-slate-50/80 to-white p-3 shadow-sm">
      <div className="flex items-center gap-1.5">
        <Zap className="h-3.5 w-3.5 text-amber-500" />
        <p className="text-[11px] font-semibold text-slate-700">Market Outcome Kosullari</p>
      </div>
      <p className="text-[10px] leading-relaxed text-slate-400 italic">
        Outcome secimi zorunlu ve sadece marketten gelen listeden secilir. Sonrasinda kosulu (yukari/asagi) ve tetik fiyatini belirle.
      </p>

      {marketOutcomesLoading ? (
        <p className="text-[10px] text-slate-500">Outcome&apos;lar yukleniyor...</p>
      ) : marketOutcomes.length === 0 ? (
        <p className="text-[10px] text-slate-500">Market slug veya market scope secilince outcome&apos;lar otomatik yuklenecek.</p>
      ) : (
        <div className="flex flex-wrap gap-1.5">
          {marketOutcomes.map((outcome) => {
            const alreadyAdded = rows.some((r) => r.tokenId === outcome.token_id);
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
                {outcome.price != null && <span className="ml-1 text-slate-400">${outcome.price.toFixed(2)}</span>}
              </button>
            );
          })}
        </div>
      )}
      {rows.length > 0 && (
        <div className="space-y-2">
          {rows.map((row) => (
            <div key={row.id} className="space-y-1.5 rounded-md border border-slate-200 bg-white p-2.5">
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
                    onValueChange={(v) => actions.onUpdateOutcomeCondition(row.id, { triggerCondition: v === '__none__' ? '' : v })}
                  >
                    <SelectTrigger className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900" size="sm">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="__none__">Seciniz...</SelectItem>
                      <SelectItem value="cross_above">Yukari Gecerse ↑</SelectItem>
                      <SelectItem value="cross_below">Asagi Gecerse ↓</SelectItem>
                      {nodeType === 'trigger.market_price' && (
                        <SelectItem value="level_above">Ustundeyse ↑</SelectItem>
                      )}
                      {nodeType === 'trigger.market_price' && (
                        <SelectItem value="level_below">Altindaysa ↓</SelectItem>
                      )}
                    </SelectContent>
                  </Select>
                </div>
                <div className="space-y-0.5">
                  <Label className="text-[10px] font-medium text-slate-600">Tetik Fiyati (cent)</Label>
                  <Input
                    type="number"
                    value={row.triggerPriceCent}
                    onChange={(e) => actions.onUpdateOutcomeCondition(row.id, { triggerPriceCent: e.target.value })}
                    placeholder="ör: 30"
                    className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900"
                  />
                </div>
                <div className="space-y-0.5">
                  <Label className="text-[10px] font-medium text-slate-600">Tavan Fiyati (cent)</Label>
                  <Input
                    type="number"
                    value={row.maxPriceCent}
                    onChange={(e) => actions.onUpdateOutcomeCondition(row.id, { maxPriceCent: e.target.value })}
                    placeholder="opsiyonel: 90"
                    className="h-8 border-slate-300 bg-white text-[11px] font-medium text-slate-900"
                  />
                  <p className="text-[10px] leading-relaxed text-slate-400">Bos birakirsan ust limit uygulanmaz.</p>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

interface ExpressionSectionProps {
  form: NodeConfigFormState;
  actions: NodeInspectorActions;
}

export function ExpressionSection({ form, actions }: ExpressionSectionProps) {
  return (
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
              if (prev.nestedExprMode) return { ...prev, nestedExprMode: false };
              const existingConfig = JSON.parse(prev.advancedJson || '{}') as Record<string, unknown>;
              const parsed = jsonLogicToNestedExprGroup(existingConfig.expression);
              const fallback: ExpressionGroup = {
                type: 'group',
                operator: 'and',
                children: [{ type: 'leaf', leftVar: 'market_price', operator: '<=', rightValue: 50, rightType: 'number' }],
              };
              return { ...prev, nestedExprMode: true, nestedExprGroup: parsed ?? fallback };
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
            onChange={(next) => actions.onFormChange((prev) => (prev ? { ...prev, nestedExprGroup: next } : prev))}
          />
        )
      ) : (
        <>
          {!form.expressionSupported && (
            <p className="text-[10px] text-amber-400">
              Mevcut expression gelismis formatta. Form yeniden yazdiginda simple formatta kaydedilir.
            </p>
          )}
          <div className="space-y-1">
            <Label className="text-[11px] font-medium text-slate-600">Baglac</Label>
            <Select
              value={form.expressionJoin}
              onValueChange={(v) => actions.onFormChange((prev) => (prev ? { ...prev, expressionJoin: v as 'and' | 'or' } : prev))}
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
            <div key={row.id} className="grid grid-cols-2 gap-2 rounded-md border border-slate-200 p-2">
              <Input
                value={row.leftVar}
                onChange={(e) => actions.onUpdateExpressionRow(row.id, { leftVar: e.target.value })}
                placeholder="market_price"
                className="h-8 border-slate-200 bg-white text-xs text-slate-900 focus-visible:ring-sky-300"
              />
              <Select
                value={row.operator}
                onValueChange={(v) => actions.onUpdateExpressionRow(row.id, { operator: v as ConditionDraft['operator'] })}
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
                onValueChange={(v) => actions.onUpdateExpressionRow(row.id, { rightType: v as PrimitiveValueType })}
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
                onChange={(e) => actions.onUpdateExpressionRow(row.id, { rightValue: e.target.value })}
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
          <Button size="sm" variant="outline" className="w-full border-slate-300 text-slate-700" onClick={actions.onAddExpressionRow}>
            + Kosul Ekle
          </Button>
        </>
      )}
    </div>
  );
}
