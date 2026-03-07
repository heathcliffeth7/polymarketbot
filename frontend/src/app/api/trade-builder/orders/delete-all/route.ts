import { NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { hardDeleteAllTradeBuilderOrders } from '@/lib/queries/trade-builder';

export const dynamic = 'force-dynamic';

export async function POST() {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const deletedCount = await hardDeleteAllTradeBuilderOrders(user.userId);
    return NextResponse.json({ success: true, deletedCount });
  } catch (err) {
    console.error('Delete all orders error:', err);
    return NextResponse.json({ error: 'Failed to delete all orders' }, { status: 500 });
  }
}
