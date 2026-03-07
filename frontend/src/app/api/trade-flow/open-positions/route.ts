import { NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { getTradeFlowOpenPositions } from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function GET() {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const payload = await getTradeFlowOpenPositions(user);
    return NextResponse.json(payload);
  } catch (err) {
    console.error('Trade flow open positions error:', err);
    return NextResponse.json(
      { error: err instanceof Error ? err.message : 'Open positions alınamadı' },
      { status: 500 }
    );
  }
}
