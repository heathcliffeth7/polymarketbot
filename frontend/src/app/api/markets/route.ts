import { NextRequest, NextResponse } from 'next/server';
import { getMarkets } from '@/lib/queries/markets';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const { searchParams } = new URL(req.url);
    const result = await getMarkets({
      page: parseInt(searchParams.get('page') || '1'),
      limit: parseInt(searchParams.get('limit') || '20'),
      status: searchParams.get('status') || undefined,
    });
    return NextResponse.json(result);
  } catch (err) {
    console.error('Markets error:', err);
    return NextResponse.json({ error: 'Failed to load markets' }, { status: 500 });
  }
}
