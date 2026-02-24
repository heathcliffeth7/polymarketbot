import { NextRequest, NextResponse } from 'next/server';
import { getFills } from '@/lib/queries/fills';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const { searchParams } = new URL(req.url);
    const result = await getFills({
      page: parseInt(searchParams.get('page') || '1'),
      limit: parseInt(searchParams.get('limit') || '20'),
      orderId: searchParams.get('orderId') ? parseInt(searchParams.get('orderId')!) : undefined,
    });
    return NextResponse.json(result);
  } catch (err) {
    console.error('Fills error:', err);
    return NextResponse.json({ error: 'Failed to load fills' }, { status: 500 });
  }
}
