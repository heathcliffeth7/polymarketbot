import {
  Background,
  Controls,
  MarkerType,
  MiniMap,
  ReactFlow,
  type EdgeChange,
  type NodeChange,
} from '@xyflow/react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import type { EdgeConditionFormState, NodeConfigFormState } from '@/lib/trade-flow-config-mappers';
import { NodeInspectorPanel, EdgeInspectorPanel, type EdgeInspectorActions, type NodeInspectorActions } from '../flow-canvas-inspector';
import {
  EDGE_STROKE_COLOR,
  NODE_PALETTE_CATEGORIES,
  type FlowEdge,
  type FlowNode,
  type NodeGroup,
  type NodePaletteCategory,
  type FlowCanvasEditorProps,
} from '../flow-canvas-constants';
import {
  minimapColor,
  type PairLockUpstreamTriggerSummary,
  type UpstreamMaxPriceResolution,
} from '../flow-canvas-utils';
import { NODE_TYPES } from '../flow-canvas-node-card';
import type { TradeBuilderOutcome, TradeFlowOpenPositionOption } from '@/lib/types';

interface FlowCanvasEditorLayoutProps {
  readOnly: boolean;
  readOnlyReason: string | null;
  editorRootRef: React.RefObject<HTMLDivElement | null>;
  canvasWrapperRef: React.RefObject<HTMLDivElement | null>;
  focusEditor: () => void;
  onCanvasPointerMove: (clientX: number, clientY: number) => void;
  leftPanelTopSlot: FlowCanvasEditorProps['leftPanelTopSlot'];
  showNodeSearch: boolean;
  setShowNodeSearch: React.Dispatch<React.SetStateAction<boolean>>;
  nodeSearchQuery: string;
  setNodeSearchQuery: React.Dispatch<React.SetStateAction<string>>;
  nodeSearchInputRef: React.RefObject<HTMLInputElement | null>;
  searchMatchedNodes: FlowNode[];
  hydrateNodeDraft: (node: FlowNode, syncCanvasSelection?: boolean) => void;
  queueNodeFocus: (nodeId: string) => void;
  nodePaletteSearch: string;
  setNodePaletteSearch: React.Dispatch<React.SetStateAction<string>>;
  nodePaletteCategory: NodePaletteCategory;
  setNodePaletteCategory: React.Dispatch<React.SetStateAction<NodePaletteCategory>>;
  filteredNodeOptions: Array<{ label: string; value: string }>;
  addNode: (nodeType: string) => void;
  addPresetPlaceOrderNode: (kind: 'place_order' | 'sell_current_position' | 'buy_current_position') => void;
  canvasNodes: FlowNode[];
  canvasEdges: FlowEdge[];
  triggerCount: number;
  logicCount: number;
  actionCount: number;
  selectedNode: FlowNode | null;
  selectedEdge: FlowEdge | null;
  selectedNodeCount: number;
  selectedEdgeCount: number;
  hasActiveSelection: boolean;
  isMultiSelection: boolean;
  deleteSelection: () => void;
  handleGroupSelected: () => void;
  handleUngroupSelected: () => void;
  nodeGroups: NodeGroup[];
  handleAssignToGroup: (groupId: string) => void;
  handleUndo: () => void;
  handleRedo: () => void;
  canUndo: boolean;
  canRedo: boolean;
  handleAutoLayout: () => void;
  handleExport: () => void;
  handleImport: () => Promise<void>;
  onNodesChange: (changes: NodeChange<FlowNode>[]) => void;
  onEdgesChange: (changes: EdgeChange<FlowEdge>[]) => void;
  onConnect: Parameters<typeof ReactFlow<FlowNode, FlowEdge>>[0]['onConnect'];
  onSelectionChange: ({ nodes, edges }: { nodes: FlowNode[]; edges: FlowEdge[] }) => void;
  nodeForm: NodeConfigFormState | null;
  nodeKeyDraft: string;
  nodeTypeDraft: string;
  nodeInspectorTab: 'basic' | 'advanced';
  openPositions: FlowCanvasEditorProps['openPositions'];
  openPositionsMeta: FlowCanvasEditorProps['openPositionsMeta'];
  openPositionsLoading: boolean;
  openPositionApplyingKey: string | null;
  canApplyOpenPosition: (position: TradeFlowOpenPositionOption) => boolean;
  marketOutcomes: TradeBuilderOutcome[];
  outcomesLoading: boolean;
  selectedNodeUpstreamAutoScope: boolean;
  selectedNodeUpstreamTriggerPrice: boolean;
  selectedNodeUpstreamMaxPriceResolution: UpstreamMaxPriceResolution;
  selectedNodeUpstreamPairLockTrigger: PairLockUpstreamTriggerSummary | null;
  userTelegramBotTokenMasked: string | null;
  userTelegramDefaultChatId: string | null;
  nodeInspectorActions: NodeInspectorActions;
  edgeForm: EdgeConditionFormState | null;
  edgeTypeDraft: string;
  edgeInspectorTab: 'basic' | 'advanced';
  edgeInspectorActions: EdgeInspectorActions;
}

export function FlowCanvasEditorLayout({
  readOnly,
  readOnlyReason,
  editorRootRef,
  canvasWrapperRef,
  focusEditor,
  onCanvasPointerMove,
  leftPanelTopSlot,
  showNodeSearch,
  setShowNodeSearch,
  nodeSearchQuery,
  setNodeSearchQuery,
  nodeSearchInputRef,
  searchMatchedNodes,
  hydrateNodeDraft,
  queueNodeFocus,
  nodePaletteSearch,
  setNodePaletteSearch,
  nodePaletteCategory,
  setNodePaletteCategory,
  filteredNodeOptions,
  addNode,
  addPresetPlaceOrderNode,
  canvasNodes,
  canvasEdges,
  triggerCount,
  logicCount,
  actionCount,
  selectedNode,
  selectedEdge,
  selectedNodeCount,
  selectedEdgeCount,
  hasActiveSelection,
  isMultiSelection,
  deleteSelection,
  handleGroupSelected,
  handleUngroupSelected,
  nodeGroups,
  handleAssignToGroup,
  handleUndo,
  handleRedo,
  canUndo,
  canRedo,
  handleAutoLayout,
  handleExport,
  handleImport,
  onNodesChange,
  onEdgesChange,
  onConnect,
  onSelectionChange,
  nodeForm,
  nodeKeyDraft,
  nodeTypeDraft,
  nodeInspectorTab,
  openPositions,
  openPositionsMeta,
  openPositionsLoading,
  openPositionApplyingKey,
  canApplyOpenPosition,
  marketOutcomes,
  outcomesLoading,
  selectedNodeUpstreamAutoScope,
  selectedNodeUpstreamTriggerPrice,
  selectedNodeUpstreamMaxPriceResolution,
  selectedNodeUpstreamPairLockTrigger,
  userTelegramBotTokenMasked,
  userTelegramDefaultChatId,
  nodeInspectorActions,
  edgeForm,
  edgeTypeDraft,
  edgeInspectorTab,
  edgeInspectorActions,
}: FlowCanvasEditorLayoutProps) {
  const isCanvasLoading =
    readOnly &&
    Boolean(
      readOnlyReason &&
        /yukleniyor|aciliyor|kaydediliyor/i.test(readOnlyReason)
    );

  return (
    <div
      ref={editorRootRef}
      tabIndex={-1}
      onMouseDownCapture={focusEditor}
      className="rounded-2xl border border-slate-200 bg-[linear-gradient(180deg,#ffffff,#f8fafc)] p-4 shadow-sm outline-none"
    >
      <div className="flex items-center justify-between">
        <div>
          <p className="text-xs font-medium tracking-wide text-slate-700">Canvas Editoru (Surukle &amp; Birak)</p>
          <p className="mt-1 text-[11px] text-slate-500">Sol panelden node ekleyin, baglanti noktalarindan edge cizerek akisi kurun.</p>
          {readOnly && (
            <p className="mt-1 text-[11px] text-amber-600">
              {readOnlyReason ?? 'Flow yuklenirken duzenleme kilitli.'}
            </p>
          )}
        </div>
        <div className="flex gap-1">
          <Button size="sm" variant="outline" className="h-7 border-slate-300 px-2 text-[11px] text-slate-600" onClick={handleUndo} disabled={readOnly || !canUndo} title="Geri Al (Ctrl+Z)">&#8617; Geri</Button>
          <Button size="sm" variant="outline" className="h-7 border-slate-300 px-2 text-[11px] text-slate-600" onClick={handleRedo} disabled={readOnly || !canRedo} title="Ileri Al (Ctrl+Shift+Z)">&#8618; Ileri</Button>
          <Button size="sm" variant="outline" className="h-7 border-slate-300 px-2 text-[11px] text-slate-600" onClick={handleAutoLayout} disabled={readOnly} title="Otomatik Duzenleme">&#9638; Layout</Button>
          <Button size="sm" variant="outline" className="h-7 border-slate-300 px-2 text-[11px] text-slate-600" onClick={handleExport} title="JSON Aktar">&#8615; Export</Button>
          <Button size="sm" variant="outline" className="h-7 border-slate-300 px-2 text-[11px] text-slate-600" onClick={() => { void handleImport(); }} disabled={readOnly} title="JSON Yukle">&#8613; Import</Button>
          <Button size="sm" variant="outline" className="h-7 border-slate-300 px-2 text-[11px] text-slate-600" onClick={() => { setShowNodeSearch(true); setTimeout(() => nodeSearchInputRef.current?.focus(), 50); }} title="Node Ara (Ctrl+K)">&#128269; Ara</Button>
        </div>
      </div>

      {showNodeSearch && (
        <div className="relative z-20 mt-2">
          <div className="rounded-lg border border-slate-300 bg-white p-2 shadow-lg">
            <Input
              ref={nodeSearchInputRef}
              value={nodeSearchQuery}
              onChange={(e) => setNodeSearchQuery(e.target.value)}
              placeholder="Node key veya tip ile ara... (Esc kapat)"
              className="h-8 border-slate-300 bg-white text-xs text-slate-900"
              onKeyDown={(e) => {
                if (e.key === 'Escape') { setShowNodeSearch(false); setNodeSearchQuery(''); }
                if (e.key === 'Enter' && searchMatchedNodes.length > 0) {
                  const target = searchMatchedNodes[0];
                  hydrateNodeDraft(target, true);
                  queueNodeFocus(target.id);
                  setShowNodeSearch(false);
                  setNodeSearchQuery('');
                }
              }}
            />
            {nodeSearchQuery.trim() && (
              <div className="mt-1 max-h-40 space-y-1 overflow-auto">
                {searchMatchedNodes.length === 0 ? (
                  <p className="text-[11px] text-slate-500">Eslesen node yok.</p>
                ) : searchMatchedNodes.map((n) => (
                  <button
                    key={n.id}
                    type="button"
                    className="w-full rounded-md px-2 py-1 text-left text-[11px] text-slate-700 hover:bg-slate-100"
                    onClick={() => {
                      hydrateNodeDraft(n, true);
                      queueNodeFocus(n.id);
                      setShowNodeSearch(false);
                      setNodeSearchQuery('');
                    }}
                  >
                    <span className="font-medium">{n.id}</span>
                    <span className="ml-2 text-slate-500">{n.data.nodeType}</span>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      <div className="mt-4 grid min-w-0 gap-3 xl:grid-cols-[220px_minmax(0,1fr)_380px]">
        <div className="min-w-0 space-y-3 rounded-xl border border-slate-200 bg-slate-50 p-3">
          {leftPanelTopSlot}
          <p className="text-xs font-medium text-slate-700">Node Paleti</p>
          <Input value={nodePaletteSearch} onChange={(e) => setNodePaletteSearch(e.target.value)} placeholder="Node ara..." className="h-8 border-slate-300 bg-white text-xs text-slate-900" />
          <div className="grid grid-cols-2 gap-1">
            {NODE_PALETTE_CATEGORIES.map((item) => (
              <button key={item.value} type="button" className={`h-8 rounded-md border text-xs ${nodePaletteCategory === item.value ? 'border-sky-300 bg-sky-100 text-sky-700' : 'border-slate-300 bg-white text-slate-600 hover:bg-slate-100'}`} onClick={() => setNodePaletteCategory(item.value)}>{item.label}</button>
            ))}
          </div>
          <div className="max-h-[320px] space-y-2 overflow-auto pr-1">
            {filteredNodeOptions.length === 0 ? (
              <p className="text-[11px] text-slate-500">Aramaya uygun node bulunamadi.</p>
            ) : filteredNodeOptions.map((option) => (
              <Button key={option.value} type="button" size="sm" variant="outline" className="w-full justify-start border-slate-300 bg-white text-slate-700 hover:bg-slate-100" disabled={readOnly} onClick={() => addNode(option.value)}>+ {option.label}</Button>
            ))}
          </div>
          <div className="space-y-2 overflow-hidden rounded-md border border-slate-200 bg-white p-2">
            <p className="text-[11px] font-medium text-slate-700">Hizli Presetler</p>
            <p className="text-[10px] text-slate-500">Presetler action.place_order node&apos;u uretir.</p>
            <Button type="button" size="sm" variant="outline" className="h-auto min-h-8 w-full justify-start whitespace-normal break-words border-slate-300 bg-white py-1.5 text-left leading-tight text-slate-700 hover:bg-slate-100" disabled={readOnly} onClick={() => addPresetPlaceOrderNode('place_order')}>+ Preset: Al / Sat</Button>
          </div>
          <div className="rounded-md border border-slate-200 bg-white p-2 text-[11px] text-slate-500">
            <p>Node: {canvasNodes.length}</p>
            <p>Edge: {canvasEdges.length}</p>
            <p>Trigger: {triggerCount} | Logic: {logicCount} | Action: {actionCount}</p>
          </div>
          <Button
            size="sm"
            variant="outline"
            className="w-full border-slate-300 text-slate-700 hover:bg-slate-100"
            disabled={readOnly || !hasActiveSelection}
            onClick={deleteSelection}
          >
            {isMultiSelection ? 'Secili Ogeleri Sil' : 'Secili Ogeyi Sil'}
          </Button>
          <div className="space-y-1.5 rounded-md border border-slate-200 bg-white p-2">
            <p className="text-[11px] font-medium text-slate-700">Node Gruplama</p>
            <Button size="sm" variant="outline" className="w-full border-slate-300 text-[11px] text-slate-700 hover:bg-slate-100" disabled={readOnly || !selectedNode} onClick={handleGroupSelected}>+ Yeni Grup Olustur</Button>
            {selectedNode?.data.groupId && (
              <Button size="sm" variant="outline" className="w-full border-slate-300 text-[11px] text-slate-700 hover:bg-slate-100" disabled={readOnly} onClick={handleUngroupSelected}>Gruptan Cikar</Button>
            )}
            {nodeGroups.length > 0 && selectedNode && (
              <div className="space-y-1">
                <p className="text-[10px] text-slate-500">Gruba Ekle:</p>
                {nodeGroups.map((g) => (
                  <button key={g.id} type="button" className="flex w-full items-center gap-1.5 rounded-md border border-slate-200 px-2 py-1 text-left text-[11px] text-slate-700 hover:bg-slate-100 disabled:cursor-not-allowed disabled:opacity-50" disabled={readOnly} onClick={() => handleAssignToGroup(g.id)}>
                    <span className="inline-block h-3 w-3 rounded-full" style={{ backgroundColor: g.color }} />
                    {g.name}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>

        <div
          ref={canvasWrapperRef}
          className="relative flow-canvas h-[60svh] min-h-[420px] min-w-0 rounded-xl border border-slate-200 bg-white xl:h-[calc(100vh-12rem)] xl:min-h-[500px]"
          onMouseDown={(event) => onCanvasPointerMove(event.clientX, event.clientY)}
          onMouseMove={(event) => onCanvasPointerMove(event.clientX, event.clientY)}
        >
          {isCanvasLoading && (
            <div className="pointer-events-none absolute inset-0 z-20 flex items-center justify-center rounded-xl bg-slate-100/80">
              <div className="rounded-lg border border-amber-300 bg-amber-50 px-4 py-3 text-center text-sm text-amber-900 shadow-sm">
                <p className="font-medium">Workflow canvas yukleniyor...</p>
                <p className="mt-1 text-xs text-amber-800">
                  {readOnlyReason ?? 'Detay API yaniti bekleniyor.'}
                </p>
              </div>
            </div>
          )}
          <ReactFlow<FlowNode, FlowEdge>
            nodes={canvasNodes}
            edges={canvasEdges}
            nodeTypes={NODE_TYPES}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            onSelectionChange={onSelectionChange}
            nodesDraggable={!readOnly}
            nodesConnectable={!readOnly}
            elementsSelectable={!readOnly}
            fitView
            minZoom={0.25}
            maxZoom={1.6}
            selectionKeyCode="Shift"
            deleteKeyCode={['Backspace', 'Delete']}
            defaultEdgeOptions={{
              type: 'smoothstep',
              markerEnd: { type: MarkerType.ArrowClosed, color: EDGE_STROKE_COLOR, width: 16, height: 16 },
              style: { stroke: EDGE_STROKE_COLOR, strokeWidth: 1.6 },
            }}
          >
            <MiniMap pannable zoomable nodeColor={minimapColor} />
            <Controls />
            <Background gap={20} size={1.1} color="#cbd5e1" />
          </ReactFlow>
        </div>

        <div className="flex min-h-[320px] min-w-0 flex-col overflow-hidden rounded-xl border border-slate-200 bg-white/95 p-3 xl:h-[calc(100vh-12rem)]">
          {selectedNode && nodeForm ? (
            <NodeInspectorPanel
              form={nodeForm}
              nodeKeyDraft={nodeKeyDraft}
              nodeTypeDraft={nodeTypeDraft}
              tab={nodeInspectorTab}
              openPositions={openPositions}
              openPositionsMeta={openPositionsMeta}
              openPositionsLoading={openPositionsLoading}
              openPositionApplyingKey={openPositionApplyingKey}
              canApplyOpenPosition={canApplyOpenPosition}
              marketOutcomes={marketOutcomes}
              marketOutcomesLoading={outcomesLoading}
              upstreamAutoScope={selectedNodeUpstreamAutoScope}
              upstreamHasTriggerPrice={selectedNodeUpstreamTriggerPrice}
              upstreamMaxPriceResolution={selectedNodeUpstreamMaxPriceResolution}
              upstreamPairLockTrigger={selectedNodeUpstreamPairLockTrigger}
              userTelegramBotTokenMasked={userTelegramBotTokenMasked ?? null}
              userTelegramDefaultChatId={userTelegramDefaultChatId ?? null}
              actions={nodeInspectorActions}
            />
          ) : selectedEdge && edgeForm ? (
            <EdgeInspectorPanel edge={selectedEdge} form={edgeForm} edgeTypeDraft={edgeTypeDraft} tab={edgeInspectorTab} actions={edgeInspectorActions} />
          ) : isMultiSelection ? (
            <div className="space-y-2 text-xs text-slate-500">
              <p>
                {selectedNodeCount} node, {selectedEdgeCount} edge secili.
              </p>
              <p>Ctrl+C ile secili grubu kopyalayabilir, Ctrl+V ile fare konumuna baglantilariyla birlikte yapistirabilirsiniz.</p>
              <p>Detay duzenlemek icin tek bir node veya edge secin.</p>
              <p className="text-[10px] text-slate-400">Delete: Secimi sil | Shift+sol tik-surukle: Kutuyla sec</p>
            </div>
          ) : (
            <div className="space-y-2 text-xs text-slate-500">
              <p>Bir node veya edge secin.</p>
              <p>Birden fazla node secmek icin Shift+sol tik-surukle kullanin.</p>
              <p>Form sekmesinde dogrudan alan girerek duzenleyebilirsiniz.</p>
              <p>JSON yalniz Advanced sekmesinde tutulur.</p>
              <p className="text-[10px] text-slate-400">Ctrl+Z: Geri Al | Ctrl+Shift+Z: Ileri Al | Ctrl+C/V: Kopyala/Yapistir | Ctrl+K: Ara</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
