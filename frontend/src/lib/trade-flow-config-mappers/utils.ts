import type { KeyValueDraft, PrimitiveValueType } from './types';

type SupportedOpenPositionTriggerCondition = 'cross_above' | 'cross_below';
type SupportedMarketPriceTriggerCondition =
  | SupportedOpenPositionTriggerCondition
  | 'level_above'
  | 'level_below';

interface OutcomeConditionValidationInput {
  nodeType: string;
  tokenId: unknown;
  outcomeLabel: unknown;
  triggerCondition: unknown;
  triggerPriceCent: unknown;
  maxPriceCent?: unknown;
  priceToBeatTriggerEnabled?: boolean;
}

export interface OutcomeConditionValidationResult {
  isValid: boolean;
  isPtbOnly: boolean;
  requiresOnceRepeatMode: boolean;
  triggerCondition: string;
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

export function safeJsonStringify(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

export function createId(prefix: string): string {
  return `${prefix}_${Math.random().toString(36).slice(2, 10)}`;
}

export function toStringValue(value: unknown): string {
  if (value == null) return '';
  if (typeof value === 'string') return value;
  if (typeof value === 'number' || typeof value === 'boolean') return String(value);
  return '';
}

export function toCentStringValue(centValue: unknown, legacyDecimalValue?: unknown): string {
  const cent = toStringValue(centValue).trim();
  if (cent) return cent;

  const legacyDecimal = Number(toStringValue(legacyDecimalValue).trim());
  if (Number.isFinite(legacyDecimal) && legacyDecimal > 0 && legacyDecimal <= 1) {
    return String(Math.round(legacyDecimal * 100));
  }

  return '';
}

export function isSupportedOpenPositionTriggerCondition(
  value: unknown
): value is SupportedOpenPositionTriggerCondition {
  return value === 'cross_above' || value === 'cross_below';
}

export function isSupportedMarketPriceTriggerCondition(
  value: unknown
): value is SupportedMarketPriceTriggerCondition {
  return (
    value === 'cross_above' ||
    value === 'cross_below' ||
    value === 'level_above' ||
    value === 'level_below'
  );
}

export function validateOutcomeConditionRow(
  input: OutcomeConditionValidationInput
): OutcomeConditionValidationResult {
  const tokenId = toStringValue(input.tokenId).trim();
  const outcomeLabel = toStringValue(input.outcomeLabel).trim();
  const triggerCondition = toStringValue(input.triggerCondition).trim();
  const triggerPriceCentRaw = toStringValue(input.triggerPriceCent).trim();
  const triggerPriceCent = Number(triggerPriceCentRaw);
  const maxPriceCentRaw = toStringValue(input.maxPriceCent).trim();
  const maxPriceCent = maxPriceCentRaw ? Number(maxPriceCentRaw) : null;
  const hasValidMaxPriceCent =
    !maxPriceCentRaw ||
    (Number.isFinite(maxPriceCent) && (maxPriceCent as number) > 0 && (maxPriceCent as number) <= 100);
  const invalidResult: OutcomeConditionValidationResult = {
    isValid: false,
    isPtbOnly: false,
    requiresOnceRepeatMode: false,
    triggerCondition,
  };

  if (!tokenId || !outcomeLabel || !hasValidMaxPriceCent) {
    return invalidResult;
  }

  if (input.nodeType === 'trigger.market_price') {
    const isPtbOnly =
      input.priceToBeatTriggerEnabled === true && !triggerCondition && !triggerPriceCentRaw;
    if (isPtbOnly) {
      return {
        isValid: true,
        isPtbOnly: true,
        requiresOnceRepeatMode: false,
        triggerCondition,
      };
    }
    if (!isSupportedMarketPriceTriggerCondition(triggerCondition)) {
      return invalidResult;
    }
    return {
      isValid:
        Number.isFinite(triggerPriceCent) && triggerPriceCent > 0 && triggerPriceCent <= 100,
      isPtbOnly: false,
      requiresOnceRepeatMode:
        triggerCondition === 'level_above' || triggerCondition === 'level_below',
      triggerCondition,
    };
  }

  if (!isSupportedOpenPositionTriggerCondition(triggerCondition)) {
    return invalidResult;
  }
  return {
    isValid: Number.isFinite(triggerPriceCent) && triggerPriceCent > 0 && triggerPriceCent <= 100,
    isPtbOnly: false,
    requiresOnceRepeatMode: false,
    triggerCondition,
  };
}

export function toTriggerMarketOnceScopeVersion(value: unknown): number {
  const parsed = Number(toStringValue(value).trim());
  if (!Number.isFinite(parsed)) return 0;
  return Math.trunc(parsed);
}

export function valueTypeOf(value: unknown): PrimitiveValueType {
  if (typeof value === 'number') return 'number';
  if (typeof value === 'boolean') return 'boolean';
  return 'string';
}

export function parsePrimitive(value: string, valueType: PrimitiveValueType): unknown {
  if (valueType === 'number') {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  if (valueType === 'boolean') {
    if (value.trim().toLowerCase() === 'true') return true;
    if (value.trim().toLowerCase() === 'false') return false;
    return null;
  }
  return value;
}

export function toBooleanValue(value: unknown): boolean {
  if (typeof value === 'boolean') return value;
  if (typeof value === 'number') return Number.isFinite(value) && value !== 0;
  if (typeof value !== 'string') return false;
  const normalized = value.trim().toLowerCase();
  return ['true', '1', 'yes', 'y', 'on'].includes(normalized);
}

export function toDateTimeLocalString(value: unknown): string {
  const raw = toStringValue(value).trim();
  if (!raw) return '';
  if (/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(raw)) return raw;

  const parsed = new Date(raw);
  if (Number.isNaN(parsed.getTime())) return '';
  const year = parsed.getFullYear();
  const month = `${parsed.getMonth() + 1}`.padStart(2, '0');
  const day = `${parsed.getDate()}`.padStart(2, '0');
  const hour = `${parsed.getHours()}`.padStart(2, '0');
  const minute = `${parsed.getMinutes()}`.padStart(2, '0');
  return `${year}-${month}-${day}T${hour}:${minute}`;
}

export function objectToRows(value: unknown): KeyValueDraft[] {
  if (!isRecord(value)) return [];
  return Object.entries(value).map(([key, rawValue]) => ({
    id: createId('kv'),
    key,
    value: toStringValue(rawValue),
    valueType: valueTypeOf(rawValue),
  }));
}

export function parseNumberArrayToStringRows(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.map((item) => toStringValue(item).trim());
}
