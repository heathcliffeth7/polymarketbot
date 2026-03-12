import { decryptConfigValue, isEncryptedConfigValue } from '@/lib/crypto-config';

export type ClaimExecutionMode = 'direct' | 'builder_relayer';

export interface ClaimRuntimeValidationState {
  enabled: boolean;
  executionMode: ClaimExecutionMode;
  hasRpcSource: boolean;
  hasUserAddressSource: boolean;
  hasPrivateKeySource: boolean;
  hasSafeAddressSource: boolean;
  hasBuilderCredsSource: boolean;
}

export interface ClaimRelayerConfigForServer {
  executionMode: ClaimExecutionMode;
  chainId: number;
  ctfContractAddress: string;
  collateralTokenAddress: string;
  userAddress: string;
  privateKey: string;
  safeAddress: string;
  builderApiKey: string;
  builderApiSecret: string;
  builderApiPassphrase: string;
}

export function normalizeClaimExecutionMode(rawValue: unknown): ClaimExecutionMode {
  return String(rawValue ?? 'direct').trim().toLowerCase() === 'builder_relayer'
    ? 'builder_relayer'
    : 'direct';
}

export function resolvePlaintextConfigValueForServer(
  inlineValue: unknown,
  envNameValue: unknown
): string {
  const envName = String(envNameValue ?? '').trim();
  const fromEnv = envName ? String(process.env[envName] ?? '').trim() : '';
  if (fromEnv) {
    return fromEnv;
  }

  const inline = String(inlineValue ?? '').trim();
  if (!inline) {
    return '';
  }
  if (!isEncryptedConfigValue(inline)) {
    return inline;
  }
  try {
    return decryptConfigValue(inline).trim();
  } catch {
    return '';
  }
}

export function resolveSensitiveConfigValueForServer(
  inlineValue: unknown,
  envNameValue: unknown
): string {
  return resolvePlaintextConfigValueForServer(inlineValue, envNameValue);
}
