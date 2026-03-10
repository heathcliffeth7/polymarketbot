import type { ExpressionGroup, ExpressionLeaf } from '@/lib/types';
import { CONDITION_OPERATORS } from './constants';
import { createEmptyConditionDraft } from './drafts';
import type { ConditionDraft, ExpressionJoin, KeyValueDraft } from './types';
import {
  createId,
  isRecord,
  objectToRows,
  parseNumberArrayToStringRows,
  parsePrimitive,
  toStringValue,
  valueTypeOf,
} from './utils';

function parseSimpleCondition(input: unknown): Omit<ConditionDraft, 'id'> | null {
  if (!isRecord(input)) return null;

  const operators = CONDITION_OPERATORS.filter((operator) => Object.prototype.hasOwnProperty.call(input, operator));
  if (operators.length !== 1) return null;

  const operator = operators[0];
  const rawOperands = input[operator];
  if (!Array.isArray(rawOperands) || rawOperands.length !== 2) return null;

  const left = rawOperands[0];
  const right = rawOperands[1];
  if (!isRecord(left) || typeof left.var !== 'string') return null;

  const rightType = valueTypeOf(right);
  return {
    leftVar: left.var,
    operator,
    rightType,
    rightValue: toStringValue(right),
  };
}

function buildSimpleCondition(draft: ConditionDraft): Record<string, unknown> {
  const primitive = parsePrimitive(draft.rightValue, draft.rightType);
  const normalizedRight =
    primitive == null
      ? draft.rightType === 'number'
        ? 0
        : draft.rightType === 'boolean'
          ? false
          : ''
      : primitive;

  return {
    [draft.operator]: [{ var: draft.leftVar || 'market_price' }, normalizedRight],
  };
}

function parseExpressionDraft(
  expression: unknown
): { rows: ConditionDraft[]; join: ExpressionJoin; supported: boolean } {
  const parsedSingle = parseSimpleCondition(expression);
  if (parsedSingle) {
    return {
      rows: [{ id: createId('expr'), ...parsedSingle }],
      join: 'and',
      supported: true,
    };
  }

  if (isRecord(expression)) {
    const join = Array.isArray(expression.and) ? 'and' : Array.isArray(expression.or) ? 'or' : null;
    if (join) {
      const expressions = (expression[join] as unknown[]) || [];
      const rows: ConditionDraft[] = [];
      for (const item of expressions) {
        const parsed = parseSimpleCondition(item);
        if (!parsed) {
          return {
            rows: [createEmptyConditionDraft()],
            join: 'and',
            supported: false,
          };
        }
        rows.push({ id: createId('expr'), ...parsed });
      }
      if (rows.length > 0) {
        return {
          rows,
          join,
          supported: true,
        };
      }
    }
  }

  return {
    rows: [createEmptyConditionDraft()],
    join: 'and',
    supported: false,
  };
}

function buildExpression(rows: ConditionDraft[], join: ExpressionJoin): Record<string, unknown> {
  const validRows = rows.filter((row) => row.leftVar.trim());
  if (validRows.length === 0) {
    return { '==': [1, 1] };
  }
  if (validRows.length === 1) {
    return buildSimpleCondition(validRows[0]);
  }
  return {
    [join]: validRows.map((row) => buildSimpleCondition(row)),
  };
}

export function buildObjectFromKeyValueDrafts(rows: KeyValueDraft[]): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const row of rows) {
    const key = row.key.trim();
    if (!key) continue;
    const parsed = parsePrimitive(row.value, row.valueType);
    if (parsed == null) continue;
    result[key] = parsed;
  }
  return result;
}

type ExpressionNode = ExpressionLeaf | ExpressionGroup;

function leafToJsonLogic(leaf: ExpressionLeaf): Record<string, unknown> {
  const leftOp = { var: leaf.leftVar || 'market_price' };

  if (leaf.operator === 'between') {
    const parts = String(leaf.rightValue).split(',').map((s) => Number(s.trim()));
    const lo = Number.isFinite(parts[0]) ? parts[0] : 0;
    const hi = Number.isFinite(parts[1]) ? parts[1] : 100;
    return { '<=': [lo, leftOp, hi] };
  }
  if (leaf.operator === 'in') {
    const items = String(leaf.rightValue).split(',').map((s) => s.trim());
    return { in: [leftOp, items] };
  }
  if (leaf.operator === 'contains') {
    return { in: [leaf.rightValue, leftOp] };
  }

  const rightVal = leaf.rightType === 'number'
    ? (Number.isFinite(Number(leaf.rightValue)) ? Number(leaf.rightValue) : 0)
    : leaf.rightType === 'boolean'
      ? String(leaf.rightValue).trim().toLowerCase() === 'true'
      : leaf.rightValue;

  return { [leaf.operator]: [leftOp, rightVal] };
}

export function nestedExprGroupToJsonLogic(group: ExpressionGroup): Record<string, unknown> {
  if (group.children.length === 0) return { '==': [1, 1] };
  const mapped = group.children.map((child: ExpressionNode) => {
    if (child.type === 'leaf') return leafToJsonLogic(child);
    return nestedExprGroupToJsonLogic(child);
  });
  if (mapped.length === 1) return mapped[0];
  return { [group.operator]: mapped };
}

function tryParseJsonLogicLeaf(obj: Record<string, unknown>): ExpressionLeaf | null {
  for (const op of ['>', '>=', '<', '<=', '==', '!='] as const) {
    if (!Array.isArray(obj[op]) || (obj[op] as unknown[]).length !== 2) continue;
    const [left, right] = obj[op] as [unknown, unknown];
    if (!left || typeof left !== 'object' || !('var' in (left as Record<string, unknown>))) continue;
    const leftVar = String((left as Record<string, unknown>).var);
    const rightType = typeof right === 'number' ? 'number' : typeof right === 'boolean' ? 'boolean' : 'string';
    return { type: 'leaf', leftVar, operator: op, rightValue: right, rightType };
  }
  if (Array.isArray(obj.in) && (obj.in as unknown[]).length === 2) {
    const [a, b] = obj.in as [unknown, unknown];
    if (a && typeof a === 'object' && 'var' in (a as Record<string, unknown>)) {
      return { type: 'leaf', leftVar: String((a as Record<string, unknown>).var), operator: 'in', rightValue: Array.isArray(b) ? (b as unknown[]).join(', ') : String(b), rightType: 'string' };
    }
    if (b && typeof b === 'object' && 'var' in (b as Record<string, unknown>)) {
      return { type: 'leaf', leftVar: String((b as Record<string, unknown>).var), operator: 'contains', rightValue: String(a), rightType: 'string' };
    }
  }
  return null;
}

function parseJsonLogicChild(item: unknown): ExpressionNode | null {
  if (!item || typeof item !== 'object' || Array.isArray(item)) return null;
  const obj = item as Record<string, unknown>;
  if (Array.isArray(obj.and)) {
    const children = (obj.and as unknown[]).map(parseJsonLogicChild).filter(Boolean) as ExpressionNode[];
    if (children.length > 0) return { type: 'group', operator: 'and', children };
  }
  if (Array.isArray(obj.or)) {
    const children = (obj.or as unknown[]).map(parseJsonLogicChild).filter(Boolean) as ExpressionNode[];
    if (children.length > 0) return { type: 'group', operator: 'or', children };
  }
  return tryParseJsonLogicLeaf(obj);
}

export function jsonLogicToNestedExprGroup(logic: unknown): ExpressionGroup | null {
  if (!logic || typeof logic !== 'object' || Array.isArray(logic)) return null;
  const obj = logic as Record<string, unknown>;
  if (Array.isArray(obj.and)) {
    const children = (obj.and as unknown[]).map(parseJsonLogicChild).filter(Boolean) as ExpressionNode[];
    if (children.length > 0) return { type: 'group', operator: 'and', children };
  }
  if (Array.isArray(obj.or)) {
    const children = (obj.or as unknown[]).map(parseJsonLogicChild).filter(Boolean) as ExpressionNode[];
    if (children.length > 0) return { type: 'group', operator: 'or', children };
  }
  const leaf = tryParseJsonLogicLeaf(obj);
  if (leaf) return { type: 'group', operator: 'and', children: [leaf] };
  return null;
}

export {
  buildExpression,
  buildSimpleCondition,
  objectToRows,
  parseExpressionDraft,
  parseNumberArrayToStringRows,
  parseSimpleCondition,
};
