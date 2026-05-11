import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { hardDeleteTradeFlowDefinition } from '@/lib/queries/trade-flow';

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

    await hardDeleteTradeFlowDefinition(user.userId, definitionId);
    return NextResponse.json({ success: true, data: null });
  } catch (err) {
    if (err instanceof Error && err.message === 'Flow definition not found') {
      return NextResponse.json({ error: err.message }, { status: 404 });
    }
    console.error('Trade flow delete alias error:', err);
    return NextResponse.json(
      { error: err instanceof Error ? err.message : 'Failed to delete flow definition' },
      { status: 500 }
    );
  }
}
