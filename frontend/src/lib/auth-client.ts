'use client';

import useSWR from 'swr';

export interface AuthClientState {
  authenticated: boolean;
  user: { userId: number; username: string } | null;
  registrationOpen: boolean;
  userCount: number;
  maxUsers: number;
}

async function fetcher(url: string): Promise<AuthClientState> {
  const res = await fetch(url, { cache: 'no-store' });
  if (!res.ok) {
    throw new Error('Failed to load auth state');
  }
  return res.json() as Promise<AuthClientState>;
}

export function useAuthState() {
  return useSWR<AuthClientState>('/api/auth', fetcher, {
    revalidateOnFocus: false,
  });
}
