import { CONTEXT_BASE_KEYS } from './constants';
import { buildObjectFromKeyValueDrafts } from './expressions';
import type { ContextFormState, KeyValueDraft } from './types';
import { createId, isRecord, safeJsonStringify, toBooleanValue, toStringValue, valueTypeOf } from './utils';

export function parseContextToForm(context: unknown): ContextFormState {
  const ctx = isRecord(context) ? context : {};
  const sourceTradeId = toStringValue(ctx.sourceTradeId);
  const marketSlug = toStringValue(ctx.marketSlug);
  const tokenId = toStringValue(ctx.tokenId);
  const outcomeLabel = toStringValue(ctx.outcomeLabel);
  const autoClaimEnabled = toBooleanValue(ctx.autoClaimEnabled);

  const extras: KeyValueDraft[] = [];
  for (const [key, value] of Object.entries(ctx)) {
    if (CONTEXT_BASE_KEYS.has(key)) continue;
    extras.push({
      id: createId('ctx'),
      key,
      value: toStringValue(value),
      valueType: valueTypeOf(value),
    });
  }

  return {
    sourceTradeId,
    marketSlug,
    tokenId,
    outcomeLabel,
    autoClaimEnabled,
    extras,
    advancedJson: safeJsonStringify(ctx),
  };
}

export function buildContextFromForm(form: ContextFormState): Record<string, unknown> {
  const context: Record<string, unknown> = {};

  const sourceTradeId = form.sourceTradeId.trim();
  if (sourceTradeId) {
    const parsed = Number(sourceTradeId);
    if (Number.isFinite(parsed)) context.sourceTradeId = parsed;
  }

  if (form.marketSlug.trim()) context.marketSlug = form.marketSlug.trim();
  if (form.tokenId.trim()) context.tokenId = form.tokenId.trim();
  if (form.outcomeLabel.trim()) context.outcomeLabel = form.outcomeLabel.trim();
  if (form.autoClaimEnabled) context.autoClaimEnabled = true;

  const extraValues = buildObjectFromKeyValueDrafts(form.extras);
  for (const [key, value] of Object.entries(extraValues)) {
    context[key] = value;
  }

  return context;
}
