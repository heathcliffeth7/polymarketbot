import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import {
  createEmptyKeyValueDraft,
  type ContextFormState,
  type PrimitiveValueType,
} from '@/lib/trade-flow-config-mappers';
import type {
  TradeFlowDefinition,
  TradeFlowGraph,
  TradeFlowValidationResult,
} from '@/lib/types';

type BusyAction = 'create' | 'save' | 'validate' | 'publish' | 'archive' | null;
type TemplateKind = 'starter' | 'sell_buy_if' | 'dca' | 'sl_tp' | 'position_monitor' | 'multi_leg_hedge';

interface FlowContextEditorProps {
  contextForm: ContextFormState;
  contextTab: 'basic' | 'advanced';
  onContextFormChange: React.Dispatch<React.SetStateAction<ContextFormState>>;
  onContextTabChange: (tab: 'basic' | 'advanced') => void;
  onApplyFromForm: () => void;
  onApplyFromAdvanced: () => void;
  onAutoClaimEnabledChange?: (enabled: boolean) => void;
}

export function FlowContextEditor({
  contextForm,
  contextTab,
  onContextFormChange,
  onContextTabChange,
  onApplyFromForm,
  onApplyFromAdvanced,
  onAutoClaimEnabledChange,
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
            <div className="md:col-span-2">
              <div className="flex items-start justify-between gap-3 rounded-md border border-zinc-800 bg-zinc-950/60 p-3">
                <div className="space-y-1">
                  <p className="text-[11px] font-medium text-zinc-300">Autoclaim</p>
                  <p className="text-[11px] text-zinc-500">
                    Acikken ayar aninda kaydedilir. Runner bir sonraki turda wallet&apos;taki
                    kazanilmis redeemable prediction&apos;lari otomatik claim etmeyi dener.
                  </p>
                </div>
                <Switch
                  checked={contextForm.autoClaimEnabled}
                  onCheckedChange={(checked) => {
                    if (onAutoClaimEnabledChange) {
                      onAutoClaimEnabledChange(checked);
                      return;
                    }
                    onContextFormChange((prev) => ({ ...prev, autoClaimEnabled: checked }));
                  }}
                />
              </div>
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

interface FlowSummaryBarProps {
  graph: TradeFlowGraph;
  validation: TradeFlowValidationResult | null;
}

export function FlowSummaryBar({ graph, validation }: FlowSummaryBarProps) {
  const autoClaimEnabled =
    graph.context?.autoClaimEnabled === true || graph.context?.autoClaimEnabled === 'true';

  return (
    <div className="rounded-lg border border-zinc-800 bg-zinc-950/40 p-3">
      <p className="text-xs text-zinc-400">Flow Ozeti</p>
      <div className="mt-2 flex flex-wrap items-center gap-4 text-xs text-zinc-300">
        <span>Node: {graph.nodes.length}</span>
        <span>Edge: {graph.edges.length}</span>
        <span>Trigger: {graph.nodes.filter((n) => n.type.startsWith('trigger.')).length}</span>
        <span>Action: {graph.nodes.filter((n) => n.type.startsWith('action.')).length}</span>
        <span>AutoClaim: {autoClaimEnabled ? 'Acik' : 'Kapali'}</span>
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
  botActive?: boolean;
  botControlAvailable?: boolean;
  onStopBot?: () => void;
  stoppingBot?: boolean;
  selectedDefinitionIds?: Set<number>;
  onToggleDefinitionSelection?: (id: number) => void;
  onSelectAllDefinitions?: () => void;
  onDeselectAllDefinitions?: () => void;
  onBulkArchive?: () => void;
  bulkArchiving?: boolean;
  autoClaimEnabled?: boolean;
  onAutoClaimEnabledChange?: (enabled: boolean) => void;
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
  botActive, botControlAvailable, onStopBot, stoppingBot,
  selectedDefinitionIds, onToggleDefinitionSelection, onSelectAllDefinitions, onDeselectAllDefinitions, onBulkArchive, bulkArchiving,
  autoClaimEnabled = false, onAutoClaimEnabledChange,
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
            {onStopBot && (
              <Button
                type="button"
                size="sm"
                variant="outline"
                className={`h-8 w-full ${botActive ? 'border-orange-300 text-orange-600 hover:bg-orange-50' : 'border-green-300 text-green-600'}`}
                disabled={workflowActionsDisabled || stoppingBot || !botControlAvailable || !botActive}
                onClick={onStopBot}
              >
                {stoppingBot ? 'Durduruluyor...' : botActive ? 'Botu Durdur' : 'Bot Durmus'}
              </Button>
            )}
            {onAutoClaimEnabledChange && (
              <label className="flex cursor-pointer items-start gap-3 rounded-md border border-emerald-200 bg-emerald-50 p-2">
                <input
                  type="checkbox"
                  className="mt-0.5 h-4 w-4 accent-emerald-600"
                  checked={autoClaimEnabled}
                  disabled={workflowActionsDisabled}
                  onChange={(e) => onAutoClaimEnabledChange(e.target.checked)}
                />
                <span className="space-y-1">
                  <span className="block text-[11px] font-medium text-emerald-900">Autoclaim</span>
                  <span className="block text-[10px] text-emerald-800">
                    Kazandigin prediction varsa checkbox&apos;i isaretledigin anda ayar kaydolur.
                    Runner bir sonraki turda wallet-wide claim dener.
                  </span>
                  <span className="block text-[10px] text-emerald-700">
                    claim.toml ve claim env ayarlari yine zorunlu.
                  </span>
                </span>
              </label>
            )}
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
          {filteredDefinitions.length > 0 && selectedDefinitionIds && onSelectAllDefinitions && onDeselectAllDefinitions && (
            <div className="flex items-center gap-2">
              <label className="flex items-center gap-1 text-[10px] text-slate-600 cursor-pointer select-none">
                <input
                  type="checkbox"
                  className="accent-red-500"
                  checked={filteredDefinitions.length > 0 && filteredDefinitions.every((d) => selectedDefinitionIds.has(d.id))}
                  onChange={(e) => { if (e.target.checked) onSelectAllDefinitions(); else onDeselectAllDefinitions(); }}
                />
                Tumunu Sec
              </label>
              {selectedDefinitionIds.size > 0 && onBulkArchive && (
                <Button type="button" size="sm" variant="outline"
                  className="h-6 border-red-300 px-2 text-[10px] text-red-600 hover:bg-red-50"
                  disabled={bulkArchiving} onClick={onBulkArchive}>
                  {bulkArchiving ? 'Siliniyor...' : `Secilenleri Sil (${selectedDefinitionIds.size})`}
                </Button>
              )}
            </div>
          )}
          <div className="max-h-48 space-y-1 overflow-auto pr-1">
            {definitionsLoading ? (
              <p className="text-[11px] text-slate-500">Workflow listesi yukleniyor...</p>
            ) : filteredDefinitions.length === 0 ? (
              <p className="text-[11px] text-slate-500">Workflow bulunamadi.</p>
            ) : (
              filteredDefinitions.map((def) => (
                <div key={def.id} className="flex items-stretch gap-1">
                  {onToggleDefinitionSelection && selectedDefinitionIds && (
                    <input
                      type="checkbox"
                      className="mt-2 accent-red-500"
                      checked={selectedDefinitionIds.has(def.id)}
                      onChange={() => onToggleDefinitionSelection(def.id)}
                    />
                  )}
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
