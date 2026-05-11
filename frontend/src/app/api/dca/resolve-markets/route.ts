import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { resolveDcaMarketInputs } from '@/lib/dca-bot/resolver';
import type { DcaResolveMarketsRequest } from '@/lib/dca-bot/schema';

export const dynamic = 'force-dynamic';

export async function POST(req: NextRequest) {
  const user = await getSessionUser().catch(() => null);
  if (!user) {
    return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
  }

  try {
    const body = (await req.json()) as Partial<DcaResolveMarketsRequest>;
    const inputs = Array.isArray(body.inputs)
      ? body.inputs.map((input) => String(input).trim()).filter(Boolean)
      : [];
    if (inputs.length === 0) {
      return NextResponse.json({ error: 'inputs is required' }, { status: 400 });
    }
    if (inputs.length > 20) {
      return NextResponse.json({ error: 'inputs length cannot exceed 20' }, { status: 400 });
    }
    const markets = await resolveDcaMarketInputs(inputs);
    return NextResponse.json({ markets });
  } catch (err) {
    console.error('DCA resolve-markets error:', err);
    return NextResponse.json({ error: 'failed_to_resolve_markets' }, { status: 500 });
  }
}
