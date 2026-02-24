import { NextRequest, NextResponse } from 'next/server';
import {
  createTradeFlowDefinition,
  getTradeFlowDefinitions,
  normalizeTradeFlowGraph,
} from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  try {
    const { searchParams } = new URL(req.url);
    const page = Number(searchParams.get('page') || '1');
    const limit = Number(searchParams.get('limit') || '20');
    const status = (searchParams.get('status') || '').trim() || undefined;
    const autoMigrateLegacy = (searchParams.get('autoMigrateLegacy') || '1') !== '0';

    if (!Number.isFinite(page) || page < 1) {
      return NextResponse.json({ error: 'page must be >= 1' }, { status: 400 });
    }
    if (!Number.isFinite(limit) || limit < 1 || limit > 100) {
      return NextResponse.json({ error: 'limit must be in [1,100]' }, { status: 400 });
    }

    const result = await getTradeFlowDefinitions({
      page: Math.floor(page),
      limit: Math.floor(limit),
      status,
      autoMigrateLegacy,
    });

    return NextResponse.json(result);
  } catch (err) {
    console.error('Trade flow definition list error:', err);
    return NextResponse.json({ error: 'Failed to load flow definitions' }, { status: 500 });
  }
}

export async function POST(req: NextRequest) {
  try {
    const body = await req.json();
    const name = String(body?.name || '').trim();
    const description = body?.description == null ? null : String(body.description);
    const legacyWorkflowId =
      body?.legacyWorkflowId == null ? undefined : Number(body.legacyWorkflowId);
    const graphJson =
      body?.graphJson == null ? { context: {}, nodes: [], edges: [] } : body.graphJson;

    if (!name) {
      return NextResponse.json({ error: 'name is required' }, { status: 400 });
    }

    if (
      legacyWorkflowId !== undefined &&
      (!Number.isFinite(legacyWorkflowId) || legacyWorkflowId <= 0)
    ) {
      return NextResponse.json({ error: 'legacyWorkflowId must be > 0' }, { status: 400 });
    }

    const normalized = normalizeTradeFlowGraph(graphJson);

    const data = await createTradeFlowDefinition({
      name,
      description,
      graphJson: normalized,
      legacyWorkflowId,
    });

    return NextResponse.json({ data }, { status: 201 });
  } catch (err) {
    console.error('Trade flow definition create error:', err);
    return NextResponse.json(
      { error: err instanceof Error ? err.message : 'Failed to create flow definition' },
      { status: 500 }
    );
  }
}
