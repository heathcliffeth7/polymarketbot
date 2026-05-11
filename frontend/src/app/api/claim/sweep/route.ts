import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { runClaimSweep, ClaimSweepServiceError } from '@/lib/claim-sweep';
import { getServiceStatus } from '@/lib/systemctl';

export const dynamic = 'force-dynamic';

export async function POST(req: NextRequest) {
  try {
    void req;
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }

    const serviceStatus = await getServiceStatus();
    const result = await runClaimSweep(user, {
      serviceActive: serviceStatus.serviceActive,
    });

    return NextResponse.json(result);
  } catch (err) {
    if (err instanceof ClaimSweepServiceError) {
      return NextResponse.json(
        {
          error: err.message,
          code: err.code,
        },
        { status: err.status }
      );
    }

    const message =
      err instanceof Error ? err.message : 'Failed to queue claim sweep jobs';
    console.error('Claim sweep error:', err);
    return NextResponse.json(
      {
        error: message,
        code: 'claim_sweep_failed',
      },
      { status: 500 }
    );
  }
}
