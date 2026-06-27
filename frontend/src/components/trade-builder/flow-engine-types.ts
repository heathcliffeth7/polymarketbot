import type React from 'react';
import type { ContextFormState } from '@/lib/trade-flow-config-mappers';
import type { TradeFlowDefinition, TradeFlowDefinitionDetail, TradeFlowGraph, TradeFlowOpenPositionOption, TradeFlowOpenPositionsMeta, TradeFlowValidationResult } from '@/lib/types';

export type BusyAction = 'create' | 'save' | 'validate' | 'publish' | 'delete' | null;

export type TemplateKind =
  | 'starter'
  | 'sell_buy_if'
  | 'dca'
  | 'sl_tp'
  | 'position_monitor'
  | 'multi_leg_hedge'
  | 'revenge_flip_10_80'
  | 'confidence_ladder_hedge_lock'
  | 'avg_rebound_pairlock_rescue_50usdc'
  | 'avg_rebound_pairlock_rescue_micro_20usdc'
  | 'pairlock_hyperliquid_70_80'
  | 'positive_quantity_flip_grid_1usdc'
  | 'positive_quantity_flip_grid_inventory_balance'
  | 'positive_flip_pairlock_compression';

export type DraftSaveStatus = 'idle' | 'pending' | 'error';
export type DefinitionSwitchPhase = 'saving_current' | 'loading_detail';

export interface DefinitionSwitchState {
  targetId: number;
  phase: DefinitionSwitchPhase;
  startedAt: number;
}

export interface DraftSwitchRecovery {
  currentDefinitionId: number;
  targetDefinitionId: number;
  message: string;
}

export interface FlowEnginePanelProps {
  defaultMarketSlug: string | null;
  defaultOutcome: { token_id: string; label: string } | null;
}

export interface FlowEngineControllerState {
  selectedDefinitionId: number | null;
  draftName: string;
  draftDescription: string;
  createName: string;
  createDescription: string;
  createTemplateKind: TemplateKind;
  isWorkflowListOpen: boolean;
  workflowListQuery: string;
  deletingDefinitionId: number | null;
  selectedDefinitionIds: Set<number>;
  bulkDeleting: boolean;
  graph: TradeFlowGraph;
  contextForm: ContextFormState;
  contextTab: 'basic' | 'advanced';
  validation: TradeFlowValidationResult | null;
  busyAction: BusyAction;
  saveStatus: DraftSaveStatus;
  message: string | null;
  error: string | null;
  autoSaveError: string | null;
  definitionSwitchState: DefinitionSwitchState | null;
  draftSwitchRecovery: DraftSwitchRecovery | null;
  stoppingFlow: boolean;
  isActionBusy: boolean;
  isEditorReadOnly: boolean;
  readOnlyReason: string | null;
  publishDisabled: boolean;
}

export interface FlowEngineControllerData {
  definitionsLoading: boolean;
  definitionsError: Error | null;
  visibleDefinitions: TradeFlowDefinition[];
  filteredDefinitions: TradeFlowDefinition[];
  detail: TradeFlowDefinitionDetail | null;
  openPositions: TradeFlowOpenPositionOption[];
  openPositionsMeta: TradeFlowOpenPositionsMeta | null;
  openPositionsLoading: boolean;
  livePrices?: Record<string, number>;
  userTelegramBotTokenMasked: string | null;
  userTelegramDefaultChatId: string | null;
  canStopSelectedFlow: boolean;
}

export interface FlowEngineControllerActions {
  setDraftName: (value: string) => void;
  setDraftDescription: (value: string) => void;
  setCreateName: (value: string) => void;
  setCreateDescription: (value: string) => void;
  setCreateTemplateKind: (value: TemplateKind) => void;
  setIsWorkflowListOpen: React.Dispatch<React.SetStateAction<boolean>>;
  setWorkflowListQuery: (value: string) => void;
  setContextForm: React.Dispatch<React.SetStateAction<ContextFormState>>;
  setContextTab: (tab: 'basic' | 'advanced') => void;
  setHasPendingCanvasNodeDraft: (hasPending: boolean) => void;
  setError: (message: string | null) => void;
  requestDefinitionSwitch: (definitionId: number, options?: { skipDraftSave?: boolean }) => Promise<boolean>;
  skipDraftSaveAndSwitch: () => Promise<boolean>;
  createFromTemplate: (kind: TemplateKind) => Promise<void>;
  saveDraft: () => Promise<void>;
  validateGraph: () => Promise<void>;
  reloadDraftFromServer: () => Promise<void>;
  publishFlow: () => Promise<void>;
  confirmAndDeleteCurrentFlow: () => Promise<void>;
  deleteFlowFromList: (definitionId: number) => Promise<void>;
  handleStopFlow: () => Promise<void>;
  updateGraphFromCanvas: (nextGraph: TradeFlowGraph, options?: { allowGraphShrink?: boolean; persistImmediately?: boolean }) => void;
  applyContextFromForm: () => Record<string, unknown>;
  applyContextFromAdvanced: () => Record<string, unknown> | null;
  applyCanvasContextPatch: (patch: Record<string, unknown>, successMessage?: string) => Promise<void>;
  toggleDefinitionSelection: (definitionId: number) => void;
  selectAllDefinitions: () => void;
  deselectAllDefinitions: () => void;
  bulkDeleteDefinitions: () => Promise<void>;
}

export interface FlowEngineController {
  state: FlowEngineControllerState;
  data: FlowEngineControllerData;
  actions: FlowEngineControllerActions;
}
