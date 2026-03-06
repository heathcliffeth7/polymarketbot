import { NextRequest, NextResponse } from 'next/server';
import {
  getTradeFlowDefinitionById,
  normalizeTradeFlowGraph,
  validateTradeFlowGraphWithRuntimeConfig,
} from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function POST(
  req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const { id } = await params;
    const definitionId = Number(id);
    if (!Number.isFinite(definitionId) || definitionId <= 0) {
      return NextResponse.json({ error: 'Invalid definition id' }, { status: 400 });
    }

    const body = await req.json().catch(() => ({}));
    let graphJson: unknown = body?.graphJson;

    if (graphJson === undefined) {
      const detail = await getTradeFlowDefinitionById(definitionId);
      if (!detail || !detail.draftVersion) {
        return NextResponse.json({ error: 'Flow definition/draft not found' }, { status: 404 });
      }
      graphJson = detail.draftVersion.graph_json;
    }

    const normalized = normalizeTradeFlowGraph(graphJson);
    const validation = await validateTradeFlowGraphWithRuntimeConfig(normalized);
    return NextResponse.json({ data: validation });
  } catch (err) {
    console.error('Trade flow validate error:', err);
    return NextResponse.json({ error: 'Failed to validate flow graph' }, { status: 500 });
  }
}
