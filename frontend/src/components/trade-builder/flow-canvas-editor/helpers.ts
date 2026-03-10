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
