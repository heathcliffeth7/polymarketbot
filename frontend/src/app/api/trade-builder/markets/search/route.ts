import { NextRequest, NextResponse } from 'next/server';
import { searchGammaMarkets } from '@/lib/queries/trade-builder';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const { searchParams } = new URL(req.url);
    const q = (searchParams.get('q') || '').trim();
    const data = await searchGammaMarkets(q);
    return NextResponse.json({ data });
  } catch (err) {
    console.error('Trade builder market search error:', err);
    return NextResponse.json({ error: 'Failed to search markets' }, { status: 500 });
  }
}
