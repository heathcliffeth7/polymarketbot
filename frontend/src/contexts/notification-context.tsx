'use client';

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { toast } from 'sonner';
import { useTradeFlowRuns, useTradeFlowRunEvents } from '@/hooks/use-trade-flow';
import type { TradeFlowEvent } from '@/lib/types';

export interface Notification {
  id: number;
  label: string;
  price: string;
  condition: string;
  market: string;
  time: string;
}

interface NotificationContextValue {
  notifications: Notification[];
  unreadCount: number;
  markAllRead: () => void;
}

const NotificationContext = createContext<NotificationContextValue>({
  notifications: [],
  unreadCount: 0,
  markAllRead: () => {},
});

export const useNotifications = () => useContext(NotificationContext);

function parseTriggerEvent(
  evt: TradeFlowEvent,
): Notification | null {
  const p = evt.payload_json;
  if (evt.event_type === 'trigger_once_fired') {
    const rawPrice = p.price;
    const price =
      typeof rawPrice === 'number' ? (Number(rawPrice) * 100).toFixed(1) : '?';
    const cond = p.triggered_condition === 'cross_above' ? '↑' : '↓';
    const label = String(
      p.triggered_outcome_label || p.node_key || 'Trigger',
    );
    return {
      id: evt.id,
      label,
      price,
      condition: cond,
      market: String(p.market_slug ?? ''),
      time: evt.created_at,
    };
  }
  if (
    evt.event_type === 'step_completed' &&
    p.triggered === true &&
    typeof p.node_type === 'string' &&
    (p.node_type as string).startsWith('trigger.')
  ) {
    const rawPrice = p.current_price;
    const price =
      typeof rawPrice === 'number' ? (Number(rawPrice) * 100).toFixed(1) : '?';
    const cond = p.trigger_condition === 'cross_above' ? '↑' : '↓';
    const label = String(p.node_key || 'Trigger');
    return {
      id: evt.id,
      label,
      price,
      condition: cond,
      market: String(p.market_slug ?? ''),
      time: evt.created_at,
    };
  }
  return null;
}

const MAX_NOTIFICATIONS = 50;
const STORAGE_KEY = 'polybot_notifications';
const STORAGE_LAST_SEEN_KEY = 'polybot_notif_last_seen_id';

function loadNotifications(): Notification[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function saveNotifications(items: Notification[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(items));
  } catch {}
}

function loadLastSeenId(): number {
  try {
    return Number(localStorage.getItem(STORAGE_LAST_SEEN_KEY)) || 0;
  } catch {
    return 0;
  }
}

function saveLastSeenId(id: number) {
  try {
    localStorage.setItem(STORAGE_LAST_SEEN_KEY, String(id));
  } catch {}
}

export function NotificationProvider({ children }: { children: ReactNode }) {
  const { data: runsData } = useTradeFlowRuns(1, 1, undefined, 'running');
  const activeRunId = runsData?.data?.[0]?.id ?? null;
  const { data: eventsData } = useTradeFlowRunEvents(
    activeRunId,
    1,
    50,
    !!activeRunId,
  );

  const [notifications, setNotifications] = useState<Notification[]>(loadNotifications);
  const [unreadCount, setUnreadCount] = useState(0);
  const lastSeenEventIdRef = useRef<number>(loadLastSeenId());
  const initializedRef = useRef(false);

  const markAllRead = useCallback(() => setUnreadCount(0), []);

  useEffect(() => {
    if (!eventsData?.data?.length) return;
    const events = eventsData.data;
    if (!initializedRef.current) {
      if (lastSeenEventIdRef.current === 0) {
        lastSeenEventIdRef.current = Math.max(...events.map((e) => e.id));
        saveLastSeenId(lastSeenEventIdRef.current);
      }
      initializedRef.current = true;
    }
    const newEvents = events.filter(
      (e: TradeFlowEvent) => e.id > lastSeenEventIdRef.current,
    );
    if (!newEvents.length) return;
    lastSeenEventIdRef.current = Math.max(...events.map((e) => e.id));
    saveLastSeenId(lastSeenEventIdRef.current);

    const incoming: Notification[] = [];
    const seenKeys = new Set<string>();
    // trigger_once_fired önce gelsin (daha iyi label verisi)
    const sorted = [...newEvents].sort((a, b) =>
      a.event_type === 'trigger_once_fired' ? -1 : b.event_type === 'trigger_once_fired' ? 1 : 0,
    );
    for (const evt of sorted) {
      const n = parseTriggerEvent(evt);
      if (!n) continue;
      const key = `${n.market}:${n.price}:${n.condition}`;
      if (seenKeys.has(key)) continue;
      seenKeys.add(key);
      incoming.push(n);
      toast.success(`${n.label} tetiklendi ${n.condition} @ ${n.price}¢`, {
        description: `Market: ${n.market}`,
        duration: 8000,
      });
    }
    if (incoming.length > 0) {
      setNotifications((prev) => {
        const next = [...incoming, ...prev].slice(0, MAX_NOTIFICATIONS);
        saveNotifications(next);
        return next;
      });
      setUnreadCount((prev) => prev + incoming.length);
    }
  }, [eventsData]);

  useEffect(() => {
    initializedRef.current = false;
  }, [activeRunId]);

  return (
    <NotificationContext.Provider
      value={{ notifications, unreadCount, markAllRead }}
    >
      {children}
    </NotificationContext.Provider>
  );
}
