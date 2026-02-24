import { NextRequest, NextResponse } from 'next/server';
import { getMarketOutcomesBySlug } from '@/lib/queries/trade-builder';

export const dynamic = 'force-dynamic';

export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ slug: string }> }
) {
  try {
    const { slug } = await params;
    const data = await getMarketOutcomesBySlug(slug);
    return NextResponse.json({ data });
  } catch (err) {
    console.error('Trade builder outcomes error:', err);
    return NextResponse.json({ error: 'Failed to load outcomes' }, { status: 500 });
  }
}
