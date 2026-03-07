import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { hardDeleteTradeBuilderOrder } from '@/lib/queries/trade-builder';

export const dynamic = 'force-dynamic';

export async function POST(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const body = await req.json();
    const ids: number[] = Array.isArray(body?.ids)
      ? body.ids.map((id: unknown) => Number(id)).filter((id: number) => Number.isFinite(id) && id > 0)
      : [];

    if (ids.length === 0) {
      return NextResponse.json({ error: 'No valid ids provided' }, { status: 400 });
    }

    const results: { id: number; success: boolean; error?: string }[] = [];
    for (const id of ids) {
      try {
        await hardDeleteTradeBuilderOrder(user.userId, id);
        results.push({ id, success: true });
      } catch (err) {
        results.push({ id, success: false, error: String(err) });
      }
    }

    return NextResponse.json({ success: true, results });
  } catch (err) {
    console.error('Bulk cancel orders error:', err);
    return NextResponse.json({ error: 'Failed to bulk cancel orders' }, { status: 500 });
  }
}
