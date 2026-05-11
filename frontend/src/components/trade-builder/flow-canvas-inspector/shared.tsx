export const EMPTY_SELECT_SENTINEL = '__none__';

export function shortText(value: string, max = 36) {
  const trimmed = value.trim();
  if (!trimmed) return '-';
  if (trimmed.length <= max) return trimmed;
  return `${trimmed.slice(0, max)}...`;
}
