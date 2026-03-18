'use client';

import { useCallback, useEffect, useRef, type MutableRefObject } from 'react';
import { formatClientRequestError } from '@/lib/http-client';
import type { DraftSaveStatus } from '@/components/trade-builder/flow-engine-types';
import type { TradeFlowDefinitionDetail, TradeFlowGraph } from '@/lib/types';

export interface DraftPersistPayload extends Record<string, unknown> {
  description?: string | null;
  graphJson?: TradeFlowGraph;
  name?: string;
}

export interface QueueDraftSaveOptions {
  errorMessage: string;
  revision: number;
  surfaceError?: boolean;
}

export interface LatestOnlyQueueJob<TResult> {
  run: () => Promise<TResult>;
  onError?: (error: unknown) => void;
  onSuccess?: (result: TResult, meta: { hasPending: boolean }) => void;
}

export interface LatestOnlyQueue<TResult> {
  clearPending: () => void;
  enqueue: (job: LatestOnlyQueueJob<TResult>) => Promise<TResult | null>;
  getInflight: () => Promise<TResult | null> | null;
  hasPending: () => boolean;
}

export function createLatestOnlyQueue<TResult>(): LatestOnlyQueue<TResult> {
  let inflight: Promise<TResult | null> | null = null;
  let pending: LatestOnlyQueueJob<TResult> | null = null;

  const process = async (): Promise<TResult | null> => {
    let lastResult: TResult | null = null;
    try {
      while (pending) {
        const current = pending;
        pending = null;
        try {
          const result = await current.run();
          lastResult = result;
          current.onSuccess?.(result, { hasPending: pending != null });
        } catch (error) {
          current.onError?.(error);
          pending = null;
          throw error;
        }
      }
      return lastResult;
    } finally {
      inflight = null;
    }
  };

  return {
    clearPending: () => {
      pending = null;
    },
    enqueue: (job) => {
      pending = job;
      if (!inflight) {
        inflight = process();
      }
      return inflight;
    },
    getInflight: () => inflight,
    hasPending: () => pending != null,
  };
}

interface UseDraftSaveQueueArgs {
  acknowledgeSuccessRef: MutableRefObject<(detail: TradeFlowDefinitionDetail) => void>;
  patchDraft: (
    definitionId: number,
    payload: DraftPersistPayload
  ) => Promise<TradeFlowDefinitionDetail>;
  revisionRef: MutableRefObject<number>;
  saveStatus: DraftSaveStatus;
  selectedDefinitionIdRef: MutableRefObject<number | null>;
  setAutoSaveError: (value: string | null) => void;
  setError: (value: string | null) => void;
  setSaveStatus: (value: DraftSaveStatus) => void;
}

export function useDraftSaveQueue({
  acknowledgeSuccessRef,
  patchDraft,
  revisionRef,
  saveStatus,
  selectedDefinitionIdRef,
  setAutoSaveError,
  setError,
  setSaveStatus,
}: UseDraftSaveQueueArgs) {
  const draftSaveQueueRef = useRef(createLatestOnlyQueue<TradeFlowDefinitionDetail>());

  useEffect(() => {
    const handleBeforeUnload = (event: BeforeUnloadEvent) => {
      if (saveStatus !== 'pending' && !draftSaveQueueRef.current.hasPending()) return;
      event.preventDefault();
      event.returnValue = '';
    };

    window.addEventListener('beforeunload', handleBeforeUnload);
    return () => window.removeEventListener('beforeunload', handleBeforeUnload);
  }, [saveStatus]);

  const queueDraftSave = useCallback(
    async (
      definitionId: number,
      payload: DraftPersistPayload,
      { errorMessage, revision, surfaceError = false }: QueueDraftSaveOptions
    ) => {
      setSaveStatus('pending');
      setAutoSaveError(null);
      return draftSaveQueueRef.current.enqueue({
        run: async () => patchDraft(definitionId, payload),
        onSuccess: (updatedDetail, meta) => {
          if (
            !meta.hasPending &&
            selectedDefinitionIdRef.current === definitionId &&
            revisionRef.current === revision
          ) {
            acknowledgeSuccessRef.current(updatedDetail);
          }
          if (!meta.hasPending) {
            setSaveStatus('idle');
            setAutoSaveError(null);
          }
        },
        onError: (err) => {
          setSaveStatus('error');
          const reason = formatClientRequestError(err, errorMessage);
          if (
            selectedDefinitionIdRef.current === definitionId &&
            revisionRef.current === revision
          ) {
            setAutoSaveError(reason);
          }
          if (
            surfaceError &&
            selectedDefinitionIdRef.current === definitionId &&
            revisionRef.current === revision
          ) {
            setError(reason);
          }
        },
      });
    },
    [
      acknowledgeSuccessRef,
      patchDraft,
      revisionRef,
      selectedDefinitionIdRef,
      setAutoSaveError,
      setError,
      setSaveStatus,
    ]
  );

  const waitForQueuedDraftSave = useCallback(async () => {
    const inflight = draftSaveQueueRef.current.getInflight();
    if (!inflight) return;
    await inflight;
  }, []);

  return {
    queueDraftSave,
    waitForQueuedDraftSave,
  };
}
