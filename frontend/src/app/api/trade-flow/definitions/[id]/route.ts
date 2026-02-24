import { NextRequest, NextResponse } from 'next/server';
import {
  getTradeFlowDefinitionById,
  normalizeTradeFlowGraph,
  updateTradeFlowDefinitionDraft,
} from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const { id } = await params;
    const definitionId = Number(id);
    if (!Number.isFinite(definitionId) || definitionId <= 0) {
      return NextResponse.json({ error: 'Invalid definition id' }, { status: 400 });
    }

    const data = await getTradeFlowDefinitionById(definitionId);
    if (!data) {
      return NextResponse.json({ error: 'Flow definition not found' }, { status: 404 });
    }

    return NextResponse.json({ data });
  } catch (err) {
    console.error('Trade flow definition detail error:', err);
    return NextResponse.json({ error: 'Failed to load flow definition' }, { status: 500 });
  }
}

export async function PATCH(
  req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const { id } = await params;
    const definitionId = Number(id);
    if (!Number.isFinite(definitionId) || definitionId <= 0) {
      return NextResponse.json({ error: 'Invalid definition id' }, { status: 400 });
    }

    const body = await req.json();
    const updates: {
      name?: string;
      description?: string | null;
      graphJson?: unknown;
    } = {};

    if (body?.name !== undefined) {
      const name = String(body.name || '').trim();
      if (!name) return NextResponse.json({ error: 'name cannot be empty' }, { status: 400 });
      updates.name = name;
    }

    if (body?.description !== undefined) {
      updates.description = body.description == null ? null : String(body.description);
    }

    if (body?.graphJson !== undefined) {
      updates.graphJson = normalizeTradeFlowGraph(body.graphJson);
    }

    const data = await updateTradeFlowDefinitionDraft(definitionId, updates);
    return NextResponse.json({ data });
  } catch (err) {
    console.error('Trade flow definition patch error:', err);
    return NextResponse.json(
      { error: err instanceof Error ? err.message : 'Failed to update flow definition' },
      { status: 500 }
    );
  }
}
