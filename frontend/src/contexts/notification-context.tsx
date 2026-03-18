'use client';

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useReducer,
  useRef,
  useMemo,
  type ReactNode,
} from 'react';
import { usePathname } from 'next/navigation';
import { toast } from 'sonner';
import { useTradeFlowRecentEvents } from '@/hooks/use-trade-flow';
import { useTradeFlowRealtime } from '@/contexts/trade-flow-realtime-context';
import type { TradeFlowEvent } from '@/lib/types';

export interface Notification {
  id: number;
  title: string;
  detail: string;
  market: string;
  time: string;
  tone: 'trigger' | 'success' | 'error';
}

interface NotificationContextValue {
  notifications: Notification[];
  unreadCount: number;
  markAllRead: () => void;
}

interface NotificationState {
  notifications: Notification[];
  unreadCount: number;
}

type NotificationAction =
  | { type: 'append'; incoming: Notification[] }
  | { type: 'mark_all_read' };

const NotificationContext = createContext<NotificationContextValue>({
  notifications: [],
  unreadCount: 0,
  markAllRead: () => {},
});

export const useNotifications = () => useContext(NotificationContext);

function formatPriceLabel(value: unknown): string {
  return typeof value === 'number' && Number.isFinite(value)
    ? `${(value * 100).toFixed(1)}c`
    : '?';
}

function resolveDefinitionLabel(evt: TradeFlowEvent, payload: Record<string, unknown>): string {
  const definitionName =
    typeof evt.definition_name === 'string' && evt.definition_name.trim().length > 0
      ? evt.definition_name.trim()
      : typeof payload.definition_name === 'string' && payload.definition_name.trim().length > 0
        ? payload.definition_name.trim()
        : '';
  return definitionName || `Flow #${evt.definition_id}`;
}

function parseEvent(evt: TradeFlowEvent): Notification | null {
  const p = evt.payload_json;
  const definitionLabel = resolveDefinitionLabel(evt, p);
  const market = String(p.market_slug ?? '').trim();

  if (
    evt.event_type === 'trigger_once_fired' ||
    evt.event_type === 'trigger_ws_price_enqueued' ||
    (
      evt.event_type === 'step_completed' &&
      p.triggered === true &&
      typeof p.node_type === 'string' &&
      (p.node_type as string).startsWith('trigger.')
    )
  ) {
    const rawPrice =
      typeof p.triggered_price === 'number'
        ? p.triggered_price
        : typeof p.price === 'number'
          ? p.price
          : p.current_price;
    const price = formatPriceLabel(rawPrice);
    const triggerCondition =
      typeof p.triggered_condition === 'string'
        ? p.triggered_condition
        : typeof p.trigger_condition === 'string'
          ? p.trigger_condition
          : '';
    const direction = triggerCondition === 'cross_below' ? '↓' : '↑';
    const outcome = String(
      p.triggered_outcome_label || p.outcome_label || p.node_key || 'Trigger'
    );
    const evaluationMode =
      typeof p.evaluation_mode === 'string'
        ? p.evaluation_mode
        : typeof p.ws_evaluation_mode_from_step === 'string'
          ? p.ws_evaluation_mode_from_step
          : '';

    return {
      id: evt.id,
      title: `${definitionLabel}: ${outcome} ${direction} @ ${price}`,
      detail: evaluationMode ? `${market || 'market?'} • ${evaluationMode}` : market || String(p.node_key || ''),
      market,
      time: evt.created_at,
      tone: 'trigger',
    };
  }

  if (evt.event_type === 'telegram_notify') {
    const status = String(p.status ?? '').trim().toLowerCase();
    const message = String(p.message ?? '').trim();
    const error = String(p.error ?? '').trim();
    const chatId = String(p.chat_id ?? '').trim();
    const sent = status === 'sent';
    const detail = sent
      ? [chatId ? `chat ${chatId}` : null, message || null].filter(Boolean).join(' • ')
      : error || 'Telegram gonderimi basarisiz.';

    return {
      id: evt.id,
      title: `${definitionLabel}: Telegram ${sent ? 'gonderildi' : 'hatasi'}`,
      detail,
      market,
      time: evt.created_at,
      tone: sent ? 'success' : 'error',
    };
  }

  return null;
}

const MAX_NOTIFICATIONS = 50;
const STORAGE_KEY = 'polybot_notifications';
const STORAGE_LAST_SEEN_KEY = 'polybot_notif_last_seen_id';

function normalizeStoredNotification(raw: unknown): Notification | null {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return null;
  const item = raw as Record<string, unknown>;
  const id = Number(item.id);
  if (!Number.isFinite(id)) return null;

  const legacyLabel = String(item.label ?? '').trim();
  const legacyCondition = String(item.condition ?? '').trim();
  const legacyPrice = String(item.price ?? '').trim();
  const legacyTitle =
    legacyLabel && legacyCondition && legacyPrice
      ? `${legacyLabel} ${legacyCondition} @ ${legacyPrice}c`
      : legacyLabel;

  return {
    id,
    title: String(item.title ?? legacyTitle ?? `Event #${id}`),
    detail: String(item.detail ?? item.market ?? '').trim(),
    market: String(item.market ?? '').trim(),
    time: String(item.time ?? ''),
    tone:
      item.tone === 'error' || item.tone === 'success' || item.tone === 'trigger'
        ? item.tone
        : 'trigger',
  };
}

function loadNotifications(): Notification[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .map((item) => normalizeStoredNotification(item))
      .filter((item): item is Notification => item != null);
  } catch {
    return [];
  }
}

function saveNotifications(items: Notification[]) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(items));
  } catch {}
}

function notificationReducer(
  state: NotificationState,
  action: NotificationAction,
): NotificationState {
  switch (action.type) {
    case 'append':
      if (action.incoming.length === 0) return state;
      return {
        notifications: [...action.incoming, ...state.notifications].slice(0, MAX_NOTIFICATIONS),
        unreadCount: state.unreadCount + action.incoming.length,
      };
    case 'mark_all_read':
      return { ...state, unreadCount: 0 };
    default:
      return state;
  }
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
  const pathname = usePathname();
  const { connectionState, flowEvents: streamEvents } = useTradeFlowRealtime();
  const isTradeBuilderPage = pathname.startsWith('/trade-builder');
  const recentEventsEnabled = !(isTradeBuilderPage && connectionState === 'open');
  const { data: eventsData } = useTradeFlowRecentEvents('running', 100, recentEventsEnabled);
  const events = useMemo(() => {
    const merged = new Map<number, TradeFlowEvent>();
    for (const evt of eventsData?.data ?? []) {
      merged.set(evt.id, evt);
    }
    for (const evt of streamEvents) {
      merged.set(evt.id, evt);
    }
    return Array.from(merged.values()).sort((a, b) => b.id - a.id);
  }, [eventsData?.data, streamEvents]);

  const [state, dispatch] = useReducer(notificationReducer, undefined, () => ({
    notifications: loadNotifications(),
    unreadCount: 0,
  }));
  const lastSeenEventIdRef = useRef<number>(loadLastSeenId());
  const initializedRef = useRef(false);

  const markAllRead = useCallback(() => dispatch({ type: 'mark_all_read' }), []);

  useEffect(() => {
    saveNotifications(state.notifications);
  }, [state.notifications]);

  useEffect(() => {
    if (!events.length) return;
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
    const sorted = [...newEvents].sort((a, b) => a.id - b.id);
    for (const evt of sorted) {
      const n = parseEvent(evt);
      if (!n) continue;
      const key = `${n.title}:${n.detail}:${n.market}`;
      if (seenKeys.has(key)) continue;
      seenKeys.add(key);
      incoming.push(n);
      const description = [n.detail, n.market && n.market !== n.detail ? n.market : null]
        .filter(Boolean)
        .join(' • ');
      if (n.tone === 'error') {
        toast.error(n.title, { description, duration: 8000 });
      } else {
        toast.success(n.title, { description, duration: 8000 });
      }
    }
    if (incoming.length > 0) {
      dispatch({ type: 'append', incoming });
    }
  }, [events]);

  return (
    <NotificationContext.Provider
      value={{ notifications: state.notifications, unreadCount: state.unreadCount, markAllRead }}
    >
      {children}
    </NotificationContext.Provider>
  );
}
