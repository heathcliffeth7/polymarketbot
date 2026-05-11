import type { UpstreamMaxPriceResolution } from '../flow-canvas-utils';

export function normalizePresetPlaceOrderConfig(config: Record<string, unknown>): void {
  config.kind = 'immediate';
  delete config.triggerCondition;
  delete config.triggerPrice;
  delete config.triggerPriceCent;
  if (typeof config.presetKind === 'string' && typeof config.refKey === 'string') {
    const presetKind = config.presetKind.trim().toLowerCase();
    const refKey = config.refKey.trim().toLowerCase();
    if (
      presetKind === 'sell_current_position' ||
      presetKind === 'buy_current_position' ||
      refKey === 'preset_sell_current_position' ||
      refKey === 'preset_buy_current_position'
    ) {
      config.executionMode = 'market';
    }
  }
}

function hasValidConfiguredMaxPrice(config: Record<string, unknown>): boolean {
  const maxPriceCent = Number(config.maxPriceCent);
  if (Number.isFinite(maxPriceCent) && maxPriceCent > 0 && maxPriceCent <= 100) {
    return true;
  }

  const maxPrice = Number(config.maxPrice);
  return Number.isFinite(maxPrice) && maxPrice > 0 && maxPrice <= 1;
}

export function applyInheritedPlaceOrderMaxPriceConfig(
  nextType: string,
  parsedConfig: Record<string, unknown>,
  resolution: UpstreamMaxPriceResolution
): void {
  if (nextType !== 'action.place_order') return;
  if (resolution.kind !== 'single' || resolution.maxPriceCent == null) return;
  if (hasValidConfiguredMaxPrice(parsedConfig)) return;
  parsedConfig.maxPriceCent = Number(resolution.maxPriceCent);
}
