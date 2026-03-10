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
  const outcomeMarketSlug =
    nodeTypeDraft === 'trigger.open_positions' ||
    nodeTypeDraft === 'trigger.market_price' ||
    nodeTypeDraft === 'trigger.position_drawdown'
      ? (nodeForm?.fields.marketSlug ?? '').trim() ||
        (nodeForm?.fields.marketScope ?? '').trim() ||
        null
      : null;

  const { data: outcomeData, isLoading: outcomesLoading } =
    useTradeBuilderOutcomes(outcomeMarketSlug);
  const marketOutcomes = useMemo(() => outcomeData?.data ?? [], [outcomeData?.data]);
  const marketOutcomeTokenIdSet = useMemo(
    () => new Set(marketOutcomes.map((outcome) => outcome.token_id)),
    [marketOutcomes]
  );

  return { marketOutcomes, marketOutcomeTokenIdSet, outcomesLoading };
}
