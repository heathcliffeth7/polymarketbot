import { readClaimRelayerConfigForServer, type UserConfigContext } from '@/lib/config';
import {
  ClaimRelayerRouteError,
  readClaimFundsActivationBalances,
  submitClaimFundsActivation,
} from '@/lib/claim-relayer';
import type {
  ClaimFundsActivationResult,
  ClaimFundsActivationStatus,
} from '@/lib/types';

export class ClaimFundsActivationServiceError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = 'ClaimFundsActivationServiceError';
    this.status = status;
    this.code = code;
  }
}

export function buildEmptyClaimFundsActivationStatus(
  overrides: Partial<ClaimFundsActivationStatus> = {}
): ClaimFundsActivationStatus {
  return {
    autoActivate: false,
    canActivate: false,
    minUsdc: 0.01,
    usdcEBalance: 0,
    pUsdBalance: 0,
    lastError: null,
    refreshedAt: null,
    ...overrides,
  };
}

export async function getClaimFundsActivationStatus(
  context: UserConfigContext,
  lastError: string | null
): Promise<ClaimFundsActivationStatus> {
  const config = await readClaimRelayerConfigForServer(context);
  const supported =
    config.executionMode === 'builder_relayer' ||
    config.executionMode === 'relayer_api_key';
  const base = {
    autoActivate: config.autoActivateFunds,
    minUsdc: config.activateMinUsdc,
    lastError,
    refreshedAt: new Date().toISOString(),
  };

  if (!supported || !config.safeAddress) {
    return buildEmptyClaimFundsActivationStatus(base);
  }

  try {
    const balances = await readClaimFundsActivationBalances(config);
    return buildEmptyClaimFundsActivationStatus({
      ...base,
      canActivate: balances.usdcEBalance >= config.activateMinUsdc,
      usdcEBalance: balances.usdcEBalance,
      pUsdBalance: balances.pUsdBalance,
    });
  } catch (err) {
    const message =
      err instanceof Error ? err.message : 'Failed to load funds activation status';
    return buildEmptyClaimFundsActivationStatus({
      ...base,
      lastError: lastError ?? message,
    });
  }
}

export async function activateClaimFunds(
  context: UserConfigContext
): Promise<ClaimFundsActivationResult> {
  const config = await readClaimRelayerConfigForServer(context);
  try {
    return await submitClaimFundsActivation(config, {
      userId: context.userId,
      ownerAddress: config.safeAddress,
    });
  } catch (err) {
    if (err instanceof ClaimRelayerRouteError) {
      throw new ClaimFundsActivationServiceError(err.status, err.code, err.message);
    }
    const message =
      err instanceof Error ? err.message : 'Failed to activate claim funds';
    throw new ClaimFundsActivationServiceError(
      500,
      'claim_funds_activation_failed',
      message
    );
  }
}
