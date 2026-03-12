import {
  createEmptyConditionDraft,
  createEmptyDrawdownRuleRow,
  createEmptyKeyValueDraft,
  createEmptyOutcomeConditionRow,
  type ConditionDraft,
  type DrawdownRuleRow,
  type NodeConfigFormState,
  type OutcomeConditionRow,
  type PrimitiveValueType,
} from '@/lib/trade-flow-config-mappers';
import type { UpstreamMaxPriceResolution } from '../flow-canvas-utils';

function getTriggerMarketSource(fields: Record<string, string>): string | null {
  const marketMode = (fields.marketMode ?? '').trim().toLowerCase();
  const marketSlug = (fields.marketSlug ?? '').trim();
  const marketScope = (fields.marketScope ?? '').trim();
  const source = marketMode === 'auto_scope' ? marketScope : marketSlug;
  return source || null;
}

function getOutcomeSource(nodeType: string, fields: Record<string, string>): string | null {
  if (nodeType === 'trigger.market_price') {
    return getTriggerMarketSource(fields);
  }
  if (nodeType === 'trigger.open_positions' || nodeType === 'trigger.position_drawdown') {
    return (fields.marketSlug ?? '').trim() || null;
  }
  return null;
}

function shouldResetDependentSelections(nodeType: string, key: string): boolean {
  if (nodeType === 'trigger.market_price') {
    return key === 'marketMode' || key === 'marketSlug' || key === 'marketScope';
  }
  if (nodeType === 'trigger.open_positions' || nodeType === 'trigger.position_drawdown') {
    return key === 'marketSlug';
  }
  return false;
}

export function updateNodeFieldState(
  prev: NodeConfigFormState | null,
  nodeType: string,
  key: string,
  value: string
): NodeConfigFormState | null {
  if (!prev) return prev;

  const nextFields = { ...prev.fields, [key]: value };
  let next: NodeConfigFormState = { ...prev, fields: nextFields };
  if (nodeType === 'action.place_order' && key === 'maxPriceCent' && prev.placeOrderMaxPriceUi) {
    next = {
      ...next,
      placeOrderMaxPriceUi: {
        ...prev.placeOrderMaxPriceUi,
        isInheritedValue: false,
      },
    };
  }

  if (!shouldResetDependentSelections(nodeType, key)) {
    return next;
  }

  const previousSource = getOutcomeSource(nodeType, prev.fields);
  const nextSource = getOutcomeSource(nodeType, nextFields);
  if (previousSource === nextSource) {
    return next;
  }

  if (nodeType === 'trigger.position_drawdown') {
    const tokenId = (next.fields.tokenId ?? '').trim();
    const outcomeLabel = (next.fields.outcomeLabel ?? '').trim();
    if (!tokenId && !outcomeLabel) {
      return next;
    }
    return {
      ...next,
      fields: {
        ...next.fields,
        tokenId: '',
        outcomeLabel: '',
      },
    };
  }

  if (next.outcomeConditionRows.length === 0) {
    return next;
  }

  next = {
    ...next,
    outcomeConditionRows: [],
  };
  return next;
}

export function updateTriggerSizeRowState(
  prev: NodeConfigFormState | null,
  index: number,
  value: string
): NodeConfigFormState | null {
  if (!prev) return prev;
  const nextRows = [...prev.triggerSizeRows];
  while (nextRows.length <= index) nextRows.push('');
  nextRows[index] = value;
  return { ...prev, triggerSizeRows: nextRows };
}

export function syncPlaceOrderTriggerRowsState(
  prev: NodeConfigFormState | null
): NodeConfigFormState | null {
  if (!prev) return prev;
  const parsedMax = Number(prev.fields.maxTriggers ?? '');
  const targetCount =
    Number.isFinite(parsedMax) && parsedMax > 1 ? Math.min(20, Math.floor(parsedMax)) : 0;
  const currentRows = prev.triggerSizeRows || [];
  const nextRows =
    targetCount > 0
      ? Array.from({ length: targetCount }, (_, i) => currentRows[i] ?? '')
      : [];
  const unchanged =
    nextRows.length === currentRows.length && nextRows.every((v, i) => v === currentRows[i]);
  if (unchanged) return prev;
  return { ...prev, triggerSizeRows: nextRows };
}

function sameDistinctValues(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

export function syncPlaceOrderInheritedMaxPriceState(
  prev: NodeConfigFormState | null,
  resolution: UpstreamMaxPriceResolution
): NodeConfigFormState | null {
  if (!prev) return prev;

  const currentMaxPriceCent = (prev.fields.maxPriceCent ?? '').trim();
  const wasInherited = prev.placeOrderMaxPriceUi?.isInheritedValue === true;
  let nextFields = prev.fields;
  let isInheritedValue = false;

  if (resolution.kind === 'single' && resolution.maxPriceCent) {
    if (wasInherited || currentMaxPriceCent.length === 0) {
      if (currentMaxPriceCent !== resolution.maxPriceCent) {
        nextFields = { ...prev.fields, maxPriceCent: resolution.maxPriceCent };
      }
      isInheritedValue = true;
    }
  } else if (wasInherited && currentMaxPriceCent.length > 0) {
    nextFields = { ...prev.fields, maxPriceCent: '' };
  }

  const nextUi = {
    isInheritedValue,
    upstreamKind: resolution.kind,
    upstreamMaxPriceCent: resolution.maxPriceCent,
    distinctUpstreamMaxPriceCents: resolution.distinctMaxPriceCents,
  } as const;

  const prevUi = prev.placeOrderMaxPriceUi;
  const fieldsUnchanged = nextFields === prev.fields;
  const uiUnchanged =
    prevUi != null &&
    prevUi.isInheritedValue === nextUi.isInheritedValue &&
    prevUi.upstreamKind === nextUi.upstreamKind &&
    prevUi.upstreamMaxPriceCent === nextUi.upstreamMaxPriceCent &&
    sameDistinctValues(prevUi.distinctUpstreamMaxPriceCents, nextUi.distinctUpstreamMaxPriceCents);

  if (fieldsUnchanged && uiUnchanged) {
    return prev;
  }

  return {
    ...prev,
    fields: nextFields,
    placeOrderMaxPriceUi: nextUi,
  };
}

export function updateExpressionRowState(
  prev: NodeConfigFormState | null,
  rowId: string,
  patch: Partial<ConditionDraft>
): NodeConfigFormState | null {
  if (!prev) return prev;
  return {
    ...prev,
    expressionRows: prev.expressionRows.map((r) => (r.id === rowId ? { ...r, ...patch } : r)),
  };
}

export function addExpressionRowState(prev: NodeConfigFormState | null): NodeConfigFormState | null {
  if (!prev) return prev;
  return { ...prev, expressionRows: [...prev.expressionRows, createEmptyConditionDraft()] };
}

export function removeExpressionRowState(
  prev: NodeConfigFormState | null,
  rowId: string
): NodeConfigFormState | null {
  if (!prev) return prev;
  const next = prev.expressionRows.filter((r) => r.id !== rowId);
  return { ...prev, expressionRows: next.length > 0 ? next : [createEmptyConditionDraft()] };
}

export function updateStatePatchRowState(
  prev: NodeConfigFormState | null,
  rowId: string,
  patch: Partial<{ key: string; value: string; valueType: PrimitiveValueType }>
): NodeConfigFormState | null {
  if (!prev) return prev;
  return {
    ...prev,
    statePatchRows: prev.statePatchRows.map((r) => (r.id === rowId ? { ...r, ...patch } : r)),
  };
}

export function addStatePatchRowState(prev: NodeConfigFormState | null): NodeConfigFormState | null {
  if (!prev) return prev;
  return { ...prev, statePatchRows: [...prev.statePatchRows, createEmptyKeyValueDraft()] };
}

export function removeStatePatchRowState(
  prev: NodeConfigFormState | null,
  rowId: string
): NodeConfigFormState | null {
  if (!prev) return prev;
  const next = prev.statePatchRows.filter((r) => r.id !== rowId);
  return { ...prev, statePatchRows: next.length > 0 ? next : [createEmptyKeyValueDraft()] };
}

export function addOutcomeConditionState(
  prev: NodeConfigFormState | null,
  tokenId: string,
  outcomeLabel: string
): NodeConfigFormState | null {
  if (!prev) return prev;
  const normalizedTokenId = tokenId.trim();
  const normalizedOutcomeLabel = outcomeLabel.trim();
  if (!normalizedTokenId || !normalizedOutcomeLabel) return prev;
  if (prev.outcomeConditionRows.some((r) => r.tokenId === normalizedTokenId)) return prev;
  const row: OutcomeConditionRow = {
    ...createEmptyOutcomeConditionRow(),
    tokenId: normalizedTokenId,
    outcomeLabel: normalizedOutcomeLabel,
  };
  return { ...prev, outcomeConditionRows: [...prev.outcomeConditionRows, row] };
}

export function removeOutcomeConditionState(
  prev: NodeConfigFormState | null,
  rowId: string
): NodeConfigFormState | null {
  if (!prev) return prev;
  return {
    ...prev,
    outcomeConditionRows: prev.outcomeConditionRows.filter((r) => r.id !== rowId),
  };
}

export function updateOutcomeConditionState(
  prev: NodeConfigFormState | null,
  rowId: string,
  patch: Partial<OutcomeConditionRow>
): NodeConfigFormState | null {
  if (!prev) return prev;
  return {
    ...prev,
    outcomeConditionRows: prev.outcomeConditionRows.map((r) => (r.id === rowId ? { ...r, ...patch } : r)),
  };
}

export function addDrawdownRuleState(prev: NodeConfigFormState | null): NodeConfigFormState | null {
  if (!prev) return prev;
  return {
    ...prev,
    drawdownRuleRows: [...(prev.drawdownRuleRows || []), createEmptyDrawdownRuleRow()],
  };
}

export function removeDrawdownRuleState(
  prev: NodeConfigFormState | null,
  rowId: string
): NodeConfigFormState | null {
  if (!prev) return prev;
  const next = (prev.drawdownRuleRows || []).filter((row) => row.id !== rowId);
  return {
    ...prev,
    drawdownRuleRows: next.length > 0 ? next : [createEmptyDrawdownRuleRow()],
  };
}

export function updateDrawdownRuleState(
  prev: NodeConfigFormState | null,
  rowId: string,
  patch: Partial<DrawdownRuleRow>
): NodeConfigFormState | null {
  if (!prev) return prev;
  return {
    ...prev,
    drawdownRuleRows: (prev.drawdownRuleRows || []).map((row) =>
      row.id === rowId ? { ...row, ...patch } : row
    ),
  };
}
