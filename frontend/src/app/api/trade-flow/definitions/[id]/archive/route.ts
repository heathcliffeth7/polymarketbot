import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { archiveTradeFlowDefinition } from '@/lib/queries/trade-flow';

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

    const data = await archiveTradeFlowDefinition(user.userId, definitionId);
    return NextResponse.json({ data });
  } catch (err) {
    if (err instanceof Error && err.message === 'Flow definition not found') {
      return NextResponse.json({ error: err.message }, { status: 404 });
    }
    console.error('Trade flow archive error:', err);
    return NextResponse.json(
      { error: err instanceof Error ? err.message : 'Failed to archive flow' },
      { status: 500 }
    );
  }
}
