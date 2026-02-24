'use client';

import { usePolling } from './use-polling';
import type { BotStatusResponse } from '@/lib/types';

export function useBotStatus() {
  return usePolling<BotStatusResponse>('/api/bot/status', 5000);
}
