import type { DcaLadderPreviewLevel, DcaPreviewRequest } from './schema';

function toNumber(value: unknown, fallback: number): number {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return fallback;
}

function clampCent(value: number): number {
  return Math.min(100, Math.max(0, Math.round(value * 100) / 100));
}

export function buildDcaLadderPreview(request: DcaPreviewRequest): DcaLadderPreviewLevel[] {
  const config = request.dcaConfig ?? {};
  const selectedOutcomes = request.selectedOutcomes ?? [];
  const levels = Math.max(1, Math.min(20, Math.floor(toNumber(config.dcaLevels, 1))));
  const initialShares = Math.max(0, toNumber(config.initialOrderShares ?? config.firstDcaShares, 1));
  const spacingCent = Math.max(0, toNumber(config.dcaLevelSpacingCent, 0));
  const spacingMultiplier = Math.max(0, toNumber(config.dcaLevelSpacingMultiplier, 1));
  const sizeMultiplier = Math.max(0, toNumber(config.dcaOrderSizeMultiplier, 1));
  const maxPriceCent = clampCent(toNumber(config.dcaEntryMaxPriceCent ?? config.maxPriceCent, 100));
  const minPriceCent = clampCent(toNumber(config.dcaEntryMinPriceCent ?? config.minBuyPriceCent, 0));

  const preview: DcaLadderPreviewLevel[] = [];
  for (const outcome of selectedOutcomes) {
    let cumulativeSpacing = 0;
    for (let level = 0; level < levels; level += 1) {
      if (level > 0) {
        cumulativeSpacing += spacingCent * (spacingMultiplier ** (level - 1));
      }
      const priceCent = clampCent(Math.max(minPriceCent, maxPriceCent - cumulativeSpacing));
      const shares = Math.round((initialShares * (sizeMultiplier ** level)) * 10000) / 10000;
      preview.push({
        level,
        outcomeLabel: outcome.outcomeLabel,
        tokenId: outcome.tokenId,
        priceCent,
        shares,
        estimatedCostUsdc: Math.round((shares * priceCent / 100) * 10000) / 10000,
      });
    }
  }
  return preview;
}

export function buildDcaRiskPreview(request: DcaPreviewRequest) {
  const config = request.dcaConfig ?? {};
  const ladderPreview = buildDcaLadderPreview(request);
  const totalCost = ladderPreview.reduce((sum, item) => sum + item.estimatedCostUsdc, 0);
  const sideMode = String(config.sideMode ?? config.dcaSideMode ?? '').trim();
  const pairEligible = request.market?.pairEligible === true && sideMode === 'two_sided_pair';
  const targetPairCostCent = toNumber(config.targetPairCostCent, 97);
  const selectedOutcomes = request.selectedOutcomes ?? [];
  const basketWorstCaseLoss = sideMode === 'multi_outcome_basket' ? totalCost : null;
  return {
    totalCost: Math.round(totalCost * 10000) / 10000,
    maxLoss: Math.round(totalCost * 10000) / 10000,
    pairEligible,
    targetPairCost: pairEligible ? targetPairCostCent : null,
    lockedProfitDisabled: !pairEligible,
    basketExposureUsdc: sideMode === 'multi_outcome_basket' ? Math.round(totalCost * 10000) / 10000 : null,
    basketWorstCaseLossUsdc: basketWorstCaseLoss == null ? null : Math.round(basketWorstCaseLoss * 10000) / 10000,
    selectedOutcomeCount: selectedOutcomes.length,
  };
}
