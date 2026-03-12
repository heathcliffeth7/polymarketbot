import { useEffect } from 'react';
import type { NodeConfigFormState } from '@/lib/trade-flow-config-mappers';
import type { UpstreamMaxPriceResolution } from '../flow-canvas-utils';
import {
  syncPlaceOrderInheritedMaxPriceState,
  syncPlaceOrderTriggerRowsState,
} from './form-state';

interface UseSyncPlaceOrderFormStateArgs {
  nodeTypeDraft: string;
  selectedNodeId: string | null;
  placeOrderMaxTriggers: string | undefined;
  upstreamMaxPriceResolution: UpstreamMaxPriceResolution;
  setNodeForm: React.Dispatch<React.SetStateAction<NodeConfigFormState | null>>;
}

export function useSyncPlaceOrderFormState({
  nodeTypeDraft,
  selectedNodeId,
  placeOrderMaxTriggers,
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
}
