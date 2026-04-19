'use client';

import { useState } from 'react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { requestJson, formatClientRequestError } from '@/lib/http-client';
import type { ClaimSweepRunResult, DashboardData } from '@/lib/types';
import { Coins } from 'lucide-react';

export function ClaimSweepCard({
  data,
  onSweepComplete,
}: {
  data: DashboardData['claimSweep'];
  onSweepComplete: () => void;
}) {
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleSweep = async () => {
    setLoading(true);
    setMessage(null);
    setError(null);

    try {
      const result = await requestJson<ClaimSweepRunResult>(
        '/api/claim/sweep',
        {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({}),
        },
        { timeoutMs: 20_000, retries: 0 }
      );
      setMessage(
        `Queue guncellendi: yeni ${result.queuedNewCount}, yeniden silahlanan ${result.rearmedCount}, zaten izlenen ${result.alreadyTrackedCount}.`
      );
      onSweepComplete();
    } catch (err) {
      setError(formatClientRequestError(err, 'Claim sweep basarisiz.'));
    } finally {
      setLoading(false);
    }
  };

  const trackedTotal =
    data.queue.pending +
    data.queue.retry +
    data.queue.processing +
    data.queue.submitted +
    data.queue.failed +
    data.queue.claimed;

  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-sm font-medium text-zinc-400">Claim Sweep</CardTitle>
        <Coins className="h-4 w-4 text-zinc-500" />
      </CardHeader>
      <CardContent className="space-y-4">
        <div>
          <div className="text-2xl font-bold text-emerald-400">
            {data.eligibleTotalUsdc.toFixed(2)} USDC
          </div>
          <p className="mt-1 text-xs text-zinc-500">
            {data.eligibleCount} adet claimable dust, esik {data.thresholdUsdc.toFixed(2)} USDC
          </p>
        </div>

        <div className="flex flex-wrap gap-2 text-[11px]">
          <Badge variant="outline" className="border-zinc-700 text-zinc-300">
            Queue {trackedTotal}
          </Badge>
          <Badge variant="outline" className="border-zinc-700 text-zinc-300">
            Pending {data.queue.pending}
          </Badge>
          <Badge variant="outline" className="border-zinc-700 text-zinc-300">
            Submitted {data.queue.submitted}
          </Badge>
          <Badge variant="outline" className="border-zinc-700 text-zinc-300">
            Failed {data.queue.failed}
          </Badge>
          <Badge variant="outline" className="border-zinc-700 text-zinc-300">
            Mode {data.executionMode}
          </Badge>
        </div>

        <div className="space-y-1 text-xs text-zinc-500">
          <p>
            Wallet:{' '}
            <span className="text-zinc-300">
              {data.walletAddress ? shortenAddress(data.walletAddress) : '-'}
            </span>
          </p>
          {data.refreshedAt && (
            <p>
              Refreshed:{' '}
              <span className="text-zinc-300">
                {new Date(data.refreshedAt).toLocaleString()}
              </span>
            </p>
          )}
          {data.lastError && (
            <p className="text-amber-400">Last claim error: {data.lastError}</p>
          )}
          {data.disabledReason && (
            <p className="text-amber-400">{data.disabledReason}</p>
          )}
          {error && <p className="text-red-400">{error}</p>}
          {message && <p className="text-emerald-400">{message}</p>}
        </div>

        <Button
          onClick={handleSweep}
          disabled={loading || !data.canSweep}
          className="bg-emerald-700 hover:bg-emerald-600"
        >
          {loading ? 'Queueing...' : 'Claim Dust to Cash'}
        </Button>
      </CardContent>
    </Card>
  );
}

function shortenAddress(address: string): string {
  if (address.length < 12) {
    return address;
  }
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}
