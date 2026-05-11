'use client';

import { usePolling } from './use-polling';
import type { DashboardData } from '@/lib/types';

export function useDashboard() {
  return usePolling<DashboardData>('/api/dashboard', 3000);
}
