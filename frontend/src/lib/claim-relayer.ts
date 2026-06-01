import { BuilderConfig } from '@polymarket/builder-signing-sdk';
import {
  createSafeMultisendTransaction,
  OperationType,
  RelayClient,
  RelayerTxType,
  type SafeTransaction,
  type Transaction,
} from '@polymarket/builder-relayer-client';
import {
  DEFAULT_COLLATERAL_ONRAMP_ADDRESS,
  DEFAULT_PUSD_TOKEN_ADDRESS,
  DEFAULT_USDCE_TOKEN_ADDRESS,
  type ClaimRelayerConfigForServer,
} from '@/lib/claim-relayer-config';
import type { ClaimFundsActivationResult } from '@/lib/types';
import {
  createWalletClient,
  createPublicClient,
  encodeAbiParameters,
  encodeFunctionData,
  encodePacked,
  formatUnits,
  getCreate2Address,
  hashTypedData,
  hexToBigInt,
  http,
  isAddress,
  keccak256,
  toBytes,
  type Address,
  type Hex,
  zeroAddress,
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
const SAFE_MULTISEND_BY_CHAIN: Record<number, Address> = {
  137: '0xA238CBeb142c10Ef7Ad8442C6D1f9E89e07e7761',
  80002: '0xA238CBeb142c10Ef7Ad8442C6D1f9E89e07e7761',
};
const CTF_ABI = [
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
  {
    type: 'function',
    name: 'mergePositions',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'collateralToken', type: 'address' },
      { name: 'parentCollectionId', type: 'bytes32' },
      { name: 'conditionId', type: 'bytes32' },
      { name: 'partition', type: 'uint256[]' },
      { name: 'amount', type: 'uint256' },
    ],
    outputs: [],
  },
] as const;
const ERC20_ABI = [
  {
    type: 'function',
    name: 'balanceOf',
    stateMutability: 'view',
    inputs: [{ name: 'account', type: 'address' }],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'allowance',
    stateMutability: 'view',
    inputs: [
      { name: 'owner', type: 'address' },
      { name: 'spender', type: 'address' },
    ],
    outputs: [{ name: '', type: 'uint256' }],
  },
  {
    type: 'function',
    name: 'approve',
    stateMutability: 'nonpayable',
    inputs: [
      { name: 'spender', type: 'address' },
      { name: 'amount', type: 'uint256' },
    ],
    outputs: [{ name: '', type: 'bool' }],
  },
] as const;
const COLLATERAL_ONRAMP_ABI = [
  {
    type: 'function',
    name: 'wrap',
    stateMutability: 'nonpayable',
    inputs: [
      { name: '_asset', type: 'address' },
      { name: '_to', type: 'address' },
      { name: '_amount', type: 'uint256' },
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

export interface ClaimMergeRequestBody {
  userId: number;
  ownerAddress: string;
  conditionId: string;
  collateralToken: string;
  partition: number[];
  amountRaw: string;
}

export interface ClaimRedeemSuccess {
  txHash: string;
}

export interface ClaimFundsActivationRequestBody {
  userId: number;
  ownerAddress: string;
}

export interface ClaimFundsActivationBalances {
  usdcEBalanceRaw: bigint;
  pUsdBalanceRaw: bigint;
  allowanceRaw: bigint;
  usdcEBalance: number;
  pUsdBalance: number;
}

export interface ClaimFundsActivationTransactionPlan {
  status: 'skipped' | 'ready';
  amountRaw: bigint;
  needsApproval: boolean;
  transactions: Transaction[];
  message: string;
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

  const transaction = {
    to: config.ctfContractAddress,
    data: encodeFunctionData({
      abi: CTF_ABI,
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
    const txHash = await submitSafeTransactionsViaBuilderRelayer(
      config,
      [transaction],
      'auto-claim redeem positions'
    );
    return { txHash };
  } catch (err) {
    throw classifyRelayerError(err);
  }
}

const SAFE_TX_TYPES = {
  SafeTx: [
    { name: 'to', type: 'address' },
    { name: 'value', type: 'uint256' },
    { name: 'data', type: 'bytes' },
    { name: 'operation', type: 'uint8' },
    { name: 'safeTxGas', type: 'uint256' },
    { name: 'baseGas', type: 'uint256' },
    { name: 'gasPrice', type: 'uint256' },
    { name: 'gasToken', type: 'address' },
    { name: 'refundReceiver', type: 'address' },
    { name: 'nonce', type: 'uint256' },
  ],
} as const;

function splitAndPackSafeSig(sig: Hex): Hex {
  const raw = sig.slice(2);
  const r = hexToBigInt(('0x' + raw.slice(0, 64)) as Hex);
  const s = hexToBigInt(('0x' + raw.slice(64, 128)) as Hex);
  let v = parseInt(raw.slice(128, 130), 16);
  if (v === 0 || v === 1) v += 31;
  else if (v === 27 || v === 28) v += 4;
  else throw new Error(`Invalid signature v value: ${v}`);
  return encodePacked(['uint256', 'uint256', 'uint8'], [r, s, v]);
}

async function fetchSafeNonce(relayerUrl: string, signerAddress: string): Promise<string> {
  const url = `${relayerUrl}/nonce?address=${signerAddress}&type=SAFE`;
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Failed to fetch Safe nonce: ${res.status} ${res.statusText}`);
  const data = await res.json();
  return String(data.nonce);
}

async function pollTransactionHash(
  relayerUrl: string,
  txId: string,
  headers: Record<string, string>,
  maxAttempts = 10,
): Promise<string> {
  for (let i = 0; i < maxAttempts; i++) {
    await new Promise((r) => setTimeout(r, 2000));
    const res = await fetch(`${relayerUrl}/transaction?id=${txId}`, { headers });
    if (!res.ok) continue;
    const data = await res.json();
    const hash = String(data.transactionHash ?? '').trim();
    if (hash) return hash;
  }
  throw new ClaimRelayerRouteError(502, 'relayer_tx_hash_timeout', true, `transaction ${txId} hash not available after polling`);
}

export async function submitClaimViaRelayerApiKey(
  config: ClaimRelayerConfigForServer,
  body: ClaimRedeemRequestBody
): Promise<ClaimRedeemSuccess> {
  validateBaseConfig(config);
  validateRequest(body);

  if (config.executionMode !== 'relayer_api_key') {
    throw new ClaimRelayerRouteError(400, 'claim_execution_mode_invalid', false, `expected relayer_api_key, got ${config.executionMode}`);
  }
  if (!config.relayerApiKey.trim()) {
    throw new ClaimRelayerRouteError(400, 'missing_relayer_api_key', false, 'relayer_api_key is required');
  }
  if (!isAddress(config.relayerApiKeyAddress)) {
    throw new ClaimRelayerRouteError(400, 'invalid_relayer_api_key_address', false, 'relayer_api_key_address must be a valid address');
  }

  const account = privateKeyToAccount(config.privateKey as Hex);
  if (!sameAddress(account.address, config.userAddress)) {
    throw new ClaimRelayerRouteError(400, 'claim_signer_mismatch', false, 'claim.private_key does not match claim.user_address');
  }
  const expectedSafe = deriveSafe(account.address, config.chainId);
  if (!sameAddress(expectedSafe, config.safeAddress)) {
    throw new ClaimRelayerRouteError(400, 'configured_safe_mismatch', false, `gnosis_safe_address does not match derived Safe (${expectedSafe})`);
  }
  if (!sameAddress(body.ownerAddress, config.safeAddress)) {
    throw new ClaimRelayerRouteError(400, 'owner_address_mismatch', false, 'ownerAddress does not match gnosis_safe_address');
  }
  if (!sameAddress(body.collateralToken, config.collateralTokenAddress)) {
    throw new ClaimRelayerRouteError(400, 'collateral_token_mismatch', false, 'collateralToken does not match collateral_token_address');
  }

  try {
    const calldata = encodeFunctionData({
      abi: CTF_ABI,
      functionName: 'redeemPositions',
      args: [config.collateralTokenAddress as Address, zeroHash, body.conditionId as Hex, body.indexSets.map((v) => BigInt(v))],
    });
    const txHash = await submitSafeTransactionsViaRelayerApiKey(
      config,
      [{ to: config.ctfContractAddress, data: calldata, value: '0' }],
      'auto-claim redeem positions'
    );
    return { txHash };
  } catch (err) {
    throw classifyRelayerError(err);
  }
}

export async function submitMergeViaBuilderRelayer(
  config: ClaimRelayerConfigForServer,
  body: ClaimMergeRequestBody
): Promise<ClaimRedeemSuccess> {
  validateConfig(config);
  validateMergeRequest(body);

  if (config.executionMode !== 'builder_relayer') {
    throw new ClaimRelayerRouteError(
      400,
      'claim_execution_mode_invalid',
      false,
      `claim.execution_mode must be builder_relayer, got ${config.executionMode}`
    );
  }
  validateSafeMergeRequest(config, body);

  try {
    const txHash = await submitSafeTransactionsViaBuilderRelayer(
      config,
      [buildMergeTransaction(config, body)],
      'positive flip pairlock merge positions'
    );
    return { txHash };
  } catch (err) {
    throw classifyRelayerError(err);
  }
}

export async function submitMergeViaRelayerApiKey(
  config: ClaimRelayerConfigForServer,
  body: ClaimMergeRequestBody
): Promise<ClaimRedeemSuccess> {
  validateBaseConfig(config);
  validateMergeRequest(body);

  if (config.executionMode !== 'relayer_api_key') {
    throw new ClaimRelayerRouteError(
      400,
      'claim_execution_mode_invalid',
      false,
      `expected relayer_api_key, got ${config.executionMode}`
    );
  }
  if (!config.relayerApiKey.trim()) {
    throw new ClaimRelayerRouteError(400, 'missing_relayer_api_key', false, 'relayer_api_key is required');
  }
  if (!isAddress(config.relayerApiKeyAddress)) {
    throw new ClaimRelayerRouteError(400, 'invalid_relayer_api_key_address', false, 'relayer_api_key_address must be a valid address');
  }
  validateSafeMergeRequest(config, body);

  try {
    const txHash = await submitSafeTransactionsViaRelayerApiKey(
      config,
      [buildMergeTransaction(config, body)],
      'positive flip pairlock merge positions'
    );
    return { txHash };
  } catch (err) {
    throw classifyRelayerError(err);
  }
}

export async function readClaimFundsActivationBalances(
  config: ClaimRelayerConfigForServer
): Promise<ClaimFundsActivationBalances> {
  validateBaseConfig(config);
  validateFundsActivationConfig(config);

  const publicClient = createPublicClient({
    chain: resolveChain(config.chainId),
    transport: http(config.rpcUrl || undefined),
  });
  const [usdcEBalanceRaw, pUsdBalanceRaw, allowanceRaw] = await Promise.all([
    publicClient.readContract({
      address: config.usdceTokenAddress as Address,
      abi: ERC20_ABI,
      functionName: 'balanceOf',
      args: [config.safeAddress as Address],
    }),
    publicClient.readContract({
      address: config.pusdTokenAddress as Address,
      abi: ERC20_ABI,
      functionName: 'balanceOf',
      args: [config.safeAddress as Address],
    }),
    publicClient.readContract({
      address: config.usdceTokenAddress as Address,
      abi: ERC20_ABI,
      functionName: 'allowance',
      args: [
        config.safeAddress as Address,
        config.collateralOnrampAddress as Address,
      ],
    }),
  ]);

  return {
    usdcEBalanceRaw,
    pUsdBalanceRaw,
    allowanceRaw,
    usdcEBalance: rawUsdcToNumber(usdcEBalanceRaw),
    pUsdBalance: rawUsdcToNumber(pUsdBalanceRaw),
  };
}

export function buildClaimFundsActivationTransactions(params: {
  safeAddress: string;
  usdceTokenAddress: string;
  collateralOnrampAddress: string;
  usdcEBalanceRaw: bigint;
  allowanceRaw: bigint;
  minUsdc: number;
}): ClaimFundsActivationTransactionPlan {
  const minRaw = usdcToRawUnits(params.minUsdc);
  const amountRaw = params.usdcEBalanceRaw;
  if (amountRaw <= BigInt(0) || amountRaw < minRaw) {
    return {
      status: 'skipped',
      amountRaw,
      needsApproval: false,
      transactions: [],
      message: 'No USDC.e balance waiting for pUSD activation.',
    };
  }

  const transactions: Transaction[] = [];
  const needsApproval = params.allowanceRaw < amountRaw;
  if (needsApproval) {
    transactions.push({
      to: params.usdceTokenAddress,
      data: encodeFunctionData({
        abi: ERC20_ABI,
        functionName: 'approve',
        args: [params.collateralOnrampAddress as Address, amountRaw],
      }),
      value: '0',
    });
  }
  transactions.push({
    to: params.collateralOnrampAddress,
    data: encodeFunctionData({
      abi: COLLATERAL_ONRAMP_ABI,
      functionName: 'wrap',
      args: [
        params.usdceTokenAddress as Address,
        params.safeAddress as Address,
        amountRaw,
      ],
    }),
    value: '0',
  });

  return {
    status: 'ready',
    amountRaw,
    needsApproval,
    transactions,
    message: needsApproval
      ? 'Submitting USDC.e approval and pUSD activation.'
      : 'Submitting pUSD activation.',
  };
}

export async function submitClaimFundsActivation(
  config: ClaimRelayerConfigForServer,
  body: ClaimFundsActivationRequestBody
): Promise<ClaimFundsActivationResult> {
  validateBaseConfig(config);
  validateFundsActivationConfig(config);
  validateFundsActivationRequest(body);

  if (!sameAddress(body.ownerAddress, config.safeAddress)) {
    throw new ClaimRelayerRouteError(
      400,
      'owner_address_mismatch',
      false,
      'ownerAddress does not match gnosis_safe_address'
    );
  }
  if (
    config.executionMode !== 'builder_relayer' &&
    config.executionMode !== 'relayer_api_key'
  ) {
    throw new ClaimRelayerRouteError(
      400,
      'funds_activation_mode_unsupported',
      false,
      'funds activation requires builder_relayer or relayer_api_key execution mode'
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
      `gnosis_safe_address does not match derived Safe (${expectedSafe})`
    );
  }

  const balances = await readClaimFundsActivationBalances(config);
  const plan = buildClaimFundsActivationTransactions({
    safeAddress: config.safeAddress,
    usdceTokenAddress: config.usdceTokenAddress,
    collateralOnrampAddress: config.collateralOnrampAddress,
    usdcEBalanceRaw: balances.usdcEBalanceRaw,
    allowanceRaw: balances.allowanceRaw,
    minUsdc: config.activateMinUsdc,
  });

  if (plan.status === 'skipped') {
    return {
      status: 'skipped',
      activatedAmountUsdc: 0,
      approveTxHash: null,
      wrapTxHash: null,
      usdcEBalance: balances.usdcEBalance,
      pUsdBalance: balances.pUsdBalance,
      message: plan.message,
    };
  }

  let txHash: string;
  try {
    txHash =
      config.executionMode === 'relayer_api_key'
        ? await submitSafeTransactionsViaRelayerApiKey(
            config,
            plan.transactions,
            'activate USDC.e funds to pUSD'
          )
        : await submitSafeTransactionsViaBuilderRelayer(
            config,
            plan.transactions,
            'activate USDC.e funds to pUSD'
          );
  } catch (err) {
    throw classifyRelayerError(err);
  }

  return {
    status: 'submitted',
    activatedAmountUsdc: rawUsdcToNumber(plan.amountRaw),
    approveTxHash: plan.needsApproval ? txHash : null,
    wrapTxHash: txHash,
    usdcEBalance: balances.usdcEBalance,
    pUsdBalance: balances.pUsdBalance,
    message: `Submitted ${rawUsdcToNumber(plan.amountRaw).toFixed(6)} USDC.e activation to pUSD.`,
  };
}

function validateBaseConfig(config: ClaimRelayerConfigForServer): void {
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
  if (!SAFE_FACTORY_BY_CHAIN[config.chainId]) {
    throw new ClaimRelayerRouteError(400, 'unsupported_chain', false, `unsupported chain id ${config.chainId}`);
  }
}

function validateConfig(config: ClaimRelayerConfigForServer): void {
  validateBaseConfig(config);
  if (!config.builderApiKey.trim() || !config.builderApiSecret.trim() || !config.builderApiPassphrase.trim()) {
    throw new ClaimRelayerRouteError(400, 'missing_builder_credentials', false, 'builder relayer credentials are incomplete');
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

function validateMergeRequest(body: ClaimMergeRequestBody): void {
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
  if (
    !Array.isArray(body.partition) ||
    body.partition.length !== 2 ||
    body.partition[0] !== 1 ||
    body.partition[1] !== 2
  ) {
    throw new ClaimRelayerRouteError(400, 'invalid_partition', false, 'partition must be exactly [1, 2]');
  }
  if (!/^[1-9][0-9]*$/.test(String(body.amountRaw ?? '').trim())) {
    throw new ClaimRelayerRouteError(400, 'invalid_amount_raw', false, 'amountRaw must be a positive integer string');
  }
}

function validateSafeMergeRequest(
  config: ClaimRelayerConfigForServer,
  body: ClaimMergeRequestBody
): void {
  const account = privateKeyToAccount(config.privateKey as Hex);
  if (!sameAddress(account.address, config.userAddress)) {
    throw new ClaimRelayerRouteError(400, 'claim_signer_mismatch', false, 'claim.private_key does not match claim.user_address');
  }
  const expectedSafe = deriveSafe(account.address, config.chainId);
  if (!sameAddress(expectedSafe, config.safeAddress)) {
    throw new ClaimRelayerRouteError(400, 'configured_safe_mismatch', false, `gnosis_safe_address does not match derived Safe (${expectedSafe})`);
  }
  if (!sameAddress(body.ownerAddress, config.safeAddress)) {
    throw new ClaimRelayerRouteError(400, 'owner_address_mismatch', false, 'ownerAddress does not match gnosis_safe_address');
  }
  if (!sameAddress(body.collateralToken, config.collateralTokenAddress)) {
    throw new ClaimRelayerRouteError(400, 'collateral_token_mismatch', false, 'collateralToken does not match collateral_token_address');
  }
}

function buildMergeTransaction(
  config: ClaimRelayerConfigForServer,
  body: ClaimMergeRequestBody
): Transaction {
  return {
    to: config.ctfContractAddress,
    data: encodeFunctionData({
      abi: CTF_ABI,
      functionName: 'mergePositions',
      args: [
        config.collateralTokenAddress as Address,
        zeroHash,
        body.conditionId as Hex,
        body.partition.map((value) => BigInt(value)),
        BigInt(body.amountRaw),
      ],
    }),
    value: '0',
  };
}

function validateFundsActivationConfig(config: ClaimRelayerConfigForServer): void {
  const usdce = config.usdceTokenAddress || DEFAULT_USDCE_TOKEN_ADDRESS;
  const pusd = config.pusdTokenAddress || DEFAULT_PUSD_TOKEN_ADDRESS;
  const onramp = config.collateralOnrampAddress || DEFAULT_COLLATERAL_ONRAMP_ADDRESS;
  if (!isAddress(usdce)) {
    throw new ClaimRelayerRouteError(400, 'invalid_usdce_token_address', false, 'usdce_token_address must be a valid address');
  }
  if (!isAddress(pusd)) {
    throw new ClaimRelayerRouteError(400, 'invalid_pusd_token_address', false, 'pusd_token_address must be a valid address');
  }
  if (!isAddress(onramp)) {
    throw new ClaimRelayerRouteError(400, 'invalid_collateral_onramp_address', false, 'collateral_onramp_address must be a valid address');
  }
  if (!Number.isFinite(config.activateMinUsdc) || config.activateMinUsdc < 0) {
    throw new ClaimRelayerRouteError(400, 'invalid_activate_min_usdc', false, 'activate_min_usdc must be >= 0');
  }
  if (!SAFE_MULTISEND_BY_CHAIN[config.chainId]) {
    throw new ClaimRelayerRouteError(400, 'unsupported_chain', false, `unsupported chain id ${config.chainId}`);
  }
}

function validateFundsActivationRequest(body: ClaimFundsActivationRequestBody): void {
  if (!Number.isFinite(body.userId) || body.userId <= 0) {
    throw new ClaimRelayerRouteError(400, 'invalid_user_id', false, 'userId must be a positive integer');
  }
  if (!isAddress(body.ownerAddress)) {
    throw new ClaimRelayerRouteError(400, 'invalid_owner_address', false, 'ownerAddress must be a valid 0x address');
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

async function submitSafeTransactionsViaBuilderRelayer(
  config: ClaimRelayerConfigForServer,
  transactions: Transaction[],
  metadata: string
): Promise<string> {
  validateConfig(config);
  const account = privateKeyToAccount(config.privateKey as Hex);
  const walletClient = createWalletClient({
    account,
    chain: resolveChain(config.chainId),
    transport: http(config.rpcUrl || undefined),
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
  const response = await client.execute(transactions, metadata);
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
  return txHash;
}

async function submitSafeTransactionsViaRelayerApiKey(
  config: ClaimRelayerConfigForServer,
  transactions: Transaction[],
  metadata: string
): Promise<string> {
  if (!config.relayerApiKey.trim()) {
    throw new ClaimRelayerRouteError(400, 'missing_relayer_api_key', false, 'relayer_api_key is required');
  }
  if (!isAddress(config.relayerApiKeyAddress)) {
    throw new ClaimRelayerRouteError(400, 'invalid_relayer_api_key_address', false, 'relayer_api_key_address must be a valid address');
  }
  if (transactions.length === 0) {
    throw new ClaimRelayerRouteError(400, 'empty_relayer_transactions', false, 'at least one transaction is required');
  }

  const account = privateKeyToAccount(config.privateKey as Hex);
  const safeTransaction = buildSafeRelayTransaction(config, transactions);
  const relayerUrl = process.env.POLYMARKET_RELAYER_URL?.trim() || DEFAULT_RELAYER_URL;
  const authHeaders: Record<string, string> = {
    'Content-Type': 'application/json',
    'RELAYER_API_KEY': config.relayerApiKey.trim(),
    'RELAYER_API_KEY_ADDRESS': config.relayerApiKeyAddress.trim(),
  };
  const nonce = await fetchSafeNonce(relayerUrl, account.address);
  const operation = Number(safeTransaction.operation ?? OperationType.Call);
  const structHash = hashTypedData({
    primaryType: 'SafeTx',
    domain: { chainId: config.chainId, verifyingContract: config.safeAddress as Address },
    types: SAFE_TX_TYPES,
    message: {
      to: safeTransaction.to as Address,
      value: BigInt(safeTransaction.value),
      data: safeTransaction.data as Hex,
      operation,
      safeTxGas: BigInt(0),
      baseGas: BigInt(0),
      gasPrice: BigInt(0),
      gasToken: zeroAddress,
      refundReceiver: zeroAddress,
      nonce: BigInt(nonce),
    },
  });

  const walletClient = createWalletClient({
    account,
    chain: resolveChain(config.chainId),
    transport: http(config.rpcUrl || undefined),
  });
  const rawSig = await walletClient.signMessage({ account, message: { raw: toBytes(structHash) } });
  const packedSig = splitAndPackSafeSig(rawSig);
  const requestBody = {
    type: 'SAFE',
    from: account.address,
    to: safeTransaction.to,
    proxyWallet: config.safeAddress,
    data: safeTransaction.data,
    nonce,
    signature: packedSig,
    signatureParams: {
      gasPrice: '0',
      operation: String(operation),
      safeTxnGas: '0',
      baseGas: '0',
      gasToken: zeroAddress,
      refundReceiver: zeroAddress,
    },
    metadata,
  };

  const res = await fetch(`${relayerUrl}/submit`, {
    method: 'POST',
    headers: authHeaders,
    body: JSON.stringify(requestBody),
  });
  if (!res.ok) {
    throw relayerHttpRouteError(res.status, await res.text());
  }
  const result = await res.json();
  let txHash = String(result.transactionHash ?? '').trim();
  if (!txHash && result.transactionID) {
    txHash = await pollTransactionHash(relayerUrl, result.transactionID, authHeaders);
  }
  if (!txHash) {
    throw new ClaimRelayerRouteError(502, 'empty_relayer_tx_hash', true, 'relayer returned no transaction hash');
  }
  return txHash;
}

function buildSafeRelayTransaction(
  config: ClaimRelayerConfigForServer,
  transactions: Transaction[]
): SafeTransaction {
  const safeTransactions = transactions.map((tx) => ({
    to: tx.to,
    data: tx.data,
    value: tx.value || '0',
    operation: OperationType.Call,
  }));
  if (safeTransactions.length === 1) {
    return safeTransactions[0];
  }

  return createSafeMultisendTransaction(
    safeTransactions,
    SAFE_MULTISEND_BY_CHAIN[config.chainId]
  );
}

function usdcToRawUnits(value: number): bigint {
  const normalized = Math.max(0, Number.isFinite(value) ? value : 0);
  return BigInt(Math.ceil(normalized * 1_000_000));
}

function rawUsdcToNumber(value: bigint): number {
  return Number(formatUnits(value, 6));
}

function relayerHttpRouteError(status: number, body: string): ClaimRelayerRouteError {
  const retryable = status >= 500 || status === 429;
  const normalized = normalizeRelayerErrorMessage(body);
  if (normalized.code === 'relayer_wallet_activation_required') {
    return new ClaimRelayerRouteError(409, normalized.code, false, normalized.message);
  }
  if (normalized.code === 'relayer_html_error') {
    return new ClaimRelayerRouteError(
      retryable ? 502 : 400,
      `relayer_http_${status}_html`,
      retryable,
      normalized.message
    );
  }
  return new ClaimRelayerRouteError(
    retryable ? 502 : 400,
    `relayer_http_${status}`,
    retryable,
    normalized.message
  );
}

function normalizeRelayerErrorMessage(raw: string): { code: string; message: string } {
  const trimmed = String(raw ?? '').trim();
  const lower = trimmed.toLowerCase();
  if (
    lower.includes('activate funds') ||
    lower.includes('funds activation') ||
    lower.includes('activation required')
  ) {
    return {
      code: 'relayer_wallet_activation_required',
      message: 'Polymarket relayer requires wallet funds activation before this action.',
    };
  }
  if (looksLikeHtml(trimmed)) {
    return {
      code: 'relayer_html_error',
      message: 'Polymarket relayer returned an HTML error page.',
    };
  }
  return {
    code: 'relayer_request_failed',
    message: trimmed || 'Polymarket relayer request failed.',
  };
}

function looksLikeHtml(value: string): boolean {
  const prefix = value.trimStart().slice(0, 256).toLowerCase();
  return prefix.startsWith('<!doctype html') || prefix.startsWith('<html') || prefix.includes('<html');
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
    const responseText =
      typeof maybeAxiosError.response?.data === 'string'
        ? maybeAxiosError.response.data
        : JSON.stringify(maybeAxiosError.response?.data ?? {});
    return relayerHttpRouteError(responseStatus, responseText || message);
  }

  const normalized = normalizeRelayerErrorMessage(message);
  if (normalized.code === 'relayer_wallet_activation_required') {
    return new ClaimRelayerRouteError(409, normalized.code, false, normalized.message);
  }
  if (normalized.code === 'relayer_html_error') {
    return new ClaimRelayerRouteError(502, 'relayer_html_error', true, normalized.message);
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
