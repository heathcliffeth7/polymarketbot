import { NextRequest, NextResponse } from 'next/server';
import { getRiskEvents } from '@/lib/queries/risk-events';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const { searchParams } = new URL(req.url);
    const result = await getRiskEvents({
      page: parseInt(searchParams.get('page') || '1'),
      limit: parseInt(searchParams.get('limit') || '30'),
      eventType: searchParams.get('eventType') || undefined,
      decision: searchParams.get('decision') || undefined,
    });
    return NextResponse.json(result);
  } catch (err) {
    console.error('Risk events error:', err);
    return NextResponse.json({ error: 'Failed to load risk events' }, { status: 500 });
  }
}
