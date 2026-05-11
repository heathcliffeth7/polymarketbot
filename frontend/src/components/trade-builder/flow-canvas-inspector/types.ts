import type {
  ConditionDraft,
  DrawdownRuleRow,
  EdgeConditionFormState,
  NodeConfigFormState,
  OutcomeConditionRow,
  PrimitiveValueType,
} from '@/lib/trade-flow-config-mappers';
import type {
  TradeBuilderOutcome,
  TradeFlowOpenPositionOption,
  TradeFlowOpenPositionsMeta,
} from '@/lib/types';
import type { FlowEdge } from '../flow-canvas-constants';
import type {
  PairLockUpstreamTriggerSummary,
  UpstreamMaxPriceResolution,
} from '../flow-canvas-utils';

export interface NodeInspectorActions {
  onNodeKeyChange: (key: string) => void;
  onNodeTypeChange: (type: string) => void;
  onTabChange: (tab: 'basic' | 'advanced') => void;
  onFormChange: React.Dispatch<React.SetStateAction<NodeConfigFormState | null>>;
  onUpdateField: (key: string, value: string) => void;
  onUpdateTriggerSizeRow: (index: number, value: string) => void;
  onCreateNode: () => void;
  onUpdateNode: () => void;
  onDeleteNode: () => void;
  onCreateFromAdvanced: () => void;
  onUpdateFromAdvanced: () => void;
  onApplyOpenPosition: (position: TradeFlowOpenPositionOption) => void;
  onUpdateExpressionRow: (rowId: string, patch: Partial<ConditionDraft>) => void;
  onAddExpressionRow: () => void;
  onRemoveExpressionRow: (rowId: string) => void;
  onUpdateStatePatchRow: (
    rowId: string,
    patch: Partial<{ key: string; value: string; valueType: PrimitiveValueType }>
  ) => void;
  onAddStatePatchRow: () => void;
  onRemoveStatePatchRow: (rowId: string) => void;
  onAddOutcomeCondition: (tokenId: string, outcomeLabel: string) => void;
  onRemoveOutcomeCondition: (rowId: string) => void;
  onUpdateOutcomeCondition: (rowId: string, patch: Partial<OutcomeConditionRow>) => void;
  onAddDrawdownRule: () => void;
  onRemoveDrawdownRule: (rowId: string) => void;
  onUpdateDrawdownRule: (rowId: string, patch: Partial<DrawdownRuleRow>) => void;
}

export interface EdgeInspectorActions {
  onEdgeTypeChange: (type: string) => void;
  onTabChange: (tab: 'basic' | 'advanced') => void;
  onFormChange: React.Dispatch<React.SetStateAction<EdgeConditionFormState | null>>;
  onUpdateConditionRow: (patch: Partial<ConditionDraft>) => void;
  onApplyBasic: () => void;
  onApplyAdvanced: () => void;
  onDeleteEdge: () => void;
}

export interface NodeInspectorPanelProps {
  form: NodeConfigFormState;
  nodeKeyDraft: string;
  nodeTypeDraft: string;
  tab: 'basic' | 'advanced';
  openPositions: TradeFlowOpenPositionOption[];
  openPositionsMeta: TradeFlowOpenPositionsMeta | null;
  openPositionsLoading: boolean;
  openPositionApplyingKey: string | null;
  canApplyOpenPosition: (p: TradeFlowOpenPositionOption) => boolean;
  marketOutcomes: TradeBuilderOutcome[];
  marketOutcomesLoading: boolean;
  upstreamAutoScope: boolean;
  upstreamHasTriggerPrice: boolean;
  upstreamMaxPriceResolution: UpstreamMaxPriceResolution;
  upstreamPairLockTrigger: PairLockUpstreamTriggerSummary | null;
  userTelegramBotTokenMasked: string | null;
  userTelegramDefaultChatId: string | null;
  actions: NodeInspectorActions;
}

export interface EdgeInspectorPanelProps {
  edge: FlowEdge;
  form: EdgeConditionFormState;
  edgeTypeDraft: string;
  tab: 'basic' | 'advanced';
  actions: EdgeInspectorActions;
}
