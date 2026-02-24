import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import {
  createEmptyKeyValueDraft,
  safeJsonStringify,
  type ContextFormState,
  type PrimitiveValueType,
} from '@/lib/trade-flow-config-mappers';
import type {
  TradeFlowDefinition,
  TradeFlowEvent,
  TradeFlowGraph,
  TradeFlowRun,
  TradeFlowValidationResult,
  TradeFlowVersion,
} from '@/lib/types';
import { formatDateTime, formatRunStatus } from './flow-engine-utils';

type BusyAction = 'create' | 'save' | 'validate' | 'publish' | 'archive' | null;
type TemplateKind = 'starter' | 'sell_buy_if' | 'dca' | 'sl_tp' | 'position_monitor' | 'multi_leg_hedge';

interface FlowContextEditorProps {
  contextForm: ContextFormState;
  contextTab: 'basic' | 'advanced';
  onContextFormChange: React.Dispatch<React.SetStateAction<ContextFormState>>;
  onContextTabChange: (tab: 'basic' | 'advanced') => void;
  onApplyFromForm: () => void;
  onApplyFromAdvanced: () => void;
}

export function FlowContextEditor({
  contextForm,
  contextTab,
  onContextFormChange,
  onContextTabChange,
  onApplyFromForm,
  onApplyFromAdvanced,
}: FlowContextEditorProps) {
  return (
    <div className="space-y-2 md:col-span-3">
      <p className="text-xs text-zinc-500">Graph Context</p>
      <Tabs value={contextTab} onValueChange={(v) => onContextTabChange(v as 'basic' | 'advanced')}>
        <TabsList className="bg-zinc-800">
          <TabsTrigger value="basic">Form</TabsTrigger>
          <TabsTrigger value="advanced">Advanced</TabsTrigger>
        </TabsList>

        <TabsContent value="basic" className="space-y-3 pt-2">
          <div className="grid gap-2 md:grid-cols-2">
            <div className="space-y-1">
              <p className="text-[11px] text-zinc-500">Source Trade ID</p>
              <Input
                type="number"
                value={contextForm.sourceTradeId}
                onChange={(e) => onContextFormChange((prev) => ({ ...prev, sourceTradeId: e.target.value }))}
                className="h-8 border-zinc-700 bg-zinc-800 text-xs text-zinc-200"
              />
            </div>
            <div className="space-y-1">
              <p className="text-[11px] text-zinc-500">Market Slug</p>
              <Input
                value={contextForm.marketSlug}
                onChange={(e) => onContextFormChange((prev) => ({ ...prev, marketSlug: e.target.value }))}
                className="h-8 border-zinc-700 bg-zinc-800 text-xs text-zinc-200"
              />
            </div>
            <div className="space-y-1">
              <p className="text-[11px] text-zinc-500">Token ID</p>
              <Input
                value={contextForm.tokenId}
                onChange={(e) => onContextFormChange((prev) => ({ ...prev, tokenId: e.target.value }))}
                className="h-8 border-zinc-700 bg-zinc-800 text-xs text-zinc-200"
              />
            </div>
            <div className="space-y-1">
              <p className="text-[11px] text-zinc-500">Outcome Label</p>
              <Input
                value={contextForm.outcomeLabel}
                onChange={(e) => onContextFormChange((prev) => ({ ...prev, outcomeLabel: e.target.value }))}
                className="h-8 border-zinc-700 bg-zinc-800 text-xs text-zinc-200"
              />
            </div>
          </div>

          <div className="space-y-2 rounded-md border border-zinc-800 bg-zinc-950/60 p-2">
            <p className="text-[11px] text-zinc-400">Ek Context Alanlari</p>
            {contextForm.extras.length === 0 ? (
              <p className="text-[11px] text-zinc-500">Ek alan yok.</p>
            ) : (
              contextForm.extras.map((row) => (
                <div key={row.id} className="grid grid-cols-3 gap-2 rounded-md border border-zinc-800 p-2">
                  <Input
                    value={row.key}
                    onChange={(e) =>
                      onContextFormChange((prev) => ({
                        ...prev,
                        extras: prev.extras.map((item) =>
                          item.id === row.id ? { ...item, key: e.target.value } : item
                        ),
                      }))
                    }
                    placeholder="key"
                    className="h-8 border-zinc-700 bg-zinc-800 text-xs text-zinc-200"
                  />
                  <select
                    value={row.valueType}
                    onChange={(e) =>
                      onContextFormChange((prev) => ({
                        ...prev,
                        extras: prev.extras.map((item) =>
                          item.id === row.id ? { ...item, valueType: e.target.value as PrimitiveValueType } : item
                        ),
                      }))
                    }
                    className="h-8 rounded-md border border-zinc-700 bg-zinc-800 px-2 text-xs text-zinc-200"
                  >
                    <option value="string">string</option>
                    <option value="number">number</option>
                    <option value="boolean">boolean</option>
                  </select>
                  <Input
                    value={row.value}
                    onChange={(e) =>
                      onContextFormChange((prev) => ({
                        ...prev,
                        extras: prev.extras.map((item) =>
                          item.id === row.id ? { ...item, value: e.target.value } : item
                        ),
                      }))
                    }
                    placeholder="value"
                    className="h-8 border-zinc-700 bg-zinc-800 text-xs text-zinc-200"
                  />
                  <div className="col-span-3 flex justify-end">
                    <Button
                      size="sm" variant="outline" className="border-zinc-700 text-zinc-300"
                      onClick={() =>
                        onContextFormChange((prev) => ({
                          ...prev,
                          extras: prev.extras.filter((item) => item.id !== row.id),
                        }))
                      }>Satir Sil</Button>
                  </div>
                </div>
              ))
            )}
            <Button
              size="sm" variant="outline" className="w-full border-zinc-700 text-zinc-300"
              onClick={() =>
                onContextFormChange((prev) => ({
                  ...prev,
                  extras: [...prev.extras, createEmptyKeyValueDraft()],
                }))
              }>+ Ek Alan Ekle</Button>
          </div>
          <Button size="sm" onClick={onApplyFromForm}>Context Uygula</Button>
        </TabsContent>

        <TabsContent value="advanced" className="space-y-2 pt-2">
          <p className="text-[11px] text-amber-400">Gelismis mod JSON icindir. Normal kullanimda Form sekmesini kullanin.</p>
          <textarea
            value={contextForm.advancedJson}
            onChange={(e) => onContextFormChange((prev) => ({ ...prev, advancedJson: e.target.value }))}
            className="min-h-24 w-full rounded-md border border-zinc-700 bg-zinc-800 p-2 text-xs text-zinc-200"
          />
          <Button size="sm" onClick={onApplyFromAdvanced}>JSON Uygula</Button>
        </TabsContent>
      </Tabs>
    </div>
  );
}

interface FlowVersionsCardProps {
  versions: TradeFlowVersion[];
  versionsLoading: boolean;
}

export function FlowVersionsCard({ versions, versionsLoading }: FlowVersionsCardProps) {
  return (
    <Card className="border-zinc-800 bg-zinc-950/40">
      <CardHeader>
        <CardTitle className="text-xs text-zinc-400">Versiyonlar</CardTitle>
      </CardHeader>
      <CardContent>
        {versionsLoading ? (
          <p className="text-xs text-zinc-500">Versiyonlar yukleniyor...</p>
        ) : versions.length === 0 ? (
          <p className="text-xs text-zinc-500">Versiyon yok.</p>
        ) : (
          <div className="space-y-2">
            {versions.map((version) => (
              <div key={version.id} className="rounded-md border border-zinc-800 bg-zinc-900/60 p-2 text-xs text-zinc-300">
                <p>v{version.version_no} | {version.status}</p>
                <p className="text-zinc-500">
                  created: {formatDateTime(version.created_at)} | published: {formatDateTime(version.published_at)}
                </p>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

interface FlowRunEventsCardProps {
  runs: TradeFlowRun[];
  runEvents: TradeFlowEvent[];
  selectedRunId: number | null;
  onSelectedRunChange: (id: number) => void;
}

export function FlowRunEventsCard({ runs, runEvents, selectedRunId, onSelectedRunChange }: FlowRunEventsCardProps) {
  return (
    <Card className="border-zinc-800 bg-zinc-950/40">
      <CardHeader>
        <CardTitle className="text-xs text-zinc-400">Run ve Olaylar</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        {runs.length === 0 ? (
          <p className="text-xs text-zinc-500">Henuz run yok.</p>
        ) : (
          <>
            <select
              value={selectedRunId ?? ''}
              onChange={(e) => onSelectedRunChange(Number(e.target.value))}
              className="h-8 w-full rounded-md border border-zinc-700 bg-zinc-800 px-2 text-xs text-zinc-200"
            >
              {runs.map((run) => (
                <option key={run.id} value={run.id}>
                  Run #{run.id} | {formatRunStatus(run.status)} | {formatDateTime(run.created_at)}
                </option>
              ))}
            </select>
            <div className="max-h-64 space-y-2 overflow-auto">
              {runEvents.length === 0 ? (
                <p className="text-xs text-zinc-500">Secili run icin olay yok.</p>
              ) : (
                runEvents.map((event) => (
                  <div key={event.id} className="rounded-md border border-zinc-800 bg-zinc-900/60 p-2 text-xs text-zinc-300">
                    <p>{event.event_type}</p>
                    <p className="text-zinc-500">{formatDateTime(event.created_at)}</p>
                    <pre className="mt-1 max-h-28 overflow-auto rounded border border-zinc-800 bg-zinc-950 p-1 text-[10px] text-zinc-400">
                      {safeJsonStringify(event.payload_json)}
                    </pre>
                  </div>
                ))
              )}
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}

interface FlowSummaryBarProps {
  graph: TradeFlowGraph;
  validation: TradeFlowValidationResult | null;
}

export function FlowSummaryBar({ graph, validation }: FlowSummaryBarProps) {
  return (
    <div className="rounded-lg border border-zinc-800 bg-zinc-950/40 p-3">
      <p className="text-xs text-zinc-400">Flow Ozeti</p>
      <div className="mt-2 flex flex-wrap items-center gap-4 text-xs text-zinc-300">
        <span>Node: {graph.nodes.length}</span>
        <span>Edge: {graph.edges.length}</span>
        <span>Trigger: {graph.nodes.filter((n) => n.type.startsWith('trigger.')).length}</span>
        <span>Action: {graph.nodes.filter((n) => n.type.startsWith('action.')).length}</span>
      </div>
      {validation && (
        <div className="mt-3 space-y-2 rounded-md border border-zinc-800 bg-zinc-900/70 p-2">
          <p className="text-xs text-zinc-300">Dogrulama sonucu: {validation.valid ? 'Gecerli' : 'Hata iceriyor'}</p>
          {validation.issues.length === 0 ? (
            <p className="text-[11px] text-zinc-500">Issue bulunmadi.</p>
          ) : (
            validation.issues.map((issue, idx) => (
              <p
                key={`${issue.code}-${idx}`}
                className={issue.severity === 'error' ? 'text-[11px] text-red-400' : 'text-[11px] text-amber-400'}
              >
                {issue.severity.toUpperCase()} | {issue.code} | {issue.message}
              </p>
            ))
          )}
        </div>
      )}
    </div>
  );
}

interface CreateFlowSlotProps {
  createName: string;
  createDescription: string;
  createTemplateKind: TemplateKind;
  busyAction: BusyAction;
  isWorkflowListOpen: boolean;
  workflowListQuery: string;
  definitionsLoading: boolean;
  filteredDefinitions: TradeFlowDefinition[];
  selectedDefinitionId: number | null;
  archivingDefinitionId: number | null;
  onCreateNameChange: (v: string) => void;
  onCreateDescriptionChange: (v: string) => void;
  onTemplateKindChange: (v: TemplateKind) => void;
  onCreateFromTemplate: (kind: TemplateKind) => void;
  onToggleWorkflowList: () => void;
  onWorkflowListQueryChange: (v: string) => void;
  onSelectDefinition: (id: number) => void;
  onArchiveFromList: (id: number) => void;
  showWorkflowActions?: boolean;
  workflowActionsDisabled?: boolean;
  onSaveDraft?: () => void;
  onValidate?: () => void;
  onPublish?: () => void;
  onArchiveFlow?: () => void;
}

export function CreateFlowSlot({
  createName, createDescription, createTemplateKind, busyAction,
  isWorkflowListOpen, workflowListQuery, definitionsLoading,
  filteredDefinitions, selectedDefinitionId, archivingDefinitionId,
  onCreateNameChange, onCreateDescriptionChange, onTemplateKindChange,
  onCreateFromTemplate, onToggleWorkflowList, onWorkflowListQueryChange,
  onSelectDefinition, onArchiveFromList,
  showWorkflowActions = false, workflowActionsDisabled = false,
  onSaveDraft, onValidate, onPublish, onArchiveFlow,
}: CreateFlowSlotProps) {
  return (
    <div className="space-y-2 overflow-hidden rounded-md border border-slate-200 bg-white p-2">
      {showWorkflowActions && (
        <div className="space-y-2 rounded-md border border-slate-200 bg-slate-50 p-2">
          <p className="text-[11px] font-medium text-slate-700">Workflow Aksiyonlari</p>
          <div className="space-y-1">
            <Button
              type="button"
              size="sm"
              className="h-8 w-full"
              disabled={workflowActionsDisabled || !onSaveDraft}
              onClick={onSaveDraft}
            >
              Draft Kaydet
            </Button>
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="h-8 w-full border-slate-300 text-slate-700 hover:bg-slate-100"
              disabled={workflowActionsDisabled || !onValidate}
              onClick={onValidate}
            >
              Dogrula
            </Button>
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="h-8 w-full border-slate-300 text-slate-700 hover:bg-slate-100"
              disabled={workflowActionsDisabled || !onPublish}
              onClick={onPublish}
            >
              Publish
            </Button>
            <Button
              type="button"
              size="sm"
              variant="outline"
              className="h-8 w-full border-red-300 text-red-600 hover:bg-red-50"
              disabled={workflowActionsDisabled || !onArchiveFlow}
              onClick={onArchiveFlow}
            >
              Sil (Arsivle)
            </Button>
          </div>
        </div>
      )}

      <p className="text-[11px] font-medium text-slate-700">Yeni Workflow Olustur</p>
      <Input value={createName} onChange={(e) => onCreateNameChange(e.target.value)} placeholder="Workflow adi" className="h-8 border-slate-300 bg-white text-xs text-slate-900" />
      <Input value={createDescription} onChange={(e) => onCreateDescriptionChange(e.target.value)} placeholder="Aciklama (opsiyonel)" className="h-8 border-slate-300 bg-white text-xs text-slate-900" />
      <select value={createTemplateKind} onChange={(e) => onTemplateKindChange(e.target.value as TemplateKind)} className="h-8 w-full rounded-md border border-slate-300 bg-white px-2 text-xs text-slate-900">
        <option value="starter">Starter Sablon</option>
        <option value="sell_buy_if">Satis + If/Else + Alis</option>
        <option value="dca">DCA (Zamana Dayali Alis)</option>
        <option value="sl_tp">Stop Loss + Take Profit</option>
        <option value="position_monitor">Pozisyon Izleme + Bildirim</option>
        <option value="multi_leg_hedge">Multi-Leg Hedge</option>
      </select>
      <Button type="button" size="sm" className="h-8 w-full" disabled={busyAction !== null} onClick={() => onCreateFromTemplate(createTemplateKind)}>
        {busyAction === 'create' ? 'Workflow Olusturuluyor...' : 'Workflow Olustur'}
      </Button>
      <p className="text-[10px] text-slate-500">Sablon secili piyasa/sonuc ile dolar, sonrasinda canvas uzerinde duzenleyebilirsiniz.</p>

      <button type="button" className="flex h-8 w-full items-center justify-between rounded-md border border-slate-300 px-2 text-left text-xs text-slate-700 hover:bg-slate-100" onClick={onToggleWorkflowList}>
        <span>Workflow Listesi</span>
        <span className="text-[10px] text-slate-500">{isWorkflowListOpen ? 'Gizle' : 'Goster'}</span>
      </button>
      <p className="text-[10px] text-slate-500">Sil butonu workflowu arsivler ve listeden kaldirir.</p>

      {isWorkflowListOpen && (
        <div className="space-y-2 rounded-md border border-slate-200 bg-slate-50 p-2">
          <Input value={workflowListQuery} onChange={(e) => onWorkflowListQueryChange(e.target.value)} placeholder="Workflow ara..." className="h-8 border-slate-300 bg-white text-xs text-slate-900" />
          <div className="max-h-48 space-y-1 overflow-auto pr-1">
            {definitionsLoading ? (
              <p className="text-[11px] text-slate-500">Workflow listesi yukleniyor...</p>
            ) : filteredDefinitions.length === 0 ? (
              <p className="text-[11px] text-slate-500">Workflow bulunamadi.</p>
            ) : (
              filteredDefinitions.map((def) => (
                <div key={def.id} className="flex items-stretch gap-1">
                  <button type="button" onClick={() => onSelectDefinition(def.id)}
                    className={`min-w-0 flex-1 rounded-md border px-2 py-1.5 text-left ${selectedDefinitionId === def.id ? 'border-sky-300 bg-sky-100' : 'border-slate-300 bg-white hover:bg-slate-100'}`}>
                    <p className="truncate text-[11px] font-medium text-slate-800">#{def.id} - {def.name}</p>
                    <p className="text-[10px] text-slate-500">{def.status}</p>
                  </button>
                  <Button type="button" size="sm" variant="outline"
                    className="h-auto min-h-0 whitespace-nowrap border-red-300 px-2 py-1 text-[11px] text-red-600 hover:bg-red-50"
                    disabled={busyAction !== null} onClick={() => onArchiveFromList(def.id)}>
                    {archivingDefinitionId === def.id ? 'Siliniyor...' : 'Sil'}
                  </Button>
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}
