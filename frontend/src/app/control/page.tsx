'use client';

import { PageShell } from '@/components/layout/page-shell';
import { BotControlPanel } from '@/components/control/bot-control-panel';
import { ModeSwitch } from '@/components/control/mode-switch';
import { KillSwitchToggle } from '@/components/control/kill-switch-toggle';
import { RelaxToggle } from '@/components/control/relax-toggle';

export default function ControlPage() {
  return (
    <PageShell title="Bot Control">
      <div className="grid gap-4 md:grid-cols-2">
        <div className="space-y-4">
          <BotControlPanel />
          <ModeSwitch />
        </div>
        <div className="space-y-4">
          <KillSwitchToggle />
          <RelaxToggle />
        </div>
      </div>
    </PageShell>
  );
}
