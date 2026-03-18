'use client';

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { FlowCanvasEditor } from '@/components/trade-builder/flow-canvas-editor';
import { useFlowEngineController } from '@/hooks/use-flow-engine-controller';
import type { FlowEnginePanelProps } from './flow-engine-types';
import {
  CreateFlowSlot,
  FlowContextEditor,
  FlowSummaryBar,
} from './flow-engine-sections';

export function FlowEnginePanel({
  defaultMarketSlug,
  defaultOutcome,
}: FlowEnginePanelProps) {
  const { state, data, actions } = useFlowEngineController({
    defaultMarketSlug,
    defaultOutcome,
  });
  const flowSelectOptions =
    data.visibleDefinitions.length > 0
      ? data.visibleDefinitions
      : data.detail?.definition &&
          state.selectedDefinitionId != null &&
          data.detail.definition.id === state.selectedDefinitionId
        ? [data.detail.definition]
        : [];

  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader>
        <CardTitle className="text-sm font-medium text-zinc-300">
          Flow Engine Otomasyon (If / Else + Birlesik Satis/Alis)
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-5">
        <p className="text-xs text-zinc-500">
          Bu bolum n8n benzeri canvas akisiyla calisir: node surukleyin, edge baglayin, if/else
          mantigini tek akis icinde kurun.
        </p>

        <div className="rounded-lg border border-zinc-800 bg-zinc-950/40 p-3">
          <p className="mb-3 text-xs text-zinc-400">Flow Secimi ve Meta</p>
          <div className="grid gap-3 md:grid-cols-3">
            <div className="space-y-2">
              <p className="text-xs text-zinc-500">Flow Tanimi</p>
              <select
                value={state.selectedDefinitionId ?? ''}
                onChange={(event) => {
                  const nextDefinitionId = Number(event.target.value);
                  if (!Number.isFinite(nextDefinitionId) || nextDefinitionId <= 0) return;
                  void actions.requestDefinitionSwitch(nextDefinitionId);
                }}
                disabled={state.isActionBusy}
                className="h-9 w-full rounded-md border border-zinc-700 bg-zinc-800 px-3 text-sm text-zinc-200"
              >
                {flowSelectOptions.length === 0 && (
                  <option value="">
                    {data.definitionsLoading
                      ? 'Flow listesi yukleniyor...'
                      : data.definitionsError
                        ? 'Flow listesi yuklenemedi'
                        : 'Flow yok'}
                  </option>
                )}
                {flowSelectOptions.map((definition) => (
                  <option key={definition.id} value={definition.id}>
                    #{definition.id} - {definition.name} ({definition.status})
                  </option>
                ))}
              </select>
              {data.definitionsLoading && (
                <p className="text-[11px] text-zinc-500">Flow listesi yukleniyor...</p>
              )}
              {data.definitionsError && !data.definitionsLoading && (
                <p className="text-[11px] text-red-400">
                  Flow listesi yuklenemedi. Secili draft korunuyor, tekrar denenir.
                </p>
              )}
            </div>
            <div className="space-y-2">
              <p className="text-xs text-zinc-500">Flow Adi (Draft)</p>
              <Input
                value={state.draftName}
                onChange={(event) => actions.setDraftName(event.target.value)}
                disabled={state.isEditorReadOnly}
                className="border-zinc-700 bg-zinc-800 text-zinc-200"
              />
            </div>
            <div className="space-y-2">
              <p className="text-xs text-zinc-500">Aciklama (Draft)</p>
              <Input
                value={state.draftDescription}
                onChange={(event) => actions.setDraftDescription(event.target.value)}
                disabled={state.isEditorReadOnly}
                className="border-zinc-700 bg-zinc-800 text-zinc-200"
              />
            </div>

            <FlowContextEditor
              contextForm={state.contextForm}
              contextTab={state.contextTab}
              onContextFormChange={actions.setContextForm}
              onContextTabChange={actions.setContextTab}
              onApplyFromForm={actions.applyContextFromForm}
              onApplyFromAdvanced={() => {
                actions.applyContextFromAdvanced();
              }}
              onAutoClaimEnabledChange={(enabled) => {
                void actions.applyCanvasContextPatch(
                  { autoClaimEnabled: enabled ? true : undefined },
                  enabled
                    ? 'Autoclaim aktif. Bir sonraki runner turunda claim kontrolu baslayacak.'
                    : 'Autoclaim kapatildi. Bir sonraki runner turunda claim denenmeyecek.'
                );
              }}
            />
          </div>

          <div className="mt-3 flex flex-wrap gap-2">
            <Button disabled={state.isActionBusy || state.isEditorReadOnly} onClick={() => void actions.saveDraft()}>
              Draft Kaydet
            </Button>
            <Button
              variant="outline"
              className="border-zinc-700 text-zinc-300"
              disabled={state.isActionBusy || state.isEditorReadOnly}
              onClick={() => void actions.validateGraph()}
            >
              Dogrula
            </Button>
            <Button
              variant="outline"
              className="border-zinc-700 text-zinc-300"
              disabled={state.isActionBusy}
              onClick={() => {
                void actions.reloadDraftFromServer();
              }}
            >
              Taslagi Sunucudan Yukle
            </Button>
            <Button
              variant="outline"
              className="border-zinc-700 text-zinc-300"
              disabled={state.publishDisabled || state.isEditorReadOnly}
              onClick={() => void actions.publishFlow()}
            >
              Publish
            </Button>
            <Button
              variant="outline"
              className="border-zinc-700 text-zinc-300"
              disabled={state.isActionBusy}
              onClick={() => {
                void actions.confirmAndDeleteCurrentFlow();
              }}
            >
              Kalici Sil
            </Button>
          </div>
          {state.saveStatus === 'pending' && (
            <p className="mt-2 text-xs text-amber-300">Kaydediliyor... Publish bekletiliyor.</p>
          )}

          {state.autoSaveError && (
            <div className="mt-3 rounded-md border border-amber-500/40 bg-amber-500/10 p-3 text-sm text-amber-300">
              <p className="font-medium">Autosave / Draft Sync Uyarisi</p>
              <p className="mt-1">{state.autoSaveError}</p>
              <p className="mt-1 text-xs text-amber-200">
                Publish gecici olarak kilitlendi. `Draft Kaydet` ile tekrar dene veya `Taslagi
                Sunucudan Yukle` ile server draft&apos;ina don.
              </p>
            </div>
          )}
          {state.isEditorReadOnly && state.readOnlyReason && (
            <p className="mt-2 text-xs text-amber-300">{state.readOnlyReason}</p>
          )}
          {state.error && <p className="mt-2 text-sm text-red-400">{state.error}</p>}
          {state.message && <p className="mt-2 text-sm text-emerald-400">{state.message}</p>}
        </div>

        <FlowCanvasEditor
          instanceKey={state.selectedDefinitionId ?? 'draft'}
          graph={state.graph}
          readOnly={state.isEditorReadOnly}
          readOnlyReason={state.readOnlyReason}
          onGraphChange={actions.updateGraphFromCanvas}
          onError={actions.setError}
          openPositions={data.openPositions}
          openPositionsMeta={data.openPositionsMeta}
          openPositionsLoading={data.openPositionsLoading}
          onApplyContextPatch={actions.applyCanvasContextPatch}
          onPendingNodeDraftChange={actions.setHasPendingCanvasNodeDraft}
          livePrices={data.livePrices}
          userTelegramBotTokenMasked={data.userTelegramBotTokenMasked}
          userTelegramDefaultChatId={data.userTelegramDefaultChatId}
          leftPanelTopSlot={
            <CreateFlowSlot
              createName={state.createName}
              createDescription={state.createDescription}
              createTemplateKind={state.createTemplateKind}
              busyAction={state.busyAction ?? (state.isActionBusy ? 'save' : null)}
              isWorkflowListOpen={state.isWorkflowListOpen}
              workflowListQuery={state.workflowListQuery}
              definitionsLoading={data.definitionsLoading}
              filteredDefinitions={data.filteredDefinitions}
              selectedDefinitionId={state.selectedDefinitionId}
              deletingDefinitionId={state.deletingDefinitionId}
              onCreateNameChange={actions.setCreateName}
              onCreateDescriptionChange={actions.setCreateDescription}
              onTemplateKindChange={actions.setCreateTemplateKind}
              onCreateFromTemplate={(kind) => {
                void actions.createFromTemplate(kind);
              }}
              onToggleWorkflowList={() => actions.setIsWorkflowListOpen((previous) => !previous)}
              onWorkflowListQueryChange={actions.setWorkflowListQuery}
              onSelectDefinition={(id) => {
                void actions.requestDefinitionSwitch(id);
              }}
              onDeleteFromList={(id) => {
                void actions.deleteFlowFromList(id);
              }}
              showWorkflowActions
              workflowActionsDisabled={state.isActionBusy || state.isEditorReadOnly}
              onSaveDraft={() => {
                void actions.saveDraft();
              }}
              onValidate={() => {
                void actions.validateGraph();
              }}
              onReloadDraft={() => {
                void actions.reloadDraftFromServer();
              }}
              onPublish={() => {
                void actions.publishFlow();
              }}
              onDeleteFlow={() => {
                void actions.confirmAndDeleteCurrentFlow();
              }}
              publishDisabled={state.publishDisabled}
              canStopFlow={data.canStopSelectedFlow}
              onStopFlow={() => {
                void actions.handleStopFlow();
              }}
              stoppingFlow={state.stoppingFlow}
              selectedDefinitionIds={state.selectedDefinitionIds}
              onToggleDefinitionSelection={actions.toggleDefinitionSelection}
              onSelectAllDefinitions={actions.selectAllDefinitions}
              onDeselectAllDefinitions={actions.deselectAllDefinitions}
              onBulkDelete={() => {
                void actions.bulkDeleteDefinitions();
              }}
              bulkDeleting={state.bulkDeleting}
              autoClaimEnabled={state.contextForm.autoClaimEnabled}
              onAutoClaimEnabledChange={(enabled) => {
                void actions.applyCanvasContextPatch(
                  { autoClaimEnabled: enabled ? true : undefined },
                  enabled
                    ? 'Autoclaim aktif. Bir sonraki runner turunda claim kontrolu baslayacak.'
                    : 'Autoclaim kapatildi. Bir sonraki runner turunda claim denenmeyecek.'
                );
              }}
            />
          }
        />

        <FlowSummaryBar
          graph={state.graph}
          validation={state.validation}
          detail={data.detail}
          autoSaveError={state.autoSaveError}
        />
      </CardContent>
    </Card>
  );
}
