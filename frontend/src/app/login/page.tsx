'use client';

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';

type AuthMode = 'login' | 'register';

interface AuthStateResponse {
  authenticated: boolean;
  user: { userId: number; username: string } | null;
  registrationOpen: boolean;
  userCount: number;
  maxUsers: number;
}

export default function LoginPage() {
  const [mode, setMode] = useState<AuthMode>('login');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [state, setState] = useState<AuthStateResponse | null>(null);
  const router = useRouter();

  useEffect(() => {
    let active = true;
    void (async () => {
      try {
        const res = await fetch('/api/auth', { cache: 'no-store' });
        const payload = (await res.json()) as AuthStateResponse;
        if (!active) return;
        setState(payload);
        if (payload.authenticated) {
          router.replace('/');
        }
        if (!payload.registrationOpen) {
          setMode('login');
        }
      } catch {
        if (!active) return;
        setState({
          authenticated: false,
          user: null,
          registrationOpen: false,
          userCount: 0,
          maxUsers: 2,
        });
      }
    })();

    return () => {
      active = false;
    };
  }, [router]);

  const canRegister = !!state?.registrationOpen;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError('');

    try {
      const endpoint = mode === 'login' ? '/api/auth' : '/api/auth/register';
      const res = await fetch(endpoint, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password }),
      });

      if (res.ok) {
        router.push('/');
        return;
      }

      const payload = (await res.json().catch(() => ({}))) as { error?: string };
      setError(payload.error || (mode === 'login' ? 'Login failed' : 'Registration failed'));

      if (mode === 'register' && res.status === 403) {
        setMode('login');
        setState((prev) =>
          prev
            ? { ...prev, registrationOpen: false, userCount: prev.maxUsers }
            : prev
        );
      }
    } catch {
      setError('Connection failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="flex min-h-screen items-center justify-center bg-zinc-950 px-4">
      <Card className="w-full max-w-md border-zinc-800 bg-zinc-900">
        <CardHeader className="space-y-4 text-center">
          <div>
            <CardTitle className="text-xl text-emerald-400">PolyBot Dashboard</CardTitle>
            <p className="mt-2 text-sm text-zinc-500">
              Username/password ile oturum acin
            </p>
          </div>
          <div className="grid grid-cols-2 gap-2 rounded-lg bg-zinc-800 p-1">
            <button
              type="button"
              onClick={() => setMode('login')}
              className={`rounded-md px-3 py-2 text-sm transition-colors ${
                mode === 'login'
                  ? 'bg-emerald-500 text-zinc-950'
                  : 'text-zinc-300 hover:bg-zinc-700'
              }`}
            >
              Login
            </button>
            <button
              type="button"
              onClick={() => canRegister && setMode('register')}
              disabled={!canRegister}
              className={`rounded-md px-3 py-2 text-sm transition-colors ${
                mode === 'register'
                  ? 'bg-emerald-500 text-zinc-950'
                  : 'text-zinc-300 hover:bg-zinc-700'
              } disabled:cursor-not-allowed disabled:text-zinc-500`}
            >
              Register
            </button>
          </div>
          {!canRegister && (
            <p className="text-xs text-zinc-500">
              Kayit kapali. Sistem en fazla {state?.maxUsers ?? 2} kullanici destekler.
            </p>
          )}
        </CardHeader>
        <CardContent>
          <form onSubmit={handleSubmit} className="space-y-4">
            {error && <p className="text-sm text-red-400">{error}</p>}
            <div className="space-y-2">
              <Label htmlFor="username" className="text-zinc-300">Username</Label>
              <Input
                id="username"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                className="border-zinc-700 bg-zinc-800 text-zinc-200"
                placeholder="orn: heathcliffeth"
                autoFocus
                autoComplete={mode === 'login' ? 'username' : 'new-username'}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="password" className="text-zinc-300">Password</Label>
              <Input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="border-zinc-700 bg-zinc-800 text-zinc-200"
                placeholder={mode === 'login' ? 'Password' : 'En az 8 karakter'}
                autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
              />
            </div>
            <Button
              type="submit"
              className="w-full"
              disabled={loading || !username.trim() || !password || (mode === 'register' && !canRegister)}
            >
              {loading
                ? mode === 'login'
                  ? 'Logging in...'
                  : 'Registering...'
                : mode === 'login'
                  ? 'Login'
                  : 'Register'}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
