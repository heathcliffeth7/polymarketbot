import { useEffect } from 'react';
import type { NodeConfigFormState } from '@/lib/trade-flow-config-mappers';
import type {
  UpstreamFixedMarketResolution,
  UpstreamMaxPriceResolution,
} from '../flow-canvas-utils';
import {
  syncPlaceOrderInheritedMarketState,
  syncPlaceOrderInheritedMaxPriceState,
  syncPlaceOrderTriggerRowsState,
} from './form-state';

interface UseSyncPlaceOrderFormStateArgs {
  nodeTypeDraft: string;
  selectedNodeId: string | null;
  placeOrderMaxTriggers: string | undefined;
  upstreamFixedMarketResolution: UpstreamFixedMarketResolution;
  upstreamMaxPriceResolution: UpstreamMaxPriceResolution;
  setNodeForm: React.Dispatch<React.SetStateAction<NodeConfigFormState | null>>;
}

export function useSyncPlaceOrderFormState({
  nodeTypeDraft,
  selectedNodeId,
  placeOrderMaxTriggers,
  upstreamFixedMarketResolution,
  upstreamMaxPriceResolution,
  setNodeForm,
}: UseSyncPlaceOrderFormStateArgs) {
  useEffect(() => {
    if (nodeTypeDraft !== 'action.place_order') return;
    setNodeForm(syncPlaceOrderTriggerRowsState);
  }, [nodeTypeDraft, placeOrderMaxTriggers, setNodeForm]);

  useEffect(() => {
    if (nodeTypeDraft !== 'action.place_order') return;
    setNodeForm((prev) =>
      syncPlaceOrderInheritedMaxPriceState(prev, upstreamMaxPriceResolution)
    );
  }, [nodeTypeDraft, selectedNodeId, upstreamMaxPriceResolution, setNodeForm]);

  useEffect(() => {
    if (nodeTypeDraft !== 'action.place_order') return;
    setNodeForm((prev) =>
      syncPlaceOrderInheritedMarketState(prev, upstreamFixedMarketResolution)
    );
  }, [nodeTypeDraft, selectedNodeId, upstreamFixedMarketResolution, setNodeForm]);
}
