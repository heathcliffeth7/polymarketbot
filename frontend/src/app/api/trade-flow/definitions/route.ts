import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import {
  compactTelemetryError,
  getPoolTelemetrySnapshot,
  isFlowTelemetryEnabled,
} from '@/lib/db';
import {
  createTradeFlowDefinition,
  getTradeFlowDefinitions,
  normalizeTradeFlowGraph,
} from '@/lib/queries/trade-flow';
import {
  FLOW_DUPLICATE_NAME_MESSAGE,
  FLOW_INVALID_NAME_MESSAGE,
  validateTradeFlowDefinitionName,
} from '@/lib/queries/trade-flow/name-policy';

export const dynamic = 'force-dynamic';

export async function GET(req: NextRequest) {
  const telemetryEnabled = isFlowTelemetryEnabled();
  const startedAt = telemetryEnabled ? performance.now() : 0;
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const { searchParams } = new URL(req.url);
    const page = Number(searchParams.get('page') || '1');
    const limit = Number(searchParams.get('limit') || '20');
    const status = (searchParams.get('status') || '').trim() || undefined;
    const autoMigrateLegacy = (searchParams.get('autoMigrateLegacy') || '0') !== '0';

    if (!Number.isFinite(page) || page < 1) {
      return NextResponse.json({ error: 'page must be >= 1' }, { status: 400 });
    }
    if (!Number.isFinite(limit) || limit < 1 || limit > 100) {
      return NextResponse.json({ error: 'limit must be in [1,100]' }, { status: 400 });
    }

    const result = await getTradeFlowDefinitions({
      userId: user.userId,
      page: Math.floor(page),
      limit: Math.floor(limit),
      status,
      autoMigrateLegacy,
    });

    if (telemetryEnabled) {
      console.log(
        `[def-list] outcome=ok elapsed=${Math.round(performance.now() - startedAt)}ms pool=${getPoolTelemetrySnapshot()}`
      );
    }
    return NextResponse.json(result);
  } catch (err) {
    if (telemetryEnabled) {
      console.log(
        `[def-list] outcome=error elapsed=${Math.round(performance.now() - startedAt)}ms pool=${getPoolTelemetrySnapshot()} err=${compactTelemetryError(err)}`
      );
    }
    console.error('Trade flow definition list error:', err);
    return NextResponse.json({ error: 'Failed to load flow definitions' }, { status: 500 });
  }
}

export async function POST(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const body = await req.json();
    const name = validateTradeFlowDefinitionName(String(body?.name || ''));
    const description = body?.description == null ? null : String(body.description);
    const legacyWorkflowId =
      body?.legacyWorkflowId == null ? undefined : Number(body.legacyWorkflowId);
    const graphJson =
      body?.graphJson == null ? { context: {}, nodes: [], edges: [] } : body.graphJson;

    if (
      legacyWorkflowId !== undefined &&
      (!Number.isFinite(legacyWorkflowId) || legacyWorkflowId <= 0)
    ) {
      return NextResponse.json({ error: 'legacyWorkflowId must be > 0' }, { status: 400 });
    }

    const normalized = normalizeTradeFlowGraph(graphJson);

    const data = await createTradeFlowDefinition({
      userId: user.userId,
      name,
      description,
      graphJson: normalized,
      legacyWorkflowId,
    });

    return NextResponse.json({ data }, { status: 201 });
  } catch (err) {
    console.error('Trade flow definition create error:', err);
    if (err instanceof Error && err.message === FLOW_DUPLICATE_NAME_MESSAGE) {
      return NextResponse.json({ error: err.message }, { status: 409 });
    }
    if (
      err instanceof Error &&
      (err.message === 'Flow name is required' || err.message === FLOW_INVALID_NAME_MESSAGE)
    ) {
      return NextResponse.json({ error: err.message }, { status: 400 });
    }
    return NextResponse.json(
      { error: err instanceof Error ? err.message : 'Failed to create flow definition' },
      { status: 500 }
    );
  }
}
