'use client';

import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import type {
  TradeFlowEvent,
  TradeFlowRealtimeHeartbeat,
  TradeFlowRealtimePriceTick,
  TradeFlowRealtimeReady,
} from '@/lib/types';

type TradeFlowRealtimeConnectionState = 'connecting' | 'open' | 'error' | 'closed';

interface TradeFlowRealtimeContextValue {
  connectionState: TradeFlowRealtimeConnectionState;
  flowEvents: TradeFlowEvent[];
  livePrices: Record<string, number>;
  latestPriceTicks: Record<string, TradeFlowRealtimePriceTick>;
  lastEventAt: string | null;
  lastEventLagMs: number | null;
}

const TradeFlowRealtimeContext = createContext<TradeFlowRealtimeContextValue>({
  connectionState: 'closed',
  flowEvents: [],
  livePrices: {},
  latestPriceTicks: {},
  lastEventAt: null,
  lastEventLagMs: null,
});

function parseRealtimeData<T>(raw: string): T | null {
  try {
    return JSON.parse(raw) as T;
  } catch {
    return null;
  }
}

function computeLagMs(iso: string | null): number | null {
  if (!iso) return null;
  const ts = new Date(iso).getTime();
  if (!Number.isFinite(ts)) return null;
  return Math.max(0, Date.now() - ts);
}

export function TradeFlowRealtimeProvider({ children }: { children: ReactNode }) {
  const [connectionState, setConnectionState] =
    useState<TradeFlowRealtimeConnectionState>('connecting');
  const [flowEvents, setFlowEvents] = useState<TradeFlowEvent[]>([]);
  const [latestPriceTicks, setLatestPriceTicks] = useState<Record<string, TradeFlowRealtimePriceTick>>({});
  const [lastEventAt, setLastEventAt] = useState<string | null>(null);
  const [lastEventLagMs, setLastEventLagMs] = useState<number | null>(null);

  useEffect(() => {
    if (typeof window === 'undefined') return;

    const source = new EventSource('/api/trade-flow/stream?status=running');

    const handleOpen = () => {
      setConnectionState('open');
    };

    const handleError = () => {
      setConnectionState('error');
    };

    const handleReady = (event: Event) => {
      const data = parseRealtimeData<TradeFlowRealtimeReady>((event as MessageEvent).data);
      const ts = data?.connected_at ?? null;
      setConnectionState('open');
      setLastEventAt(ts);
      setLastEventLagMs(computeLagMs(ts));
    };

    const handleHeartbeat = (event: Event) => {
      const data = parseRealtimeData<TradeFlowRealtimeHeartbeat>((event as MessageEvent).data);
      const ts = data?.now ?? null;
      setConnectionState('open');
      setLastEventAt(ts);
      setLastEventLagMs(computeLagMs(ts));
    };

    const handleFlowEvent = (event: Event) => {
      const data = parseRealtimeData<TradeFlowEvent>((event as MessageEvent).data);
      if (!data) return;
      setConnectionState('open');
      setLastEventAt(data.created_at);
      setLastEventLagMs(computeLagMs(data.created_at));
      setFlowEvents((prev) => {
        const next = [data, ...prev.filter((item) => item.id !== data.id)];
        next.sort((a, b) => b.id - a.id);
        return next.slice(0, 200);
      });
    };

    const handlePriceTick = (event: Event) => {
      const data = parseRealtimeData<TradeFlowRealtimePriceTick>((event as MessageEvent).data);
      if (!data) return;
      setConnectionState('open');
      setLastEventAt(data.created_at);
      setLastEventLagMs(computeLagMs(data.created_at));
      setLatestPriceTicks((prev) => ({
        ...prev,
        [data.token_id]: data,
      }));
    };

    source.addEventListener('open', handleOpen as EventListener);
    source.addEventListener('ready', handleReady as EventListener);
    source.addEventListener('heartbeat', handleHeartbeat as EventListener);
    source.addEventListener('flow_event', handleFlowEvent as EventListener);
    source.addEventListener('price_tick', handlePriceTick as EventListener);
    source.onerror = handleError;

    return () => {
      source.removeEventListener('open', handleOpen as EventListener);
      source.removeEventListener('ready', handleReady as EventListener);
      source.removeEventListener('heartbeat', handleHeartbeat as EventListener);
      source.removeEventListener('flow_event', handleFlowEvent as EventListener);
      source.removeEventListener('price_tick', handlePriceTick as EventListener);
      source.close();
    };
  }, []);

  const livePrices = useMemo(
    () =>
      Object.fromEntries(
        Object.entries(latestPriceTicks).map(([tokenId, tick]) => [tokenId, tick.price])
      ),
    [latestPriceTicks]
  );

  return (
    <TradeFlowRealtimeContext.Provider
      value={{
        connectionState,
        flowEvents,
        livePrices,
        latestPriceTicks,
        lastEventAt,
        lastEventLagMs,
      }}
    >
      {children}
    </TradeFlowRealtimeContext.Provider>
  );
}

export function useTradeFlowRealtime() {
  return useContext(TradeFlowRealtimeContext);
}
