import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import {
  mapTradeFlowMutationHttpError,
  publishTradeFlowDefinition,
} from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function POST(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const { id } = await params;
    const definitionId = Number(id);
    if (!Number.isFinite(definitionId) || definitionId <= 0) {
      return NextResponse.json({ error: 'Invalid definition id' }, { status: 400 });
    }

    const data = await publishTradeFlowDefinition(user, definitionId);
    return NextResponse.json({ data });
  } catch (err) {
    if (err instanceof Error && err.message === 'Flow definition not found') {
      return NextResponse.json({ error: err.message }, { status: 404 });
    }
    if (
      err instanceof Error &&
      err.message.includes('trigger.market_price custom_range mutated during')
    ) {
      return NextResponse.json({ error: err.message }, { status: 400 });
    }
    const mapped = mapTradeFlowMutationHttpError(err, 'Failed to publish flow');
    if (mapped.status === 423) {
      return NextResponse.json(mapped.body, { status: mapped.status });
    }
    console.error('Trade flow publish error:', err);
    return NextResponse.json(mapped.body, { status: mapped.status });
  }
}
