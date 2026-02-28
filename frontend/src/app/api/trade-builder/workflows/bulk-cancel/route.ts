import { NextRequest, NextResponse } from 'next/server';
import { hardDeleteTradeBuilderWorkflow } from '@/lib/queries/trade-builder';

export const dynamic = 'force-dynamic';

export async function POST(req: NextRequest) {
  try {
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
        await hardDeleteTradeBuilderWorkflow(id);
        results.push({ id, success: true });
      } catch (err) {
        results.push({ id, success: false, error: String(err) });
      }
    }

    return NextResponse.json({ success: true, results });
  } catch (err) {
    console.error('Bulk cancel workflows error:', err);
    return NextResponse.json({ error: 'Failed to bulk cancel workflows' }, { status: 500 });
  }
}
