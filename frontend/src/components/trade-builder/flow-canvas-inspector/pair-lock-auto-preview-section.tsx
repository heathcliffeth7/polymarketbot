import type { TradeBuilderOutcome } from '@/lib/types';
import {
  estimatePairLockAutoRemainingBudgetPreview,
  normalizePairLockSizingMode,
} from './pair-lock-inspector';

interface PairLockAutoPreviewSectionProps {
  visible: boolean;
  fields: Record<string, string>;
  marketOutcomes: TradeBuilderOutcome[];
  marketOutcomesLoading: boolean;
}

function formatUsdc(value: number): string {
  return `${value.toFixed(2)} USDC`;
}

function formatPrice(value: number): string {
  return `${(value * 100).toFixed(2)}c`;
}

function formatQty(value: number): string {
  return value.toFixed(4);
}

function resolveBlockedReason(reason: string | null): string {
  switch (reason) {
    case 'missing_outcomes':
      return 'Market outcome verisi henuz yuklenmedi.';
    case 'missing_primary_outcome':
      return 'Ana bacak outcome secimi ile eslesen bir outcome bulunamadi.';
    case 'missing_counter_outcome':
      return 'Karsi bacak outcome secimi ile eslesen bir outcome bulunamadi.';
    case 'missing_price':
      return 'Preview icin gerekli outcome fiyatlari hazir degil.';
    case 'invalid_budget':
      return 'Toplam butce, ilk bacak butcesinden buyuk olmali.';
    case 'above_max_total':
      return 'Mevcut fiyat ciftinde toplam fiyat `pairMaxTotalCent` tavanini asiyor.';
    default:
      return 'Preview hazir degil.';
  }
}

export function PairLockAutoPreviewSection({
  visible,
  fields,
  marketOutcomes,
  marketOutcomesLoading,
}: PairLockAutoPreviewSectionProps) {
  if (!visible || normalizePairLockSizingMode(fields.pairSizingMode ?? '') !== 'auto_remaining_budget') {
    return null;
  }

  const preview = estimatePairLockAutoRemainingBudgetPreview(fields, marketOutcomes);
  const showSkeleton = marketOutcomesLoading && marketOutcomes.length === 0;

  return (
    <div className="space-y-2 rounded-md border border-emerald-200 bg-emerald-50 px-2 py-2 text-[10px] leading-relaxed text-emerald-800">
      <p className="font-semibold">Auto Pair Preview</p>
      {showSkeleton ? (
        <p>Outcome fiyatlari yukleniyor...</p>
      ) : !preview || preview.blockedReason ? (
        <p className="text-amber-700">{resolveBlockedReason(preview?.blockedReason ?? null)}</p>
      ) : (
        <>
          <p>
            Ana fiyat <span className="font-semibold">{formatPrice(preview.primaryPrice)}</span> | karsi fiyat{' '}
            <span className="font-semibold">{formatPrice(preview.counterPrice)}</span>
          </p>
          <p>
            Ilk bacak <span className="font-semibold">{formatUsdc(preview.primaryBudgetUsdc)}</span> | kalan butce{' '}
            <span className="font-semibold">{formatUsdc(preview.remainingBudgetUsdc)}</span>
          </p>
          <p>
            Tahmini ortak qty <span className="font-semibold">{formatQty(preview.commonQty)}</span> | residue{' '}
            <span className="font-semibold">{formatQty(preview.residueQty)}</span>
          </p>
          <p>
            Tahmini net pair kar{' '}
            <span className="font-semibold">{formatUsdc(preview.projectedNetProfitUsdc)}</span>
          </p>
        </>
      )}
    </div>
  );
}
