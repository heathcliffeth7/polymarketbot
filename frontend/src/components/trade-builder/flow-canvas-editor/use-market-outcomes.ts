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
  const outcomeMarketSlug = useMemo(() => {
    const marketSlug = (nodeForm?.fields.marketSlug ?? '').trim();
    if (!marketSlug) return null;

    if (
      nodeTypeDraft === 'trigger.open_positions' ||
      nodeTypeDraft === 'trigger.position_drawdown'
    ) {
      return marketSlug;
    }

    if (nodeTypeDraft !== 'trigger.market_price') {
      return null;
    }

    const marketMode = (nodeForm?.fields.marketMode ?? '').trim().toLowerCase();
    return marketMode === 'auto_scope' ? null : marketSlug;
  }, [nodeForm?.fields.marketMode, nodeForm?.fields.marketSlug, nodeTypeDraft]);

  const { data: outcomeData, isLoading: outcomesLoading } =
    useTradeBuilderOutcomes(outcomeMarketSlug);
  const marketOutcomes = useMemo(() => outcomeData?.data ?? [], [outcomeData?.data]);
  const marketOutcomeTokenIdSet = useMemo(
    () => new Set(marketOutcomes.map((outcome) => outcome.token_id)),
    [marketOutcomes]
  );

  return { marketOutcomes, marketOutcomeTokenIdSet, outcomesLoading };
}
