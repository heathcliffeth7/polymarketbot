import { Client } from 'pg';
import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { pool } from '@/lib/db';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

const ALLOWED_RUN_STATUSES = new Set([
  'queued',
  'running',
  'completed',
  'failed',
  'canceled',
]);

function encodeSseEvent(event: string, data: unknown): string {
  return `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;
}

export async function GET(req: NextRequest) {
  const user = await getSessionUser();
  if (!user) {
    return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
  }

  const { searchParams } = new URL(req.url);
  const status = (searchParams.get('status') || 'running').trim();
  if (!ALLOWED_RUN_STATUSES.has(status)) {
    return NextResponse.json({ error: 'Invalid run status' }, { status: 400 });
  }

  if (!process.env.DATABASE_URL) {
    return NextResponse.json({ error: 'DATABASE_URL is missing' }, { status: 500 });
  }

  const defsRes = await pool.query(
    `SELECT id
     FROM trade_flow_definitions
     WHERE user_id = $1`,
    [user.userId]
  );
  const allowedDefinitionIds = new Set<number>(
    defsRes.rows
      .map((row) => Number(row.id))
      .filter((id) => Number.isFinite(id))
  );

  const client = new Client({
    connectionString: process.env.DATABASE_URL,
  });
  await client.connect();
  await client.query('LISTEN trade_flow_realtime');

  const encoder = new TextEncoder();
  let heartbeatId: ReturnType<typeof setInterval> | null = null;
  let closed = false;

  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      const send = (event: string, payload: unknown) => {
        if (closed) return;
        controller.enqueue(encoder.encode(encodeSseEvent(event, payload)));
      };

      const cleanup = async () => {
        if (closed) return;
        closed = true;
        if (heartbeatId) {
          clearInterval(heartbeatId);
          heartbeatId = null;
        }
        client.removeAllListeners('notification');
        req.signal.removeEventListener('abort', handleAbort);
        try {
          await client.query('UNLISTEN trade_flow_realtime');
        } catch {}
        try {
          await client.end();
        } catch {}
        try {
          controller.close();
        } catch {}
      };

      const handleAbort = () => {
        void cleanup();
      };

      client.on('notification', (msg) => {
        if (closed || msg.channel !== 'trade_flow_realtime' || !msg.payload) return;
        try {
          const payload = JSON.parse(msg.payload) as Record<string, unknown>;
          const kind = typeof payload.kind === 'string' ? payload.kind : null;
          const definitionId = Number(payload.definition_id);
          if (!Number.isFinite(definitionId) || !allowedDefinitionIds.has(definitionId)) {
            return;
          }
          if (status === 'running' && kind === 'flow_event' && payload.run_id == null) {
            return;
          }
          if (kind === 'flow_event') {
            send('flow_event', payload);
            return;
          }
          if (kind === 'price_tick') {
            send('price_tick', payload);
          }
        } catch {
          // Ignore malformed payloads; stream should stay alive.
        }
      });

      req.signal.addEventListener('abort', handleAbort, { once: true });
      controller.enqueue(encoder.encode('retry: 1000\n\n'));
      heartbeatId = setInterval(() => {
        send('heartbeat', {
          kind: 'heartbeat',
          now: new Date().toISOString(),
        });
      }, 15000);

      send('ready', {
        kind: 'ready',
        connected_at: new Date().toISOString(),
      });
    },
    cancel() {
      if (heartbeatId) {
        clearInterval(heartbeatId);
        heartbeatId = null;
      }
      if (!closed) {
        closed = true;
        client.removeAllListeners('notification');
        void client.query('UNLISTEN trade_flow_realtime').catch(() => {});
        void client.end().catch(() => {});
      }
    },
  });

  return new Response(stream, {
    headers: {
      'Content-Type': 'text/event-stream; charset=utf-8',
      'Cache-Control': 'no-cache, no-transform',
      Connection: 'keep-alive',
    },
  });
}
