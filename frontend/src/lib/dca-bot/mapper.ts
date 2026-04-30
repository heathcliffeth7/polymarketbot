import type { DcaSelectedOutcome } from './schema';

export function selectedOutcomesFromConfig(config: Record<string, unknown>): DcaSelectedOutcome[] {
  const raw = config.selectedOutcomes;
  if (!Array.isArray(raw)) return [];
  return raw
    .map((item) => {
      if (!item || typeof item !== 'object' || Array.isArray(item)) return null;
      const row = item as Record<string, unknown>;
      const slug = String(row.slug ?? '').trim();
      const outcomeLabel = String(row.outcomeLabel ?? row.outcome ?? row.label ?? '').trim();
      const tokenId = String(row.tokenId ?? row.token_id ?? '').trim();
      return slug && outcomeLabel && tokenId ? { slug, outcomeLabel, tokenId } : null;
    })
    .filter((item): item is DcaSelectedOutcome => item != null);
}
