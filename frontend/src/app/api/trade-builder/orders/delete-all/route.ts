import { NextResponse } from 'next/server';
import { hardDeleteAllTradeBuilderOrders } from '@/lib/queries/trade-builder';

export const dynamic = 'force-dynamic';

export async function POST() {
  try {
    const deletedCount = await hardDeleteAllTradeBuilderOrders();
    return NextResponse.json({ success: true, deletedCount });
  } catch (err) {
    console.error('Delete all orders error:', err);
    return NextResponse.json({ error: 'Failed to delete all orders' }, { status: 500 });
  }
}
