import { NextRequest, NextResponse } from 'next/server';
import { getSessionUser } from '@/lib/auth';
import { readConfig, writeConfig } from '@/lib/config';
import { checkControlCapability, controlService } from '@/lib/systemctl';

export const dynamic = 'force-dynamic';

export async function GET() {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const [risk, capability] = await Promise.all([
      readConfig('risk', user),
      checkControlCapability(),
    ]);
    return NextResponse.json({
      kill_switch_mode: risk.kill_switch_mode,
      manual_kill_switch_active: risk.manual_kill_switch_active,
      control_available: capability.available,
      control_reason: capability.reason,
      control_reason_code: capability.reasonCode,
    });
  } catch (err) {
    console.error('Kill switch read error:', err);
    return NextResponse.json({ error: 'Failed to read kill switch status' }, { status: 500 });
  }
}

export async function POST(req: NextRequest) {
  try {
    const user = await getSessionUser();
    if (!user) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    const { active } = await req.json();
    const risk = await readConfig('risk', user);

    if (risk.kill_switch_mode === 'disabled' && active) {
      return NextResponse.json(
        { error: 'Cannot activate kill switch when mode is disabled' },
        { status: 400 }
      );
    }

    risk.manual_kill_switch_active = !!active;
    await writeConfig('risk', risk, user);
    const restart = await controlService('restart');

    if (restart.controlAvailable && !restart.success) {
      return NextResponse.json({ error: restart.message }, { status: 500 });
    }

    return NextResponse.json({
      success: true,
      manual_kill_switch_active: risk.manual_kill_switch_active,
      restart_applied: restart.success,
      control_available: restart.controlAvailable,
      restart_message: restart.message,
      control_reason: restart.controlReason,
      control_reason_code: restart.controlReasonCode,
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Failed to toggle kill switch';
    console.error('Kill switch error:', err);
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
