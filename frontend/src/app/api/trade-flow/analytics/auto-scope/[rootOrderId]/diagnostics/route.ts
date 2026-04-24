import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { getAutoScopeTradeDiagnostic } from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ rootOrderId: string }> }
) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }

    const { rootOrderId: id } = await params;
    const rootOrderId = Number(id);
    if (!Number.isFinite(rootOrderId) || rootOrderId <= 0) {
      return NextResponse.json({ error: 'Invalid root order id' }, { status: 400 });
    }

    const result = await getAutoScopeTradeDiagnostic({
      userId: user.userId,
      rootOrderId: Math.floor(rootOrderId),
    });

    if (!result.diagnostic && result.rows.length === 0) {
      return NextResponse.json({ error: 'Trade diagnostic not found' }, { status: 404 });
    }

    return NextResponse.json(result);
  } catch (err) {
    console.error('Trade flow auto-scope diagnostic error:', err);
    return NextResponse.json(
      { error: 'Failed to load auto-scope trade diagnostic' },
      { status: 500 }
    );
  }
}
