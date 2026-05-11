import type { QueryResult } from 'pg';
import type { Queryable } from './shared';

export const FLOW_DEFINITION_BUSY_CODE = 'flow_definition_busy';
export const FLOW_DEFINITION_BUSY_MESSAGE =
  'Bu flow üzerinde başka bir işlem çalışıyor. Birkaç saniye bekleyip tekrar dene.';

const TRADE_FLOW_DEFINITION_LOCK_NAMESPACE = 41049;

export class FlowDefinitionBusyError extends Error {
  readonly code = FLOW_DEFINITION_BUSY_CODE;
  readonly definitionId: number;
  readonly retryable = true;

  constructor(definitionId: number) {
    super(FLOW_DEFINITION_BUSY_MESSAGE);
    this.name = 'FlowDefinitionBusyError';
    this.definitionId = definitionId;
  }
}

function errorCode(error: unknown): string | null {
  if (
    error &&
    typeof error === 'object' &&
    'code' in error &&
    typeof (error as { code?: unknown }).code === 'string'
  ) {
    return (error as { code: string }).code;
  }
  return null;
}

export function isFlowDefinitionBusyError(error: unknown): error is FlowDefinitionBusyError {
  return error instanceof FlowDefinitionBusyError;
}

export function isPostgresLockTimeoutError(error: unknown): boolean {
  return errorCode(error) === '55P03';
}

export async function acquireTradeFlowDefinitionMutationLock(
  queryable: Queryable,
  definitionId: number
): Promise<void> {
  const res = (await queryable.query(
    'SELECT pg_try_advisory_xact_lock($1, $2) AS locked',
    [TRADE_FLOW_DEFINITION_LOCK_NAMESPACE, definitionId]
  )) as QueryResult<{ locked: boolean }>;
  if (!res.rows[0]?.locked) {
    throw new FlowDefinitionBusyError(definitionId);
  }
}

export function mapTradeFlowMutationHttpError(
  error: unknown,
  fallback: string
): { body: { error: string; code?: string; retryable?: boolean }; status: number } {
  if (isFlowDefinitionBusyError(error) || isPostgresLockTimeoutError(error)) {
    return {
      status: 423,
      body: {
        error: FLOW_DEFINITION_BUSY_MESSAGE,
        code: FLOW_DEFINITION_BUSY_CODE,
        retryable: true,
      },
    };
  }

  return {
    status: 500,
    body: {
      error: error instanceof Error ? error.message : fallback,
    },
  };
}
