import { NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { getClaimSweepStatus } from '@/lib/claim-sweep';
import { getDashboardData } from '@/lib/queries/dashboard';
import { getServiceStatus } from '@/lib/systemctl';
import { getMarketDiscoveryStatus } from '@/lib/queries/bot-runs';

export const dynamic = 'force-dynamic';

export async function GET() {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const [data, serviceStatus] = await Promise.all([
      getDashboardData(user),
      getServiceStatus(),
    ]);
    const [marketDiscovery, claimSweep] = await Promise.all([
      getMarketDiscoveryStatus(data.botStatus.lastRun?.started_at ?? null),
      getClaimSweepStatus(user, { serviceActive: serviceStatus.serviceActive }),
    ]);

    data.botStatus.serviceActive = serviceStatus.serviceActive;
    data.botStatus.controlAvailable = serviceStatus.controlAvailable;
    data.botStatus.controlReason = serviceStatus.controlReason;
    data.botStatus.controlReasonCode = serviceStatus.controlReasonCode;
    data.botStatus.marketDiscoveryState = marketDiscovery.state;
    data.botStatus.selectedMarketSlug = marketDiscovery.selectedMarketSlug;
    data.botStatus.marketDiscoveryMessage = marketDiscovery.message;
    data.claimSweep = claimSweep;
    return NextResponse.json(data);
  } catch (err) {
    console.error('Dashboard error:', err);
    return NextResponse.json({ error: 'Failed to load dashboard' }, { status: 500 });
  }
}
