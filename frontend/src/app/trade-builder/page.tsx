'use client';

import { useEffect, useMemo, useRef, useState } from 'react';
import { PageShell } from '@/components/layout/page-shell';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { FlowEnginePanel } from '@/components/trade-builder/flow-engine-panel';
import { createTradeFlowDefinition } from '@/hooks/use-trade-flow';
import {
  cancelTradeBuilderOrder,
  cancelTradeBuilderWorkflow,
  createTradeBuilderOrder,
  createTradeBuilderWorkflow,
  patchTradeBuilderOrder,
  patchTradeBuilderWorkflow,
  useTradeBuilderOrderEvents,
  useTradeBuilderMarketSearch,
  useTradeBuilderOrders,
  useTradeBuilderOutcomes,
  useTradeBuilderWorkflowEvents,
  useTradeBuilderWorkflows,
} from '@/hooks/use-trade-builder';
import { createStarterTradeFlowGraph } from '@/lib/trade-flow-templates';
import type {
  BuyTriggerMode,
  TradeBuilderOrder,
  TradeBuilderOrderEvent,
  TradeBuilderWorkflowDetail,
  TradeBuilderWorkflowEvent,
  TradeFlowDefinition,
} from '@/lib/types';

const EVENT_FILTER_OPTIONS = [
  { value: 'all', label: 'Tüm olaylar' },
  { value: 'filled', label: 'Doldu' },
  { value: 'reprice', label: 'Yeniden fiyatlandı' },
  { value: 'submitted', label: 'Gönderildi' },
  { value: 'blocked_by_risk', label: 'Risk engeli' },
  { value: 'processing_error', label: 'İşleme hatası' },
] as const;

const WORKFLOW_STATUS_OPTIONS = [
  { value: 'all', label: 'Tüm iş akışları' },
  { value: 'armed', label: 'Hazır' },
  { value: 'running', label: 'Çalışıyor' },
  { value: 'completed', label: 'Tamamlandı' },
  { value: 'canceled', label: 'İptal edildi' },
  { value: 'expired', label: 'Süresi doldu' },
  { value: 'error', label: 'Hata' },
] as const;

const ORDER_STATUS_OPTIONS = [
  { value: 'all', label: 'Tümü' },
  { value: 'pending', label: 'Beklemede' },
  { value: 'armed', label: 'Hazır' },
  { value: 'open', label: 'Açık' },
  { value: 'partially_filled', label: 'Kısmi doldu' },
  { value: 'completed', label: 'Tamamlandı' },
  { value: 'canceled', label: 'İptal edildi' },
  { value: 'expired', label: 'Süresi doldu' },
  { value: 'blocked', label: 'Engellendi' },
  { value: 'error', label: 'Hata' },
] as const;

const SIDE_OPTIONS = [
  { value: 'buy', label: 'Al' },
  { value: 'sell', label: 'Sat' },
] as const;

const TRIGGER_OPTIONS = [
  { value: 'cross_above', label: 'Fiyat üstüne çıkınca' },
  { value: 'cross_below', label: 'Fiyat altına inince' },
] as const;

const WORKFLOW_TRIGGER_OPTIONS = [
  { value: 'none', label: 'Yok' },
  { value: 'cross_above', label: 'Fiyat üstüne çıkınca' },
  { value: 'cross_below', label: 'Fiyat altına inince' },
] as const;

const BUY_TRIGGER_MODE_OPTIONS = [
  { value: 'sell_progress_only', label: 'Sadece satış ilerlemesi' },
  { value: 'price_only', label: 'Sadece fiyat koşulu' },
  { value: 'sell_progress_and_price', label: 'Satış ilerlemesi + fiyat koşulu' },
] as const;

const FLOW_TEMPLATE_OPTIONS = [
  { value: 'starter', label: 'Starter' },
] as const;

const ORDER_KIND_LABELS: Record<string, string> = {
  immediate: 'Anlık',
  conditional: 'Koşullu',
};

const STATUS_LABELS: Record<string, string> = {
  pending: 'Beklemede',
  armed: 'Hazır',
  running: 'Çalışıyor',
  open: 'Açık',
  partially_filled: 'Kısmi doldu',
  completed: 'Tamamlandı',
  canceled: 'İptal edildi',
  canceled_requested: 'İptal talebi',
  expired: 'Süresi doldu',
  blocked: 'Engellendi',
  error: 'Hata',
  waiting_sell_progress: 'Satış ilerlemesi bekleniyor',
};

const EVENT_TYPE_LABELS: Record<string, string> = {
  created: 'Oluşturuldu',
  submitted: 'Gönderildi',
  filled: 'Doldu',
  reprice: 'Yeniden fiyatlandı',
  blocked_by_risk: 'Risk engeli',
  processing_error: 'İşleme hatası',
  canceled: 'İptal edildi',
  canceled_requested: 'İptal talebi',
  completed: 'Tamamlandı',
};

export default function TradeBuilderPage() {
  const [marketQuery, setMarketQuery] = useState('');
  const [selectedMarketSlug, setSelectedMarketSlug] = useState<string | null>(null);
  const [selectedOutcomeTokenId, setSelectedOutcomeTokenId] = useState<string>('');
  const [selectedOutcomeLabel, setSelectedOutcomeLabel] = useState<string>('');

  const [immediateSide, setImmediateSide] = useState<'buy' | 'sell'>('buy');
  const [immediateSizeUsdc, setImmediateSizeUsdc] = useState(20);
  const [immediateMinDistance, setImmediateMinDistance] = useState(1);

  const [conditionalSide, setConditionalSide] = useState<'buy' | 'sell'>('buy');
  const [triggerCondition, setTriggerCondition] = useState<'cross_above' | 'cross_below'>('cross_above');
  const [triggerPriceCent, setTriggerPriceCent] = useState(50);
  const [conditionalSizeUsdc, setConditionalSizeUsdc] = useState(20);
  const [conditionalMinDistance, setConditionalMinDistance] = useState(1);
  const [expiresAt, setExpiresAt] = useState('');
  const [maxTriggers, setMaxTriggers] = useState(3);

  const [statusFilter, setStatusFilter] = useState('');
  const [page, setPage] = useState(1);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [workflowName, setWorkflowName] = useState('');
  const [sourceTradeId, setSourceTradeId] = useState(0);
  const [sellTargetPct, setSellTargetPct] = useState(50);
  const [buyStartAfterSellProgressPct, setBuyStartAfterSellProgressPct] = useState(10);
  const [buyAllocationPct, setBuyAllocationPct] = useState(100);
  const [buyTriggerMode, setBuyTriggerMode] = useState<BuyTriggerMode>('sell_progress_and_price');
  const [workflowExpiresAt, setWorkflowExpiresAt] = useState('');

  const [wfSellMarketSlug, setWfSellMarketSlug] = useState('');
  const [wfSellTokenId, setWfSellTokenId] = useState('');
  const [wfSellOutcomeLabel, setWfSellOutcomeLabel] = useState('');
  const [wfSellSide, setWfSellSide] = useState<'buy' | 'sell'>('sell');
  const [wfSellTriggerCondition, setWfSellTriggerCondition] = useState<'none' | 'cross_above' | 'cross_below'>('cross_above');
  const [wfSellTriggerPriceCent, setWfSellTriggerPriceCent] = useState(50);
  const [wfSellMinDistance, setWfSellMinDistance] = useState(1);

  const [wfBuyMarketSlug, setWfBuyMarketSlug] = useState('');
  const [wfBuyTokenId, setWfBuyTokenId] = useState('');
  const [wfBuyOutcomeLabel, setWfBuyOutcomeLabel] = useState('');
  const [wfBuySide, setWfBuySide] = useState<'buy' | 'sell'>('buy');
  const [wfBuyTriggerCondition, setWfBuyTriggerCondition] = useState<'none' | 'cross_above' | 'cross_below'>('cross_above');
  const [wfBuyTriggerPriceCent, setWfBuyTriggerPriceCent] = useState(50);
  const [wfBuyMinDistance, setWfBuyMinDistance] = useState(1);

  const [workflowStatusFilter, setWorkflowStatusFilter] = useState('');
  const [workflowPage, setWorkflowPage] = useState(1);
  const [flowCreateName, setFlowCreateName] = useState('');
  const [flowCreateTemplate, setFlowCreateTemplate] = useState<'starter'>('starter');
  const [flowCreateBusy, setFlowCreateBusy] = useState(false);
  const [flowEngineTargetDefinitionId, setFlowEngineTargetDefinitionId] = useState<number | null>(null);
  const [flowEngineCreatedDef, setFlowEngineCreatedDef] = useState<TradeFlowDefinition | null>(null);
  const flowEngineSectionRef = useRef<HTMLDivElement | null>(null);

  const { data: marketsData } = useTradeBuilderMarketSearch(marketQuery);
  const markets = useMemo(() => marketsData?.data ?? [], [marketsData?.data]);

  const { data: outcomesData } = useTradeBuilderOutcomes(selectedMarketSlug);
  const outcomes = useMemo(() => outcomesData?.data ?? [], [outcomesData?.data]);

  const { data: ordersData, mutate } = useTradeBuilderOrders(page, 20, statusFilter || undefined);
  const { data: workflowsData, mutate: mutateWorkflows } = useTradeBuilderWorkflows(
    workflowPage,
    20,
    workflowStatusFilter || undefined
  );

  useEffect(() => {
    if (!selectedMarketSlug && markets.length > 0) {
      setSelectedMarketSlug(markets[0].slug);
    }
  }, [markets, selectedMarketSlug]);

  useEffect(() => {
    if (outcomes.length === 0) return;
    if (!selectedOutcomeTokenId || !outcomes.some((x) => x.token_id === selectedOutcomeTokenId)) {
      setSelectedOutcomeTokenId(outcomes[0].token_id);
      setSelectedOutcomeLabel(outcomes[0].label);
    }
  }, [outcomes, selectedOutcomeTokenId]);

  const selectedOutcome = useMemo(
    () => outcomes.find((x) => x.token_id === selectedOutcomeTokenId) || null,
    [outcomes, selectedOutcomeTokenId]
  );

  useEffect(() => {
    if (!selectedMarketSlug || !selectedOutcome) return;
    if (!wfSellMarketSlug) setWfSellMarketSlug(selectedMarketSlug);
    if (!wfSellTokenId) setWfSellTokenId(selectedOutcome.token_id);
    if (!wfSellOutcomeLabel) setWfSellOutcomeLabel(selectedOutcome.label);
    if (!wfBuyMarketSlug) setWfBuyMarketSlug(selectedMarketSlug);
    if (!wfBuyTokenId) setWfBuyTokenId(selectedOutcome.token_id);
    if (!wfBuyOutcomeLabel) setWfBuyOutcomeLabel(selectedOutcome.label);
  }, [
    selectedMarketSlug,
    selectedOutcome,
    wfSellMarketSlug,
    wfSellTokenId,
    wfSellOutcomeLabel,
    wfBuyMarketSlug,
    wfBuyTokenId,
    wfBuyOutcomeLabel,
  ]);

  useEffect(() => {
    if (flowEngineTargetDefinitionId == null) return;
    const timeout = window.setTimeout(() => {
      setFlowEngineTargetDefinitionId(null);
    }, 2000);
    return () => window.clearTimeout(timeout);
  }, [flowEngineTargetDefinitionId]);

  const createImmediate = async () => {
    if (!selectedMarketSlug || !selectedOutcome) {
      setError('Önce piyasa ve sonuç seçin');
      return;
    }
    if (immediateMinDistance <= 0) {
      setError('Minimum fiyat mesafesi 0\'dan büyük olmalı');
      return;
    }

    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      await createTradeBuilderOrder({
        kind: 'immediate',
        marketSlug: selectedMarketSlug,
        tokenId: selectedOutcome.token_id,
        outcomeLabel: selectedOutcome.label,
        side: immediateSide,
        sizeUsdc: immediateSizeUsdc,
        minPriceDistanceCent: immediateMinDistance,
      });
      setMessage('Anlık emir oluşturuldu');
      mutate();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Emir oluşturulamadı');
    } finally {
      setBusy(false);
    }
  };

  const createConditional = async () => {
    if (!selectedMarketSlug || !selectedOutcome) {
      setError('Önce piyasa ve sonuç seçin');
      return;
    }
    if (!expiresAt) {
      setError('Koşullu emir için bitiş zamanı zorunludur');
      return;
    }
    if (conditionalMinDistance <= 0) {
      setError('Minimum fiyat mesafesi 0\'dan büyük olmalı');
      return;
    }

    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      await createTradeBuilderOrder({
        kind: 'conditional',
        marketSlug: selectedMarketSlug,
        tokenId: selectedOutcome.token_id,
        outcomeLabel: selectedOutcome.label,
        side: conditionalSide,
        triggerCondition,
        triggerPriceCent,
        sizeUsdc: conditionalSizeUsdc,
        minPriceDistanceCent: conditionalMinDistance,
        expiresAt,
        maxTriggers,
      });
      setMessage('Koşullu emir oluşturuldu');
      mutate();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Emir oluşturulamadı');
    } finally {
      setBusy(false);
    }
  };

  const createWorkflow = async () => {
    if (sourceTradeId <= 0) {
      setError('Kaynak İşlem ID değeri 0\'dan büyük olmalı');
      return;
    }
    if (!wfSellMarketSlug || !wfSellTokenId || !wfSellOutcomeLabel) {
      setError('Satış adımı için piyasa/token/sonuç zorunludur');
      return;
    }
    if (!wfBuyMarketSlug || !wfBuyTokenId || !wfBuyOutcomeLabel) {
      setError('Alış adımı için piyasa/token/sonuç zorunludur');
      return;
    }
    if (sellTargetPct <= 0 || sellTargetPct > 100) {
      setError('Satış hedef yüzdesi (0,100] aralığında olmalı');
      return;
    }
    if (buyStartAfterSellProgressPct < 0 || buyStartAfterSellProgressPct > 100) {
      setError('Alış başlama eşiği [0,100] aralığında olmalı');
      return;
    }
    if (buyAllocationPct <= 0 || buyAllocationPct > 100) {
      setError('Alış tahsis yüzdesi (0,100] aralığında olmalı');
      return;
    }
    if (wfSellMinDistance <= 0 || wfBuyMinDistance <= 0) {
      setError('Minimum fiyat mesafesi 0\'dan büyük olmalı');
      return;
    }
    if (wfSellTriggerCondition !== 'none' && (wfSellTriggerPriceCent <= 0 || wfSellTriggerPriceCent > 100)) {
      setError('Satış tetik fiyatı (0,100] aralığında olmalı');
      return;
    }
    if (wfBuyTriggerCondition !== 'none' && (wfBuyTriggerPriceCent <= 0 || wfBuyTriggerPriceCent > 100)) {
      setError('Alış tetik fiyatı (0,100] aralığında olmalı');
      return;
    }

    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      await createTradeBuilderWorkflow({
        name: workflowName || undefined,
        sourceTradeId,
        sellTargetPct,
        buyStartAfterSellProgressPct,
        buyTriggerMode,
        buyAllocationPct,
        expiresAt: workflowExpiresAt || null,
        sellLeg: {
          marketSlug: wfSellMarketSlug,
          tokenId: wfSellTokenId,
          outcomeLabel: wfSellOutcomeLabel,
          side: wfSellSide,
          triggerCondition: wfSellTriggerCondition === 'none' ? undefined : wfSellTriggerCondition,
          triggerPriceCent: wfSellTriggerCondition === 'none' ? undefined : wfSellTriggerPriceCent,
          minPriceDistanceCent: wfSellMinDistance,
        },
        buyLeg: {
          marketSlug: wfBuyMarketSlug,
          tokenId: wfBuyTokenId,
          outcomeLabel: wfBuyOutcomeLabel,
          side: wfBuySide,
          triggerCondition: wfBuyTriggerCondition === 'none' ? undefined : wfBuyTriggerCondition,
          triggerPriceCent: wfBuyTriggerCondition === 'none' ? undefined : wfBuyTriggerPriceCent,
          minPriceDistanceCent: wfBuyMinDistance,
        },
      });
      setMessage('İş akışı otomasyonu oluşturuldu');
      mutateWorkflows();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'İş akışı oluşturulamadı');
    } finally {
      setBusy(false);
    }
  };

  const createFlowWorkflow = async () => {
    const name = flowCreateName.trim();
    if (!name) {
      setError('Workflow adı zorunlu');
      return;
    }

    setFlowCreateBusy(true);
    setError(null);
    setMessage(null);
    try {
      if (flowCreateTemplate !== 'starter') {
        throw new Error(`Desteklenmeyen workflow şablonu: ${flowCreateTemplate}`);
      }
      const templateGraph = createStarterTradeFlowGraph(selectedMarketSlug, selectedOutcome);

      const created = await createTradeFlowDefinition({
        name,
        description: null,
        graphJson: templateGraph,
      });
      const createdId = created.data.definition.id;

      setFlowCreateName('');
      setFlowEngineTargetDefinitionId(createdId);
      setFlowEngineCreatedDef(created.data.definition);
      setMessage(`Workflow oluşturuldu (#${createdId}). Flow editörüne yönlendirildi.`);

      window.requestAnimationFrame(() => {
        flowEngineSectionRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' });
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Workflow oluşturulamadı');
    } finally {
      setFlowCreateBusy(false);
    }
  };

  return (
    <PageShell title="İşlem Oluşturucu">
      <div className="space-y-6">
        <Card className="border-zinc-800 bg-zinc-900">
          <CardHeader>
            <CardTitle className="text-sm font-medium text-zinc-400">Piyasa ve Sonuç Seçimi</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid gap-3 md:grid-cols-3">
              <div className="space-y-2">
                <p className="text-xs text-zinc-500">Piyasa Ara</p>
                <Input
                  value={marketQuery}
                  onChange={(e) => setMarketQuery(e.target.value)}
                  placeholder="ör: draw, premier league..."
                  className="border-zinc-700 bg-zinc-800 text-zinc-200"
                />
                <p className="text-[11px] text-zinc-500">Piyasa adı veya slug girerek sonuç listesini daraltın.</p>
              </div>

              <div className="space-y-2">
                <p className="text-xs text-zinc-500">Piyasa Slug</p>
                <select
                  value={selectedMarketSlug ?? ''}
                  onChange={(e) => setSelectedMarketSlug(e.target.value || null)}
                  className="h-9 w-full rounded-md border border-zinc-700 bg-zinc-800 px-3 text-sm text-zinc-200"
                >
                  <option value="">Piyasa seçin</option>
                  {markets.map((market) => (
                    <option key={market.slug} value={market.slug}>
                      {market.slug}
                    </option>
                  ))}
                </select>
                <p className="text-[11px] text-zinc-500">Emirlerin çalışacağı kesin piyasa kimliğini seçersiniz.</p>
              </div>

              <div className="space-y-2">
                <p className="text-xs text-zinc-500">Sonuç</p>
                <select
                  value={selectedOutcomeTokenId}
                  onChange={(e) => {
                    const tokenId = e.target.value;
                    setSelectedOutcomeTokenId(tokenId);
                    const selected = outcomes.find((o) => o.token_id === tokenId);
                    if (selected) setSelectedOutcomeLabel(selected.label);
                  }}
                  className="h-9 w-full rounded-md border border-zinc-700 bg-zinc-800 px-3 text-sm text-zinc-200"
                >
                  <option value="">Sonuç seçin</option>
                  {outcomes.map((outcome) => (
                    <option key={outcome.token_id} value={outcome.token_id}>
                      {outcome.label} ({outcome.token_id.slice(0, 8)}...)
                    </option>
                  ))}
                </select>
                <p className="text-[11px] text-zinc-500">İşlem yapılacak sonuç token seçimini belirler.</p>
              </div>
            </div>

            {selectedOutcomeLabel && (
              <p className="text-xs text-zinc-400">
                Seçilen sonuç: <span className="text-zinc-200">{selectedOutcomeLabel}</span>
              </p>
            )}
          </CardContent>
        </Card>

        <div className="grid gap-4 lg:grid-cols-2">
          <Card className="border-zinc-800 bg-zinc-900">
            <CardHeader>
              <CardTitle className="text-sm font-medium text-zinc-400">Anlık Emir</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <p className="text-xs text-zinc-500">Bu emir, koşul beklemeden hemen işleme alınır.</p>
              <div className="grid gap-3 md:grid-cols-2">
                <FieldSelect
                  label="Yön"
                  value={immediateSide}
                  options={SIDE_OPTIONS}
                  onChange={(v) => setImmediateSide(v as 'buy' | 'sell')}
                  hint="Al: long açar, Sat: mevcut pozisyonu azaltır/kapatır."
                />
                <FieldInput
                  label="Tutar (USDC)"
                  type="number"
                  value={immediateSizeUsdc}
                  onChange={(v) => setImmediateSizeUsdc(v)}
                  hint="Her tetiklenmede kullanılacak notional tutar."
                />
                <FieldInput
                  label="Minimum Fiyat Mesafesi (cent)"
                  type="number"
                  value={immediateMinDistance}
                  onChange={(v) => setImmediateMinDistance(v)}
                  hint="Sık fiyat güncellemesini azaltmak için minimum fark."
                />
              </div>
              <Button onClick={createImmediate} disabled={busy}>Anlık Emir Oluştur</Button>
            </CardContent>
          </Card>

          <Card className="border-zinc-800 bg-zinc-900">
            <CardHeader>
              <CardTitle className="text-sm font-medium text-zinc-400">Koşullu Emir</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <p className="text-xs text-zinc-500">Fiyat koşulu oluştuğunda ve süre dolmadan emri çalıştırır.</p>
              <div className="grid gap-3 md:grid-cols-2">
                <FieldSelect
                  label="Yön"
                  value={conditionalSide}
                  options={SIDE_OPTIONS}
                  onChange={(v) => setConditionalSide(v as 'buy' | 'sell')}
                  hint="Bu yön, koşul gerçekleştiğinde gönderilecek emri belirler."
                />
                <FieldSelect
                  label="Tetik Koşulu"
                  value={triggerCondition}
                  options={TRIGGER_OPTIONS}
                  onChange={(v) => setTriggerCondition(v as 'cross_above' | 'cross_below')}
                  hint="Fiyatın hangi yönde eşiği geçmesi gerektiğini tanımlar."
                />
                <FieldInput
                  label="Tetik Fiyatı (cent 0-100)"
                  type="number"
                  value={triggerPriceCent}
                  onChange={(v) => setTriggerPriceCent(v)}
                  hint="0-100 arası cent cinsinden tetik eşiği."
                />
                <FieldInput
                  label="Tutar (USDC)"
                  type="number"
                  value={conditionalSizeUsdc}
                  onChange={(v) => setConditionalSizeUsdc(v)}
                  hint="Koşul gerçekleştiğinde kullanılacak toplam USDC."
                />
                <FieldInput
                  label="Minimum Fiyat Mesafesi (cent)"
                  type="number"
                  value={conditionalMinDistance}
                  onChange={(v) => setConditionalMinDistance(v)}
                  hint="Yeniden fiyatlamada minimum cent farkını belirler."
                />
                <FieldInput
                  label="Maksimum Tetikleme (1-20)"
                  type="number"
                  value={maxTriggers}
                  onChange={(v) => setMaxTriggers(v)}
                  hint="Aynı emrin en fazla kaç kez tetiklenebileceği."
                />
                <div className="space-y-2 md:col-span-2">
                  <p className="text-xs text-zinc-500">Bitiş Zamanı</p>
                  <Input
                    type="datetime-local"
                    value={expiresAt}
                    onChange={(e) => setExpiresAt(e.target.value)}
                    className="border-zinc-700 bg-zinc-800 text-zinc-200"
                  />
                  <p className="text-[11px] text-zinc-500">Belirlenen tarihten sonra koşul sağlansa bile emir açılmaz.</p>
                </div>
              </div>
              <Button onClick={createConditional} disabled={busy}>Koşullu Emir Oluştur</Button>
            </CardContent>
          </Card>
        </div>

        <Card className="border-zinc-800 bg-zinc-900">
          <CardHeader>
            <CardTitle className="text-sm font-medium text-zinc-400">
              İş Akışı Otomasyonu (Satış Sonrası Alış)
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <p className="text-xs text-zinc-500">
              Önce satış adımını, ardından kurala bağlı alış adımını otomatik yöneten akış oluşturur.
            </p>
            <div className="grid gap-3 md:grid-cols-3">
              <FieldText
                label="İş Akışı Adı"
                value={workflowName}
                onChange={setWorkflowName}
                placeholder="Derbi korunma"
                hint="Boş bırakılırsa sistem varsayılan bir ad atar."
              />
              <FieldInput
                label="Kaynak İşlem ID"
                type="number"
                value={sourceTradeId}
                onChange={(v) => setSourceTradeId(v)}
                hint="Bu akışın bağlı olacağı mevcut trade kaydı."
              />
              <FieldInput
                label="Satış Hedefi %"
                type="number"
                value={sellTargetPct}
                onChange={(v) => setSellTargetPct(v)}
                hint="Satış adımında hedeflenen tamamlanma oranı."
              />
              <FieldInput
                label="Alış Başlangıcı Satış %"
                type="number"
                value={buyStartAfterSellProgressPct}
                onChange={(v) => setBuyStartAfterSellProgressPct(v)}
                hint="Satış ilerlemesi bu eşiğe gelince alış adımı aktive olur."
              />
              <FieldInput
                label="Alış Tahsisi %"
                type="number"
                value={buyAllocationPct}
                onChange={(v) => setBuyAllocationPct(v)}
                hint="Alış adımı için ayrılacak pay yüzdesi."
              />
              <FieldSelect
                label="Alış Tetik Modu"
                value={buyTriggerMode}
                options={BUY_TRIGGER_MODE_OPTIONS}
                onChange={(v) => setBuyTriggerMode(v as BuyTriggerMode)}
                hint="Alış adımının hangi koşul kombinasyonuyla başlayacağını seçin."
              />
              <div className="space-y-2 md:col-span-3">
                <p className="text-xs text-zinc-500">İş Akışı Bitiş Zamanı</p>
                <Input
                  type="datetime-local"
                  value={workflowExpiresAt}
                  onChange={(e) => setWorkflowExpiresAt(e.target.value)}
                  className="border-zinc-700 bg-zinc-800 text-zinc-200"
                />
                <p className="text-[11px] text-zinc-500">Bitiş zamanından sonra akış yeni emir üretmez.</p>
              </div>
            </div>

            <div className="grid gap-4 lg:grid-cols-2">
              <Card className="border-zinc-800 bg-zinc-950/40">
                <CardHeader>
                  <CardTitle className="text-xs text-zinc-400">Satış Adımı</CardTitle>
                </CardHeader>
                <CardContent className="space-y-3">
                  <FieldText
                    label="Piyasa Slug"
                    value={wfSellMarketSlug}
                    onChange={setWfSellMarketSlug}
                    placeholder="event-or-market-slug"
                    hint="Satış emrinin gönderileceği piyasa kimliği."
                  />
                  <FieldText
                    label="Token ID"
                    value={wfSellTokenId}
                    onChange={setWfSellTokenId}
                    placeholder="clob token id"
                    hint="Doğru outcome token ID değerini girin."
                  />
                  <FieldText
                    label="Sonuç Etiketi"
                    value={wfSellOutcomeLabel}
                    onChange={setWfSellOutcomeLabel}
                    placeholder="YES/NO/Draw..."
                    hint="Panelde görünmesi için insan okunur sonuç adı."
                  />
                  <FieldSelect
                    label="Yön"
                    value={wfSellSide}
                    options={SIDE_OPTIONS}
                    onChange={(v) => setWfSellSide(v as 'buy' | 'sell')}
                    hint="Satış adımında çoğunlukla Sat seçilir."
                  />
                  <FieldSelect
                    label="Tetik"
                    value={wfSellTriggerCondition}
                    options={WORKFLOW_TRIGGER_OPTIONS}
                    onChange={(v) => setWfSellTriggerCondition(v as 'none' | 'cross_above' | 'cross_below')}
                    hint="Bu adımın fiyat koşulu ile tetiklenmesini kontrol eder."
                  />
                  {wfSellTriggerCondition !== 'none' && (
                    <FieldInput
                      label="Tetik Fiyatı (cent)"
                      type="number"
                      value={wfSellTriggerPriceCent}
                      onChange={(v) => setWfSellTriggerPriceCent(v)}
                      hint="Satış emri için fiyat eşik değeri."
                    />
                  )}
                  <FieldInput
                    label="Minimum Fiyat Mesafesi (cent)"
                    type="number"
                    value={wfSellMinDistance}
                    onChange={(v) => setWfSellMinDistance(v)}
                    hint="Emir güncelleme sıklığını sınırlayan cent farkı."
                  />
                </CardContent>
              </Card>

              <Card className="border-zinc-800 bg-zinc-950/40">
                <CardHeader>
                  <CardTitle className="text-xs text-zinc-400">Alış Adımı</CardTitle>
                </CardHeader>
                <CardContent className="space-y-3">
                  <FieldText
                    label="Piyasa Slug"
                    value={wfBuyMarketSlug}
                    onChange={setWfBuyMarketSlug}
                    placeholder="event-or-market-slug"
                    hint="Alış emrinin çalışacağı piyasa kimliği."
                  />
                  <FieldText
                    label="Token ID"
                    value={wfBuyTokenId}
                    onChange={setWfBuyTokenId}
                    placeholder="clob token id"
                    hint="Alış için hedef outcome token ID."
                  />
                  <FieldText
                    label="Sonuç Etiketi"
                    value={wfBuyOutcomeLabel}
                    onChange={setWfBuyOutcomeLabel}
                    placeholder="YES/NO/Draw..."
                    hint="Takip ekranında gösterilecek sonuç adı."
                  />
                  <FieldSelect
                    label="Yön"
                    value={wfBuySide}
                    options={SIDE_OPTIONS}
                    onChange={(v) => setWfBuySide(v as 'buy' | 'sell')}
                    hint="Alış adımında çoğunlukla Al seçilir."
                  />
                  <FieldSelect
                    label="Tetik"
                    value={wfBuyTriggerCondition}
                    options={WORKFLOW_TRIGGER_OPTIONS}
                    onChange={(v) => setWfBuyTriggerCondition(v as 'none' | 'cross_above' | 'cross_below')}
                    hint="Alış adımının fiyat koşulunu tanımlar."
                  />
                  {wfBuyTriggerCondition !== 'none' && (
                    <FieldInput
                      label="Tetik Fiyatı (cent)"
                      type="number"
                      value={wfBuyTriggerPriceCent}
                      onChange={(v) => setWfBuyTriggerPriceCent(v)}
                      hint="Alış adımı için fiyat eşik değeri."
                    />
                  )}
                  <FieldInput
                    label="Minimum Fiyat Mesafesi (cent)"
                    type="number"
                    value={wfBuyMinDistance}
                    onChange={(v) => setWfBuyMinDistance(v)}
                    hint="Aynı fiyat çevresinde gereksiz emir güncellemesini önler."
                  />
                </CardContent>
              </Card>
            </div>

            <Button onClick={createWorkflow} disabled={busy}>
              İş Akışı Otomasyonu Oluştur
            </Button>
          </CardContent>
        </Card>

        <Card className="border-zinc-800 bg-zinc-900">
          <CardHeader>
            <CardTitle className="text-sm font-medium text-zinc-400">
              Yeni Workflow Oluştur (Flow Engine)
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <p className="text-xs text-zinc-500">
              Bu bölüm `trade_flow_definitions` tablosuna yeni workflow kaydı oluşturur ve editöre yönlendirir.
            </p>
            <div className="grid gap-3 md:grid-cols-2">
              <div className="space-y-2">
                <p className="text-xs text-zinc-500">Workflow Adı</p>
                <Input
                  value={flowCreateName}
                  onChange={(e) => setFlowCreateName(e.target.value)}
                  placeholder="Örn: Yeni hedge workflow"
                  className="border-zinc-700 bg-zinc-800 text-zinc-200"
                />
              </div>
              <div className="space-y-2">
                <p className="text-xs text-zinc-500">Şablon</p>
                <select
                  value={flowCreateTemplate}
                  onChange={(e) => setFlowCreateTemplate(e.target.value as 'starter')}
                  className="h-9 w-full rounded-md border border-zinc-700 bg-zinc-800 px-3 text-sm text-zinc-200"
                >
                  {FLOW_TEMPLATE_OPTIONS.map((template) => (
                    <option key={template.value} value={template.value}>
                      {template.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <Button onClick={createFlowWorkflow} disabled={flowCreateBusy}>
              {flowCreateBusy ? 'Oluşturuluyor...' : 'Workflow Oluştur ve Editöre Git'}
            </Button>
          </CardContent>
        </Card>

        <div ref={flowEngineSectionRef}>
          <FlowEnginePanel
            defaultMarketSlug={selectedMarketSlug}
            defaultOutcome={
              selectedOutcome
                ? {
                    token_id: selectedOutcome.token_id,
                    label: selectedOutcome.label,
                  }
                : null
            }
            externalSelectDefinitionId={flowEngineTargetDefinitionId}
            externalCreatedDefinition={flowEngineCreatedDef}
          />
        </div>

        {error && <p className="text-sm text-red-400">{error}</p>}
        {message && <p className="text-sm text-emerald-400">{message}</p>}

        <Card className="border-zinc-800 bg-zinc-900">
          <CardHeader>
            <CardTitle className="text-sm font-medium text-zinc-400">Aktif İşlem Oluşturucu Emirleri</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center gap-3">
              <select
                value={statusFilter || 'all'}
                onChange={(e) => {
                  setStatusFilter(e.target.value === 'all' ? '' : e.target.value);
                  setPage(1);
                }}
                className="h-9 rounded-md border border-zinc-700 bg-zinc-800 px-3 text-sm text-zinc-200"
              >
                {ORDER_STATUS_OPTIONS.map((status) => (
                  <option key={status.value} value={status.value}>
                    {status.label}
                  </option>
                ))}
              </select>
            </div>

            <div className="space-y-3">
              {(ordersData?.data ?? []).length === 0 ? (
                <p className="text-sm text-zinc-500">Henüz işlem oluşturucu emri yok</p>
              ) : (
                (ordersData?.data ?? []).map((order) => (
                  <OrderRow
                    key={order.id}
                    order={order}
                    onUpdated={mutate}
                  />
                ))
              )}
            </div>

            {ordersData && ordersData.totalPages > 1 && (
              <div className="flex items-center justify-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  className="border-zinc-700 text-zinc-300"
                  onClick={() => setPage((p) => Math.max(1, p - 1))}
                  disabled={page === 1}
                >
                  Önceki
                </Button>
                <span className="text-sm text-zinc-400">
                  Sayfa {page} / {ordersData.totalPages}
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  className="border-zinc-700 text-zinc-300"
                  onClick={() => setPage((p) => Math.min(ordersData.totalPages, p + 1))}
                  disabled={page === ordersData.totalPages}
                >
                  Sonraki
                </Button>
              </div>
            )}
          </CardContent>
        </Card>

        <Card className="border-zinc-800 bg-zinc-900">
          <CardHeader>
            <CardTitle className="text-sm font-medium text-zinc-400">İş Akışı Otomasyonları</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center gap-3">
              <select
                value={workflowStatusFilter || 'all'}
                onChange={(e) => {
                  setWorkflowStatusFilter(e.target.value === 'all' ? '' : e.target.value);
                  setWorkflowPage(1);
                }}
                className="h-9 rounded-md border border-zinc-700 bg-zinc-800 px-3 text-sm text-zinc-200"
              >
                {WORKFLOW_STATUS_OPTIONS.map((status) => (
                  <option key={status.value} value={status.value}>
                    {status.label}
                  </option>
                ))}
              </select>
            </div>

            <div className="space-y-3">
              {(workflowsData?.data ?? []).length === 0 ? (
                <p className="text-sm text-zinc-500">Henüz iş akışı otomasyonu yok</p>
              ) : (
                (workflowsData?.data ?? []).map((item) => (
                  <WorkflowRow
                    key={item.workflow.id}
                    detail={item}
                    onUpdated={mutateWorkflows}
                  />
                ))
              )}
            </div>

            {workflowsData && workflowsData.totalPages > 1 && (
              <div className="flex items-center justify-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  className="border-zinc-700 text-zinc-300"
                  onClick={() => setWorkflowPage((p) => Math.max(1, p - 1))}
                  disabled={workflowPage === 1}
                >
                  Önceki
                </Button>
                <span className="text-sm text-zinc-400">
                  Sayfa {workflowPage} / {workflowsData.totalPages}
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  className="border-zinc-700 text-zinc-300"
                  onClick={() => setWorkflowPage((p) => Math.min(workflowsData.totalPages, p + 1))}
                  disabled={workflowPage === workflowsData.totalPages}
                >
                  Sonraki
                </Button>
              </div>
            )}
          </CardContent>
        </Card>
      </div>
    </PageShell>
  );
}

function FieldInput({
  label,
  type,
  value,
  onChange,
  hint,
}: {
  label: string;
  type: string;
  value: number;
  onChange: (v: number) => void;
  hint?: string;
}) {
  return (
    <div className="space-y-2">
      <p className="text-xs text-zinc-500">{label}</p>
      <Input
        type={type}
        value={value}
        onChange={(e) => {
          const next = parseFloat(e.target.value);
          onChange(Number.isFinite(next) ? next : 0);
        }}
        className="border-zinc-700 bg-zinc-800 text-zinc-200"
      />
      {hint && <p className="text-[11px] text-zinc-500">{hint}</p>}
    </div>
  );
}

function FieldText({
  label,
  value,
  onChange,
  placeholder,
  hint,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  hint?: string;
}) {
  return (
    <div className="space-y-2">
      <p className="text-xs text-zinc-500">{label}</p>
      <Input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="border-zinc-700 bg-zinc-800 text-zinc-200"
      />
      {hint && <p className="text-[11px] text-zinc-500">{hint}</p>}
    </div>
  );
}

type FieldSelectOption = {
  value: string;
  label: string;
};

function FieldSelect({
  label,
  value,
  options,
  onChange,
  hint,
}: {
  label: string;
  value: string;
  options: readonly FieldSelectOption[];
  onChange: (v: string) => void;
  hint?: string;
}) {
  return (
    <div className="space-y-2">
      <p className="text-xs text-zinc-500">{label}</p>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="h-9 w-full rounded-md border border-zinc-700 bg-zinc-800 px-3 text-sm text-zinc-200"
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
      {hint && <p className="text-[11px] text-zinc-500">{hint}</p>}
    </div>
  );
}

function OrderRow({ order, onUpdated }: { order: TradeBuilderOrder; onUpdated: () => void }) {
  const [minDistance, setMinDistance] = useState(order.min_price_distance_cent);
  const [maxTriggers, setMaxTriggers] = useState(order.max_triggers);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [eventsOpen, setEventsOpen] = useState(false);
  const [eventsPage, setEventsPage] = useState(1);
  const [eventTypeFilter, setEventTypeFilter] = useState('');
  const [showRawJson, setShowRawJson] = useState(false);
  const {
    data: eventsData,
    isLoading: eventsLoading,
    error: eventsLoadError,
  } = useTradeBuilderOrderEvents(
    order.id,
    eventsPage,
    25,
    eventTypeFilter || undefined,
    eventsOpen
  );

  const updateOrder = async () => {
    setSaving(true);
    setError(null);
    try {
      await patchTradeBuilderOrder(order.id, {
        minPriceDistanceCent: minDistance,
        maxTriggers,
      });
      onUpdated();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Emir güncellenemedi');
    } finally {
      setSaving(false);
    }
  };

  const cancelOrder = async () => {
    setSaving(true);
    setError(null);
    try {
      await cancelTradeBuilderOrder(order.id);
      onUpdated();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Emir iptal edilemedi');
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="rounded-lg border border-zinc-800 p-3 text-xs text-zinc-300">
      <div className="mb-2 flex flex-wrap items-center gap-3">
        <span className="font-semibold">#{order.id}</span>
        <span>{formatOrderKind(order.kind)}</span>
        <span>{formatSide(order.side)}</span>
        <span className="text-zinc-500">{formatStatus(order.status)}</span>
        <span className="text-zinc-500">{order.market_slug}</span>
        <span>{order.outcome_label}</span>
      </div>

      <div className="grid gap-2 md:grid-cols-5">
        <div>
          <p className="text-zinc-500">Tetik</p>
          <p>
            {order.trigger_condition ? formatTriggerCondition(order.trigger_condition) : '-'}{' '}
            {order.trigger_price != null ? `@ ${(order.trigger_price * 100).toFixed(2)}c` : ''}
          </p>
        </div>
        <div>
          <p className="text-zinc-500">Tutar (USDC)</p>
          <p>{order.size_usdc.toFixed(2)}</p>
        </div>
        <div>
          <p className="text-zinc-500">Tetiklenme</p>
          <p>{order.triggers_fired}/{order.max_triggers}</p>
        </div>
        <div>
          <p className="text-zinc-500">Son Fiyat</p>
          <p>{order.last_seen_price != null ? (order.last_seen_price * 100).toFixed(2) + 'c' : '-'}</p>
        </div>
        <div>
          <p className="text-zinc-500">Min. Mesafe (cent)</p>
          <Input
            type="number"
            value={minDistance}
            onChange={(e) => {
              const v = parseFloat(e.target.value);
              setMinDistance(Number.isFinite(v) ? v : 0);
            }}
            className="mt-1 h-8 border-zinc-700 bg-zinc-800 text-zinc-200"
          />
        </div>
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-2">
        <Input
          type="number"
          value={maxTriggers}
          onChange={(e) => {
            const v = parseInt(e.target.value, 10);
            setMaxTriggers(Number.isFinite(v) ? v : 1);
          }}
          className="h-8 w-24 border-zinc-700 bg-zinc-800 text-zinc-200"
        />
        <Button size="sm" disabled={saving} onClick={updateOrder}>
          {saving ? 'Kaydediliyor...' : 'Güncelle'}
        </Button>
        <Button size="sm" variant="outline" className="border-zinc-700 text-zinc-300" disabled={saving} onClick={cancelOrder}>
          İptal Et
        </Button>
        <Button
          size="sm"
          variant="outline"
          className="border-zinc-700 text-zinc-300"
          onClick={() => setEventsOpen((prev) => !prev)}
        >
          {eventsOpen ? 'Olayları Gizle' : 'Olaylar'}
        </Button>
      </div>

      {eventsOpen && (
        <div className="mt-3 rounded-lg border border-zinc-800 bg-zinc-950/40 p-3">
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <span className="text-zinc-400">Olay Geçmişi</span>
            <select
              value={eventTypeFilter || 'all'}
              onChange={(e) => {
                setEventTypeFilter(e.target.value === 'all' ? '' : e.target.value);
                setEventsPage(1);
              }}
              className="h-8 rounded-md border border-zinc-700 bg-zinc-800 px-2 text-xs text-zinc-200"
            >
              {EVENT_FILTER_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
            <Button
              size="sm"
              variant="outline"
              className="h-8 border-zinc-700 text-zinc-300"
              onClick={() => setShowRawJson((prev) => !prev)}
            >
              {showRawJson ? 'JSON Gizle' : 'JSON Göster'}
            </Button>
            {eventsData && <span className="text-zinc-500">{eventsData.total} olay</span>}
          </div>

          {eventsLoading ? (
            <p className="text-zinc-500">Olaylar yükleniyor...</p>
          ) : eventsLoadError ? (
            <p className="text-red-400">
              {eventsLoadError instanceof Error ? eventsLoadError.message : 'Olaylar yüklenemedi'}
            </p>
          ) : (eventsData?.data ?? []).length === 0 ? (
            <p className="text-zinc-500">Henüz olay yok</p>
          ) : (
            <div className="space-y-2">
              {(eventsData?.data ?? []).map((event) => (
                <div key={event.id} className="rounded-md border border-zinc-800 bg-zinc-900/60 p-2">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="rounded bg-zinc-800 px-2 py-0.5 text-[10px] uppercase tracking-wide text-zinc-200">
                      {formatEventType(event.event_type)}
                    </span>
                    <span className="text-zinc-500">{formatEventTime(event.created_at)}</span>
                  </div>
                  <p className="mt-1 text-zinc-300">{formatTradeBuilderEventSummary(event)}</p>
                  {showRawJson && (
                    <pre className="mt-2 max-h-48 overflow-auto rounded border border-zinc-800 bg-zinc-900 p-2 text-[10px] text-zinc-400">
                      {safeJsonStringify(event.payload_json)}
                    </pre>
                  )}
                </div>
              ))}
            </div>
          )}

          {eventsData && eventsData.totalPages > 1 && (
            <div className="mt-3 flex items-center justify-center gap-2">
              <Button
                variant="outline"
                size="sm"
                className="h-8 border-zinc-700 text-zinc-300"
                onClick={() => setEventsPage((p) => Math.max(1, p - 1))}
                disabled={eventsPage === 1}
              >
                Önceki
              </Button>
              <span className="text-zinc-500">
                Sayfa {eventsPage} / {eventsData.totalPages}
              </span>
              <Button
                variant="outline"
                size="sm"
                className="h-8 border-zinc-700 text-zinc-300"
                onClick={() => setEventsPage((p) => Math.min(eventsData.totalPages, p + 1))}
                disabled={eventsPage === eventsData.totalPages}
              >
                Sonraki
              </Button>
            </div>
          )}
        </div>
      )}

      {error && <p className="mt-2 text-red-400">{error}</p>}
      {order.last_error && <p className="mt-2 text-red-400">{order.last_error}</p>}
    </div>
  );
}

function WorkflowRow({
  detail,
  onUpdated,
}: {
  detail: TradeBuilderWorkflowDetail;
  onUpdated: () => void;
}) {
  const workflow = detail.workflow;
  const legs = detail.legs;
  const sellLeg = legs.find((x) => x.leg_type === 'sell') || null;
  const buyLeg = legs.find((x) => x.leg_type === 'buy') || null;

  const [buyStartPct, setBuyStartPct] = useState(workflow.buy_start_after_sell_progress_pct);
  const [buyAllocationPct, setBuyAllocationPct] = useState(workflow.buy_allocation_pct);
  const [triggerMode, setTriggerMode] = useState<BuyTriggerMode>(workflow.buy_trigger_mode);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [eventsOpen, setEventsOpen] = useState(false);
  const [eventsPage, setEventsPage] = useState(1);
  const [showRawJson, setShowRawJson] = useState(false);

  const {
    data: eventsData,
    isLoading: eventsLoading,
    error: eventsError,
  } = useTradeBuilderWorkflowEvents(workflow.id, eventsPage, 25, undefined, eventsOpen);

  const buyProgressPct = buyLeg
    ? buyLeg.target_notional_usdc > 0
      ? Math.min(100, (buyLeg.filled_notional_usdc / buyLeg.target_notional_usdc) * 100)
      : 0
    : 0;

  const updateWorkflow = async () => {
    setSaving(true);
    setError(null);
    try {
      await patchTradeBuilderWorkflow(workflow.id, {
        buyStartAfterSellProgressPct: buyStartPct,
        buyAllocationPct,
        buyTriggerMode: triggerMode,
      });
      onUpdated();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'İş akışı güncellenemedi');
    } finally {
      setSaving(false);
    }
  };

  const cancelWorkflow = async () => {
    setSaving(true);
    setError(null);
    try {
      await cancelTradeBuilderWorkflow(workflow.id);
      onUpdated();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'İş akışı iptal edilemedi');
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="rounded-lg border border-zinc-800 p-3 text-xs text-zinc-300">
      <div className="mb-2 flex flex-wrap items-center gap-3">
        <span className="font-semibold">Akış #{workflow.id}</span>
        <span>{workflow.name}</span>
        <span className="text-zinc-500">{formatStatus(workflow.status)}</span>
        <span className="text-zinc-500">kaynak işlem #{workflow.source_trade_id}</span>
      </div>

      <div className="grid gap-2 md:grid-cols-4">
        <div>
          <p className="text-zinc-500">Satış Hedefi %</p>
          <p>{workflow.sell_target_pct.toFixed(2)}</p>
        </div>
        <div>
          <p className="text-zinc-500">Alış Başlangıcı %</p>
          <Input
            type="number"
            value={buyStartPct}
            onChange={(e) => {
              const v = parseFloat(e.target.value);
              setBuyStartPct(Number.isFinite(v) ? v : 0);
            }}
            className="mt-1 h-8 border-zinc-700 bg-zinc-800 text-zinc-200"
          />
        </div>
        <div>
          <p className="text-zinc-500">Alış Tahsisi %</p>
          <Input
            type="number"
            value={buyAllocationPct}
            onChange={(e) => {
              const v = parseFloat(e.target.value);
              setBuyAllocationPct(Number.isFinite(v) ? v : 100);
            }}
            className="mt-1 h-8 border-zinc-700 bg-zinc-800 text-zinc-200"
          />
        </div>
        <div>
          <p className="text-zinc-500">Alış Tetik Modu</p>
          <select
            value={triggerMode}
            onChange={(e) => setTriggerMode(e.target.value as BuyTriggerMode)}
            className="mt-1 h-8 w-full rounded-md border border-zinc-700 bg-zinc-800 px-2 text-xs text-zinc-200"
          >
            {BUY_TRIGGER_MODE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </div>
      </div>

      <div className="mt-3 grid gap-2 md:grid-cols-2">
        <div className="rounded border border-zinc-800 bg-zinc-950/40 p-2">
          <p className="text-zinc-500">Satış Adımı</p>
          {sellLeg ? (
            <p>
              {formatSide(sellLeg.side)} {sellLeg.outcome_label} | hedef {sellLeg.target_notional_usdc.toFixed(2)} USDC | durum{' '}
              {formatStatus(sellLeg.status)}
            </p>
          ) : (
            <p>-</p>
          )}
        </div>
        <div className="rounded border border-zinc-800 bg-zinc-950/40 p-2">
          <p className="text-zinc-500">Alış Adımı</p>
          {buyLeg ? (
            <p>
              {formatSide(buyLeg.side)} {buyLeg.outcome_label} | dolan {buyLeg.filled_notional_usdc.toFixed(2)} /{' '}
              {buyLeg.target_notional_usdc.toFixed(2)} USDC ({buyProgressPct.toFixed(1)}%) | adet {buyLeg.filled_qty.toFixed(4)}
            </p>
          ) : (
            <p>-</p>
          )}
        </div>
      </div>

      <div className="mt-3 flex flex-wrap items-center gap-2">
        <Button size="sm" disabled={saving} onClick={updateWorkflow}>
          {saving ? 'Kaydediliyor...' : 'Güncelle'}
        </Button>
        <Button
          size="sm"
          variant="outline"
          className="border-zinc-700 text-zinc-300"
          disabled={saving}
          onClick={cancelWorkflow}
        >
          İptal Et
        </Button>
        <Button
          size="sm"
          variant="outline"
          className="border-zinc-700 text-zinc-300"
          onClick={() => setEventsOpen((prev) => !prev)}
        >
          {eventsOpen ? 'Olayları Gizle' : 'Olaylar'}
        </Button>
      </div>

      {eventsOpen && (
        <div className="mt-3 rounded-lg border border-zinc-800 bg-zinc-950/40 p-3">
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <span className="text-zinc-400">İş Akışı Olayları</span>
            <Button
              size="sm"
              variant="outline"
              className="h-8 border-zinc-700 text-zinc-300"
              onClick={() => setShowRawJson((prev) => !prev)}
            >
              {showRawJson ? 'JSON Gizle' : 'JSON Göster'}
            </Button>
            {eventsData && <span className="text-zinc-500">{eventsData.total} olay</span>}
          </div>

          {eventsLoading ? (
            <p className="text-zinc-500">Olaylar yükleniyor...</p>
          ) : eventsError ? (
            <p className="text-red-400">
              {eventsError instanceof Error ? eventsError.message : 'Olaylar yüklenemedi'}
            </p>
          ) : (eventsData?.data ?? []).length === 0 ? (
            <p className="text-zinc-500">Henüz olay yok</p>
          ) : (
            <div className="space-y-2">
              {(eventsData?.data ?? []).map((event) => (
                <div key={event.id} className="rounded-md border border-zinc-800 bg-zinc-900/60 p-2">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="rounded bg-zinc-800 px-2 py-0.5 text-[10px] uppercase tracking-wide text-zinc-200">
                      {formatEventType(event.event_type)}
                    </span>
                    <span className="text-zinc-500">{formatEventTime(event.created_at)}</span>
                  </div>
                  <p className="mt-1 text-zinc-300">{formatWorkflowEventSummary(event)}</p>
                  {showRawJson && (
                    <pre className="mt-2 max-h-48 overflow-auto rounded border border-zinc-800 bg-zinc-900 p-2 text-[10px] text-zinc-400">
                      {safeJsonStringify(event.payload_json)}
                    </pre>
                  )}
                </div>
              ))}
            </div>
          )}

          {eventsData && eventsData.totalPages > 1 && (
            <div className="mt-3 flex items-center justify-center gap-2">
              <Button
                variant="outline"
                size="sm"
                className="h-8 border-zinc-700 text-zinc-300"
                onClick={() => setEventsPage((p) => Math.max(1, p - 1))}
                disabled={eventsPage === 1}
              >
                Önceki
              </Button>
              <span className="text-zinc-500">
                Sayfa {eventsPage} / {eventsData.totalPages}
              </span>
              <Button
                variant="outline"
                size="sm"
                className="h-8 border-zinc-700 text-zinc-300"
                onClick={() => setEventsPage((p) => Math.min(eventsData.totalPages, p + 1))}
                disabled={eventsPage === eventsData.totalPages}
              >
                Sonraki
              </Button>
            </div>
          )}
        </div>
      )}

      {error && <p className="mt-2 text-red-400">{error}</p>}
      {workflow.last_error && <p className="mt-2 text-red-400">{workflow.last_error}</p>}
    </div>
  );
}

function formatEventTime(ts: string): string {
  const date = new Date(ts);
  if (Number.isNaN(date.getTime())) return ts;
  return date.toLocaleString();
}

function formatOrderKind(kind: string): string {
  return ORDER_KIND_LABELS[kind] || kind;
}

function formatStatus(status: string): string {
  return STATUS_LABELS[status] || status;
}

function formatSide(side: string): string {
  return side === 'buy' ? 'Al' : side === 'sell' ? 'Sat' : side;
}

function formatTriggerCondition(condition: string): string {
  if (condition === 'cross_above') return 'Fiyat üstüne çıkınca';
  if (condition === 'cross_below') return 'Fiyat altına inince';
  if (condition === 'none') return 'Yok';
  return condition;
}

function formatEventType(eventType: string): string {
  return EVENT_TYPE_LABELS[eventType] || eventType;
}

function formatTradeBuilderEventSummary(event: TradeBuilderOrderEvent): string {
  const payload = toPayloadRecord(event.payload_json);
  if (!payload) return 'İçerik detayı yok';

  const parts: string[] = [];
  const status = payloadString(payload, 'status') || payloadString(payload, 'normalized_status');
  if (status) parts.push(`durum=${formatStatus(status)}`);

  const error = payloadString(payload, 'error') || payloadString(payload, 'reject_reason');
  if (error) parts.push(`hata=${error}`);

  const exchangeOrderId =
    payloadString(payload, 'exchange_order_id') ||
    payloadString(payload, 'new_exchange_order_id') ||
    payloadString(payload, 'prev_exchange_order_id');
  if (exchangeOrderId) parts.push(`emir=${exchangeOrderId}`);

  const executionPrice = payloadNumber(payload, 'execution_price') ?? payloadNumber(payload, 'target_price');
  if (executionPrice != null) parts.push(`fiyat=${(executionPrice * 100).toFixed(2)}c`);

  const size = payloadNumber(payload, 'size') ?? payloadNumber(payload, 'filled_size');
  if (size != null) parts.push(`adet=${size.toFixed(4)}`);

  const nextStatus = payloadString(payload, 'next_status');
  if (nextStatus) parts.push(`sonraki=${formatStatus(nextStatus)}`);

  if (parts.length > 0) return parts.join(' | ');
  return 'İçerik kaydedildi';
}

function formatWorkflowEventSummary(event: TradeBuilderWorkflowEvent): string {
  const payload = toPayloadRecord(event.payload_json);
  if (!payload) return 'İçerik detayı yok';

  const parts: string[] = [];
  const status = payloadString(payload, 'status');
  if (status) parts.push(`durum=${formatStatus(status)}`);
  const leg = payloadString(payload, 'leg_type');
  if (leg) parts.push(`adım=${leg === 'sell' ? 'satış' : leg === 'buy' ? 'alış' : leg}`);
  const orderId = payloadString(payload, 'builder_order_id');
  if (orderId) parts.push(`builder_order_id=${orderId}`);
  const size =
    payloadNumber(payload, 'size_usdc') ??
    payloadNumber(payload, 'buy_filled_usdc') ??
    payloadNumber(payload, 'target_notional_usdc');
  if (size != null) parts.push(`tutar=${size.toFixed(2)} USDC`);
  const error = payloadString(payload, 'error');
  if (error) parts.push(`hata=${error}`);

  if (parts.length > 0) return parts.join(' | ');
  return 'İçerik kaydedildi';
}

function toPayloadRecord(payload: unknown): Record<string, unknown> | null {
  if (!payload || typeof payload !== 'object' || Array.isArray(payload)) {
    return null;
  }
  return payload as Record<string, unknown>;
}

function payloadString(payload: Record<string, unknown>, key: string): string | null {
  const value = payload[key];
  if (typeof value === 'string') {
    return value.trim() ? value : null;
  }
  if (typeof value === 'number' || typeof value === 'boolean') {
    return String(value);
  }
  return null;
}

function payloadNumber(payload: Record<string, unknown>, key: string): number | null {
  const value = payload[key];
  if (typeof value === 'number' && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === 'string') {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) {
      return parsed;
    }
  }
  return null;
}

function safeJsonStringify(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}
