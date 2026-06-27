import { NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import {
  draftAllPublishedTradeFlowDefinitions,
  mapTradeFlowMutationHttpError,
} from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function POST() {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }

    const data = await draftAllPublishedTradeFlowDefinitions(user.userId);
    return NextResponse.json({ data });
  } catch (err) {
    const mapped = mapTradeFlowMutationHttpError(err, 'Failed to draft published flows');
    if (mapped.status === 423) {
      return NextResponse.json(mapped.body, { status: mapped.status });
    }
    console.error('Trade flow draft-all error:', err);
    return NextResponse.json(mapped.body, { status: mapped.status });
  }
}
