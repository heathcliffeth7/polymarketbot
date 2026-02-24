'use client';

import useSWR from 'swr';

const fetcher = async (url: string) => {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json();
};

export function useConfigs() {
  return useSWR<Record<string, { data: Record<string, unknown>; writable: boolean }>>(
    '/api/config',
    fetcher,
    { revalidateOnFocus: false }
  );
}

export function useConfig(file: string) {
  return useSWR<{ data: Record<string, unknown>; writable: boolean }>(
    `/api/config/${file}`,
    fetcher,
    { revalidateOnFocus: false }
  );
}

export async function saveConfig(file: string, data: Record<string, unknown>) {
  const res = await fetch(`/api/config/${file}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  });
  if (!res.ok) {
    const err = await res.json();
    throw new Error(err.error || 'Failed to save config');
  }
  return res.json();
}
