'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { cn } from '@/lib/utils';
import { useAuthState } from '@/lib/auth-client';
import {
  BarChart3,
  LayoutDashboard,
  Settings,
  Power,
  Zap,
  LogOut,
} from 'lucide-react';

const navItems = [
  { href: '/', label: 'Dashboard', mobileLabel: 'Panel', icon: LayoutDashboard },
  { href: '/trade-builder', label: 'İşlem Oluşturucu', mobileLabel: 'Oluştur', icon: Zap },
  { href: '/trade-analysis', label: 'İşlem Analizi', mobileLabel: 'Analiz', icon: BarChart3 },
  { href: '/settings', label: 'Settings', mobileLabel: 'Ayar', icon: Settings },
  { href: '/control', label: 'Bot Control', mobileLabel: 'Bot', icon: Power },
];

export function Sidebar() {
  const pathname = usePathname();
  const { data } = useAuthState();

  const handleLogout = async () => {
    await fetch('/api/auth', { method: 'DELETE', credentials: 'same-origin' });
    window.location.href = '/login';
  };

  return (
    <>
      <aside className="hidden h-screen w-64 shrink-0 flex-col border-r border-zinc-800 bg-zinc-950 md:flex">
        <div className="flex h-14 items-center border-b border-zinc-800 px-4">
          <h1 className="text-lg font-bold text-emerald-400">PolyBot</h1>
          <span className="ml-2 text-xs text-zinc-500">Dashboard</span>
        </div>
        <nav className="flex-1 space-y-1 p-3">
          {navItems.map((item) => {
            const isActive = item.href === '/' ? pathname === '/' : pathname.startsWith(item.href);
            return (
              <Link
                key={item.href}
                href={item.href}
                className={cn(
                  'flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors',
                  isActive
                    ? 'bg-zinc-800 text-emerald-400'
                    : 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200'
                )}
              >
                <item.icon className="h-4 w-4" />
                {item.label}
              </Link>
            );
          })}
        </nav>
        <div className="border-t border-zinc-800 p-3">
          <div className="mb-3 rounded-lg border border-zinc-800 bg-zinc-900 px-3 py-2">
            <p className="text-[11px] uppercase tracking-wide text-zinc-500">Current User</p>
            <p className="mt-1 text-sm text-zinc-200">
              {data?.user?.username || 'Unknown'}
            </p>
          </div>
          <button
            onClick={handleLogout}
            className="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm text-zinc-400 transition-colors hover:bg-zinc-900 hover:text-red-400"
          >
            <LogOut className="h-4 w-4" />
            Logout
          </button>
        </div>
      </aside>

      <nav className="fixed inset-x-0 bottom-0 z-50 border-t border-zinc-800 bg-zinc-950/95 px-2 pb-[calc(env(safe-area-inset-bottom)+0.35rem)] pt-1.5 backdrop-blur md:hidden">
        <div className="grid grid-cols-5 gap-1">
          {navItems.map((item) => {
            const isActive = item.href === '/' ? pathname === '/' : pathname.startsWith(item.href);
            return (
              <Link
                key={item.href}
                href={item.href}
                className={cn(
                  'flex min-h-12 flex-col items-center justify-center gap-1 rounded-md px-1 text-[10px] font-medium transition-colors',
                  isActive
                    ? 'bg-zinc-800 text-emerald-400'
                    : 'text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200'
                )}
              >
                <item.icon className="h-4 w-4 shrink-0" />
                <span className="max-w-full truncate">{item.mobileLabel}</span>
              </Link>
            );
          })}
        </div>
      </nav>
    </>
  );
}
