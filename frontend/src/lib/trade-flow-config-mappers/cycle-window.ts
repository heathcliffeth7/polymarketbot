import type { TradeFlowGraph } from '@/lib/types';
import { isRecord, toBooleanValue, toStringValue } from './utils';

export interface TriggerMarketPriceCustomRangeSnapshot {
  nodeKey: string;
  startSec: number;
  endSec: number;
  autoSellOnWindowEnd: boolean;
}

export interface TriggerMarketPriceCustomRangeDiff {
  nodeKey: string;
  before: TriggerMarketPriceCustomRangeSnapshot | null;
  after: TriggerMarketPriceCustomRangeSnapshot | null;
}

export interface TriggerMarketPriceCycleWindowFields {
  cycleWindowMode: string;
  cycleWindowSecs: string;
  cycleWindowStartSec: string;
  cycleWindowEndSec: string;
  autoSellOnWindowEnd?: string;
}

function toInteger(value: unknown): number | undefined {
  const raw = Number(toStringValue(value).trim());
  return Number.isInteger(raw) ? raw : undefined;
}

export function readTriggerMarketPriceCycleWindowFields(
  config: Record<string, unknown>
): TriggerMarketPriceCycleWindowFields {
  const cycleWindowModeRaw = toStringValue(config.cycleWindowMode).trim().toLowerCase();
  const cycleWindowMode =
    cycleWindowModeRaw === 'first' ||
    cycleWindowModeRaw === 'last' ||
    cycleWindowModeRaw === 'custom_range'
      ? cycleWindowModeRaw
      : 'off';

  const fields: TriggerMarketPriceCycleWindowFields = {
    cycleWindowMode,
    cycleWindowSecs: toStringValue(config.cycleWindowSecs),
    cycleWindowStartSec: toStringValue(config.cycleWindowStartSec),
    cycleWindowEndSec: toStringValue(config.cycleWindowEndSec),
  };
  if (cycleWindowMode === 'custom_range') {
    fields.autoSellOnWindowEnd = config.autoSellOnWindowEnd === true ? 'true' : 'false';
  }
  return fields;
}

export function normalizeTriggerMarketPriceCycleWindowConfig(
  config: Record<string, unknown>
): Record<string, unknown> {
  const nextConfig = { ...config };
  const cycleWindowModeRaw = toStringValue(nextConfig.cycleWindowMode).trim().toLowerCase();
  if (cycleWindowModeRaw === 'first' || cycleWindowModeRaw === 'last') {
    const cycleWindowSecs = toInteger(nextConfig.cycleWindowSecs);
    if (cycleWindowSecs != null && cycleWindowSecs > 0) {
      nextConfig.cycleWindowMode = cycleWindowModeRaw;
      nextConfig.cycleWindowSecs = cycleWindowSecs;
    } else {
      delete nextConfig.cycleWindowMode;
      delete nextConfig.cycleWindowSecs;
    }
    delete nextConfig.cycleWindowStartSec;
    delete nextConfig.cycleWindowEndSec;
    delete nextConfig.autoSellOnWindowEnd;
    return nextConfig;
  }

  if (cycleWindowModeRaw === 'custom_range') {
    const startSec = toInteger(nextConfig.cycleWindowStartSec);
    const endSec = toInteger(nextConfig.cycleWindowEndSec);
    if (
      startSec != null &&
      startSec >= 0 &&
      endSec != null &&
      endSec > startSec
    ) {
      nextConfig.cycleWindowMode = 'custom_range';
      nextConfig.cycleWindowStartSec = startSec;
      nextConfig.cycleWindowEndSec = endSec;
    } else {
      delete nextConfig.cycleWindowMode;
      delete nextConfig.cycleWindowStartSec;
      delete nextConfig.cycleWindowEndSec;
    }
    delete nextConfig.cycleWindowSecs;
    if (nextConfig.autoSellOnWindowEnd !== true) {
      delete nextConfig.autoSellOnWindowEnd;
    }
    return nextConfig;
  }

  delete nextConfig.cycleWindowMode;
  delete nextConfig.cycleWindowSecs;
  delete nextConfig.cycleWindowStartSec;
  delete nextConfig.cycleWindowEndSec;
  delete nextConfig.autoSellOnWindowEnd;
  return nextConfig;
}

export function extractTriggerMarketPriceCustomRangeSnapshot(
  nodeKey: string,
  config: Record<string, unknown>
): TriggerMarketPriceCustomRangeSnapshot | null {
  if (toStringValue(config.cycleWindowMode).trim().toLowerCase() !== 'custom_range') {
    return null;
  }
  const startSec = toInteger(config.cycleWindowStartSec);
  const endSec = toInteger(config.cycleWindowEndSec);
  if (startSec == null || startSec < 0 || endSec == null || endSec <= startSec) {
    return null;
  }
  return {
    nodeKey,
    startSec,
    endSec,
    autoSellOnWindowEnd: toBooleanValue(config.autoSellOnWindowEnd),
  };
}

export function collectTriggerMarketPriceCustomRangeSnapshots(
  graphOrJson: TradeFlowGraph | unknown
): TriggerMarketPriceCustomRangeSnapshot[] {
  const nodes = isRecord(graphOrJson) && Array.isArray(graphOrJson.nodes)
    ? graphOrJson.nodes
    : [];
  return nodes
    .flatMap((node) => {
      if (!isRecord(node)) return [];
      const nodeKey = toStringValue(node.key).trim();
      const nodeType = toStringValue(node.type).trim();
      if (!nodeKey || nodeType !== 'trigger.market_price' || !isRecord(node.config)) {
        return [];
      }
      const snapshot = extractTriggerMarketPriceCustomRangeSnapshot(nodeKey, node.config);
      return snapshot ? [snapshot] : [];
    })
    .sort((left, right) => left.nodeKey.localeCompare(right.nodeKey));
}

export function diffTriggerMarketPriceCustomRangeSnapshots(
  before: TriggerMarketPriceCustomRangeSnapshot[],
  after: TriggerMarketPriceCustomRangeSnapshot[]
): TriggerMarketPriceCustomRangeDiff[] {
  const beforeByKey = new Map(before.map((snapshot) => [snapshot.nodeKey, snapshot]));
  const afterByKey = new Map(after.map((snapshot) => [snapshot.nodeKey, snapshot]));
  const keys = new Set([...beforeByKey.keys(), ...afterByKey.keys()]);

  return [...keys]
    .sort((left, right) => left.localeCompare(right))
    .flatMap((nodeKey) => {
      const left = beforeByKey.get(nodeKey) ?? null;
      const right = afterByKey.get(nodeKey) ?? null;
      if (
        left?.startSec === right?.startSec &&
        left?.endSec === right?.endSec &&
        left?.autoSellOnWindowEnd === right?.autoSellOnWindowEnd
      ) {
        return [];
      }
      return [{ nodeKey, before: left, after: right }];
    });
}
