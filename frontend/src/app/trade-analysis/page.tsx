'use client';

import { PageShell } from '@/components/layout/page-shell';
import { AutoScopeAnalysisTable } from '@/components/trade-analysis/auto-scope-analysis-table';

export default function TradeAnalysisPage() {
  return (
    <PageShell title="İşlem Analizi">
      <div className="space-y-6">
        <AutoScopeAnalysisTable />
      </div>
    </PageShell>
  );
}
