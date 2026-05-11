const FLOW_NAME_MAX_LENGTH = 80;
const FLOW_NAME_MIN_LENGTH = 2;
const FLOW_NAME_PATTERN = /^[\p{L}\p{N}][\p{L}\p{N} ._:/+\-()[\]#&']*$/u;

export const FLOW_DUPLICATE_NAME_MESSAGE = 'Flow name is already in use';
export const FLOW_INVALID_NAME_MESSAGE =
  "Flow name must be 2-80 chars and use only letters, numbers, spaces, dot, dash, underscore, slash, colon, plus, ampersand, apostrophe, brackets, or parentheses";
export const FLOW_NAME_UNIQUE_INDEX = 'uq_trade_flow_definitions_user_name_active';

export function normalizeTradeFlowDefinitionName(raw: string): string {
  return raw.trim().replace(/\s+/g, ' ');
}

export function validateTradeFlowDefinitionName(raw: string): string {
  const normalized = normalizeTradeFlowDefinitionName(raw);
  if (!normalized) {
    throw new Error('Flow name is required');
  }
  if (normalized.length < FLOW_NAME_MIN_LENGTH || normalized.length > FLOW_NAME_MAX_LENGTH) {
    throw new Error(FLOW_INVALID_NAME_MESSAGE);
  }
  if (!FLOW_NAME_PATTERN.test(normalized)) {
    throw new Error(FLOW_INVALID_NAME_MESSAGE);
  }
  return normalized;
}

export function isTradeFlowNameValidationMessage(message: string): boolean {
  return message === 'Flow name is required' || message === 'Flow name cannot be empty' || message === FLOW_INVALID_NAME_MESSAGE;
}

export function isTradeFlowNameUniqueViolation(err: unknown): boolean {
  if (!err || typeof err !== 'object') return false;
  const code =
    'code' in err && typeof err.code === 'string'
      ? err.code
      : '';
  const constraint =
    'constraint' in err && typeof err.constraint === 'string'
      ? err.constraint
      : '';
  return code === '23505' && constraint === FLOW_NAME_UNIQUE_INDEX;
}
