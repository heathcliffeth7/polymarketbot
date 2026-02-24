'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { cn } from '@/lib/utils';
import {
  LayoutDashboard,
  ArrowLeftRight,
  ClipboardList,
  ShieldAlert,
  Settings,
  Power,
  BarChart3,
  Zap,
  LogOut,
} from 'lucide-react';

const navItems = [
  { href: '/', label: 'Dashboard', icon: LayoutDashboard },
  { href: '/trades', label: 'Trades', icon: ArrowLeftRight },
  { href: '/orders', label: 'Orders & Fills', icon: ClipboardList },
  { href: '/risk', label: 'Risk Events', icon: ShieldAlert },
  { href: '/market', label: 'Market', icon: BarChart3 },
  { href: '/trade-builder', label: 'İşlem Oluşturucu', icon: Zap },
  { href: '/settings', label: 'Settings', icon: Settings },
  { href: '/control', label: 'Bot Control', icon: Power },
];

export function Sidebar() {
  const pathname = usePathname();

  const handleLogout = async () => {
    await fetch('/api/auth', { method: 'DELETE' });
    window.location.href = '/login';
  };

  return (
    <aside className="flex h-screen w-64 flex-col border-r border-zinc-800 bg-zinc-950">
      <div className="flex h-14 items-center border-b border-zinc-800 px-4">
        <h1 className="text-lg font-bold text-emerald-400">PolyBot</h1>
        <span className="ml-2 text-xs text-zinc-500">Dashboard</span>
      </div>
      <nav className="flex-1 space-y-1 p-3">
        {navItems.map((item) => {
          const isActive = pathname === item.href;
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
        <button
          onClick={handleLogout}
          className="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm text-zinc-400 transition-colors hover:bg-zinc-900 hover:text-red-400"
        >
          <LogOut className="h-4 w-4" />
          Logout
        </button>
      </div>
    </aside>
  );
}
