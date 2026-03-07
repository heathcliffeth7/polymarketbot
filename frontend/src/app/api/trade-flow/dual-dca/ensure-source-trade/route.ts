import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { ensureDualDcaSourceTrade } from '@/lib/queries/trade-flow';
import type { TradeFlowEnsureDualDcaSourceTradeRequest } from '@/lib/types';

export const dynamic = 'force-dynamic';

function parseOptionalPositiveInt(value: unknown): number | null {
  if (value == null || value === '') return null;
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) return null;
  return Math.floor(parsed);
}

export async function POST(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const body = (await req.json()) as Partial<TradeFlowEnsureDualDcaSourceTradeRequest> | null;
    const payload: TradeFlowEnsureDualDcaSourceTradeRequest = {
      asset: String(body?.asset || '').trim().toLowerCase() as TradeFlowEnsureDualDcaSourceTradeRequest['asset'],
      timeframe: String(body?.timeframe || '').trim().toLowerCase() as TradeFlowEnsureDualDcaSourceTradeRequest['timeframe'],
      definitionId: parseOptionalPositiveInt(body?.definitionId),
      nodeKey: body?.nodeKey == null ? null : String(body.nodeKey),
    };

    const data = await ensureDualDcaSourceTrade(user.userId, payload);
    return NextResponse.json({ data });
  } catch (err) {
    console.error('Trade flow dual_dca ensure source trade error:', err);
    return NextResponse.json(
      { error: err instanceof Error ? err.message : 'Dual DCA source trade olusturulamadi' },
      { status: 500 }
    );
  }
}
