import { decryptConfigValue, isEncryptedConfigValue } from '@/lib/crypto-config';

export type ClaimExecutionMode = 'direct' | 'builder_relayer' | 'relayer_api_key';

export const DEFAULT_USDCE_TOKEN_ADDRESS = '0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174';
export const DEFAULT_PUSD_TOKEN_ADDRESS = '0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB';
export const DEFAULT_COLLATERAL_ONRAMP_ADDRESS = '0x93070a847efEf7F70739046A929D47a521F5B8ee';

export interface ClaimRuntimeValidationState {
  enabled: boolean;
  executionMode: ClaimExecutionMode;
  hasRpcSource: boolean;
  hasUserAddressSource: boolean;
  hasPrivateKeySource: boolean;
  hasSafeAddressSource: boolean;
  hasBuilderCredsSource: boolean;
  hasRelayerApiKeySource: boolean;
}

export interface ClaimRelayerConfigForServer {
  executionMode: ClaimExecutionMode;
  chainId: number;
  rpcUrl: string;
  ctfContractAddress: string;
  collateralTokenAddress: string;
  autoActivateFunds: boolean;
  activateMinUsdc: number;
  usdceTokenAddress: string;
  pusdTokenAddress: string;
  collateralOnrampAddress: string;
  userAddress: string;
  privateKey: string;
  safeAddress: string;
  builderApiKey: string;
  builderApiSecret: string;
  builderApiPassphrase: string;
  relayerApiKey: string;
  relayerApiKeyAddress: string;
}

export function normalizeClaimExecutionMode(rawValue: unknown): ClaimExecutionMode {
  const normalized = String(rawValue ?? 'direct').trim().toLowerCase();
  if (normalized === 'builder_relayer') return 'builder_relayer';
  if (normalized === 'relayer_api_key') return 'relayer_api_key';
  return 'direct';
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
