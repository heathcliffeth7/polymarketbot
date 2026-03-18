import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import {
  compactTelemetryError,
  getPoolTelemetrySnapshot,
  isFlowTelemetryEnabled,
} from '@/lib/db';
import {
  getTradeFlowDefinitionById,
  hardDeleteTradeFlowDefinition,
  normalizeTradeFlowGraph,
  updateTradeFlowDefinitionDraft,
} from '@/lib/queries/trade-flow';

export const dynamic = 'force-dynamic';

export async function GET(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const telemetryEnabled = isFlowTelemetryEnabled();
  const startedAt = telemetryEnabled ? performance.now() : 0;
  let definitionId: number | null = null;
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const { id } = await params;
    definitionId = Number(id);
    if (!Number.isFinite(definitionId) || definitionId <= 0) {
      return NextResponse.json({ error: 'Invalid definition id' }, { status: 400 });
    }

    const data = await getTradeFlowDefinitionById(user.userId, definitionId);
    if (!data) {
      if (telemetryEnabled) {
        console.log(
          `[detail-read] outcome=not_found def=${definitionId} elapsed=${Math.round(performance.now() - startedAt)}ms pool=${getPoolTelemetrySnapshot()}`
        );
      }
      return NextResponse.json({ error: 'Flow definition not found' }, { status: 404 });
    }

    if (telemetryEnabled) {
      console.log(
        `[detail-read] outcome=ok def=${definitionId} elapsed=${Math.round(performance.now() - startedAt)}ms pool=${getPoolTelemetrySnapshot()}`
      );
    }
    return NextResponse.json({ data });
  } catch (err) {
    if (telemetryEnabled) {
      console.log(
        `[detail-read] outcome=error def=${definitionId ?? 'na'} elapsed=${Math.round(performance.now() - startedAt)}ms pool=${getPoolTelemetrySnapshot()} err=${compactTelemetryError(err)}`
      );
    }
    console.error('Trade flow definition detail error:', err);
    return NextResponse.json({ error: 'Failed to load flow definition' }, { status: 500 });
  }
}

export async function PATCH(
  req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const telemetryEnabled = isFlowTelemetryEnabled();
  const startedAt = telemetryEnabled ? performance.now() : 0;
  if (telemetryEnabled) {
    console.log(`[patch-entry] content-length=${req.headers.get('content-length') ?? 'na'} pool=${getPoolTelemetrySnapshot()}`);
  }
  let definitionId: number | null = null;
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const { id } = await params;
    definitionId = Number(id);
    if (!Number.isFinite(definitionId) || definitionId <= 0) {
      return NextResponse.json({ error: 'Invalid definition id' }, { status: 400 });
    }
    if (telemetryEnabled) {
      console.log(`[patch-auth] def=${definitionId} elapsed=${Math.round(performance.now() - startedAt)}ms`);
    }

    const body = await req.json();
    if (telemetryEnabled) {
      console.log(`[patch-body] def=${definitionId} elapsed=${Math.round(performance.now() - startedAt)}ms`);
    }
    const updates: {
      name?: string;
      description?: string | null;
      graphJson?: unknown;
      syncNormalizedTables?: boolean;
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

    if (body?.syncNormalizedTables !== undefined) {
      updates.syncNormalizedTables = body.syncNormalizedTables === true;
    }

    const data = await updateTradeFlowDefinitionDraft(user.userId, definitionId, updates);
    if (telemetryEnabled) {
      console.log(
        `[patch-timing] outcome=ok def=${definitionId} elapsed=${Math.round(performance.now() - startedAt)}ms pool=${getPoolTelemetrySnapshot()}`
      );
    }
    return NextResponse.json({ data });
  } catch (err) {
    if (telemetryEnabled) {
      console.log(
        `[patch-timing] outcome=error def=${definitionId ?? 'na'} elapsed=${Math.round(performance.now() - startedAt)}ms pool=${getPoolTelemetrySnapshot()} err=${compactTelemetryError(err)}`
      );
    }
    if (err instanceof Error && err.message === 'Flow definition not found') {
      return NextResponse.json({ error: err.message }, { status: 404 });
    }
    if (err instanceof Error && err.message === 'Flow name is already in use') {
      return NextResponse.json({ error: err.message }, { status: 409 });
    }
    console.error('Trade flow definition patch error:', err);
    return NextResponse.json(
      { error: err instanceof Error ? err.message : 'Failed to update flow definition' },
      { status: 500 }
    );
  }
}

export async function DELETE(
  _req: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const { id } = await params;
    const definitionId = Number(id);
    if (!Number.isFinite(definitionId) || definitionId <= 0) {
      return NextResponse.json({ error: 'Invalid definition id' }, { status: 400 });
    }

    await hardDeleteTradeFlowDefinition(user.userId, definitionId);
    return NextResponse.json({ success: true, data: null });
  } catch (err) {
    if (err instanceof Error && err.message === 'Flow definition not found') {
      return NextResponse.json({ error: err.message }, { status: 404 });
    }
    console.error('Trade flow definition delete error:', err);
    return NextResponse.json(
      { error: err instanceof Error ? err.message : 'Failed to delete flow definition' },
      { status: 500 }
    );
  }
}
