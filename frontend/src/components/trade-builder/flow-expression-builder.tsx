'use client';

import { useCallback } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import type {
  ExpressionGroup,
  ExpressionGroupOperator,
  ExpressionLeaf,
} from '@/lib/types';

type ExpressionNode = ExpressionLeaf | ExpressionGroup;

const LEAF_OPERATORS: ExpressionLeaf['operator'][] = [
  '>', '>=', '<', '<=', '==', '!=', 'in', 'contains', 'between',
];

const RIGHT_TYPES: ExpressionLeaf['rightType'][] = ['number', 'string', 'boolean'];

function createLeaf(): ExpressionLeaf {
  return { type: 'leaf', leftVar: 'market_price', operator: '<=', rightValue: 50, rightType: 'number' };
}

function createGroup(): ExpressionGroup {
  return { type: 'group', operator: 'and', children: [createLeaf()] };
}

function parseRightValue(raw: string, rightType: ExpressionLeaf['rightType']): unknown {
  if (rightType === 'number') {
    const n = Number(raw);
    return Number.isFinite(n) ? n : 0;
  }
  if (rightType === 'boolean') return raw.trim().toLowerCase() === 'true';
  return raw;
}

function displayRightValue(value: unknown): string {
  if (value == null) return '';
  return String(value);
}

interface LeafEditorProps {
  leaf: ExpressionLeaf;
  onChange: (next: ExpressionLeaf) => void;
  onRemove: () => void;
}

function LeafEditor({ leaf, onChange, onRemove }: LeafEditorProps) {
  return (
    <div className="flex flex-wrap items-center gap-1.5 rounded-md border border-slate-200 bg-white p-1.5">
      <Input
        value={leaf.leftVar}
        onChange={(e) => onChange({ ...leaf, leftVar: e.target.value })}
        placeholder="variable"
        className="h-7 w-28 border-slate-300 bg-white text-xs text-slate-900"
      />
      <select
        value={leaf.operator}
        onChange={(e) => onChange({ ...leaf, operator: e.target.value as ExpressionLeaf['operator'] })}
        className="h-7 rounded-md border border-slate-300 bg-white px-1 text-xs text-slate-900"
      >
        {LEAF_OPERATORS.map((op) => (
          <option key={op} value={op}>{op}</option>
        ))}
      </select>
      <Input
        value={displayRightValue(leaf.rightValue)}
        onChange={(e) => onChange({ ...leaf, rightValue: parseRightValue(e.target.value, leaf.rightType) })}
        placeholder="value"
        className="h-7 w-20 border-slate-300 bg-white text-xs text-slate-900"
      />
      <select
        value={leaf.rightType}
        onChange={(e) => onChange({ ...leaf, rightType: e.target.value as ExpressionLeaf['rightType'] })}
        className="h-7 rounded-md border border-slate-300 bg-white px-1 text-[10px] text-slate-600"
      >
        {RIGHT_TYPES.map((t) => (
          <option key={t} value={t}>{t}</option>
        ))}
      </select>
      <Button size="sm" variant="outline" className="h-6 border-red-200 px-1.5 text-[10px] text-red-500 hover:bg-red-50" onClick={onRemove}>
        x
      </Button>
    </div>
  );
}

interface GroupEditorProps {
  group: ExpressionGroup;
  onChange: (next: ExpressionGroup) => void;
  onRemove?: () => void;
  depth: number;
}

function GroupEditor({ group, onChange, onRemove, depth }: GroupEditorProps) {
  const updateChild = useCallback(
    (index: number, next: ExpressionNode) => {
      const children = [...group.children];
      children[index] = next;
      onChange({ ...group, children });
    },
    [group, onChange]
  );

  const removeChild = useCallback(
    (index: number) => {
      const children = group.children.filter((_, i) => i !== index);
      if (children.length === 0) children.push(createLeaf());
      onChange({ ...group, children });
    },
    [group, onChange]
  );

  const addLeaf = useCallback(() => {
    onChange({ ...group, children: [...group.children, createLeaf()] });
  }, [group, onChange]);

  const addNestedGroup = useCallback(() => {
    if (depth >= 3) return;
    onChange({ ...group, children: [...group.children, createGroup()] });
  }, [depth, group, onChange]);

  const bgClass = depth === 0 ? 'bg-slate-50' : depth === 1 ? 'bg-slate-100/60' : 'bg-slate-200/40';

  return (
    <div className={`space-y-1.5 rounded-lg border border-slate-300 p-2 ${bgClass}`}>
      <div className="flex items-center gap-2">
        <select
          value={group.operator}
          onChange={(e) => onChange({ ...group, operator: e.target.value as ExpressionGroupOperator })}
          className="h-7 rounded-md border border-slate-300 bg-white px-2 text-xs font-medium text-slate-700"
        >
          <option value="and">AND</option>
          <option value="or">OR</option>
        </select>
        <span className="text-[10px] text-slate-500">
          {group.children.length} kosul
        </span>
        <div className="flex-1" />
        {onRemove && (
          <Button size="sm" variant="outline" className="h-6 border-red-200 px-1.5 text-[10px] text-red-500 hover:bg-red-50" onClick={onRemove}>
            Grubu Sil
          </Button>
        )}
      </div>

      {group.children.map((child, idx) =>
        child.type === 'leaf' ? (
          <LeafEditor
            key={idx}
            leaf={child}
            onChange={(next) => updateChild(idx, next)}
            onRemove={() => removeChild(idx)}
          />
        ) : (
          <GroupEditor
            key={idx}
            group={child}
            onChange={(next) => updateChild(idx, next)}
            onRemove={() => removeChild(idx)}
            depth={depth + 1}
          />
        )
      )}

      <div className="flex gap-1">
        <Button size="sm" variant="outline" className="h-6 border-slate-300 px-2 text-[10px] text-slate-600" onClick={addLeaf}>
          + Kosul
        </Button>
        {depth < 3 && (
          <Button size="sm" variant="outline" className="h-6 border-slate-300 px-2 text-[10px] text-slate-600" onClick={addNestedGroup}>
            + Grup
          </Button>
        )}
      </div>
    </div>
  );
}

interface ExpressionBuilderProps {
  value: ExpressionGroup;
  onChange: (next: ExpressionGroup) => void;
}

export function ExpressionBuilder({ value, onChange }: ExpressionBuilderProps) {
  return <GroupEditor group={value} onChange={onChange} depth={0} />;
}

export function expressionGroupToJsonLogic(group: ExpressionGroup): Record<string, unknown> {
  if (group.children.length === 0) return { '==': [1, 1] };

  const mapped = group.children.map((child) => {
    if (child.type === 'leaf') return leafToJsonLogic(child);
    return expressionGroupToJsonLogic(child);
  });

  if (mapped.length === 1) return mapped[0];
  return { [group.operator]: mapped };
}

function leafToJsonLogic(leaf: ExpressionLeaf): Record<string, unknown> {
  const leftOperand = { var: leaf.leftVar || 'market_price' };

  if (leaf.operator === 'between') {
    const parts = String(leaf.rightValue).split(',').map((s) => Number(s.trim()));
    const lo = Number.isFinite(parts[0]) ? parts[0] : 0;
    const hi = Number.isFinite(parts[1]) ? parts[1] : 100;
    return { '<=': [lo, leftOperand, hi] };
  }

  if (leaf.operator === 'in') {
    const items = String(leaf.rightValue).split(',').map((s) => s.trim());
    return { in: [leftOperand, items] };
  }

  if (leaf.operator === 'contains') {
    return { in: [leaf.rightValue, leftOperand] };
  }

  const rightVal = leaf.rightType === 'number'
    ? (Number.isFinite(Number(leaf.rightValue)) ? Number(leaf.rightValue) : 0)
    : leaf.rightType === 'boolean'
      ? String(leaf.rightValue).trim().toLowerCase() === 'true'
      : leaf.rightValue;

  return { [leaf.operator]: [leftOperand, rightVal] };
}

export function jsonLogicToExpressionGroup(logic: unknown): ExpressionGroup | null {
  if (!logic || typeof logic !== 'object' || Array.isArray(logic)) return null;
  const obj = logic as Record<string, unknown>;

  if (Array.isArray(obj.and)) {
    const children = (obj.and as unknown[]).map(parseChild).filter(Boolean) as ExpressionNode[];
    if (children.length > 0) return { type: 'group', operator: 'and', children };
  }

  if (Array.isArray(obj.or)) {
    const children = (obj.or as unknown[]).map(parseChild).filter(Boolean) as ExpressionNode[];
    if (children.length > 0) return { type: 'group', operator: 'or', children };
  }

  const leaf = tryParseLeaf(obj);
  if (leaf) return { type: 'group', operator: 'and', children: [leaf] };

  return null;
}

function parseChild(item: unknown): ExpressionNode | null {
  if (!item || typeof item !== 'object' || Array.isArray(item)) return null;
  const obj = item as Record<string, unknown>;

  if (Array.isArray(obj.and)) {
    const children = (obj.and as unknown[]).map(parseChild).filter(Boolean) as ExpressionNode[];
    if (children.length > 0) return { type: 'group', operator: 'and', children };
  }

  if (Array.isArray(obj.or)) {
    const children = (obj.or as unknown[]).map(parseChild).filter(Boolean) as ExpressionNode[];
    if (children.length > 0) return { type: 'group', operator: 'or', children };
  }

  return tryParseLeaf(obj);
}

function tryParseLeaf(obj: Record<string, unknown>): ExpressionLeaf | null {
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
      return { type: 'leaf', leftVar: String((a as Record<string, unknown>).var), operator: 'in', rightValue: Array.isArray(b) ? b.join(', ') : String(b), rightType: 'string' };
    }
    if (b && typeof b === 'object' && 'var' in (b as Record<string, unknown>)) {
      return { type: 'leaf', leftVar: String((b as Record<string, unknown>).var), operator: 'contains', rightValue: String(a), rightType: 'string' };
    }
  }

  return null;
}
