'use client';

import { useMemo, useState } from 'react';
import { PageShell } from '@/components/layout/page-shell';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { FlowEnginePanel } from '@/components/trade-builder/flow-engine-panel';
import {
  useTradeBuilderMarketSearch,
  useTradeBuilderOutcomes,
} from '@/hooks/use-trade-builder';

export default function TradeBuilderPage() {
  const [marketQuery, setMarketQuery] = useState('');
  const [selectedMarketSlug, setSelectedMarketSlug] = useState<string | null>(null);
  const [selectedOutcomeTokenId, setSelectedOutcomeTokenId] = useState<string>('');

  const { data: marketsData } = useTradeBuilderMarketSearch(marketQuery);
  const markets = useMemo(() => marketsData?.data ?? [], [marketsData?.data]);

  const effectiveMarketSlug = useMemo(() => {
    if (selectedMarketSlug && markets.some((market) => market.slug === selectedMarketSlug)) {
      return selectedMarketSlug;
    }
    return markets[0]?.slug ?? null;
  }, [markets, selectedMarketSlug]);

  const { data: outcomesData } = useTradeBuilderOutcomes(effectiveMarketSlug);
  const outcomes = useMemo(() => outcomesData?.data ?? [], [outcomesData?.data]);

  const effectiveSelectedOutcomeTokenId = useMemo(() => {
    if (
      selectedOutcomeTokenId &&
      outcomes.some((outcome) => outcome.token_id === selectedOutcomeTokenId)
    ) {
      return selectedOutcomeTokenId;
    }
    return outcomes[0]?.token_id ?? '';
  }, [outcomes, selectedOutcomeTokenId]);

  const selectedOutcome = useMemo(
    () => outcomes.find((x) => x.token_id === effectiveSelectedOutcomeTokenId) || null,
    [outcomes, effectiveSelectedOutcomeTokenId]
  );

  return (
    <PageShell title="Trade Builder">
      <div className="space-y-6">
        <Card className="border-zinc-800 bg-zinc-900">
          <CardHeader>
            <CardTitle className="text-sm font-medium text-zinc-400">Piyasa ve Sonuc Secimi</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid gap-3 md:grid-cols-3">
              <div className="space-y-2">
                <p className="text-xs text-zinc-500">Piyasa Ara</p>
                <Input
                  value={marketQuery}
                  onChange={(e) => setMarketQuery(e.target.value)}
                  placeholder="orn: draw, premier league..."
                  className="border-zinc-700 bg-zinc-800 text-zinc-200"
                />
              </div>
              <div className="space-y-2">
                <p className="text-xs text-zinc-500">Piyasa Slug</p>
                <select
                  value={effectiveMarketSlug ?? ''}
                  onChange={(e) => setSelectedMarketSlug(e.target.value || null)}
                  className="h-9 w-full rounded-md border border-zinc-700 bg-zinc-800 px-3 text-sm text-zinc-200"
                >
                  <option value="">Piyasa secin</option>
                  {markets.map((market) => (
                    <option key={market.slug} value={market.slug}>
                      {market.slug}
                    </option>
                  ))}
                </select>
              </div>
              <div className="space-y-2">
                <p className="text-xs text-zinc-500">Sonuc</p>
                <select
                  value={effectiveSelectedOutcomeTokenId}
                  onChange={(e) => {
                    setSelectedOutcomeTokenId(e.target.value);
                  }}
                  className="h-9 w-full rounded-md border border-zinc-700 bg-zinc-800 px-3 text-sm text-zinc-200"
                >
                  <option value="">Sonuc secin</option>
                  {outcomes.map((outcome) => (
                    <option key={outcome.token_id} value={outcome.token_id}>
                      {outcome.label} ({outcome.token_id.slice(0, 8)}...)
                    </option>
                  ))}
                </select>
              </div>
            </div>
            {selectedOutcome?.label && (
              <p className="text-xs text-zinc-400">
                Secilen sonuc: <span className="text-zinc-200">{selectedOutcome.label}</span>
              </p>
            )}
          </CardContent>
        </Card>

        <FlowEnginePanel
          defaultMarketSlug={effectiveMarketSlug}
          defaultOutcome={
            selectedOutcome
              ? { token_id: selectedOutcome.token_id, label: selectedOutcome.label }
              : null
          }
        />
      </div>
    </PageShell>
  );
}
