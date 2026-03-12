import { NextRequest, NextResponse } from 'next/server';
import { readClaimRelayerConfigForServer } from '@/lib/config';
import {
  ClaimRelayerRouteError,
  type ClaimRedeemRequestBody,
  submitClaimViaBuilderRelayer,
} from '@/lib/claim-relayer';

export const runtime = 'nodejs';
export const dynamic = 'force-dynamic';

export async function POST(req: NextRequest) {
  try {
    const expectedToken = String(process.env.CLAIM_RELAYER_ADAPTER_TOKEN ?? '').trim();
    if (!expectedToken) {
      return NextResponse.json(
        {
          code: 'missing_adapter_token',
          retryable: false,
          message: 'CLAIM_RELAYER_ADAPTER_TOKEN is not configured',
        },
        { status: 500 }
      );
    }

    const authHeader = req.headers.get('authorization') ?? '';
    const actualToken = authHeader.startsWith('Bearer ') ? authHeader.slice(7).trim() : '';
    if (!actualToken || actualToken !== expectedToken) {
      return NextResponse.json(
        {
          code: 'unauthorized',
          retryable: false,
          message: 'Unauthorized',
        },
        { status: 401 }
      );
    }

    const body = (await req.json()) as ClaimRedeemRequestBody;
    if (!Number.isFinite(Number(body?.userId)) || Number(body.userId) <= 0) {
      return NextResponse.json(
        {
          code: 'invalid_user_id',
          retryable: false,
          message: 'userId must be a positive integer',
        },
        { status: 400 }
      );
    }
    const context = { userId: Number(body.userId), username: `internal-${body.userId}` };
    const config = await readClaimRelayerConfigForServer(context);
    const result = await submitClaimViaBuilderRelayer(config, body);

    return NextResponse.json(result);
  } catch (err) {
    if (err instanceof ClaimRelayerRouteError) {
      return NextResponse.json(
        {
          code: err.code,
          retryable: err.retryable,
          message: err.message,
        },
        { status: err.status }
      );
    }

    const message = err instanceof Error ? err.message : 'Failed to submit builder relayer claim';
    console.error('Internal claim redeem error:', err);
    return NextResponse.json(
      {
        code: 'internal_error',
        retryable: true,
        message,
      },
      { status: 500 }
    );
  }
}
