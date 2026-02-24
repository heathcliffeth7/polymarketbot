import { NextResponse } from 'next/server';
import { getServiceStatus } from '@/lib/systemctl';
import { getLastBotRun, getMarketDiscoveryStatus } from '@/lib/queries/bot-runs';

export const dynamic = 'force-dynamic';

export async function GET() {
  try {
    const [serviceStatus, lastRun] = await Promise.all([
      getServiceStatus(),
      getLastBotRun(),
    ]);
    const marketDiscovery = await getMarketDiscoveryStatus(lastRun?.started_at ?? null);

    return NextResponse.json({
      serviceActive: serviceStatus.serviceActive,
      lastRun,
      controlAvailable: serviceStatus.controlAvailable,
      controlReason: serviceStatus.controlReason,
      controlReasonCode: serviceStatus.controlReasonCode,
      marketDiscoveryState: marketDiscovery.state,
      selectedMarketSlug: marketDiscovery.selectedMarketSlug,
      marketDiscoveryMessage: marketDiscovery.message,
    });
  } catch (err) {
    console.error('Bot status error:', err);
    return NextResponse.json({ error: 'Failed to get bot status' }, { status: 500 });
  }
}
