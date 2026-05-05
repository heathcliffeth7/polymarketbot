import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import {
  activateClaimFunds,
  ClaimFundsActivationServiceError,
} from '@/lib/claim-funds-activation';

export const runtime = 'nodejs';
export const dynamic = 'force-dynamic';

export async function POST(req: NextRequest) {
  try {
    void req;
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }

    const result = await activateClaimFunds(user);
    return NextResponse.json(result);
  } catch (err) {
    if (err instanceof ClaimFundsActivationServiceError) {
      return NextResponse.json(
        {
          error: err.message,
          code: err.code,
        },
        { status: err.status }
      );
    }

    const message =
      err instanceof Error ? err.message : 'Failed to activate claim funds';
    console.error('Claim funds activation error:', err);
    return NextResponse.json(
      {
        error: message,
        code: 'claim_funds_activation_failed',
      },
      { status: 500 }
    );
  }
}
