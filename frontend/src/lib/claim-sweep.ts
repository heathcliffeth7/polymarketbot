import {
  readConfig,
  readClaimRelayerConfigForServer,
  readEffectiveClaimConfigForServer,
  readPositionWalletAddress,
  type UserConfigContext,
} from '@/lib/config';
import {
  getClaimSweepQueueStatus,
  getLatestClaimSweepError,
  queueClaimSweepJobs,
} from '@/lib/queries/claim-sweep';
import type { ClaimSweepQueueStatus, ClaimSweepRunResult, ClaimSweepStatus } from '@/lib/types';

const CLAIM_SWEEP_THRESHOLD_USDC = 0.01;
const CLAIM_SWEEP_PAGE_SIZE = 200;
const CLAIM_SWEEP_CACHE_TTL_MS = 30_000;
const ETH_ADDRESS_RE = /^0x[a-fA-F0-9]{40}$/;

interface DataApiPositionRow {
  proxyWallet?: string | null;
  conditionId?: string | null;
  marketSlug?: string | null;
  slug?: string | null;
  currentValue?: number | string | null;
  curPrice?: number | string | null;
  size?: number | string | null;
  balance?: number | string | null;
  redeemable?: boolean | null;
}

interface ClaimSweepDustEntry {
  ownerAddress: string;
  conditionId: string;
  marketSlug: string | null;
  currentValue: number;
}

interface ClaimSweepDustSnapshot {
  walletAddress: string;
  ownerAddresses: string[];
  eligibleEntries: ClaimSweepDustEntry[];
  eligibleCount: number;
  eligibleTotalUsdc: number;
  refreshedAt: string;
}

interface ClaimSweepPrerequisites {
  status: ClaimSweepStatus;
  ownerAddresses: string[];
  walletAddress: string;
}

type CachedDustSnapshot = {
  expiresAt: number;
  snapshot: ClaimSweepDustSnapshot;
};

const dustSnapshotCache = new Map<string, CachedDustSnapshot>();

export class ClaimSweepServiceError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = 'ClaimSweepServiceError';
    this.status = status;
    this.code = code;
  }
}

function emptyQueue(): ClaimSweepQueueStatus {
  return {
    pending: 0,
    retry: 0,
    processing: 0,
    submitted: 0,
    failed: 0,
    claimed: 0,
  };
}

export function buildEmptyClaimSweepStatus(
  overrides: Partial<ClaimSweepStatus> = {}
): ClaimSweepStatus {
  return {
    thresholdUsdc: CLAIM_SWEEP_THRESHOLD_USDC,
    walletAddress: null,
    executionMode: 'direct',
    claimEnabled: false,
    canSweep: false,
    disabledReasonCode: null,
    disabledReason: null,
    eligibleCount: 0,
    eligibleTotalUsdc: 0,
    queue: emptyQueue(),
    lastError: null,
    refreshedAt: null,
    ...overrides,
  };
}

export async function getClaimSweepStatus(
  context: UserConfigContext,
  options: { serviceActive: boolean }
): Promise<ClaimSweepStatus> {
  const claimState = await readEffectiveClaimConfigForServer(context);
  const relayerConfig = await readClaimRelayerConfigForServer(context);
  const walletAddress = normalizeAddress(await readPositionWalletAddress(context));
  const configuredOwnerAddresses = uniqueAddresses([
    walletAddress,
    normalizeAddress(relayerConfig.userAddress),
  ]);

  const queue = await getClaimSweepQueueStatus(configuredOwnerAddresses);
  const lastError = await getLatestClaimSweepError(configuredOwnerAddresses);

  let snapshot: ClaimSweepDustSnapshot | null = null;
  let positionsFetchFailed = false;

  if (walletAddress) {
    try {
      snapshot = await getDustSnapshot(context, walletAddress, false);
    } catch {
      positionsFetchFailed = true;
    }
  }

  const ownerAddresses = uniqueAddresses([
    ...configuredOwnerAddresses,
    ...(snapshot?.ownerAddresses ?? []),
  ]);

  let effectiveQueue = queue;
  let effectiveLastError = lastError;
  if (ownerAddresses.join('|') !== configuredOwnerAddresses.join('|')) {
    [effectiveQueue, effectiveLastError] = await Promise.all([
      getClaimSweepQueueStatus(ownerAddresses),
      getLatestClaimSweepError(ownerAddresses),
    ]);
  }

  const disabledReason = resolveDisabledReason({
    claimState,
    walletAddress,
    serviceActive: options.serviceActive,
    positionsFetchFailed,
    eligibleCount: snapshot?.eligibleCount ?? 0,
  });

  return buildEmptyClaimSweepStatus({
    walletAddress,
    executionMode: relayerConfig.executionMode,
    claimEnabled: claimState.enabled,
    canSweep: disabledReason == null,
    disabledReasonCode: disabledReason?.code ?? null,
    disabledReason: disabledReason?.message ?? null,
    eligibleCount: snapshot?.eligibleCount ?? 0,
    eligibleTotalUsdc: snapshot?.eligibleTotalUsdc ?? 0,
    queue: effectiveQueue,
    lastError: effectiveLastError,
    refreshedAt: snapshot?.refreshedAt ?? null,
  });
}

export async function runClaimSweep(
  context: UserConfigContext,
  options: { serviceActive: boolean }
): Promise<ClaimSweepRunResult> {
  const { status, ownerAddresses, walletAddress } = await loadClaimSweepPrerequisites(
    context,
    options
  );

  if (!status.canSweep) {
    if (status.disabledReasonCode === 'no_eligible_claims') {
      return {
        thresholdUsdc: status.thresholdUsdc,
        walletAddress: status.walletAddress,
        eligibleCount: 0,
        eligibleTotalUsdc: 0,
        queuedNewCount: 0,
        rearmedCount: 0,
        alreadyTrackedCount: 0,
        queue: status.queue,
        refreshedAt: status.refreshedAt ?? new Date().toISOString(),
      };
    }
    throw new ClaimSweepServiceError(
      status.disabledReasonCode === 'bot_inactive' ? 409 : 400,
      status.disabledReasonCode ?? 'claim_sweep_unavailable',
      status.disabledReason ?? 'Claim sweep is unavailable'
    );
  }

  const maxAttempts = await getConfiguredMaxAttempts(context);
  const snapshot = await getDustSnapshot(context, walletAddress, true);
  if (snapshot.eligibleEntries.length === 0) {
    const queue = await getClaimSweepQueueStatus(ownerAddresses);
    return {
      thresholdUsdc: CLAIM_SWEEP_THRESHOLD_USDC,
      walletAddress,
      eligibleCount: 0,
      eligibleTotalUsdc: 0,
      queuedNewCount: 0,
      rearmedCount: 0,
      alreadyTrackedCount: 0,
      queue,
      refreshedAt: snapshot.refreshedAt,
    };
  }

  const queueResult = await queueClaimSweepJobs(snapshot.eligibleEntries, maxAttempts);
  const queue = await getClaimSweepQueueStatus(
    uniqueAddresses([...ownerAddresses, ...snapshot.ownerAddresses])
  );

  return {
    thresholdUsdc: CLAIM_SWEEP_THRESHOLD_USDC,
    walletAddress,
    eligibleCount: snapshot.eligibleCount,
    eligibleTotalUsdc: snapshot.eligibleTotalUsdc,
    queuedNewCount: queueResult.queuedNewCount,
    rearmedCount: queueResult.rearmedCount,
    alreadyTrackedCount: queueResult.alreadyTrackedCount,
    queue,
    refreshedAt: snapshot.refreshedAt,
  };
}

async function loadClaimSweepPrerequisites(
  context: UserConfigContext,
  options: { serviceActive: boolean }
): Promise<ClaimSweepPrerequisites> {
  const status = await getClaimSweepStatus(context, options);
  const relayerConfig = await readClaimRelayerConfigForServer(context);
  const ownerAddresses = uniqueAddresses([
    normalizeAddress(status.walletAddress),
    normalizeAddress(relayerConfig.userAddress),
  ]);

  return {
    status,
    ownerAddresses,
    walletAddress: status.walletAddress ?? '',
  };
}

async function getDustSnapshot(
  context: UserConfigContext,
  walletAddress: string,
  forceRefresh: boolean
): Promise<ClaimSweepDustSnapshot> {
  const cacheKey = `${context.userId}:${walletAddress}:${CLAIM_SWEEP_THRESHOLD_USDC}`;
  const cached = dustSnapshotCache.get(cacheKey);
  const now = Date.now();

  if (!forceRefresh && cached && cached.expiresAt > now) {
    return cached.snapshot;
  }

  const url = new URL('https://data-api.polymarket.com/positions');
  const byCondition = new Map<string, ClaimSweepDustEntry>();

  for (let offset = 0; ; offset += CLAIM_SWEEP_PAGE_SIZE) {
    url.search = '';
    url.searchParams.set('user', walletAddress);
    url.searchParams.set('redeemable', 'true');
    url.searchParams.set('sizeThreshold', '0');
    url.searchParams.set('limit', String(CLAIM_SWEEP_PAGE_SIZE));
    url.searchParams.set('offset', String(offset));

    const res = await fetch(url.toString(), { cache: 'no-store' });
    if (!res.ok) {
      throw new Error(`positions endpoint returned HTTP ${res.status}`);
    }

    const rows = (await res.json()) as DataApiPositionRow[];
    if (!Array.isArray(rows) || rows.length === 0) {
      break;
    }

    for (const row of rows) {
      if (row.redeemable === false) {
        continue;
      }
      const ownerAddress = normalizeAddress(row.proxyWallet || walletAddress);
      const conditionId = normalizeConditionId(row.conditionId);
      const currentValue = resolveCurrentValue(row);
      if (!ownerAddress || !conditionId || currentValue <= 0) {
        continue;
      }

      const key = `${ownerAddress}:${conditionId}`;
      const existing = byCondition.get(key);
      if (existing) {
        existing.currentValue += currentValue;
        if (!existing.marketSlug) {
          existing.marketSlug = normalizeMarketSlug(row.marketSlug || row.slug);
        }
        continue;
      }

      byCondition.set(key, {
        ownerAddress,
        conditionId,
        marketSlug: normalizeMarketSlug(row.marketSlug || row.slug),
        currentValue,
      });
    }

    if (rows.length < CLAIM_SWEEP_PAGE_SIZE) {
      break;
    }
  }

  const eligibleEntries = Array.from(byCondition.values())
    .filter((entry) => entry.currentValue >= CLAIM_SWEEP_THRESHOLD_USDC)
    .sort((left, right) => right.currentValue - left.currentValue);
  const snapshot: ClaimSweepDustSnapshot = {
    walletAddress,
    ownerAddresses: uniqueAddresses(eligibleEntries.map((entry) => entry.ownerAddress)),
    eligibleEntries,
    eligibleCount: eligibleEntries.length,
    eligibleTotalUsdc: roundUsdc(
      eligibleEntries.reduce((sum, entry) => sum + entry.currentValue, 0)
    ),
    refreshedAt: new Date().toISOString(),
  };

  dustSnapshotCache.set(cacheKey, {
    expiresAt: now + CLAIM_SWEEP_CACHE_TTL_MS,
    snapshot,
  });

  return snapshot;
}

function resolveDisabledReason(params: {
  claimState: Awaited<ReturnType<typeof readEffectiveClaimConfigForServer>>;
  walletAddress: string | null;
  serviceActive: boolean;
  positionsFetchFailed: boolean;
  eligibleCount: number;
}): { code: string; message: string } | null {
  const { claimState, walletAddress, serviceActive, positionsFetchFailed, eligibleCount } = params;

  if (!claimState.enabled) {
    return {
      code: 'claim_disabled',
      message: 'Claim config devre disi. Settings -> Claim altindan auto-claim etkinlestirilmeli.',
    };
  }
  if (!claimState.hasRpcSource || !claimState.hasUserAddressSource || !claimState.hasPrivateKeySource) {
    return {
      code: 'claim_incomplete',
      message: 'Claim config eksik. RPC, user address ve private key kaynaklari tamamlanmali.',
    };
  }
  if (
    claimState.executionMode === 'builder_relayer' &&
    (!claimState.hasSafeAddressSource || !claimState.hasBuilderCredsSource)
  ) {
    return {
      code: 'claim_incomplete',
      message: 'Builder relayer claim icin Safe adresi ve builder credential alanlari tamamlanmali.',
    };
  }
  if (
    claimState.executionMode === 'relayer_api_key' &&
    (!claimState.hasSafeAddressSource || !claimState.hasRelayerApiKeySource)
  ) {
    return {
      code: 'claim_incomplete',
      message: 'Relayer API Key claim icin Safe adresi ve relayer API key alanlari tamamlanmali.',
    };
  }
  if (!walletAddress) {
    return {
      code: 'no_wallet',
      message: 'Claim sweep icin position wallet adresi bulunamadi.',
    };
  }
  if (!serviceActive) {
    return {
      code: 'bot_inactive',
      message: 'Bot aktif degil. Dust queue isleyebilmesi icin dextrabot calismali.',
    };
  }
  if (positionsFetchFailed) {
    return {
      code: 'positions_fetch_failed',
      message: 'Redeemable positions yuklenemedi. Biraz sonra tekrar dene.',
    };
  }
  if (eligibleCount === 0) {
    return {
      code: 'no_eligible_claims',
      message: `>= ${CLAIM_SWEEP_THRESHOLD_USDC.toFixed(2)} USDC claimable dust bulunmuyor.`,
    };
  }
  return null;
}

function normalizeAddress(rawValue: string | null | undefined): string | null {
  const value = String(rawValue ?? '').trim().toLowerCase();
  if (!ETH_ADDRESS_RE.test(value)) {
    return null;
  }
  return value;
}

function normalizeConditionId(rawValue: string | null | undefined): string | null {
  const value = String(rawValue ?? '').trim().toLowerCase();
  if (!/^0x[a-f0-9]{64}$/.test(value)) {
    return null;
  }
  return value;
}

function normalizeMarketSlug(rawValue: string | null | undefined): string | null {
  const value = String(rawValue ?? '').trim();
  return value || null;
}

function resolveCurrentValue(row: DataApiPositionRow): number {
  const currentValue = parseNumber(row.currentValue);
  if (currentValue > 0) {
    return currentValue;
  }
  const price = parseNumber(row.curPrice);
  const size = parseNumber(row.size) || parseNumber(row.balance);
  if (price > 0 && size > 0) {
    return price * size;
  }
  return 0;
}

function parseNumber(rawValue: unknown): number {
  if (typeof rawValue === 'number') {
    return Number.isFinite(rawValue) ? rawValue : 0;
  }
  if (typeof rawValue === 'string') {
    const parsed = Number.parseFloat(rawValue);
    return Number.isFinite(parsed) ? parsed : 0;
  }
  return 0;
}

function roundUsdc(value: number): number {
  return Number(value.toFixed(6));
}

function uniqueAddresses(values: Array<string | null>): string[] {
  return Array.from(new Set(values.filter((value): value is string => !!value)));
}

async function getConfiguredMaxAttempts(context: UserConfigContext): Promise<number> {
  const raw: Record<string, unknown> = await readConfig('claim', context).catch(
    () => ({}) as Record<string, unknown>
  );
  const value = Number(raw['max_attempts']);
  if (Number.isFinite(value) && value > 0) {
    return Math.trunc(value);
  }
  return 5;
}
