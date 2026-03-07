import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { getTradeBuilderWorkflowEvents } from '@/lib/queries/trade-builder';

export const dynamic = 'force-dynamic';

export async function GET(
  req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const { id } = await params;
    const workflowId = Number(id);
    if (!Number.isFinite(workflowId) || workflowId <= 0) {
      return NextResponse.json({ error: 'Invalid workflow id' }, { status: 400 });
    }

    const { searchParams } = new URL(req.url);
    const rawPage = Number(searchParams.get('page') || '1');
    const rawLimit = Number(searchParams.get('limit') || '25');
    const eventType = (searchParams.get('eventType') || '').trim() || undefined;

    if (!Number.isFinite(rawPage) || rawPage < 1) {
      return NextResponse.json({ error: 'page must be >= 1' }, { status: 400 });
    }
    if (!Number.isFinite(rawLimit) || rawLimit < 1 || rawLimit > 100) {
      return NextResponse.json({ error: 'limit must be in [1, 100]' }, { status: 400 });
    }

    const result = await getTradeBuilderWorkflowEvents({
      userId: user.userId,
      workflowId,
      page: Math.floor(rawPage),
      limit: Math.floor(rawLimit),
      eventType,
    });
    return NextResponse.json(result);
  } catch (err) {
    console.error('Trade builder workflow events list error:', err);
    return NextResponse.json({ error: 'Failed to load workflow events' }, { status: 500 });
  }
}
