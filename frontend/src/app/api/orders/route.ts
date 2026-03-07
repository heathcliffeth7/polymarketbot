import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { getOrders } from '@/lib/queries/orders';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const { searchParams } = new URL(req.url);
    const result = await getOrders({
      userId: user.userId,
      page: parseInt(searchParams.get('page') || '1'),
      limit: parseInt(searchParams.get('limit') || '20'),
      tradeId: searchParams.get('tradeId') ? parseInt(searchParams.get('tradeId')!) : undefined,
      status: searchParams.get('status') || undefined,
      intent: searchParams.get('intent') || undefined,
    });
    return NextResponse.json(result);
  } catch (err) {
    console.error('Orders error:', err);
    return NextResponse.json({ error: 'Failed to load orders' }, { status: 500 });
  }
}
