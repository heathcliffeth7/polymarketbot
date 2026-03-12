import { BuilderConfig } from '@polymarket/builder-signing-sdk';
import { RelayClient, RelayerTxType } from '@polymarket/builder-relayer-client';
import type { ClaimRelayerConfigForServer } from '@/lib/claim-relayer-config';
import {
  createWalletClient,
  encodeAbiParameters,
  encodeFunctionData,
  getCreate2Address,
  http,
  isAddress,
  keccak256,
  type Address,
  type Hex,
  zeroHash,
} from 'viem';
import { privateKeyToAccount } from 'viem/accounts';
import { polygon, polygonAmoy } from 'viem/chains';

const DEFAULT_RELAYER_URL = 'https://relayer-v2.polymarket.com';
const SAFE_FACTORY_BY_CHAIN: Record<number, Address> = {
  137: '0xaacFeEa03eb1561C4e67d661e40682Bd20E3541b',
  80002: '0xaacFeEa03eb1561C4e67d661e40682Bd20E3541b',
};
const SAFE_INIT_CODE_HASH =
  '0x2bce2127ff07fb632d16c8347c4ebf501f4841168bed00d9e6ef715ddb6fcecf';
const CTF_REDEEM_ABI = [
  {
    type: 'function',
    name: 'redeemPositions',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'collateralToken', type: 'address' },
      { name: 'parentCollectionId', type: 'bytes32' },
      { name: 'conditionId', type: 'bytes32' },
      { name: 'indexSets', type: 'uint256[]' },
    ],
    outputs: [],
  },
] as const;

export interface ClaimRedeemRequestBody {
  userId: number;
  ownerAddress: string;
  conditionId: string;
  collateralToken: string;
  indexSets: number[];
}

export interface ClaimRedeemSuccess {
  txHash: string;
}

export class ClaimRelayerRouteError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string,
    public readonly retryable: boolean,
    message: string
  ) {
    super(message);
    this.name = 'ClaimRelayerRouteError';
  }
}

export async function submitClaimViaBuilderRelayer(
  config: ClaimRelayerConfigForServer,
  body: ClaimRedeemRequestBody
): Promise<ClaimRedeemSuccess> {
  validateConfig(config);
  validateRequest(body);

  if (config.executionMode !== 'builder_relayer') {
    throw new ClaimRelayerRouteError(
      400,
      'claim_execution_mode_invalid',
      false,
      `claim.execution_mode must be builder_relayer, got ${config.executionMode}`
    );
  }

  const account = privateKeyToAccount(config.privateKey as Hex);
  if (!sameAddress(account.address, config.userAddress)) {
    throw new ClaimRelayerRouteError(
      400,
      'claim_signer_mismatch',
      false,
      'claim.private_key does not match claim.user_address'
    );
  }

  const expectedSafe = deriveSafe(account.address, config.chainId);
  if (!sameAddress(expectedSafe, config.safeAddress)) {
    throw new ClaimRelayerRouteError(
      400,
      'configured_safe_mismatch',
      false,
      `exchange.gnosis_safe_address does not match derived Safe for claim.user_address (${expectedSafe})`
    );
  }

  if (!sameAddress(body.ownerAddress, config.safeAddress)) {
    throw new ClaimRelayerRouteError(
      400,
      'owner_address_mismatch',
      false,
      'request ownerAddress does not match configured exchange.gnosis_safe_address'
    );
  }

  if (!sameAddress(body.collateralToken, config.collateralTokenAddress)) {
    throw new ClaimRelayerRouteError(
      400,
      'collateral_token_mismatch',
      false,
      'request collateralToken does not match configured claim.collateral_token_address'
    );
  }

  const walletClient = createWalletClient({
    account,
    chain: resolveChain(config.chainId),
    transport: http(),
  });
  const builderConfig = new BuilderConfig({
    localBuilderCreds: {
      key: config.builderApiKey,
      secret: config.builderApiSecret,
      passphrase: config.builderApiPassphrase,
    },
  }) as unknown as ConstructorParameters<typeof RelayClient>[3];
  const client = new RelayClient(
    process.env.POLYMARKET_RELAYER_URL?.trim() || DEFAULT_RELAYER_URL,
    config.chainId,
    walletClient,
    builderConfig,
    RelayerTxType.SAFE
  );
  const transaction = {
    to: config.ctfContractAddress,
    data: encodeFunctionData({
      abi: CTF_REDEEM_ABI,
      functionName: 'redeemPositions',
      args: [
        config.collateralTokenAddress as Address,
        zeroHash,
        body.conditionId as Hex,
        body.indexSets.map((value) => BigInt(value)),
      ],
    }),
    value: '0',
  };

  try {
    const response = await client.execute([transaction], 'auto-claim redeem positions');
    const waited = response.transactionHash ? undefined : await response.wait();
    const txHash = (response.transactionHash || waited?.transactionHash || '').trim();
    if (!txHash) {
      throw new ClaimRelayerRouteError(
        502,
        'empty_relayer_tx_hash',
        true,
        'relayer returned an empty transaction hash'
      );
    }
    return { txHash };
  } catch (err) {
    throw classifyRelayerError(err);
  }
}

function validateConfig(config: ClaimRelayerConfigForServer): void {
  if (!isAddress(config.userAddress)) {
    throw new ClaimRelayerRouteError(400, 'invalid_claim_user_address', false, 'claim.user_address must be a valid 0x address');
  }
  if (!/^0x[a-fA-F0-9]{64}$/.test(config.privateKey)) {
    throw new ClaimRelayerRouteError(400, 'invalid_claim_private_key', false, 'claim.private_key must be a valid 0x private key');
  }
  if (!isAddress(config.safeAddress)) {
    throw new ClaimRelayerRouteError(400, 'invalid_safe_address', false, 'exchange.gnosis_safe_address must be a valid 0x address');
  }
  if (!isAddress(config.ctfContractAddress)) {
    throw new ClaimRelayerRouteError(400, 'invalid_ctf_contract_address', false, 'claim.ctf_contract_address must be a valid 0x address');
  }
  if (!isAddress(config.collateralTokenAddress)) {
    throw new ClaimRelayerRouteError(400, 'invalid_collateral_token_address', false, 'claim.collateral_token_address must be a valid 0x address');
  }
  if (!config.builderApiKey.trim() || !config.builderApiSecret.trim() || !config.builderApiPassphrase.trim()) {
    throw new ClaimRelayerRouteError(400, 'missing_builder_credentials', false, 'builder relayer credentials are incomplete');
  }
  if (!SAFE_FACTORY_BY_CHAIN[config.chainId]) {
    throw new ClaimRelayerRouteError(400, 'unsupported_chain', false, `unsupported chain id ${config.chainId} for builder relayer`);
  }
}

function validateRequest(body: ClaimRedeemRequestBody): void {
  if (!Number.isFinite(body.userId) || body.userId <= 0) {
    throw new ClaimRelayerRouteError(400, 'invalid_user_id', false, 'userId must be a positive integer');
  }
  if (!isAddress(body.ownerAddress)) {
    throw new ClaimRelayerRouteError(400, 'invalid_owner_address', false, 'ownerAddress must be a valid 0x address');
  }
  if (!isAddress(body.collateralToken)) {
    throw new ClaimRelayerRouteError(400, 'invalid_collateral_token', false, 'collateralToken must be a valid 0x address');
  }
  if (!/^0x[a-fA-F0-9]{64}$/.test(body.conditionId)) {
    throw new ClaimRelayerRouteError(400, 'invalid_condition_id', false, 'conditionId must be a 32-byte 0x hash');
  }
  if (!Array.isArray(body.indexSets) || body.indexSets.length === 0 || body.indexSets.some((value) => !Number.isInteger(value) || value <= 0)) {
    throw new ClaimRelayerRouteError(400, 'invalid_index_sets', false, 'indexSets must be a non-empty array of positive integers');
  }
}

function deriveSafe(ownerAddress: Address, chainId: number): Address {
  const safeFactory = SAFE_FACTORY_BY_CHAIN[chainId];
  return getCreate2Address({
    bytecodeHash: SAFE_INIT_CODE_HASH,
    from: safeFactory,
    salt: keccak256(
      encodeAbiParameters([{ name: 'address', type: 'address' }], [ownerAddress])
    ),
  });
}

function resolveChain(chainId: number) {
  if (chainId === 137) return polygon;
  if (chainId === 80002) return polygonAmoy;
  throw new ClaimRelayerRouteError(400, 'unsupported_chain', false, `unsupported chain id ${chainId}`);
}

function sameAddress(left: string, right: string): boolean {
  return left.trim().toLowerCase() === right.trim().toLowerCase();
}

function classifyRelayerError(err: unknown): ClaimRelayerRouteError {
  if (err instanceof ClaimRelayerRouteError) {
    return err;
  }

  const message = err instanceof Error ? err.message : String(err);
  const maybeAxiosError = err as {
    response?: { status?: number; data?: unknown };
    code?: string;
  };
  const responseStatus = maybeAxiosError.response?.status;
  if (responseStatus) {
    const retryable = responseStatus === 429 || responseStatus >= 500;
    const responseText =
      typeof maybeAxiosError.response?.data === 'string'
        ? maybeAxiosError.response.data
        : JSON.stringify(maybeAxiosError.response?.data ?? {});
    return new ClaimRelayerRouteError(
      retryable ? 502 : 400,
      `relayer_http_${responseStatus}`,
      retryable,
      responseText || message
    );
  }

  if (message.toLowerCase().includes('safe not deployed')) {
    return new ClaimRelayerRouteError(409, 'safe_not_deployed', false, message);
  }
  if (message.toLowerCase().includes('network error') || message.toLowerCase().includes('timeout')) {
    return new ClaimRelayerRouteError(502, 'relayer_network_error', true, message);
  }
  if (message.toLowerCase().includes('unsupported chain') || message.toLowerCase().includes('invalid network')) {
    return new ClaimRelayerRouteError(400, 'unsupported_chain', false, message);
  }
  if (message.includes('"status":429') || message.toLowerCase().includes('too many requests') || message.toLowerCase().includes('quota exceeded') || message.toLowerCase().includes('rate limit')) {
    return new ClaimRelayerRouteError(502, 'relayer_rate_limited', true, message);
  }

  return new ClaimRelayerRouteError(400, 'relayer_request_failed', false, message);
}
