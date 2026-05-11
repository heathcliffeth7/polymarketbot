import type { DcaMarketSelectionMode, DcaPreviewRequest, DcaSideMode } from './schema';

const MARKET_SELECTION_MODES = new Set<DcaMarketSelectionMode>([
  'manual_slug',
  'manual_slug_list',
  'auto_group_top_n',
  'auto_scope',
]);

const SIDE_MODES = new Set<DcaSideMode>([
  'one_sided',
  'two_sided_pair',
  'multi_outcome_basket',
]);

function toNumber(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return null;
}

export function validateDcaPreviewRequest(request: DcaPreviewRequest): string[] {
  const errors: string[] = [];
  const config = request.dcaConfig ?? {};
  const selectedOutcomes = request.selectedOutcomes ?? [];
  const sideMode = String(config.sideMode ?? config.dcaSideMode ?? '').trim() as DcaSideMode;
  if (!SIDE_MODES.has(sideMode)) {
    errors.push('sideMode must be one_sided, two_sided_pair, or multi_outcome_basket.');
  }
  if (sideMode === 'one_sided' && selectedOutcomes.length !== 1) {
    errors.push('one_sided requires exactly 1 selected outcome.');
  }
  if (sideMode === 'two_sided_pair') {
    if (selectedOutcomes.length !== 2) errors.push('two_sided_pair requires exactly 2 selected outcomes.');
    const slugs = new Set(selectedOutcomes.map((outcome) => outcome.slug));
    if (slugs.size > 1) errors.push('two_sided_pair requires both outcomes from the same slug.');
    if (request.market && request.market.pairEligible !== true) {
      errors.push(`two_sided_pair requires pairEligible=true (${request.market.pairEligibilityReason}).`);
    }
    const targetPairCostCent = toNumber(config.targetPairCostCent);
    if (targetPairCostCent != null && targetPairCostCent >= 100) {
      errors.push('targetPairCostCent must be < 100.');
    }
  }
  if (sideMode === 'multi_outcome_basket') {
    if (selectedOutcomes.length < 2) errors.push('multi_outcome_basket requires at least 2 selected outcomes.');
    if (config.targetPairCostCent != null || config.pairMaxTotalCent != null || config.targetLockedProfitUsdc != null) {
      errors.push('multi_outcome_basket disables pair-cost and locked-profit fields.');
    }
  }
  return errors;
}

export function validateDcaMarketSelectionConfig(config: Record<string, unknown>): string[] {
  const errors: string[] = [];
  const mode = String(config.marketSelectionMode ?? config.dcaMarketSelectionMode ?? '').trim() as DcaMarketSelectionMode;
  if (!MARKET_SELECTION_MODES.has(mode)) {
    errors.push('marketSelectionMode must be manual_slug, manual_slug_list, auto_group_top_n, or auto_scope.');
  }
  if (mode === 'manual_slug' && !String(config.manualSlug ?? '').trim()) {
    errors.push('manual_slug requires manualSlug.');
  }
  if (mode === 'manual_slug_list') {
    const manualSlugs = Array.isArray(config.manualSlugs) ? config.manualSlugs : [];
    const maxActiveSlugs = toNumber(config.maxActiveSlugs) ?? 1;
    if (manualSlugs.length < 1) errors.push('manual_slug_list requires at least 1 manual slug.');
    if (maxActiveSlugs > manualSlugs.length) errors.push('maxActiveSlugs cannot exceed manualSlugs length.');
  }
  if (mode === 'auto_group_top_n') {
    const candidateSlugLimit = toNumber(config.candidateSlugLimit) ?? 0;
    const maxActiveSlugs = toNumber(config.maxActiveSlugs) ?? 1;
    if (!String(config.marketGroup ?? config.autoGroup ?? '').trim()) errors.push('auto_group_top_n requires marketGroup.');
    if (candidateSlugLimit < maxActiveSlugs) errors.push('candidateSlugLimit must be >= maxActiveSlugs.');
  }
  if (mode === 'auto_scope' && !String(config.marketScope ?? '').trim()) {
    errors.push('auto_scope requires marketScope.');
  }
  return errors;
}
