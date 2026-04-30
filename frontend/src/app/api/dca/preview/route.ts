import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { buildDcaLadderPreview, buildDcaRiskPreview } from '@/lib/dca-bot/ladder-preview';
import type { DcaPreviewRequest } from '@/lib/dca-bot/schema';
import { validateDcaMarketSelectionConfig, validateDcaPreviewRequest } from '@/lib/dca-bot/validation';

export const dynamic = 'force-dynamic';

export async function POST(req: NextRequest) {
  const user = await getSessionUser().catch(() => null);
  if (!user) {
    return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
  }

  try {
    const body = (await req.json()) as DcaPreviewRequest;
    const dcaConfig = body.dcaConfig ?? {};
    const errors = [
      ...validateDcaMarketSelectionConfig(dcaConfig),
      ...validateDcaPreviewRequest(body),
    ];
    if (errors.length > 0) {
      return NextResponse.json({ error: 'invalid_dca_config', errors }, { status: 400 });
    }
    return NextResponse.json({
      ladderPreview: buildDcaLadderPreview(body),
      riskPreview: buildDcaRiskPreview(body),
    });
  } catch (err) {
    console.error('DCA preview error:', err);
    return NextResponse.json({ error: 'failed_to_preview_dca' }, { status: 500 });
  }
}
