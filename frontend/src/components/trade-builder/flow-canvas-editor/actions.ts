import type {
  ConditionDraft,
  DrawdownRuleRow,
  EdgeConditionFormState,
  NodeConfigFormState,
  OutcomeConditionRow,
  PrimitiveValueType,
} from '@/lib/trade-flow-config-mappers';
import type { TradeFlowOpenPositionOption } from '@/lib/types';
import type { EdgeInspectorActions, NodeInspectorActions } from '../flow-canvas-inspector';

interface NodeInspectorActionArgs {
  setNodeKeyDraft: (key: string) => void;
  setHasPendingNodeDraft: (value: boolean) => void;
  handleNodeTypeChange: (type: string) => void;
  setNodeInspectorTab: (tab: 'basic' | 'advanced') => void;
  setNodeForm: React.Dispatch<React.SetStateAction<NodeConfigFormState | null>>;
  updateNodeField: (key: string, value: string) => void;
  updateTriggerSizeRow: (index: number, value: string) => void;
  createOrUpdateNode: (mode: 'create' | 'update', source: 'basic' | 'advanced') => void;
  deleteSelectedNode: () => void;
  applyOpenPositionSelection: (position: TradeFlowOpenPositionOption) => Promise<void>;
  updateExpressionRow: (rowId: string, patch: Partial<ConditionDraft>) => void;
  addExpressionRow: () => void;
  removeExpressionRow: (rowId: string) => void;
  updateStatePatchRow: (
    rowId: string,
    patch: Partial<{ key: string; value: string; valueType: PrimitiveValueType }>
  ) => void;
  addStatePatchRow: () => void;
  removeStatePatchRow: (rowId: string) => void;
  addOutcomeCondition: (tokenId: string, outcomeLabel: string) => void;
  removeOutcomeCondition: (rowId: string) => void;
  updateOutcomeCondition: (rowId: string, patch: Partial<OutcomeConditionRow>) => void;
  addDrawdownRule: () => void;
  removeDrawdownRule: (rowId: string) => void;
  updateDrawdownRule: (rowId: string, patch: Partial<DrawdownRuleRow>) => void;
}

export function createNodeInspectorActions(args: NodeInspectorActionArgs): NodeInspectorActions {
  return {
    onNodeKeyChange: (key) => {
      args.setNodeKeyDraft(key);
      args.setHasPendingNodeDraft(true);
    },
    onNodeTypeChange: args.handleNodeTypeChange,
    onTabChange: args.setNodeInspectorTab,
    onFormChange: (updater) => {
      args.setHasPendingNodeDraft(true);
      args.setNodeForm(updater);
    },
    onUpdateField: args.updateNodeField,
    onUpdateTriggerSizeRow: args.updateTriggerSizeRow,
    onCreateNode: () => args.createOrUpdateNode('create', 'basic'),
    onUpdateNode: () => args.createOrUpdateNode('update', 'basic'),
    onDeleteNode: args.deleteSelectedNode,
    onCreateFromAdvanced: () => args.createOrUpdateNode('create', 'advanced'),
    onUpdateFromAdvanced: () => args.createOrUpdateNode('update', 'advanced'),
    onApplyOpenPosition: (position) => {
      void args.applyOpenPositionSelection(position);
    },
    onUpdateExpressionRow: args.updateExpressionRow,
    onAddExpressionRow: args.addExpressionRow,
    onRemoveExpressionRow: args.removeExpressionRow,
    onUpdateStatePatchRow: args.updateStatePatchRow,
    onAddStatePatchRow: args.addStatePatchRow,
    onRemoveStatePatchRow: args.removeStatePatchRow,
    onAddOutcomeCondition: args.addOutcomeCondition,
    onRemoveOutcomeCondition: args.removeOutcomeCondition,
    onUpdateOutcomeCondition: args.updateOutcomeCondition,
    onAddDrawdownRule: args.addDrawdownRule,
    onRemoveDrawdownRule: args.removeDrawdownRule,
    onUpdateDrawdownRule: args.updateDrawdownRule,
  };
}

interface EdgeInspectorActionArgs {
  setEdgeTypeDraft: (type: string) => void;
  setEdgeInspectorTab: (tab: 'basic' | 'advanced') => void;
  setEdgeForm: React.Dispatch<React.SetStateAction<EdgeConditionFormState | null>>;
  updateEdgeConditionRow: (patch: Partial<ConditionDraft>) => void;
  applyEdge: (source: 'basic' | 'advanced') => void;
  deleteSelectedEdge: () => void;
}

export function createEdgeInspectorActions(args: EdgeInspectorActionArgs): EdgeInspectorActions {
  return {
    onEdgeTypeChange: args.setEdgeTypeDraft,
    onTabChange: args.setEdgeInspectorTab,
    onFormChange: args.setEdgeForm,
    onUpdateConditionRow: args.updateEdgeConditionRow,
    onApplyBasic: () => args.applyEdge('basic'),
    onApplyAdvanced: () => args.applyEdge('advanced'),
    onDeleteEdge: args.deleteSelectedEdge,
  };
}
