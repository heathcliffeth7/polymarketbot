import { NextResponse } from 'next/server';
import { getDashboardData } from '@/lib/queries/dashboard';
import { getServiceStatus } from '@/lib/systemctl';
import { getMarketDiscoveryStatus } from '@/lib/queries/bot-runs';

export const dynamic = 'force-dynamic';

export async function GET() {
  try {
    const [data, serviceStatus] = await Promise.all([
      getDashboardData(),
      getServiceStatus(),
    ]);
    const marketDiscovery = await getMarketDiscoveryStatus(data.botStatus.lastRun?.started_at ?? null);

    data.botStatus.serviceActive = serviceStatus.serviceActive;
    data.botStatus.controlAvailable = serviceStatus.controlAvailable;
    data.botStatus.controlReason = serviceStatus.controlReason;
    data.botStatus.controlReasonCode = serviceStatus.controlReasonCode;
    data.botStatus.marketDiscoveryState = marketDiscovery.state;
    data.botStatus.selectedMarketSlug = marketDiscovery.selectedMarketSlug;
    data.botStatus.marketDiscoveryMessage = marketDiscovery.message;
    return NextResponse.json(data);
  } catch (err) {
    console.error('Dashboard error:', err);
    return NextResponse.json({ error: 'Failed to load dashboard' }, { status: 500 });
  }
}
