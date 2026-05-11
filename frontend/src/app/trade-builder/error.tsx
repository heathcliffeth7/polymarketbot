'use client';

import { useEffect } from 'react';

export default function TradeBuilderError({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    console.error('[TradeBuilder Error Boundary]', error);
  }, [error]);

  return (
    <div className="flex min-h-[60vh] flex-col items-center justify-center gap-4 text-zinc-300">
      <h2 className="text-lg font-semibold text-red-400">Trade Builder Hatasi</h2>
      <pre className="max-w-2xl overflow-auto rounded border border-zinc-700 bg-zinc-900 p-4 text-xs text-zinc-400">
        {error.message}
        {'\n\n'}
        {error.stack}
      </pre>
      <button
        onClick={reset}
        className="rounded bg-emerald-600 px-4 py-2 text-sm text-white hover:bg-emerald-500"
      >
        Tekrar Dene
      </button>
    </div>
  );
}
