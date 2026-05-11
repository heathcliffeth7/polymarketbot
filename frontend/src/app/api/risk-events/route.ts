import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { getRiskEvents } from '@/lib/queries/risk-events';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const { searchParams } = new URL(req.url);
    const result = await getRiskEvents({
      userId: user.userId,
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
