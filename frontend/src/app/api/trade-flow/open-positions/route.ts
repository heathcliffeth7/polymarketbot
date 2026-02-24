import { NextResponse } from 'next/server';
import { getTradeFlowOpenPositions } from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function GET() {
  try {
    const payload = await getTradeFlowOpenPositions();
    return NextResponse.json(payload);
  } catch (err) {
    console.error('Trade flow open positions error:', err);
    return NextResponse.json(
      { error: err instanceof Error ? err.message : 'Open positions alınamadı' },
      { status: 500 }
    );
  }
}
