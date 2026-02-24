import { NextRequest, NextResponse } from 'next/server';
import { getTrades } from '@/lib/queries/trades';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const { searchParams } = new URL(req.url);
    const result = await getTrades({
      page: parseInt(searchParams.get('page') || '1'),
      limit: parseInt(searchParams.get('limit') || '20'),
      state: searchParams.get('state') || undefined,
      from: searchParams.get('from') || undefined,
      to: searchParams.get('to') || undefined,
    });
    return NextResponse.json(result);
  } catch (err) {
    console.error('Trades error:', err);
    return NextResponse.json({ error: 'Failed to load trades' }, { status: 500 });
  }
}
