import { useMemo } from 'react';
import { useTradeBuilderOutcomes } from '@/hooks/use-trade-builder';
import type { NodeConfigFormState } from '@/lib/trade-flow-config-mappers';

interface UseMarketOutcomesArgs {
  nodeTypeDraft: string;
  nodeForm: NodeConfigFormState | null;
}

export function useMarketOutcomes({
  nodeTypeDraft,
  nodeForm,
}: UseMarketOutcomesArgs) {
  const outcomeSource = useMemo(() => {
    const marketSlug = (nodeForm?.fields.marketSlug ?? '').trim();
    const marketScope = (nodeForm?.fields.marketScope ?? '').trim();

    if (
      nodeTypeDraft === 'trigger.open_positions' ||
      nodeTypeDraft === 'trigger.position_drawdown'
    ) {
      if (!marketSlug) return null;
      return marketSlug;
    }

    if (nodeTypeDraft !== 'trigger.market_price') {
      return null;
    }

    const marketMode = (nodeForm?.fields.marketMode ?? '').trim().toLowerCase();
    if (marketMode === 'auto_scope') {
      return marketScope || null;
    }

    return marketSlug || null;
  }, [
    nodeForm?.fields.marketMode,
    nodeForm?.fields.marketScope,
    nodeForm?.fields.marketSlug,
    nodeTypeDraft,
  ]);

  const { data: outcomeData, isLoading: outcomesLoading } =
    useTradeBuilderOutcomes(outcomeSource);
  const marketOutcomes = useMemo(() => outcomeData?.data ?? [], [outcomeData?.data]);
  const marketOutcomeTokenIdSet = useMemo(
    () => new Set(marketOutcomes.map((outcome) => outcome.token_id)),
    [marketOutcomes]
  );

  return { marketOutcomes, marketOutcomeTokenIdSet, outcomesLoading };
}
