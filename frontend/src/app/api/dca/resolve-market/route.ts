import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { resolveDcaMarketInput } from '@/lib/dca-bot/resolver';
import type { DcaResolveMarketRequest } from '@/lib/dca-bot/schema';

export const dynamic = 'force-dynamic';

export async function POST(req: NextRequest) {
  const user = await getSessionUser().catch(() => null);
  if (!user) {
    return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
  }

  try {
    const body = (await req.json()) as Partial<DcaResolveMarketRequest>;
    const input = String(body.input ?? '').trim();
    if (!input) {
      return NextResponse.json({ error: 'input is required' }, { status: 400 });
    }
    const market = await resolveDcaMarketInput(input);
    if (!market) {
      return NextResponse.json({ error: 'market_not_found' }, { status: 404 });
    }
    return NextResponse.json({ market });
  } catch (err) {
    console.error('DCA resolve-market error:', err);
    return NextResponse.json({ error: 'failed_to_resolve_market' }, { status: 500 });
  }
}
