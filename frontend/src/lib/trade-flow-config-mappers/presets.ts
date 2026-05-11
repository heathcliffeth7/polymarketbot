import {
  PRESET_PLACE_ORDER_KINDS,
  QUICK_PRESET_BUY_SELL_KINDS,
  QUICK_PRESET_BUY_SELL_REF_KEYS,
  RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME,
  TRIGGER_MARKET_ONCE_SCOPE_VERSION,
} from './constants';
import { toStringValue, toTriggerMarketOnceScopeVersion } from './utils';

export function isPresetBuySellPlaceOrderMarker(presetKind: unknown, refKey: unknown): boolean {
  const kind = toStringValue(presetKind).trim().toLowerCase();
  if (QUICK_PRESET_BUY_SELL_KINDS.has(kind)) return true;
  const ref = toStringValue(refKey).trim().toLowerCase();
  return QUICK_PRESET_BUY_SELL_REF_KEYS.has(ref);
}

export function isPresetPlaceOrderMarker(presetKind: unknown, refKey: unknown): boolean {
  const kind = toStringValue(presetKind).trim().toLowerCase();
  if (PRESET_PLACE_ORDER_KINDS.has(kind)) return true;
  const ref = toStringValue(refKey).trim().toLowerCase();
  return ref.startsWith('preset_');
}

export function normalizeResolveMarketScope(scope: unknown): { asset: string; timeframe: string } | null {
  const key = toStringValue(scope).trim().toLowerCase();
  if (!key) return null;
  return RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[key] || null;
}

export function toResolveMarketScope(assetRaw: unknown, timeframeRaw: unknown): string | null {
  const asset = toStringValue(assetRaw).trim().toLowerCase();
  const timeframe = toStringValue(timeframeRaw).trim().toLowerCase();
  if (!asset || !timeframe) return null;
  const scope = `${asset}_${timeframe}_updown`;
  return RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[scope] ? scope : null;
}

export function resolveTriggerMarketOnceScope(
  cfg: Record<string, unknown>,
  marketMode: 'auto_scope' | 'fixed',
  repeatMode: 'once' | 'loop'
): 'run' | 'market' {
  const onceScopeRaw = toStringValue(cfg.onceScope).trim().toLowerCase();
  const onceScopeVersion = toTriggerMarketOnceScopeVersion(cfg.onceScopeVersion);

  if (
    marketMode === 'auto_scope' &&
    repeatMode === 'once' &&
    onceScopeVersion < TRIGGER_MARKET_ONCE_SCOPE_VERSION
  ) {
    return 'market';
  }

  return onceScopeRaw === 'market' ? 'market' : 'run';
}
