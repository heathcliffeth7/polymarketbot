import { NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { getTradeFlowOverlapSummary } from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function GET() {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }

    const data = await getTradeFlowOverlapSummary(user.userId);
    return NextResponse.json({ data });
  } catch (err) {
    console.error('Trade flow overlap error:', err);
    return NextResponse.json({ error: 'Failed to load trade flow overlap' }, { status: 500 });
  }
}
