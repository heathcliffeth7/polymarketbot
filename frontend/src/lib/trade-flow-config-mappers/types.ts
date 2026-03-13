export type PrimitiveValueType = 'string' | 'number' | 'boolean';
export type ExpressionJoin = 'and' | 'or';
export type ConditionOperator = '>' | '>=' | '<' | '<=' | '==' | '!=';

export interface KeyValueDraft {
  id: string;
  key: string;
  value: string;
  valueType: PrimitiveValueType;
}

export interface ConditionDraft {
  id: string;
  leftVar: string;
  operator: ConditionOperator;
  rightType: PrimitiveValueType;
  rightValue: string;
}

export interface OutcomeConditionRow {
  id: string;
  tokenId: string;
  outcomeLabel: string;
  triggerCondition: string;
  triggerPriceCent: string;
  maxPriceCent: string;
}

export interface DrawdownRuleRow {
  id: string;
  direction: 'down' | 'up';
  lossPct: string;
  durationValue: string;
}

export interface PlaceOrderMaxPriceUiState {
  isInheritedValue: boolean;
  upstreamKind: 'none' | 'single' | 'multiple';
  upstreamMaxPriceCent: string | null;
  distinctUpstreamMaxPriceCents: string[];
}

export interface PlaceOrderMarketSeedUiState {
  isInheritedMarketSlug: boolean;
  isInheritedTokenId: boolean;
  isInheritedOutcomeLabel: boolean;
  upstreamKind: 'none' | 'single' | 'multiple';
  upstreamOutcomeKind: 'none' | 'single' | 'multiple';
  upstreamMarketSlug: string | null;
  upstreamTokenId: string | null;
  upstreamOutcomeLabel: string | null;
  distinctUpstreamMarketSlugs: string[];
  distinctUpstreamOutcomeLabels: string[];
}

export interface NodeConfigFormState {
  fields: Record<string, string>;
  triggerSizeRows: string[];
  outcomeConditionRows: OutcomeConditionRow[];
  drawdownRuleRows: DrawdownRuleRow[];
  expressionRows: ConditionDraft[];
  expressionJoin: ExpressionJoin;
  expressionSupported: boolean;
  nestedExprMode: boolean;
  nestedExprGroup: import('@/lib/types').ExpressionGroup | null;
  statePatchRows: KeyValueDraft[];
  placeOrderMaxPriceUi?: PlaceOrderMaxPriceUiState;
  placeOrderMarketSeedUi?: PlaceOrderMarketSeedUiState;
  advancedJson: string;
}

export interface EdgeConditionFormState {
  enabled: boolean;
  conditionRow: ConditionDraft;
  conditionSupported: boolean;
  advancedJson: string;
}

export interface ContextFormState {
  sourceTradeId: string;
  marketSlug: string;
  tokenId: string;
  outcomeLabel: string;
  autoClaimEnabled: boolean;
  extras: KeyValueDraft[];
  advancedJson: string;
}

export interface NodeFieldOption {
  label: string;
  value: string;
}

export interface NodeFieldSchema {
  key: string;
  label: string;
  input: 'text' | 'number' | 'datetime-local' | 'textarea' | 'select' | 'checkbox';
  help?: string;
  placeholder?: string;
  options?: NodeFieldOption[];
}
