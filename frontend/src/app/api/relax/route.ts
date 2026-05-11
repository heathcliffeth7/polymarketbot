import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { readConfig, writeConfig } from '@/lib/config';
import { checkControlCapability, controlService } from '@/lib/systemctl';

export const dynamic = 'force-dynamic';

const RELAX_CONFIG_KEY = 'max_price_relax_enabled';

function resolveRelaxEnabled(strategy: Record<string, unknown>): boolean {
  return strategy[RELAX_CONFIG_KEY] !== false;
}

function parseRelaxEnabled(body: Record<string, unknown>): boolean | null {
  if (typeof body.enabled === 'boolean') {
    return body.enabled;
  }

  const action = String(body.action ?? '').trim().toLowerCase();
  if (action === 'on') return true;
  if (action === 'off') return false;
  return null;
}

export async function GET() {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }

    const [strategy, capability] = await Promise.all([
      readConfig('strategy', user),
      checkControlCapability(),
    ]);

    return NextResponse.json({
      max_price_relax_enabled: resolveRelaxEnabled(strategy),
      control_available: capability.available,
      control_reason: capability.reason,
      control_reason_code: capability.reasonCode,
    });
  } catch (err) {
    console.error('Relax read error:', err);
    return NextResponse.json({ error: 'Failed to read relax status' }, { status: 500 });
  }
}

export async function POST(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }

    const body = (await req.json().catch(() => ({}))) as Record<string, unknown>;
    const enabled = parseRelaxEnabled(body);
    if (enabled == null) {
      return NextResponse.json(
        { error: 'enabled boolean or action on/off is required' },
        { status: 400 }
      );
    }

    const strategy = await readConfig('strategy', user);
    strategy[RELAX_CONFIG_KEY] = enabled;
    await writeConfig('strategy', strategy, user);

    const restart = await controlService('restart');
    if (restart.controlAvailable && !restart.success) {
      return NextResponse.json({ error: restart.message }, { status: 500 });
    }

    return NextResponse.json({
      success: true,
      max_price_relax_enabled: enabled,
      restart_applied: restart.success,
      control_available: restart.controlAvailable,
      restart_message: restart.message,
      control_reason: restart.controlReason,
      control_reason_code: restart.controlReasonCode,
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Failed to toggle relax';
    console.error('Relax toggle error:', err);
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
